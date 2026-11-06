//! Finding a runtime: the override variable, then the absolute PATH entries, then the folders
//! installers use; links resolved, a regular executable file required, and `--version` read in a
//! private folder and judged against the accepted range.

use std::ffi::{OsStr, OsString};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use crate::folder::ProbeFolder;
use crate::process::start_thread;
use crate::version::read_version;
use crate::{AgentError, Exit, Fit, PROBE_BUDGET, PROBE_OUTPUT_BYTES, RuntimeKind, Version, Workspace, sys};

/// The most candidates one search probes; past it the search stops, so a PATH of thousands of
/// entries cannot make it hang.
const MAX_CANDIDATES: usize = 16;
/// The most version folders of the Node version manager looked into.
const MAX_NODE_VERSIONS: usize = 32;
/// The most PATH entries read.
const MAX_PATH_ENTRIES: usize = 256;
/// How long the probe waits between looks at a child that has not ended yet.
const POLL: Duration = Duration::from_millis(10);

/// The parts of the host's environment the search and a spawned runtime read: taken from this
/// process by [`HostEnv::from_process`], or given whole, as the tests do.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HostEnv {
    /// The PATH; only its absolute entries are searched.
    pub path: Option<OsString>,
    /// `ARCHFORGE_CLAUDE`: the absolute path of `claude`, which replaces the search.
    pub claude_override: Option<OsString>,
    /// `ARCHFORGE_CODEX`: the absolute path of `codex`, which replaces the search.
    pub codex_override: Option<OsString>,
    /// The folders installers put the runtimes in, searched after the PATH.
    pub known_dirs: Vec<PathBuf>,
    /// `LANG` and the `LC_*` variables, which a spawned runtime keeps.
    pub locale: Vec<(String, OsString)>,
}

impl HostEnv {
    /// This process's environment: the PATH, the two overrides, the locale variables, and the
    /// installers' folders worked out from `HOME` and `NPM_CONFIG_PREFIX`.
    pub fn from_process() -> HostEnv {
        let home = std::env::var_os("HOME").map(PathBuf::from).filter(|h| h.is_absolute());
        let npm_prefix = std::env::var_os("NPM_CONFIG_PREFIX").map(PathBuf::from).filter(|p| p.is_absolute());
        let locale = std::env::vars_os().filter_map(|(k, v)| k.into_string().ok().map(|k| (k, v))).filter(|(k, _)| is_locale_var(k)).collect();
        HostEnv {
            path: std::env::var_os("PATH"),
            claude_override: std::env::var_os(RuntimeKind::Claude.override_var()),
            codex_override: std::env::var_os(RuntimeKind::Codex.override_var()),
            known_dirs: known_dirs(home.as_deref(), npm_prefix.as_deref()),
            locale,
        }
    }

    /// The override for `kind`, when it is set and not empty.
    fn override_for(&self, kind: RuntimeKind) -> Option<&OsStr> {
        let value = match kind {
            RuntimeKind::Claude => self.claude_override.as_deref(),
            RuntimeKind::Codex => self.codex_override.as_deref(),
        };
        value.filter(|v| !v.is_empty())
    }

    /// The PATH's absolute entries, in order; relative and empty entries (which would mean the
    /// current folder) are skipped.
    fn absolute_path_entries(&self) -> Vec<PathBuf> {
        match &self.path {
            Some(path) => {
                let entries: Vec<PathBuf> = std::env::split_paths(path).take(MAX_PATH_ENTRIES.saturating_add(1)).collect();
                if entries.len() > MAX_PATH_ENTRIES {
                    log::warn!("the PATH has more than {MAX_PATH_ENTRIES} entries; only the first {MAX_PATH_ENTRIES} are searched");
                }
                entries.into_iter().take(MAX_PATH_ENTRIES).filter(|d| d.is_absolute()).collect()
            }
            None => Vec::new(),
        }
    }
}

/// Whether `name` is a locale variable a runtime keeps: `LANG` or `LC_*`.
pub(crate) fn is_locale_var(name: &str) -> bool {
    name == "LANG" || name.starts_with("LC_")
}

/// The folders installers put `claude` and `codex` in, after the PATH: the native installer's
/// `~/.local/bin` and Claude's older `~/.claude/local`, npm's usual global prefixes, Volta, Bun,
/// the Node version manager's version folders (newest first), Homebrew and `/usr/local/bin`. An
/// application started from the macOS Finder gets a PATH of the system folders alone, so these
/// are where a runtime installed from a terminal is found.
pub(crate) fn known_dirs(home: Option<&Path>, npm_prefix: Option<&Path>) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(prefix) = npm_prefix {
        dirs.push(prefix.join("bin"));
    }
    if let Some(home) = home {
        for d in [".local/bin", ".claude/local", ".npm-global/bin", ".npm/bin", ".volta/bin", ".bun/bin"] {
            dirs.push(home.join(d));
        }
        dirs.extend(node_version_dirs(&home.join(".nvm/versions/node")));
    }
    if cfg!(target_os = "macos") {
        dirs.push(PathBuf::from("/opt/homebrew/bin"));
    }
    if cfg!(target_os = "linux") {
        dirs.push(PathBuf::from("/home/linuxbrew/.linuxbrew/bin"));
    }
    dirs.push(PathBuf::from("/usr/local/bin"));
    dirs
}

/// The `bin` folder of each Node version under `root` (`v22.3.0`, …), newest first, at most
/// [`MAX_NODE_VERSIONS`] of them.
fn node_version_dirs(root: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(root) else { return Vec::new() };
    let read = MAX_NODE_VERSIONS.saturating_mul(4);
    let entries: Vec<_> = entries.take(read.saturating_add(1)).collect();
    if entries.len() > read {
        log::warn!("{} holds more than {read} entries; only the first {read} read are searched", root.display());
    }
    let mut versions: Vec<(Version, PathBuf)> = entries
        .into_iter()
        .take(read)
        .filter_map(Result::ok)
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            let version = Version::parse(name.strip_prefix('v')?)?;
            Some((version, e.path().join("bin")))
        })
        .collect();
    versions.sort_by_key(|v| std::cmp::Reverse(v.0));
    versions.into_iter().take(MAX_NODE_VERSIONS).map(|(_, p)| p).collect()
}

/// A runtime found, of a version Archforge accepts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Runtime {
    kind: RuntimeKind,
    path: PathBuf,
    found_at: PathBuf,
    version: Version,
    tested: bool,
    child_path: OsString,
    locale: Vec<(String, OsString)>,
}

impl Runtime {
    /// Which runtime this is.
    pub fn kind(&self) -> RuntimeKind {
        self.kind
    }

    /// The program, with every link resolved: what is run.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Where the search found it, before links were resolved.
    pub fn found_at(&self) -> &Path {
        &self.found_at
    }

    /// Its version.
    pub fn version(&self) -> Version {
        self.version
    }

    /// False when the version is newer than the one Archforge was tested with; the interface then
    /// says "not yet tested with this version".
    pub fn tested(&self) -> bool {
        self.tested
    }

    /// The PATH the runtime is given: the folder it was found in first (where an npm, Volta, nvm or
    /// Homebrew install's `node` lives), then the host PATH's absolute entries.
    pub fn child_path(&self) -> &OsStr {
        &self.child_path
    }

    /// The host's locale variables the runtime keeps.
    pub fn locale(&self) -> &[(String, OsString)] {
        &self.locale
    }
}

/// Why a runtime that was found cannot be used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unsupported {
    /// Its version is at or past the first version not supported.
    TooNew { version: Version, below: Version },
    /// `--version` printed something that is not a version in the runtime's form.
    UnreadableVersion { output: String },
    /// `--version` failed.
    ProbeFailed { exit: Exit, output: String },
    /// `--version` did not finish within [`PROBE_BUDGET`].
    ProbeTimedOut { waited: Duration },
    /// `--version` could not be run.
    ProbeNotRun { detail: String },
    /// The path is not a regular executable file.
    NotExecutable,
    /// The override variable names a relative path.
    OverrideNotAbsolute,
    /// Found only in an installers' folder (not on the PATH) that another account could have
    /// written to: `detail` names the place and why.
    Untrusted { detail: String },
}

impl std::fmt::Display for Unsupported {
    /// Writes the reason as the end of a sentence.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Unsupported::TooNew { version, below } => write!(f, "version {version} is not supported (supported: below {below})"),
            Unsupported::UnreadableVersion { output } => write!(f, "its version cannot be read from {output:?}"),
            Unsupported::ProbeFailed { exit, output } => write!(f, "asking its version failed with {exit}: {output:?}"),
            Unsupported::ProbeTimedOut { waited } => write!(f, "asking its version took longer than {} s", waited.as_secs()),
            Unsupported::ProbeNotRun { detail } => write!(f, "asking its version could not run it: {detail}"),
            Unsupported::NotExecutable => f.write_str("it is not an executable file"),
            Unsupported::OverrideNotAbsolute => f.write_str("the override names a relative path; give an absolute one"),
            Unsupported::Untrusted { detail } => write!(f, "it is not on the PATH and its location is not safe: {detail}"),
        }
    }
}

/// What the search for a runtime found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Discovery {
    /// A runtime of an accepted version.
    Found(Runtime),
    /// A runtime older than the floor: counts as missing until it is updated.
    TooOld { kind: RuntimeKind, path: PathBuf, version: Version, floor: Version },
    /// A runtime that cannot be used.
    Unsupported { kind: RuntimeKind, path: PathBuf, reason: Unsupported },
    /// No runtime: `looked` lists the places searched; `override_set` says the override variable
    /// named the one place.
    Missing { kind: RuntimeKind, looked: Vec<PathBuf>, override_set: bool },
    /// Agent runtimes are not available on this system yet.
    NotAvailable { kind: RuntimeKind },
}

impl Discovery {
    /// The runtime found, if one of an accepted version was.
    pub fn runtime(&self) -> Option<&Runtime> {
        match self {
            Discovery::Found(runtime) => Some(runtime),
            _ => None,
        }
    }

    /// The runtime found, or the error that says why there is none.
    pub fn into_result(self) -> Result<Runtime, AgentError> {
        match self {
            Discovery::Found(runtime) => Ok(runtime),
            Discovery::TooOld { kind, path, version, floor } => Err(AgentError::TooOld { runtime: kind, path, found: version, floor }),
            Discovery::Unsupported { kind, path, reason } => Err(AgentError::Unsupported { runtime: kind, path, reason: reason.to_string() }),
            Discovery::Missing { kind, looked, override_set } => {
                let places = looked.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join(", ");
                let looked = if override_set {
                    format!("{} names {places}, which does not exist", kind.override_var())
                } else {
                    format!("{} is not set, and no {} is in {places}", kind.override_var(), kind.program())
                };
                Err(AgentError::Missing { runtime: kind, looked })
            }
            Discovery::NotAvailable { .. } => Err(AgentError::NotAvailable),
        }
    }
}

/// Looks for runtime `kind`. When the override variable is set, it alone is used; there is no
/// fallback to the search. Otherwise the absolute PATH entries and then the installers' folders are
/// searched, and the first candidate of an accepted version wins; when none is accepted, the first
/// candidate's problem is answered. A candidate found only in an installers' folder must sit where
/// no other account can write (owned by this user or the system, and not writable by others). Only
/// `LANG` and `LC_*` entries of `env.locale` are kept. The version probe runs in a private folder of
/// `workspace`; an error is a workspace that cannot hold one. Each probe may take up to
/// [`PROBE_BUDGET`], so call it away from the interface thread.
pub fn find(kind: RuntimeKind, env: &HostEnv, workspace: &Workspace) -> Result<Discovery, AgentError> {
    if !sys::AVAILABLE {
        return Ok(Discovery::NotAvailable { kind });
    }
    let locale: Vec<(String, OsString)> =
        env.locale.iter().filter(|(k, v)| is_locale_var(k) && !v.as_encoded_bytes().contains(&0)).cloned().collect();
    if locale.len() < env.locale.len() {
        log::warn!("{} locale entries that are not LANG or LC_*, or hold a NUL, were left out", env.locale.len().saturating_sub(locale.len()));
    }
    let env = &HostEnv { locale, ..env.clone() };
    if let Some(named) = env.override_for(kind) {
        let named = PathBuf::from(named);
        if !named.is_absolute() {
            return Ok(Discovery::Unsupported { kind, path: named, reason: Unsupported::OverrideNotAbsolute });
        }
        return match resolve(&named) {
            Resolved::Missing => Ok(Discovery::Missing { kind, looked: vec![named], override_set: true }),
            Resolved::NotExecutable => Ok(Discovery::Unsupported { kind, path: named, reason: Unsupported::NotExecutable }),
            Resolved::Program(path) => probe(kind, named, path, env, workspace),
        };
    }
    // Each folder once, and whether it came from the PATH (chosen by the person) or only from the
    // installers' list.
    let mut looked: Vec<(PathBuf, bool)> = Vec::new();
    let on_path = env.absolute_path_entries().into_iter().map(|d| (d, true));
    let known = env.known_dirs.iter().filter(|d| d.is_absolute()).cloned().map(|d| (d, false));
    for (dir, from_path) in on_path.chain(known) {
        if !looked.iter().any(|(d, _)| *d == dir) {
            looked.push((dir, from_path));
        }
    }
    let mut candidates: Vec<(PathBuf, PathBuf, bool)> = Vec::new();
    for (dir, from_path) in &looked {
        let at = dir.join(kind.program());
        if let Resolved::Program(path) = resolve(&at)
            && !candidates.iter().any(|(_, p, _)| *p == path)
        {
            if candidates.len() >= MAX_CANDIDATES {
                log::warn!("more than {MAX_CANDIDATES} copies of {kind} were found; only the first {MAX_CANDIDATES} are checked");
                break;
            }
            candidates.push((at, path, *from_path));
        }
    }
    let mut first_problem = None;
    for (at, path, from_path) in candidates {
        let checked = match (from_path, untrusted(&at, &path)) {
            (false, Some(detail)) => Discovery::Unsupported { kind, path, reason: Unsupported::Untrusted { detail } },
            _ => probe(kind, at, path, env, workspace)?,
        };
        match checked {
            found @ Discovery::Found(_) => return Ok(found),
            problem => {
                first_problem.get_or_insert(problem);
            }
        }
    }
    Ok(first_problem.unwrap_or(Discovery::Missing { kind, looked: looked.into_iter().map(|(d, _)| d).collect(), override_set: false }))
}

/// Why a program found at `at` and resolving to `path` could have been planted by another account,
/// or `None`: the folder it was found in, the program and the program's folder must each be owned
/// by this user or the system and not writable by others.
fn untrusted(at: &Path, path: &Path) -> Option<String> {
    let uid = sys::current_uid();
    [at.parent(), Some(path), path.parent()].into_iter().flatten().find_map(|place| match std::fs::metadata(place) {
        Ok(meta) => sys::shared_write_problem(&meta, uid).map(|why| format!("{} {why}", place.display())),
        Err(e) => Some(format!("{} cannot be read: {e}", place.display())),
    })
}

/// What a candidate path is.
enum Resolved {
    /// Nothing is there (or it cannot be reached).
    Missing,
    /// Something is there, but not a regular executable file.
    NotExecutable,
    /// A regular executable file, at this path with every link resolved.
    Program(PathBuf),
}

/// Resolves `at`'s links and checks the target is a regular executable file.
fn resolve(at: &Path) -> Resolved {
    let Ok(path) = at.canonicalize() else { return Resolved::Missing };
    match std::fs::metadata(&path) {
        Ok(meta) if sys::is_executable(&meta) => Resolved::Program(path),
        Ok(_) => Resolved::NotExecutable,
        Err(_) => Resolved::Missing,
    }
}

/// Runs `path --version` in a private folder and judges the answer.
fn probe(kind: RuntimeKind, found_at: PathBuf, path: PathBuf, env: &HostEnv, workspace: &Workspace) -> Result<Discovery, AgentError> {
    let child_path = child_path(&found_at, env);
    let folder = workspace.probe_folder()?;
    let unsupported = |reason| Ok(Discovery::Unsupported { kind, path: path.clone(), reason });
    let out = match run_probe(kind, &path, &child_path, &env.locale, &folder) {
        Ok(Some(out)) => out,
        Ok(None) => return unsupported(Unsupported::ProbeTimedOut { waited: PROBE_BUDGET }),
        Err(e) => return unsupported(Unsupported::ProbeNotRun { detail: e.to_string() }),
    };
    drop(folder);
    if !out.exit.success() {
        return unsupported(Unsupported::ProbeFailed { exit: out.exit, output: excerpt(&format!("{} {}", out.stdout.trim(), out.stderr.trim())) });
    }
    let Some(version) = read_version(kind, &out.stdout) else { return unsupported(Unsupported::UnreadableVersion { output: excerpt(&out.stdout) }) };
    let range = kind.range();
    let fit = range.fit(version);
    Ok(match fit {
        Fit::TooOld => Discovery::TooOld { kind, path, version, floor: range.floor },
        Fit::TooNew => Discovery::Unsupported { kind, path, reason: Unsupported::TooNew { version, below: range.below } },
        Fit::Tested | Fit::NewerThanTested => {
            let tested = fit == Fit::Tested;
            if !tested {
                log::warn!("{kind} {version} at {} is newer than {}, the version Archforge was tested with", path.display(), range.tested);
            }
            Discovery::Found(Runtime { kind, path, found_at, version, tested, child_path, locale: env.locale.clone() })
        }
    })
}

/// The first 200 characters of `text`, trimmed, for a message.
fn excerpt(text: &str) -> String {
    text.trim().chars().take(200).collect()
}

/// The PATH a runtime is given: the folder it was found in, then the host PATH's absolute entries,
/// each once. An entry holding the separator is left out (and logged), as it cannot be written into
/// a PATH; so every entry kept joins, and the join cannot fail.
fn child_path(found_at: &Path, env: &HostEnv) -> OsString {
    let mut dirs: Vec<PathBuf> = Vec::new();
    for dir in found_at.parent().map(Path::to_path_buf).into_iter().chain(env.absolute_path_entries()) {
        if dirs.contains(&dir) {
            continue;
        }
        if std::env::join_paths([&dir]).is_ok() {
            dirs.push(dir);
        } else {
            log::warn!("{} holds the PATH separator and is left out of the runtime's PATH", dir.display());
        }
    }
    std::env::join_paths(dirs).unwrap_or_default()
}

/// What the probe read.
struct ProbeOutput {
    exit: Exit,
    stdout: String,
    stderr: String,
}

/// Runs `path --version` in its own process group, with the cleared environment and `folder` as
/// its home, settings tree, temporary and working folder. `None` when it did not end within
/// [`PROBE_BUDGET`]; it was then killed with its whole group.
fn run_probe(
    kind: RuntimeKind,
    path: &Path,
    child_path: &OsStr,
    locale: &[(String, OsString)],
    folder: &ProbeFolder,
) -> std::io::Result<Option<ProbeOutput>> {
    let dir = folder.path();
    let mut command = Command::new(path);
    command
        .arg("--version")
        .env_clear()
        .env("PATH", child_path)
        .envs(locale.iter().map(|(k, v)| (k, v)))
        .env("HOME", dir)
        .env(kind.config_var(), dir)
        .env("TMPDIR", dir)
        .current_dir(dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    sys::own_group(&mut command);
    let mut child = crate::process::spawn_retrying(&mut command)?;
    let pid = child.id();
    let (tx, rx) = mpsc::channel();
    let mut readers = Ok(());
    if let Some(out) = child.stdout.take() {
        let tx = tx.clone();
        readers = start_thread(format!("{kind}-probe-stdout"), move || drop(tx.send((true, capped_read(out, kind)))));
    }
    if let Some(err) = child.stderr.take()
        && readers.is_ok()
    {
        readers = start_thread(format!("{kind}-probe-stderr"), move || drop(tx.send((false, capped_read(err, kind)))));
    }
    if let Err(e) = readers {
        sys::kill_group(pid);
        let _ = child.wait();
        return Err(e);
    }
    let started = Instant::now();
    loop {
        match sys::has_ended(&child) {
            Ok(true) => break,
            Ok(false) if started.elapsed() < PROBE_BUDGET => std::thread::sleep(POLL),
            Ok(false) => {
                sys::kill_group(pid);
                let _ = child.wait();
                return Ok(None);
            }
            Err(e) => {
                sys::kill_group(pid);
                let _ = child.wait();
                return Err(e);
            }
        }
    }
    // Anything the probe left running in its group goes with it, before the probe is collected.
    sys::kill_group(pid);
    let exit = sys::exit_of(child.wait()?);
    let (mut stdout, mut stderr) = (String::new(), String::new());
    for _ in 0..2 {
        match rx.recv_timeout(Duration::from_secs(1)) {
            Ok((true, text)) => stdout = text,
            Ok((false, text)) => stderr = text,
            Err(_) => break,
        }
    }
    Ok(Some(ProbeOutput { exit, stdout, stderr }))
}

/// Reads `stream` to its end, keeping its first [`PROBE_OUTPUT_BYTES`], so a talkative probe never
/// blocks on a full pipe.
fn capped_read(stream: impl Read, kind: RuntimeKind) -> String {
    let mut kept = Vec::new();
    let mut limited = stream.take(u64::try_from(PROBE_OUTPUT_BYTES).unwrap_or(u64::MAX));
    let _ = limited.read_to_end(&mut kept);
    let dropped = std::io::copy(&mut limited.into_inner(), &mut std::io::sink()).unwrap_or(0);
    if dropped > 0 {
        log::warn!("{kind} --version wrote {dropped} bytes past the {PROBE_OUTPUT_BYTES} kept");
    }
    String::from_utf8_lossy(&kept).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The installers' folders come in a fixed order, the home's first.
    #[test]
    fn the_installers_folders_come_from_home_and_npm_prefix() {
        let dirs = known_dirs(Some(Path::new("/h")), Some(Path::new("/npm")));
        assert_eq!(dirs.first(), Some(&PathBuf::from("/npm/bin")));
        assert!(dirs.contains(&PathBuf::from("/h/.local/bin")) && dirs.contains(&PathBuf::from("/h/.claude/local")));
        assert_eq!(dirs.last(), Some(&PathBuf::from("/usr/local/bin")));
        assert!(known_dirs(None, None).iter().all(|d| !d.starts_with("/h")));
    }

    /// Node version folders are taken newest first, and anything not a version is skipped.
    #[test]
    fn node_versions_are_searched_newest_first() {
        let root = std::env::temp_dir().join(format!("archforge-agents-nvm-{}", std::process::id()));
        for v in ["v18.2.0", "v22.11.0", "v20.0.1", "system", "v1.2"] {
            std::fs::create_dir_all(root.join(v)).unwrap();
        }
        assert_eq!(node_version_dirs(&root), vec![root.join("v22.11.0/bin"), root.join("v20.0.1/bin"), root.join("v18.2.0/bin")]);
        std::fs::remove_dir_all(&root).unwrap();
        assert!(node_version_dirs(&root).is_empty());
    }

    /// Only `LANG` and `LC_*` count as locale variables.
    #[test]
    fn only_lang_and_lc_variables_are_locale() {
        assert!(is_locale_var("LANG") && is_locale_var("LC_ALL") && is_locale_var("LC_CTYPE"));
        assert!(!is_locale_var("LANGUAGE") && !is_locale_var("HOME") && !is_locale_var("XLC_ALL"));
    }

    /// The child's PATH puts the runtime's folder first and skips relative entries.
    #[test]
    fn the_child_path_starts_with_the_runtime_s_folders() {
        let env = HostEnv { path: Some(std::env::join_paths(["/usr/bin", "rel", "", "/bin", "/a"]).unwrap()), ..HostEnv::default() };
        let joined = child_path(Path::new("/a/claude"), &env);
        let dirs: Vec<PathBuf> = std::env::split_paths(&joined).collect();
        assert_eq!(dirs, ["/a", "/usr/bin", "/bin"].map(PathBuf::from));
    }
}
