//! Who a process is: its start time and its command line, read through the operating system.
//! Absence is decided positively; a helper that fails is an error, never "no such process", so
//! recovery never takes a live process for a dead one. A zombie (killed, not yet reaped) counts as
//! absent.

use crate::Error;

/// A running process as recovery sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Identity {
    /// When it started, as the operating system says it: compared exactly, never interpreted.
    pub started: String,
    pub args: Args,
}

/// A process's command line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Args {
    /// The arguments one by one (Linux, Windows).
    List(Vec<String>),
    /// The arguments joined with spaces, as `ps` prints them (macOS and the other unixes).
    #[cfg_attr(any(target_os = "linux", windows), allow(dead_code))]
    Joined(String),
    /// Unreadable: too long, or another user's process. Neither dolt nor ours.
    Unknown,
}

impl Identity {
    /// Whether the process is a `dolt sql-server`, by any path.
    pub fn is_dolt_server(&self) -> bool {
        match &self.args {
            Args::List(a) => match (a.first(), a.get(1)) {
                (Some(program), Some(verb)) => {
                    let name = file_name(program).to_ascii_lowercase();
                    (name == "dolt" || name == "dolt.exe") && verb == "sql-server"
                }
                _ => false,
            },
            Args::Joined(s) => s.match_indices("dolt sql-server").any(|(at, m)| {
                let before = s.get(..at).and_then(|b| b.chars().last());
                let after = s.get(at + m.len()..).and_then(|a| a.chars().next());
                matches!(before, None | Some('/')) && matches!(after, None | Some(' '))
            }),
            Args::Unknown => false,
        }
    }

    /// Whether the command line is `expected`, the one this crate started: exactly so for a list. A
    /// joined line is compared as `ps` writes `expected`, since `ps` rewrites every character
    /// outside ASCII; like the joining itself, that form is not one-to-one (a line spelling `M-C`
    /// out reads as the byte it stands for), so a random run folder name stays part of the proof.
    pub fn is(&self, expected: &[String]) -> bool {
        match &self.args {
            Args::List(a) => a == expected,
            Args::Joined(s) => *s == ps_text(&expected.join(" ")),
            Args::Unknown => false,
        }
    }
}

/// `text` as `ps` prints it in the C locale, measured on macOS 27.2: a byte of a character outside
/// ASCII becomes `M^` and a letter (0x80 to 0x9f), `\240` (0xa0) or `M-` and a sign (above);
/// printable ASCII stays, and a control character, which no path of this crate holds, is left as
/// it is and so never matches. Under a UTF-8 locale `ps` keeps such characters, but only where
/// that locale exists: asked for one the machine lacks, it writes this form again, and the C
/// locale is always there.
#[cfg_attr(any(target_os = "linux", windows), allow(dead_code))]
pub(crate) fn ps_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for b in text.bytes() {
        match b {
            ..=0x7f => out.push(char::from(b)),
            0x80..=0x9f => out.extend(['M', '^', char::from(b ^ 0xc0)]),
            0xa0 => out.push_str("\\240"),
            _ => out.extend(['M', '-', char::from(b & 0x7f)]),
        }
    }
    out
}

/// The last component of a path written with `/` or `\`.
fn file_name(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

/// The identity of `pid`; `None` when no such process exists or it is a zombie.
pub(crate) fn identity(pid: u32) -> Result<Option<Identity>, Error> {
    platform::identity(pid)
}

/// The most of a list of processes that is read: the 900 processes of a busy desktop took 190 KiB.
#[cfg_attr(target_os = "linux", allow(dead_code))]
const LIST_CAP: u64 = 4 << 20;
/// The most entries of `/proc` a list goes through.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
const MOST_ENTRIES: usize = 1 << 17;
/// The most of a command line read from `/proc`; a longer one is nobody's.
#[cfg_attr(windows, allow(dead_code))]
const CMDLINE_CAP: u64 = 4096;

/// The ids of the processes whose command line is exactly `command`: not one that only begins
/// with it, holds it or differs in a character. For finding a server whose process id nobody
/// recorded. The answer only proposes. By the time it is read a process may have ended and its
/// id gone to another program, and where the list is of joined lines it is not one-to-one (see
/// [`Identity::is`]; blanks around a line do not count, so a command line that begins or ends
/// with one is never found): before any signal a caller proves each id again, through
/// [`identity`] and [`Identity::is`]. A list that fails, is cut short or is too long to be read
/// whole is an error, never "none"; an empty `command` is no process's. On unix only this user's
/// processes are looked at. A server run by another user is out of scope: this crate starts
/// every server itself, as the user Archforge runs as, and could not stop another user's.
pub(crate) fn running(command: &[String]) -> Result<Vec<u32>, Error> {
    platform::running(command)
}

/// A list's output, if its helper succeeded and the output is whole: one that filled `cap` may
/// have lost its end, and a process with it.
#[cfg_attr(target_os = "linux", allow(dead_code))]
fn whole(helper: &std::path::Path, out: crate::runner::Output, cap: u64) -> Result<crate::runner::Output, Error> {
    if !out.success {
        return Err(crate::runner::failed(helper, &out));
    }
    if out.stdout.len() as u64 >= cap {
        let output = format!("its list of processes is longer than {cap} bytes");
        return Err(Error::HelperFailed { helper: helper.display().to_string(), status: "too long".into(), output });
    }
    Ok(out)
}

/// `ps -o pid=,args=` output: the ids of the lines whose command line is `wanted`, a command line
/// as `ps` writes it. A line that cannot be read is no process. `None` for a list that does not
/// end with a line's end: a whole one does, and is never empty, since `ps` lists itself.
#[cfg_attr(any(target_os = "linux", windows), allow(dead_code))]
pub(crate) fn parse_ps_list(out: &str, wanted: &str) -> Option<Vec<u32>> {
    let pid = |line: &str| {
        let (pid, args) = line.trim_start().split_once(' ')?;
        pid.parse().ok().filter(|p| crate::recovery::valid_pid(*p) && !wanted.is_empty() && args.trim() == wanted)
    };
    out.ends_with('\n').then(|| out.lines().filter_map(pid).collect())
}

/// The ids under `proc`, a `/proc`, of this user's processes whose `cmdline` is `command`, going
/// through at most `most` entries in at most `within`. This user's are those owned as `self`
/// there is: another user's is never opened. A process that ended meanwhile is none, as is one
/// that may not be read; any other failure is an error, and so is a `command` longer than what
/// is read of a command line, which could not be told from a longer one.
#[cfg(unix)]
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub(crate) fn scan_proc(proc: &std::path::Path, command: &[String], most: usize, within: std::time::Duration) -> Result<Vec<u32>, Error> {
    use std::io::Read;
    use std::os::unix::fs::MetadataExt;
    let too_long = |output: String| Error::HelperFailed { helper: proc.display().to_string(), status: "too long".into(), output };
    if command.iter().map(|a| a.len() as u64 + 1).sum::<u64>() > CMDLINE_CAP {
        return Err(too_long(format!("the command line looked for is over {CMDLINE_CAP} bytes")));
    }
    let me = std::fs::metadata(proc.join("self")).map_err(|e| Error::io("cannot read", proc.join("self"), e))?.uid();
    let started = std::time::Instant::now();
    let mut pids = Vec::new();
    for (n, entry) in std::fs::read_dir(proc).map_err(|e| Error::io("cannot list", proc, e))?.enumerate() {
        if n >= most {
            return Err(too_long(format!("more than {most} entries")));
        }
        if started.elapsed() >= within {
            return Err(Error::HelperTimedOut { helper: proc.display().to_string(), waited: within });
        }
        let entry = entry.map_err(|e| Error::io("cannot list", proc, e))?;
        let Some(pid) = entry.file_name().to_str().and_then(|n| n.parse().ok()).filter(|p| crate::recovery::valid_pid(*p)) else { continue };
        // One that ended has no owner to read any more.
        if !entry.metadata().is_ok_and(|m| m.uid() == me) {
            continue;
        }
        let file = entry.path().join("cmdline");
        let mut bytes = Vec::new();
        match std::fs::File::open(&file).and_then(|f| f.take(CMDLINE_CAP + 1).read_to_end(&mut bytes)) {
            Ok(_) if !command.is_empty() && parse_cmdline(&bytes).is_some_and(|line| line == command) => pids.push(pid),
            Ok(_) => {}
            // ESRCH: the process ended between the open and the read.
            Err(e) if matches!(e.kind(), std::io::ErrorKind::NotFound | std::io::ErrorKind::PermissionDenied) || e.raw_os_error() == Some(3) => {}
            Err(e) => return Err(Error::io("cannot read", &file, e)),
        }
    }
    Ok(pids)
}

/// PowerShell's JSON for every process, one object or a list of them: the ids of those whose
/// command line splits into `command`. `None` for an answer that cannot be read.
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn parse_cim_list(json: &str, command: &[String]) -> Option<Vec<u32>> {
    let all = match serde_json::from_str(json.trim()).ok()? {
        serde_json::Value::Array(a) => a,
        one @ serde_json::Value::Object(_) => vec![one],
        _ => return None,
    };
    let mut pids = Vec::new();
    for process in &all {
        let pid = process.get("ProcessId")?.as_u64().and_then(|p| u32::try_from(p).ok())?;
        if process.get("CommandLine")?.as_str().is_some_and(|line| comparable(line) && split_windows(line) == command)
            && crate::recovery::valid_pid(pid)
        {
            pids.push(pid);
        }
    }
    Some(pids)
}

#[cfg(target_os = "linux")]
mod platform {
    use std::io::Read;
    use std::path::PathBuf;

    use super::{Args, Identity, parse_cmdline, parse_stat};
    use crate::Error;

    const CAP: u64 = super::CMDLINE_CAP;

    pub fn identity(pid: u32) -> Result<Option<Identity>, Error> {
        let dir = PathBuf::from(format!("/proc/{pid}"));
        let Some(stat) = read(&dir.join("stat"))? else { return Ok(None) };
        let Some((state, started)) = parse_stat(&stat) else {
            return Err(Error::HelperFailed {
                helper: dir.join("stat").display().to_string(),
                status: "unreadable".into(),
                output: stat.chars().take(200).collect(),
            });
        };
        if state == 'Z' {
            return Ok(None);
        }
        let args = match read_bytes(&dir.join("cmdline"))? {
            None => return Ok(None),
            Some(bytes) if bytes.len() as u64 > CAP => Args::Unknown,
            Some(bytes) => parse_cmdline(&bytes).map_or(Args::Unknown, Args::List),
        };
        Ok(Some(Identity { started, args }))
    }

    /// This user's processes as `/proc` lists them, within a helper's budget.
    pub fn running(command: &[String]) -> Result<Vec<u32>, Error> {
        super::scan_proc(std::path::Path::new("/proc"), command, super::MOST_ENTRIES, crate::runner::HELPER_BUDGET)
    }

    fn read(path: &std::path::Path) -> Result<Option<String>, Error> {
        Ok(read_bytes(path)?.map(|b| String::from_utf8_lossy(&b).into_owned()))
    }

    /// At most one byte past the cap, so an oversized file is recognised without reading it all.
    fn read_bytes(path: &std::path::Path) -> Result<Option<Vec<u8>>, Error> {
        match std::fs::File::open(path) {
            Ok(f) => {
                let mut bytes = Vec::new();
                f.take(CAP + 1).read_to_end(&mut bytes).map_err(|e| Error::io("cannot read", path, e))?;
                Ok(Some(bytes))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(Error::io("cannot read", path, e)),
        }
    }
}

#[cfg(all(unix, not(target_os = "linux")))]
mod platform {
    use std::ffi::OsStr;
    use std::path::Path;

    use super::{Identity, parse_ps};
    use crate::Error;
    use crate::runner;

    pub fn identity(pid: u32) -> Result<Option<Identity>, Error> {
        let ps = Path::new("/bin/ps");
        let pid = pid.to_string();
        let args = [OsStr::new("-ww"), OsStr::new("-o"), OsStr::new("state=,lstart=,args="), OsStr::new("-p"), OsStr::new(&pid)];
        let out = runner::run(ps, &args, &env(), runner::HELPER_BUDGET)?;
        if out.code == Some(1) && out.stdout.trim().is_empty() && out.stderr.trim().is_empty() {
            return Ok(None);
        }
        if !out.success {
            return Err(runner::failed(ps, &out));
        }
        parse_ps(&out.stdout).ok_or_else(|| runner::failed(ps, &out))
    }

    /// This user's processes, with or without a terminal, in the locale [`identity`] reads them in.
    pub fn running(command: &[String]) -> Result<Vec<u32>, Error> {
        let ps = Path::new("/bin/ps");
        let args = [OsStr::new("-xww"), OsStr::new("-o"), OsStr::new("pid=,args=")];
        let out = super::whole(ps, runner::run_capped(ps, &args, &env(), runner::HELPER_BUDGET, super::LIST_CAP)?, super::LIST_CAP)?;
        super::parse_ps_list(&out.stdout, &super::ps_text(&command.join(" "))).ok_or_else(|| runner::failed(ps, &out))
    }

    /// `ps` prints start times in the local zone and language, which change while a process runs
    /// (travel, a new time zone): a fixed zone and the C locale keep a start time the same, and the
    /// C locale writes a command line the one way [`super::ps_text`] knows.
    pub fn env() -> Vec<(std::ffi::OsString, std::ffi::OsString)> {
        let mut env = runner::base_env();
        env.push(("TZ".into(), "UTC0".into()));
        env.push(("LC_ALL".into(), "C".into()));
        env
    }

    #[cfg(test)]
    mod tests {
        #[test]
        fn ps_runs_in_a_fixed_zone_and_locale() {
            let env = super::env();
            assert!(env.iter().any(|(k, v)| k == "TZ" && v == "UTC0"));
            assert!(env.iter().any(|(k, v)| k == "LC_ALL" && v == "C"));
        }

        /// A running process whose arguments hold characters outside ASCII, each kind of byte `ps`
        /// rewrites among them, is the command line it was started with, and no other.
        #[test]
        fn a_command_line_outside_ascii_is_recognised_as_started() {
            let odd = "é ü Ä\u{a0}…😀 M-C \\240";
            let mut child = std::process::Command::new("/bin/sh").args(["-c", "read x", odd]).stdin(std::process::Stdio::piped()).spawn().unwrap();
            let line = |last: &str| ["/bin/sh", "-c", "read x", last].map(String::from);
            let until = std::time::Instant::now() + std::time::Duration::from_secs(5);
            let mut id = super::identity(child.id()).unwrap().unwrap();
            // Between fork and exec the child still shows this test's command line.
            while !id.is(&line(odd)) && std::time::Instant::now() < until {
                std::thread::sleep(std::time::Duration::from_millis(20));
                id = super::identity(child.id()).unwrap().unwrap();
            }
            let (ours, other) = (id.is(&line(odd)), id.is(&line("e u A")));
            drop(child.stdin.take());
            child.wait().unwrap();
            assert!(ours, "{id:?}");
            assert!(!other);
        }

        /// The same process reads the same start time whatever this process's zone says.
        #[test]
        fn a_start_time_does_not_move_with_the_time_zone() {
            let me = std::process::id();
            let a = super::identity(me).unwrap().unwrap();
            let ps = std::path::Path::new("/bin/ps");
            let mut tokyo = crate::runner::base_env();
            tokyo.push(("TZ".into(), "Asia/Tokyo".into()));
            let pid = me.to_string();
            let args = ["-ww", "-o", "state=,lstart=,args=", "-p", &pid].map(std::ffi::OsStr::new);
            let other = crate::runner::run(ps, &args, &tokyo, crate::runner::HELPER_BUDGET).unwrap();
            let shifted = super::super::parse_ps(&other.stdout).unwrap().unwrap();
            assert_ne!(shifted.started, a.started, "the zone does change ps's output, so the fix matters");
            assert_eq!(super::identity(me).unwrap().unwrap().started, a.started);
        }
    }
}

#[cfg(windows)]
mod platform {
    use std::ffi::OsStr;

    use super::{Identity, cim_answer};
    use crate::Error;
    use crate::runner;

    pub fn identity(pid: u32) -> Result<Option<Identity>, Error> {
        let ps = runner::system32(r"WindowsPowerShell\v1.0\powershell.exe");
        let script = format!(
            "Get-CimInstance Win32_Process -Filter 'ProcessId={pid}' | Select-Object ParentProcessId,CreationDate,CommandLine | ConvertTo-Json -Compress"
        );
        let args = [OsStr::new("-NoProfile"), OsStr::new("-NonInteractive"), OsStr::new("-Command"), OsStr::new(&script)];
        let out = runner::run(&ps, &args, &runner::base_env(), runner::POWERSHELL_BUDGET)?;
        cim_answer(&out).ok_or_else(|| runner::failed(&ps, &out))
    }

    /// One query for every process the query shows; the command line looked for is no part of it,
    /// so the query's own process is never an answer. It answers in UTF-8, for a command line
    /// outside ASCII, and without progress records, which would read as a failure.
    pub fn running(command: &[String]) -> Result<Vec<u32>, Error> {
        let ps = runner::system32(r"WindowsPowerShell\v1.0\powershell.exe");
        let script = "$ProgressPreference='SilentlyContinue'; [Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false); \
                      Get-CimInstance Win32_Process | Select-Object ProcessId,CommandLine | ConvertTo-Json -Compress";
        let args = [OsStr::new("-NoProfile"), OsStr::new("-NonInteractive"), OsStr::new("-Command"), OsStr::new(script)];
        let out =
            super::whole(&ps, runner::run_capped(&ps, &args, &runner::base_env(), runner::POWERSHELL_BUDGET, super::LIST_CAP)?, super::LIST_CAP)?;
        Some(&out)
            .filter(|o| o.stderr.trim().is_empty())
            .and_then(|o| super::parse_cim_list(&o.stdout, command))
            .ok_or_else(|| runner::failed(&ps, &out))
    }
}

/// `ps -o state=,lstart=,args=` output: the state token, exactly 24 characters of start time, then
/// the command line. `Some(None)` for a zombie; `None` when it cannot be read.
#[cfg_attr(any(target_os = "linux", windows), allow(dead_code))]
pub(crate) fn parse_ps(out: &str) -> Option<Option<Identity>> {
    let line = out.lines().find(|l| !l.trim().is_empty())?.trim_start();
    let (state, rest) = line.split_once(char::is_whitespace)?;
    if state.starts_with('Z') {
        return Some(None);
    }
    let rest = rest.trim_start();
    let started = rest.get(..24)?.to_string();
    let args = rest.get(24..)?.trim();
    if args.is_empty() {
        return None;
    }
    Some(Some(Identity { started, args: Args::Joined(args.to_string()) }))
}

/// `/proc/<pid>/stat`: the state (field 3) and the start time (field 22), counted after the last
/// `)`, since the process name may itself hold `)` and spaces.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub(crate) fn parse_stat(stat: &str) -> Option<(char, String)> {
    let rest = stat.get(stat.rfind(')')? + 1..)?;
    let fields: Vec<&str> = rest.split_whitespace().collect();
    let state = fields.first()?.chars().next()?;
    let started = fields.get(19)?;
    Some((state, (*started).to_string()))
}

/// `/proc/<pid>/cmdline`: NUL-separated arguments. `None` for one that is not UTF-8: read with
/// the bytes that are not replaced, it would be the line of a command that holds the replacement
/// character there, and no command this crate starts is anything but UTF-8.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub(crate) fn parse_cmdline(bytes: &[u8]) -> Option<Vec<String>> {
    let mut args = bytes.split(|b| *b == 0).map(|a| std::str::from_utf8(a).ok().map(String::from)).collect::<Option<Vec<String>>>()?;
    if args.last().is_some_and(String::is_empty) {
        args.pop();
    }
    Some(args)
}

/// Whether a Windows command line may be compared: one that holds the replacement character may
/// have held something else there (half a surrogate pair, which cannot be written in UTF-8).
#[cfg_attr(not(windows), allow(dead_code))]
fn comparable(line: &str) -> bool {
    !line.contains(char::REPLACEMENT_CHARACTER)
}

/// What PowerShell's answer means: absent only for a clean exit with nothing on either stream (a
/// query that failed without failing the exit still writes to stderr), else one process, else
/// `None` for an answer that cannot be read.
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn cim_answer(out: &crate::runner::Output) -> Option<Option<Identity>> {
    if !out.success || !out.stderr.trim().is_empty() {
        return None;
    }
    if out.stdout.trim().is_empty() {
        return Some(None);
    }
    parse_cim(&out.stdout).map(Some)
}

/// PowerShell's JSON for one process. CreationDate is kept as its raw JSON text.
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn parse_cim(json: &str) -> Option<Identity> {
    let v: serde_json::Value = serde_json::from_str(json.trim()).ok()?;
    let started = v.get("CreationDate")?.to_string();
    let args = match v.get("CommandLine")? {
        serde_json::Value::String(s) if comparable(s) => Args::List(split_windows(s)),
        _ => Args::Unknown,
    };
    Some(Identity { started, args })
}

/// Splits a Windows command line as programs receive it: the program name up to its closing quote
/// or the first blank, then arguments where `2n` backslashes before a quote are `n` backslashes and
/// the quote toggles quoting, `2n+1` are `n` and a literal quote, and `""` inside quotes is a quote.
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn split_windows(line: &str) -> Vec<String> {
    let chars: Vec<char> = line.chars().collect();
    let mut args = Vec::new();
    let mut i = 0;
    // The program name: no escapes.
    let mut program = String::new();
    if chars.first() == Some(&'"') {
        i = 1;
        while let Some(&c) = chars.get(i) {
            i += 1;
            if c == '"' {
                break;
            }
            program.push(c);
        }
    } else {
        while let Some(&c) = chars.get(i) {
            if c == ' ' || c == '\t' {
                break;
            }
            program.push(c);
            i += 1;
        }
    }
    args.push(program);
    loop {
        while matches!(chars.get(i), Some(' ' | '\t')) {
            i += 1;
        }
        if i >= chars.len() {
            break;
        }
        let mut arg = String::new();
        let mut quoted = false;
        while let Some(&c) = chars.get(i) {
            match c {
                '\\' => {
                    let mut n = 0;
                    while chars.get(i) == Some(&'\\') {
                        n += 1;
                        i += 1;
                    }
                    if chars.get(i) == Some(&'"') {
                        arg.extend(std::iter::repeat_n('\\', n / 2));
                        if n % 2 == 1 {
                            arg.push('"');
                            i += 1;
                        }
                    } else {
                        arg.extend(std::iter::repeat_n('\\', n));
                    }
                }
                '"' => {
                    if quoted && chars.get(i + 1) == Some(&'"') {
                        arg.push('"');
                        i += 2;
                    } else {
                        quoted = !quoted;
                        i += 1;
                    }
                }
                ' ' | '\t' if !quoted => break,
                _ => {
                    arg.push(c);
                    i += 1;
                }
            }
        }
        args.push(arg);
    }
    args
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list(a: &[&str]) -> Identity {
        Identity { started: "t".into(), args: Args::List(a.iter().map(|s| s.to_string()).collect()) }
    }

    #[test]
    fn ps_output_is_read_by_its_fixed_columns_and_a_zombie_is_absent() {
        let out = "S    Tue Oct 28 16:53:53 2026     /opt/homebrew/bin/dolt sql-server --config /x y/server.yaml\n";
        let id = parse_ps(out).unwrap().unwrap();
        assert_eq!(id.started, "Tue Oct 28 16:53:53 2026");
        assert_eq!(id.args, Args::Joined("/opt/homebrew/bin/dolt sql-server --config /x y/server.yaml".into()));
        assert!(id.is_dolt_server());
        assert!(id.is(&["/opt/homebrew/bin/dolt".into(), "sql-server".into(), "--config".into(), "/x y/server.yaml".into()]));
        assert_eq!(parse_ps("Z+   Tue Oct 28 16:53:53 2026     <defunct>\n"), Some(None));
        assert_eq!(parse_ps("garbage"), None);
        assert_eq!(parse_ps("S    Tue Oct 28\n"), None);
    }

    /// The expected line takes the form `ps` gives it, byte kind by byte kind, and a joined line
    /// matches only the command whose form it is.
    #[test]
    fn a_command_line_is_written_as_ps_writes_it() {
        assert_eq!(ps_text("/bin/dolt sql-server --config /a b\\c~/server.yaml"), "/bin/dolt sql-server --config /a b\\c~/server.yaml");
        assert_eq!(ps_text("é ü"), "M-CM-) M-CM-<");
        assert_eq!(ps_text("Ä…😀"), "M-CM^DM-bM^@M-&M-pM^_M^XM^@", "the bytes 0x80 to 0x9f");
        assert_eq!(ps_text("a\u{a0}b"), "aM-B\\240b", "a no-break space ends in the byte 0xa0");
        let joined = Identity { started: "t".into(), args: Args::Joined("/é/dolt sql-server".replace('é', "M-CM-)")) };
        assert!(joined.is(&["/é/dolt".into(), "sql-server".into()]) && joined.is_dolt_server());
        assert!(!joined.is(&["/e/dolt".into(), "sql-server".into()]));
    }

    /// A running process is found by its whole command line, characters outside ASCII and
    /// all, and by no line that begins with it, goes on past it or differs in a character.
    /// The list's own helper is no answer: the process is the only one found.
    #[cfg(unix)]
    #[test]
    fn a_running_process_is_found_by_its_exact_command_line_only() {
        use std::os::unix::process::ExitStatusExt;
        let unique = format!("é ü {}", crate::secret::random_name(12).unwrap());
        let line = |more: &[&str]| ["/bin/sh", "-c", "read x", &unique].iter().chain(more).map(|a| a.to_string()).collect::<Vec<_>>();
        let mut child = std::process::Command::new("/bin/sh").args(&line(&[])[1..]).stdin(std::process::Stdio::piped()).spawn().unwrap();
        // Between fork and exec the child still shows this test's command line.
        let until = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let mut found = running(&line(&[])).unwrap();
        while found.is_empty() && std::time::Instant::now() < until {
            std::thread::sleep(std::time::Duration::from_millis(20));
            found = running(&line(&[])).unwrap();
        }
        let longer = running(&line(&["more"])).unwrap();
        let shorter = running(&line(&[])[..3]).unwrap();
        let other = running(&["/bin/sh", "-c", "read x", &unique.replace('ü', "u")].map(String::from)).unwrap();
        // Its input closes, so it ends by itself: nothing here sent it a signal.
        drop(child.stdin.take());
        let status = child.wait().unwrap();
        assert_eq!(found, [child.id()]);
        assert!(!longer.contains(&child.id()) && !shorter.contains(&child.id()) && other.is_empty(), "{longer:?} {shorter:?} {other:?}");
        assert_eq!((status.signal(), running(&line(&[])).unwrap()), (None, Vec::new()), "and it is not found once it ended");
    }

    /// Each line of the list is one process: only the exact line counts, the id column may be
    /// padded, and what cannot be an id is no process.
    #[test]
    fn a_list_from_ps_gives_the_ids_of_exact_lines_only() {
        let wanted = ps_text("/é/dolt sql-server --config /r/af-abcdefgh/server.yaml");
        let out = format!(
            "    7 {wanted}\n12345 {wanted} \n  8 {wanted} --more\n  9 /x{wanted}\n 10 /bin/ps -xww -o pid=,args=\n 11 {}\n0 {wanted}\n\
             4294967295 {wanted}\nx {wanted}\n{wanted}\n\n 12\n",
            wanted.replace("M-CM-)", "e")
        );
        assert_eq!(parse_ps_list(&out, &wanted), Some(vec![7, 12345]));
        assert_eq!(parse_ps_list("  7 \n", ""), Some(Vec::new()), "no command line is no process's");
        assert_eq!((parse_ps_list("", &wanted), parse_ps_list(&format!("7 {wanted}"), &wanted)), (None, None), "a list cut short");
    }

    /// A list whose helper failed, or that filled what is read of it, is an error, not a list.
    #[test]
    fn a_list_that_failed_or_was_cut_is_an_error() {
        let out = |success, stdout: &str| crate::runner::Output { code: Some(0), success, stdout: stdout.into(), stderr: String::new() };
        let ps = std::path::Path::new("/bin/ps");
        assert_eq!(whole(ps, out(true, "123"), 4).unwrap().stdout, "123");
        assert!(matches!(whole(ps, out(true, "1234"), 4), Err(Error::HelperFailed { status, .. }) if status == "too long"));
        assert!(matches!(whole(ps, out(false, "123"), 4), Err(Error::HelperFailed { status, .. }) if status == "exit 0"));
    }

    /// A `/proc` made for the test: only a numbered entry of this user whose whole command line is
    /// the one looked for is found, bytes outside ASCII included; an entry without a command line
    /// is a process that ended; too many entries, too long a time or a line is an error.
    #[cfg(unix)]
    #[test]
    fn a_scan_of_proc_gives_the_ids_of_exact_command_lines_only() {
        let dir = crate::home::tests::Scratch::new("proc");
        let command = ["/é ü/dolt", "sql-server", "--config", "/r/af-abcdefgh/server.yaml"].map(String::from);
        let exact = "/é ü/dolt\0sql-server\0--config\0/r/af-abcdefgh/server.yaml\0";
        let long = format!("{exact}{}", "x".repeat(5000));
        let entries = [("41", exact), ("42", &format!("{exact}--more\0")), ("43", "/é ü/dolt\0sql-server\0"), ("self", exact), ("0", exact)];
        for (name, cmdline) in entries.into_iter().chain([("44", long.as_str()), ("45", &exact.replace('é', "e")), ("46", "")]) {
            std::fs::create_dir(dir.0.join(name)).unwrap();
            std::fs::write(dir.0.join(name).join("cmdline"), cmdline).unwrap();
        }
        std::fs::create_dir(dir.0.join("47")).unwrap();
        // A command that holds the replacement character, and a process with a byte that is no
        // UTF-8 in its place, which a lossy reading would turn into that character.
        for (name, cmdline) in [("51", "/r/\u{fffd}/dolt\0".as_bytes()), ("52", b"/r/\xff/dolt\0")] {
            std::fs::create_dir(dir.0.join(name)).unwrap();
            std::fs::write(dir.0.join(name).join("cmdline"), cmdline).unwrap();
        }
        // A process that may not be read is passed over (when this test's user is not one who may).
        std::fs::create_dir(dir.0.join("49")).unwrap();
        std::fs::write(dir.0.join("49/cmdline"), exact).unwrap();
        std::fs::set_permissions(dir.0.join("49/cmdline"), std::os::unix::fs::PermissionsExt::from_mode(0o000)).unwrap();
        if std::fs::File::open(dir.0.join("49/cmdline")).is_ok() {
            std::fs::remove_dir_all(dir.0.join("49")).unwrap();
        }
        let scan = |command: &[String], most, within| scan_proc(&dir.0, command, most, within);
        let long = std::time::Duration::from_secs(60);
        assert_eq!(scan(&command, 100, long).unwrap(), [41]);
        assert_eq!(scan(&[], 100, long).unwrap(), Vec::<u32>::new());
        assert_eq!(scan(&["/r/\u{fffd}/dolt".to_string()], 100, long).unwrap(), [51]);
        assert!(matches!(scan(&["a".to_string(), "y".repeat(4095)], 100, long), Err(Error::HelperFailed { status, .. }) if status == "too long"));
        assert!(matches!(scan(&command, 9, long), Err(Error::HelperFailed { status, .. }) if status == "too long"));
        assert!(matches!(scan(&command, 100, std::time::Duration::ZERO), Err(Error::HelperTimedOut { .. })));
        assert!(matches!(scan_proc(&dir.0.join("none"), &command, 100, long), Err(Error::Io { .. })));
        // Anything but "it ended" stops the scan: here a folder where the command line should be.
        std::fs::create_dir_all(dir.0.join("48").join("cmdline")).unwrap();
        assert!(matches!(scan(&command, 100, long), Err(Error::Io { .. })));
        // With another user as the list's own, here the owner of `/`, none of them is this user's.
        std::fs::remove_dir_all(dir.0.join("48")).unwrap();
        std::fs::remove_dir_all(dir.0.join("self")).unwrap();
        std::os::unix::fs::symlink("/", dir.0.join("self")).unwrap();
        let root = std::os::unix::fs::MetadataExt::uid(&std::fs::metadata(&dir.0).unwrap()) == 0;
        assert_eq!(scan(&command, 100, long).unwrap(), if root { vec![41] } else { Vec::new() });
    }

    /// PowerShell's list: one object or many, a process without a command line, a quoted program
    /// with blanks; a line that goes on, stops short or is another's is not found, and an answer
    /// that is not such a list is none.
    #[test]
    fn a_list_from_cim_gives_the_ids_of_exact_command_lines_only() {
        let command = [r"C:\Program Files\Archforge\dolt.exe", "sql-server", "--config", r"C:\r\af-abcdefgh\server.yaml"].map(String::from);
        let line = r#"\"C:\\Program Files\\Archforge\\dolt.exe\" sql-server --config C:\\r\\af-abcdefgh\\server.yaml"#;
        let row = |pid: &str, line: &str| format!(r#"{{"ProcessId":{pid},"CommandLine":{line}}}"#);
        let quoted = |l: &str| format!("\"{l}\"");
        let rows = [
            row("7", &quoted(line)),
            row("8", &quoted(&format!("{line} --more"))),
            row("9", &quoted(&line.replace(" --config", ""))),
            row("4", "null"),
            row("0", &quoted(line)),
            row("10", &quoted(&line.replace("Archforge", "Other"))),
        ];
        assert_eq!(parse_cim_list(&format!("[{}]", rows.join(",")), &command), Some(vec![7]));
        assert_eq!(parse_cim_list(&rows[0], &command), Some(vec![7]), "one process comes as one object");
        assert_eq!(parse_cim_list(&format!("[{}]", row("4", "null")), &command), Some(Vec::new()));
        // A line with the replacement character may have held something else there: never found.
        assert_eq!(parse_cim_list(&row("7", "\"C:\\\\\u{fffd}.exe\""), &["C:\\\u{fffd}.exe".to_string()]), Some(Vec::new()));
        assert_eq!(parse_cim_list(&row("7", "\"C:\\\\x.exe\""), &["C:\\x.exe".to_string()]), Some(vec![7]));
        for bad in [
            "",
            "not json",
            "7",
            r#"[{"CommandLine":"x"}]"#,
            r#"[{"ProcessId":"7","CommandLine":"x"}]"#,
            r#"[{"ProcessId":7}]"#,
            r#"[{"ProcessId":4294967296,"CommandLine":"x"}]"#,
        ] {
            assert_eq!(parse_cim_list(bad, &command), None, "{bad}");
        }
    }

    #[test]
    fn proc_stat_survives_a_name_with_brackets_and_spaces() {
        let stat = "4242 (a) b) S 1 4242 4242 0 -1 4194560 100 0 0 0 1 2 0 0 20 0 1 0 987654 1000 10";
        assert_eq!(parse_stat(stat), Some(('S', "987654".to_string())));
        assert_eq!(parse_stat("4242 (x) Z 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 555").map(|s| s.0), Some('Z'));
        assert_eq!(parse_stat("no brackets"), None);
        assert_eq!(
            parse_cmdline(b"/usr/bin/dolt\0sql-server\0--config\0/r/server.yaml\0").unwrap(),
            ["/usr/bin/dolt", "sql-server", "--config", "/r/server.yaml"]
        );
        let outside = list(&["/é ü/dolt", "sql-server"]);
        assert_eq!(
            parse_cmdline("/é ü/dolt\0sql-server\0".as_bytes()).map(Args::List),
            Some(outside.args.clone()),
            "Linux reads the bytes themselves"
        );
        // A byte that is no UTF-8 is not read as the replacement character a command may hold.
        assert_eq!(parse_cmdline("/r/\u{fffd}/x\0".as_bytes()), Some(vec!["/r/\u{fffd}/x".to_string()]));
        assert_eq!(parse_cmdline(b"/r/\xff/x\0"), None);
        assert!(outside.is(&["/é ü/dolt".into(), "sql-server".into()]) && outside.is_dolt_server());
    }

    #[test]
    fn windows_command_lines_split_as_programs_receive_them() {
        assert_eq!(
            split_windows(r#""C:\Program Files\Archforge\dolt.exe" sql-server --config "C:\Users\Ada Example\AppData\af-abc\server.yaml""#),
            [r"C:\Program Files\Archforge\dolt.exe", "sql-server", "--config", r"C:\Users\Ada Example\AppData\af-abc\server.yaml"]
        );
        assert_eq!(split_windows(r#"a.exe "x\\" y"#), ["a.exe", r"x\", "y"]);
        assert_eq!(split_windows(r#"a.exe x\"y "q ""z"" r" s\\t"#), ["a.exe", r#"x"y"#, r#"q "z" r"#, r"s\\t"]);
        assert_eq!(split_windows("a.exe"), ["a.exe"]);
        let id = parse_cim(r#"{"ParentProcessId":4,"CreationDate":"\/Date(1761667200000)\/","CommandLine":"\"C:\\d\\DOLT.EXE\" sql-server --config C:\\r\\server.yaml"}"#).unwrap();
        assert!(id.is_dolt_server(), "the file name compares without regard to case");
        assert_eq!(id.started, r#""/Date(1761667200000)/""#);
        let out = |code, stdout: &str, stderr: &str| crate::runner::Output {
            code: Some(code),
            success: code == 0,
            stdout: stdout.into(),
            stderr: stderr.into(),
        };
        assert_eq!(cim_answer(&out(0, "", "")), Some(None), "a clean empty answer is absence");
        assert_eq!(cim_answer(&out(0, "", "Get-CimInstance : Access denied")), None, "a query that failed is no answer");
        assert_eq!(cim_answer(&out(1, "", "")), None);
        assert_eq!(cim_answer(&out(0, "not json", "")), None);
        let other = parse_cim(r#"{"ParentProcessId":4,"CreationDate":"x","CommandLine":null}"#).unwrap();
        assert!(!other.is_dolt_server() && !other.is(&["x".into()]));
        let replaced = parse_cim("{\"CreationDate\":\"x\",\"CommandLine\":\"C:\\\\\u{fffd}\\\\dolt.exe sql-server\"}").unwrap();
        assert_eq!(replaced.args, Args::Unknown, "a line with the replacement character may have held something else");
    }

    #[test]
    fn a_dolt_server_is_recognised_by_its_program_and_verb_only() {
        assert!(list(&["/usr/local/bin/dolt", "sql-server", "--config", "c"]).is_dolt_server());
        assert!(!list(&["/usr/bin/sleep", "30"]).is_dolt_server());
        assert!(!list(&["/usr/bin/notdolt", "sql-server"]).is_dolt_server());
        assert!(!list(&["/usr/bin/dolt", "sql"]).is_dolt_server());
        let joined = |s: &str| Identity { started: "t".into(), args: Args::Joined(s.into()) };
        assert!(joined("dolt sql-server").is_dolt_server());
        assert!(!joined("/bin/xdolt sql-server").is_dolt_server());
        assert!(!joined("/bin/dolt sql-serverx").is_dolt_server());
        assert!(!Identity { started: "t".into(), args: Args::Unknown }.is_dolt_server());
    }
}
