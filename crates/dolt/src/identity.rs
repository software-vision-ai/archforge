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

#[cfg(target_os = "linux")]
mod platform {
    use std::io::Read;
    use std::path::PathBuf;

    use super::{Args, Identity, parse_cmdline, parse_stat};
    use crate::Error;

    const CAP: u64 = 4096;

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
            Some(bytes) => Args::List(parse_cmdline(&bytes)),
        };
        Ok(Some(Identity { started, args }))
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

/// `/proc/<pid>/cmdline`: NUL-separated arguments.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub(crate) fn parse_cmdline(bytes: &[u8]) -> Vec<String> {
    let mut args: Vec<String> = bytes.split(|b| *b == 0).map(|a| String::from_utf8_lossy(a).into_owned()).collect();
    if args.last().is_some_and(String::is_empty) {
        args.pop();
    }
    args
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
        serde_json::Value::String(s) => Args::List(split_windows(s)),
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

    #[test]
    fn proc_stat_survives_a_name_with_brackets_and_spaces() {
        let stat = "4242 (a) b) S 1 4242 4242 0 -1 4194560 100 0 0 0 1 2 0 0 20 0 1 0 987654 1000 10";
        assert_eq!(parse_stat(stat), Some(('S', "987654".to_string())));
        assert_eq!(parse_stat("4242 (x) Z 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 555").map(|s| s.0), Some('Z'));
        assert_eq!(parse_stat("no brackets"), None);
        assert_eq!(
            parse_cmdline(b"/usr/bin/dolt\0sql-server\0--config\0/r/server.yaml\0"),
            ["/usr/bin/dolt", "sql-server", "--config", "/r/server.yaml"]
        );
        let outside = list(&["/é ü/dolt", "sql-server"]);
        assert_eq!(Args::List(parse_cmdline("/é ü/dolt\0sql-server\0".as_bytes())), outside.args, "Linux reads the bytes themselves");
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
