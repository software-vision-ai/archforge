//! The runtimes found, checked, run and supervised, against fake runtimes: small shell scripts
//! that answer `--version` as the real programs do and then behave as each test needs.
#![cfg(unix)]

use std::collections::BTreeSet;
use std::ffi::OsString;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use archforge_agents::{
    AgentError, ConversationFolder, ConversationId, Discovery, Dropped, ErrorKind, HostEnv, Limits, OutputEvent, Runtime, RuntimeKind,
    RuntimeProcess, SpawnRequest, Unsupported, Version, Workspace, find, spawn,
};
use archforge_secrets::Secret;

/// A key no output of this crate may ever show.
const CANARY: &str = "sk-ant-api03-CANARY-7d1f0c9e2b4a";

/// What Claude 2.1.317 prints for `--version`.
const CLAUDE_TESTED: &str = "2.1.317 (Claude Code)";

/// A fresh folder in the temporary directory for one test, named with the process id.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("archforge-agents-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir.canonicalize().unwrap()
}

/// Writes an executable shell script at `path`.
fn script(path: &Path, body: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, format!("#!/bin/sh\n{body}\n")).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

/// A fake runtime at `path` that prints `version` for `--version` and otherwise runs `body`.
fn fake(path: &Path, version: &str, body: &str) {
    script(path, &format!("if [ \"$1\" = \"--version\" ]; then echo \"{version}\"; exit 0; fi\n{body}"));
}

/// A host environment with only `path` entries on the PATH and `known` as the installers' folders.
fn host(path: &[&Path], known: &[&Path]) -> HostEnv {
    HostEnv {
        path: Some(std::env::join_paths(path).unwrap()),
        known_dirs: known.iter().map(|p| p.to_path_buf()).collect(),
        locale: vec![("LANG".into(), "C.UTF-8".into())],
        ..HostEnv::default()
    }
}

/// The workspace of a test, in its scratch folder.
fn workspace(base: &Path) -> Workspace {
    Workspace::open(&base.join("workspace")).unwrap()
}

/// A Claude fake at `base/bin/claude` running `body`, found through the override, with a fresh
/// conversation folder.
fn claude(base: &Path, body: &str) -> (Runtime, ConversationFolder) {
    let program = base.join("bin/claude");
    fake(&program, CLAUDE_TESTED, body);
    let env = HostEnv { claude_override: Some(program.into()), ..host(&[Path::new("/usr/bin"), Path::new("/bin")], &[]) };
    let ws = workspace(base);
    let runtime = find(RuntimeKind::Claude, &env, &ws).unwrap().into_result().unwrap();
    let folder = ws.conversation(&ConversationId::new("c1").unwrap()).unwrap();
    (runtime, folder)
}

/// The canary as a secret.
fn canary() -> Secret {
    Secret::new(CANARY.to_owned()).unwrap()
}

/// The next line of output within five seconds; panics on anything else.
fn next_line(process: &RuntimeProcess) -> String {
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        match process.next_event(Duration::from_millis(100)) {
            Some(OutputEvent::Line(line)) => return line,
            Some(other) => panic!("expected a line, got {other:?}"),
            None => {}
        }
    }
    panic!("no line within five seconds");
}

/// Every output event up to the end, within five seconds.
fn all_events(process: &RuntimeProcess) -> Vec<OutputEvent> {
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut events = Vec::new();
    while Instant::now() < deadline {
        match process.next_event(Duration::from_millis(100)) {
            Some(OutputEvent::End) => return events,
            Some(event) => events.push(event),
            None => {}
        }
    }
    panic!("the output did not end within five seconds: {events:?}");
}

/// Waits up to five seconds for the runtime to end.
fn wait_exit(process: &mut RuntimeProcess) -> archforge_agents::Exit {
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if let Some(exit) = process.status().unwrap() {
            return exit;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("the runtime did not end within five seconds");
}

/// Waits up to five seconds for the error tail to hold `text`.
fn wait_tail(process: &RuntimeProcess, text: &str) -> String {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let tail = process.stderr_tail();
        if tail.contains(text) || Instant::now() >= deadline {
            return tail;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Whether process `pid` still exists (a zombie not yet collected by the system counts).
fn alive(pid: i32) -> bool {
    rustix::process::test_kill_process(rustix::process::Pid::from_raw(pid).unwrap()).is_ok()
}

/// Whether any process is left in the group `pgid` (its leader collected).
fn group_alive(pgid: i32) -> bool {
    rustix::process::test_kill_process_group(rustix::process::Pid::from_raw(pgid).unwrap()).is_ok()
}

/// Waits up to ten seconds for the group `pgid` to be empty, and says whether it is.
fn group_gone(pgid: i32) -> bool {
    let deadline = Instant::now() + Duration::from_secs(10);
    while group_alive(pgid) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    !group_alive(pgid)
}

/// Every way a value can be shown: debug, pretty debug, and display for errors.
fn shown(error: &AgentError) -> String {
    format!("{error:?}\n{error:#?}\n{error}")
}

// Discovery.

/// The override wins over the PATH, the PATH over the installers' folders, and the first absolute
/// PATH entry over the later ones.
#[test]
fn discovery_takes_the_override_then_the_path_then_the_installers_folders() {
    let base = scratch("order");
    let ws = workspace(&base);
    let (first, second, known, named) = (base.join("first"), base.join("second"), base.join("known"), base.join("named"));
    for dir in [&second, &known, &named] {
        fake(&dir.join("claude"), CLAUDE_TESTED, "exit 0");
    }
    let found = |env: &HostEnv| find(RuntimeKind::Claude, env, &ws).unwrap().runtime().map(|r| r.path().to_path_buf());

    // Only the installers' folder has it.
    assert_eq!(found(&host(&[&first], &[&known])), Some(known.join("claude")));
    // The PATH has it: the PATH wins, and of its entries the first that has it.
    assert_eq!(found(&host(&[&first, &second], &[&known])), Some(second.join("claude")));
    fake(&first.join("claude"), CLAUDE_TESTED, "exit 0");
    assert_eq!(found(&host(&[&first, &second], &[&known])), Some(first.join("claude")));
    // The override wins over everything.
    let env = HostEnv { claude_override: Some(named.join("claude").into()), ..host(&[&first], &[&known]) };
    assert_eq!(found(&env), Some(named.join("claude")));
    // An empty override counts as unset.
    let env = HostEnv { claude_override: Some(OsString::new()), ..host(&[&first], &[]) };
    assert_eq!(found(&env), Some(first.join("claude")));
    // Codex's override is its own.
    let env = HostEnv { codex_override: Some(named.join("claude").into()), ..host(&[&first], &[]) };
    assert_eq!(found(&env), Some(first.join("claude")));
    std::fs::remove_dir_all(&base).unwrap();
}

/// A relative or empty PATH entry (the current folder) is never searched.
#[test]
fn relative_and_empty_path_entries_are_ignored() {
    let base = scratch("relative");
    let ws = workspace(&base);
    let dir = base.join("rel");
    fake(&dir.join("claude"), CLAUDE_TESTED, "exit 0");
    // The same folder, written relative to the current folder.
    let cwd = std::env::current_dir().unwrap();
    let up = "../".repeat(cwd.components().count().saturating_sub(1));
    let relative = PathBuf::from(format!("{up}{}", dir.display().to_string().trim_start_matches('/')));
    assert!(relative.is_relative() && relative.join("claude").is_file(), "{}", relative.display());
    let env = HostEnv { path: Some(std::env::join_paths([relative.as_path(), Path::new("")]).unwrap()), ..HostEnv::default() };
    match find(RuntimeKind::Claude, &env, &ws).unwrap() {
        Discovery::Missing { looked, override_set: false, .. } => assert!(looked.is_empty(), "{looked:?}"),
        other => panic!("expected missing, got {other:?}"),
    }
    // Working from inside the folder makes no difference either.
    let env = HostEnv { path: Some(OsString::from(":.:")), ..HostEnv::default() };
    assert!(matches!(find(RuntimeKind::Claude, &env, &ws).unwrap(), Discovery::Missing { .. }));
    std::fs::remove_dir_all(&base).unwrap();
}

/// Links are resolved to the program they point at; a file without an execute bit or a folder
/// named like the program is passed over.
#[test]
fn links_are_resolved_and_only_executable_files_count() {
    let base = scratch("links");
    let ws = workspace(&base);
    let (real, linked, plain, folder) = (base.join("real"), base.join("linked"), base.join("plain"), base.join("folder"));
    fake(&real.join("cli.js"), CLAUDE_TESTED, "exit 0");
    std::fs::create_dir_all(&linked).unwrap();
    std::os::unix::fs::symlink(real.join("cli.js"), linked.join("claude")).unwrap();
    std::fs::create_dir_all(&plain).unwrap();
    std::fs::write(plain.join("claude"), "#!/bin/sh\necho 2.1.317\n").unwrap();
    std::fs::create_dir_all(folder.join("claude")).unwrap();

    let env = host(&[&plain, &folder, &linked], &[]);
    let runtime = find(RuntimeKind::Claude, &env, &ws).unwrap().into_result().unwrap();
    assert_eq!(runtime.path(), real.join("cli.js"));
    assert_eq!(runtime.found_at(), linked.join("claude"));
    // The child's PATH starts with where it was found, where an npm install's `node` lives.
    let dirs: Vec<PathBuf> = std::env::split_paths(runtime.child_path()).collect();
    assert_eq!(dirs.first(), Some(&linked));
    assert!(!dirs.contains(&real), "{dirs:?}");
    // Neither the plain file nor the folder is a candidate.
    assert!(matches!(find(RuntimeKind::Claude, &host(&[&plain, &folder], &[]), &ws).unwrap(), Discovery::Missing { .. }));
    // A link that points nowhere is missing.
    let dangling = base.join("dangling");
    std::fs::create_dir_all(&dangling).unwrap();
    std::os::unix::fs::symlink(base.join("nothing"), dangling.join("claude")).unwrap();
    assert!(matches!(find(RuntimeKind::Claude, &host(&[&dangling], &[]), &ws).unwrap(), Discovery::Missing { .. }));
    std::fs::remove_dir_all(&base).unwrap();
}

/// An override that is relative, missing or not executable is answered as such, with no fallback
/// to a runtime on the PATH.
#[test]
fn a_bad_override_is_reported_and_never_falls_back() {
    let base = scratch("override");
    let ws = workspace(&base);
    let on_path = base.join("bin");
    fake(&on_path.join("codex"), "codex-cli 0.166.0", "exit 0");
    let with = |value: &str| HostEnv { codex_override: Some(value.into()), ..host(&[&on_path], &[]) };
    assert!(matches!(
        find(RuntimeKind::Codex, &with("bin/codex"), &ws).unwrap(),
        Discovery::Unsupported { reason: Unsupported::OverrideNotAbsolute, .. }
    ));
    let missing = base.join("none/codex");
    match find(RuntimeKind::Codex, &with(missing.to_str().unwrap()), &ws).unwrap() {
        Discovery::Missing { looked, override_set: true, .. } => assert_eq!(looked, vec![missing.clone()]),
        other => panic!("expected missing, got {other:?}"),
    }
    let error = find(RuntimeKind::Codex, &with(missing.to_str().unwrap()), &ws).unwrap().into_result().unwrap_err();
    assert_eq!(error.kind(), ErrorKind::MissingRuntime);
    assert!(error.to_string().contains("ARCHFORGE_CODEX names"), "{error}");
    let plain = base.join("plain-codex");
    std::fs::write(&plain, "").unwrap();
    assert!(matches!(
        find(RuntimeKind::Codex, &with(plain.to_str().unwrap()), &ws).unwrap(),
        Discovery::Unsupported { reason: Unsupported::NotExecutable, .. }
    ));
    std::fs::remove_dir_all(&base).unwrap();
}

/// The version decides: tested, newer within the range (accepted, not tested), too old, the next
/// major refused, and output that is not a version refused.
#[test]
fn the_version_decides_tested_newer_too_old_and_unsupported() {
    let base = scratch("versions");
    let ws = workspace(&base);
    let probe = |kind: RuntimeKind, answer: &str| {
        let program = base.join(format!("v{}/{}", answer.len(), kind.program()));
        let _ = std::fs::remove_file(&program);
        script(&program, &format!("echo '{answer}'"));
        let env = match kind {
            RuntimeKind::Claude => HostEnv { claude_override: Some(program.into()), ..HostEnv::default() },
            RuntimeKind::Codex => HostEnv { codex_override: Some(program.into()), ..HostEnv::default() },
        };
        find(kind, &env, &ws).unwrap()
    };
    let found = probe(RuntimeKind::Claude, "2.1.317 (Claude Code)");
    let runtime = found.runtime().unwrap();
    assert_eq!((runtime.version(), runtime.tested()), (Version::new(2, 1, 317), true));
    let found = probe(RuntimeKind::Claude, "2.4.0 (Claude Code)");
    assert_eq!(found.runtime().map(|r| r.tested()), Some(false), "newer in the range: accepted, not yet tested");
    let found = probe(RuntimeKind::Codex, "codex-cli 0.170.1");
    assert_eq!(found.runtime().map(|r| (r.version(), r.tested())), Some((Version::new(0, 170, 1), false)));
    assert!(matches!(probe(RuntimeKind::Claude, "2.1.200 (Claude Code)"), Discovery::TooOld { version, floor, .. }
        if version == Version::new(2, 1, 200) && floor == Version::new(2, 1, 286)));
    assert!(matches!(probe(RuntimeKind::Claude, "3.0.0 (Claude Code)"), Discovery::Unsupported { reason: Unsupported::TooNew { .. }, .. }));
    assert!(matches!(probe(RuntimeKind::Codex, "codex-cli 1.0.0"), Discovery::Unsupported { reason: Unsupported::TooNew { .. }, .. }));
    assert!(matches!(probe(RuntimeKind::Codex, "0.166.0"), Discovery::Unsupported { reason: Unsupported::UnreadableVersion { .. }, .. }));
    let too_old = probe(RuntimeKind::Codex, "codex-cli 0.100.0").into_result().unwrap_err();
    assert_eq!(too_old.kind(), ErrorKind::MissingRuntime);
    std::fs::remove_dir_all(&base).unwrap();
}

/// A probe that fails or hangs is refused, and a hanging one is killed within the budget.
#[test]
fn a_failing_or_hanging_version_probe_is_refused() {
    let base = scratch("probe");
    let ws = workspace(&base);
    let failing = base.join("failing/claude");
    script(&failing, "echo 'no such option' >&2; exit 2");
    let env = HostEnv { claude_override: Some(failing.into()), ..HostEnv::default() };
    assert!(matches!(find(RuntimeKind::Claude, &env, &ws).unwrap(), Discovery::Unsupported { reason: Unsupported::ProbeFailed { exit, output }, .. }
        if exit.code == Some(2) && output.contains("no such option")));
    let hanging = base.join("hanging/claude");
    script(&hanging, "/bin/sleep 30");
    let env = HostEnv { claude_override: Some(hanging.into()), ..HostEnv::default() };
    let started = Instant::now();
    assert!(matches!(find(RuntimeKind::Claude, &env, &ws).unwrap(), Discovery::Unsupported { reason: Unsupported::ProbeTimedOut { .. }, .. }));
    assert!(started.elapsed() < Duration::from_secs(10));
    // Every probe folder is gone again.
    let left: Vec<_> = std::fs::read_dir(ws.root()).unwrap().filter_map(Result::ok).map(|e| e.file_name()).collect();
    assert!(left.is_empty(), "{left:?}");
    std::fs::remove_dir_all(&base).unwrap();
}

/// A runtime found only in an installers' folder that other users can write to is refused; the same
/// folder on the PATH, which the person chose, is searched as it is.
#[test]
fn an_installers_folder_others_can_write_to_is_not_trusted() {
    let base = scratch("untrusted");
    let ws = workspace(&base);
    let shared = base.join("shared");
    fake(&shared.join("claude"), CLAUDE_TESTED, "exit 0");
    std::fs::set_permissions(&shared, std::fs::Permissions::from_mode(0o777)).unwrap();
    match find(RuntimeKind::Claude, &host(&[], &[&shared]), &ws).unwrap() {
        Discovery::Unsupported { reason: Unsupported::Untrusted { detail }, .. } => assert!(detail.contains("written by other users"), "{detail}"),
        other => panic!("expected untrusted, got {other:?}"),
    }
    assert!(find(RuntimeKind::Claude, &host(&[&shared], &[]), &ws).unwrap().runtime().is_some());
    // A program others can write to is refused the same way.
    std::fs::set_permissions(&shared, std::fs::Permissions::from_mode(0o755)).unwrap();
    std::fs::set_permissions(shared.join("claude"), std::fs::Permissions::from_mode(0o777)).unwrap();
    assert!(matches!(
        find(RuntimeKind::Claude, &host(&[], &[&shared]), &ws).unwrap(),
        Discovery::Unsupported { reason: Unsupported::Untrusted { .. }, .. }
    ));
    std::fs::set_permissions(shared.join("claude"), std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(find(RuntimeKind::Claude, &host(&[], &[&shared]), &ws).unwrap().runtime().is_some());
    std::fs::remove_dir_all(&base).unwrap();
}

/// Only `LANG` and `LC_*` entries of the host's locale reach a runtime, whatever else is put there.
#[test]
fn only_locale_variables_pass_through_the_host_locale() {
    let base = scratch("locale");
    let ws = workspace(&base);
    let program = base.join("bin/claude");
    fake(&program, CLAUDE_TESTED, "exit 0");
    let env = HostEnv {
        claude_override: Some(program.into()),
        locale: vec![
            ("LANG".into(), "C.UTF-8".into()),
            ("PATH".into(), "/evil".into()),
            ("ANTHROPIC_API_KEY".into(), CANARY.into()),
            ("LC_ALL".into(), "C\0x".into()),
            ("LC_CTYPE".into(), "C".into()),
        ],
        ..HostEnv::default()
    };
    let runtime = find(RuntimeKind::Claude, &env, &ws).unwrap().into_result().unwrap();
    let names: Vec<&str> = runtime.locale().iter().map(|(k, _)| k.as_str()).collect();
    assert_eq!(names, ["LANG", "LC_CTYPE"]);
    std::fs::remove_dir_all(&base).unwrap();
}

/// An old runtime earlier on the PATH does not hide an accepted one later; with only old ones, the
/// first one's problem is answered.
#[test]
fn the_first_accepted_runtime_wins_over_an_older_one() {
    let base = scratch("first-accepted");
    let ws = workspace(&base);
    let (old, good) = (base.join("old"), base.join("good"));
    fake(&old.join("codex"), "codex-cli 0.100.0", "exit 0");
    assert!(matches!(find(RuntimeKind::Codex, &host(&[&old], &[&good]), &ws).unwrap(), Discovery::TooOld { .. }));
    fake(&good.join("codex"), "codex-cli 0.166.0", "exit 0");
    let found = find(RuntimeKind::Codex, &host(&[&old], &[&good]), &ws).unwrap();
    assert_eq!(found.runtime().map(|r| r.path().to_path_buf()), Some(good.join("codex")));
    std::fs::remove_dir_all(&base).unwrap();
}

// Private folders.

/// The workspace and a conversation's folders are made private (mode 0700) and removed whole.
#[test]
fn conversation_folders_are_private_and_removed() {
    let base = scratch("folders");
    let ws = workspace(&base);
    let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(ws.root()), 0o700);
    let id = ConversationId::new("6f1c0a52-9d3e").unwrap();
    let folder = ws.conversation(&id).unwrap();
    for dir in [folder.path(), folder.home(), folder.config(), folder.work(), folder.tmp()] {
        assert_eq!(mode(dir), 0o700, "{}", dir.display());
        assert!(dir.starts_with(ws.root()));
    }
    assert_eq!(std::fs::read_dir(folder.work()).unwrap().count(), 0, "the working folder starts empty");
    std::fs::write(folder.config().join("transcript.jsonl"), "{}").unwrap();
    // Opening it again keeps what the runtime wrote, for a resume.
    let again = ws.conversation(&id).unwrap();
    assert!(again.config().join("transcript.jsonl").is_file());
    assert!(ws.remove_conversation(&id).unwrap());
    assert!(!folder.path().exists());
    assert!(!ws.remove_conversation(&id).unwrap(), "removing it twice finds nothing");
    std::fs::remove_dir_all(&base).unwrap();
}

/// A workspace others can read, a relative one, and a conversation folder that is a link are refused
/// and left as they are.
#[test]
fn folders_that_are_not_private_are_refused_and_left_alone() {
    let base = scratch("unsafe-folders");
    let shared = base.join("shared");
    std::fs::create_dir_all(&shared).unwrap();
    std::fs::set_permissions(&shared, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(matches!(Workspace::open(&shared), Err(AgentError::Folder { problem, .. }) if problem.contains("mode 700")));
    assert_eq!(std::fs::metadata(&shared).unwrap().permissions().mode() & 0o777, 0o755, "never changed");
    assert!(matches!(Workspace::open(Path::new("relative/ws")), Err(AgentError::Folder { .. })));

    let ws = workspace(&base);
    let target = base.join("target");
    std::fs::create_dir_all(&target).unwrap();
    std::fs::write(target.join("keep"), "x").unwrap();
    std::os::unix::fs::symlink(&target, ws.root().join("conversation-linked")).unwrap();
    let id = ConversationId::new("linked").unwrap();
    assert!(matches!(ws.conversation(&id), Err(AgentError::Folder { problem, .. }) if problem.contains("link")));
    assert!(matches!(ws.remove_conversation(&id), Err(AgentError::Folder { .. })));
    assert!(target.join("keep").is_file());
    std::fs::remove_dir_all(&base).unwrap();
}

// Spawning.

/// The runtime sees only the allowed variables, its private folders and the caller's variables;
/// the key arrives through its variable and nowhere else.
#[test]
fn the_environment_is_cleared_but_for_the_allowlist_and_the_given_variables() {
    let base = scratch("env");
    let (runtime, folder) = claude(&base, "echo \"cwd=$(/bin/pwd -P)\"; echo \"args=$*\"; exec /usr/bin/env");
    let request = SpawnRequest {
        args: vec!["-p".into(), "--bare".into()],
        vars: vec![("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC".into(), "1".into())],
        secret_vars: vec![("ANTHROPIC_API_KEY".into(), canary())],
        limits: Limits::default(),
        ..SpawnRequest::default()
    };
    let request_debug = format!("{request:?}");
    assert!(!request_debug.contains(CANARY), "{request_debug}");
    let process = spawn(&runtime, &folder, request).unwrap();
    let process_debug = format!("{process:?}");
    assert!(!process_debug.contains(CANARY), "{process_debug}");
    let lines: Vec<String> = all_events(&process)
        .into_iter()
        .map(|e| match e {
            OutputEvent::Line(l) => l,
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(lines.first().map(String::as_str), Some(format!("cwd={}", folder.work().display()).as_str()));
    assert_eq!(lines.get(1).map(String::as_str), Some("args=-p --bare"), "the key is not on the command line");
    let vars: Vec<(&str, &str)> = lines.iter().skip(2).filter_map(|l| l.split_once('=')).collect();
    let names: BTreeSet<&str> = vars.iter().map(|(k, _)| *k).collect();
    // What the shell running the fake adds by itself.
    let shell_added = ["PWD", "SHLVL", "_", "OLDPWD"];
    let expected = ["PATH", "LANG", "HOME", "TMPDIR", "CLAUDE_CONFIG_DIR", "CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC", "ANTHROPIC_API_KEY"];
    let unexpected: Vec<&&str> = names.iter().filter(|n| !expected.contains(n) && !shell_added.contains(n)).collect();
    assert!(unexpected.is_empty(), "unexpected variables {unexpected:?} in {vars:?}");
    for name in expected {
        assert!(names.contains(name), "{name} missing from {vars:?}");
    }
    let value = |name: &str| vars.iter().find(|(k, _)| *k == name).map(|(_, v)| *v).unwrap();
    assert_eq!(value("HOME"), folder.home().to_str().unwrap());
    assert_eq!(value("TMPDIR"), folder.tmp().to_str().unwrap());
    assert_eq!(value("CLAUDE_CONFIG_DIR"), folder.config().to_str().unwrap());
    assert_eq!(value("PATH"), runtime.child_path().to_str().unwrap());
    assert_eq!(value("LANG"), "C.UTF-8");
    assert_eq!(value("ANTHROPIC_API_KEY"), CANARY, "the key reaches the runtime through its variable");
    std::fs::remove_dir_all(&base).unwrap();
}

/// No error, `Debug` or error tail shows the key: a key in the command line or a plain variable is
/// refused by name, and a runtime that echoes its key to its error stream has it scrubbed.
#[test]
fn the_key_never_appears_in_errors_debug_or_the_error_tail() {
    let base = scratch("canary");
    let (runtime, folder) = claude(&base, "echo \"auth failed for key=$ANTHROPIC_API_KEY\" >&2; exit 1");
    let refused = |args: Vec<OsString>, vars: Vec<(String, OsString)>| {
        let request = SpawnRequest { args, vars, secret_vars: vec![("ANTHROPIC_API_KEY".into(), canary())], ..SpawnRequest::default() };
        spawn(&runtime, &folder, request).unwrap_err()
    };
    let in_args = refused(vec![format!("--api-key={CANARY}").into()], vec![]);
    assert!(matches!(&in_args, AgentError::SecretExposed { variable, place: "command line" } if variable == "ANTHROPIC_API_KEY"));
    let in_vars = refused(vec![], vec![("OTHER".into(), CANARY.into())]);
    assert!(matches!(&in_vars, AgentError::SecretExposed { place: "plain variables", .. }));
    // A key pasted where a name belongs is not repeated either.
    let request = SpawnRequest { secret_vars: vec![(CANARY.into(), canary())], ..SpawnRequest::default() };
    let bad_name = spawn(&runtime, &folder, request).unwrap_err();
    for error in [&in_args, &in_vars, &bad_name] {
        assert!(!shown(error).contains(CANARY), "{}", shown(error));
    }

    let request = SpawnRequest { secret_vars: vec![("ANTHROPIC_API_KEY".into(), canary())], ..SpawnRequest::default() };
    let mut process = spawn(&runtime, &folder, request).unwrap();
    let exit = wait_exit(&mut process);
    assert_eq!(exit.code, Some(1));
    let tail = wait_tail(&process, "auth failed");
    assert!(tail.contains("auth failed for key=<redacted>") && !tail.contains(CANARY), "{tail}");
    let error = process.exit_error(exit);
    assert!(matches!(error, AgentError::Exited { code: Some(1), .. }) && error.kind() == ErrorKind::Other);
    assert!(!shown(&error).contains(CANARY) && shown(&error).contains("<redacted>"), "{}", shown(&error));
    assert!(!format!("{process:?}").contains(CANARY));
    std::fs::remove_dir_all(&base).unwrap();
}

/// A runtime that cannot be started is a typed error in the missing-runtime category.
#[test]
fn a_runtime_gone_before_its_start_is_a_missing_runtime() {
    let base = scratch("gone");
    let (runtime, folder) = claude(&base, "exit 0");
    std::fs::remove_file(runtime.path()).unwrap();
    let error = spawn(&runtime, &folder, SpawnRequest::default()).unwrap_err();
    assert!(matches!(error, AgentError::Spawn { not_found: true, .. }) && error.kind() == ErrorKind::MissingRuntime, "{error:?}");
    std::fs::remove_dir_all(&base).unwrap();
}

// Supervision.

/// A line past the cap is dropped and reported by its length; the lines around it arrive.
#[test]
fn an_output_line_past_the_cap_is_dropped_and_reported() {
    let base = scratch("line-cap");
    let (runtime, folder) = claude(&base, "echo first; /usr/bin/head -c 5000 /dev/zero | /usr/bin/tr '\\0' x; echo; echo after");
    let limits = Limits { max_line_bytes: 1024, ..Limits::default() };
    let process = spawn(&runtime, &folder, SpawnRequest { limits, ..SpawnRequest::default() }).unwrap();
    assert_eq!(
        all_events(&process),
        vec![OutputEvent::Line("first".into()), OutputEvent::Dropped(Dropped::TooLong { bytes: 5000 }), OutputEvent::Line("after".into())]
    );
    assert_eq!(process.next_event(Duration::from_millis(10)), Some(OutputEvent::End), "the end repeats");
    std::fs::remove_dir_all(&base).unwrap();
}

/// The error stream keeps only its last bytes, from a line start, and a crash carries them.
#[test]
fn the_error_tail_is_capped_and_carried_by_the_exit_error() {
    let base = scratch("tail");
    let (runtime, folder) = claude(&base, "i=0; while [ $i -lt 200 ]; do echo \"line $i\" >&2; i=$((i+1)); done; exit 3");
    let limits = Limits { stderr_tail_bytes: 256, ..Limits::default() };
    let mut process = spawn(&runtime, &folder, SpawnRequest { limits, ..SpawnRequest::default() }).unwrap();
    let exit = wait_exit(&mut process);
    let tail = wait_tail(&process, "line 199");
    assert!(tail.len() <= 256 && tail.starts_with("line ") && tail.ends_with("line 199\n"), "{tail:?}");
    assert!(!tail.contains("line 0\n"));
    match process.exit_error(exit) {
        AgentError::Exited { code: Some(3), stderr_tail, .. } => assert_eq!(stderr_tail, tail),
        other => panic!("{other:?}"),
    }
    std::fs::remove_dir_all(&base).unwrap();
}

/// An interrupt reaches the runtime as SIGINT and leaves it running for the next turn.
#[test]
fn an_interrupt_stops_the_turn_and_the_runtime_stays_up() {
    let base = scratch("sigint");
    let (runtime, folder) = claude(&base, "trap 'echo interrupted' INT; echo ready; while :; do /bin/sleep 0.05; done");
    let mut process = spawn(&runtime, &folder, SpawnRequest::default()).unwrap();
    assert_eq!(next_line(&process), "ready");
    process.interrupt().unwrap();
    assert_eq!(next_line(&process), "interrupted");
    assert_eq!(process.status().unwrap(), None, "still running");
    process.interrupt().unwrap();
    assert_eq!(next_line(&process), "interrupted");
    let exit = process.shutdown(Duration::from_millis(200)).unwrap();
    assert!(!exit.success());
    assert!(matches!(process.interrupt(), Err(AgentError::NotRunning { .. })));
    std::fs::remove_dir_all(&base).unwrap();
}

/// Closing the input lets a runtime finish on its own; one that ignores SIGTERM is killed after the
/// grace period.
#[test]
fn shutdown_closes_the_input_then_terminates_then_kills() {
    let base = scratch("shutdown");
    let (runtime, folder) = claude(&base, "while read line; do echo \"got $line\"; done; exit 0");
    let mut process = spawn(&runtime, &folder, SpawnRequest::default()).unwrap();
    process.send_line("hello").unwrap();
    assert_eq!(next_line(&process), "got hello");
    assert!(matches!(process.send_line("two\nlines"), Err(AgentError::InvalidLine { .. })));
    assert_eq!(process.shutdown(Duration::from_secs(5)).unwrap().code, Some(0));
    assert!(matches!(process.send_line("late"), Err(AgentError::InputClosed { .. })));
    drop(process);

    let (runtime, folder) = claude(&base, "trap '' TERM; echo ready; while :; do /bin/sleep 0.05; done");
    let mut process = spawn(&runtime, &folder, SpawnRequest::default()).unwrap();
    assert_eq!(next_line(&process), "ready");
    let started = Instant::now();
    let exit = process.shutdown(Duration::from_millis(200)).unwrap();
    assert_eq!(exit.signal, Some(9), "{exit:?}");
    assert!(started.elapsed() >= Duration::from_millis(400) && started.elapsed() < Duration::from_secs(5));
    assert!(matches!(process.exit_error(exit), AgentError::Crashed { signal: 9, .. }));
    assert!(matches!(process.shutdown(Duration::from_secs(61)), Err(AgentError::InvalidLimit { .. })));
    std::fs::remove_dir_all(&base).unwrap();
}

/// Sending never waits on a runtime that does not read: once its pipe and the queue are full, a
/// line is refused with a backlog error; a line past the limit is refused at once.
#[test]
fn sending_to_a_runtime_that_does_not_read_is_refused_not_blocked() {
    let base = scratch("backlog");
    let (runtime, folder) = claude(&base, "echo ready; /bin/sleep 300");
    let limits = Limits { max_line_bytes: 128 * 1024, queued_lines: 2, ..Limits::default() };
    let mut process = spawn(&runtime, &folder, SpawnRequest { limits, ..SpawnRequest::default() }).unwrap();
    assert_eq!(next_line(&process), "ready");
    assert!(matches!(process.send_line(&"x".repeat(128 * 1024)), Err(AgentError::InvalidLine { .. })));
    let line = "y".repeat(64 * 1024);
    let started = Instant::now();
    let mut refused = None;
    for sent in 0..200 {
        match process.send_line(&line) {
            Ok(()) => std::thread::sleep(Duration::from_millis(5)),
            Err(e) => {
                refused = Some((sent, e));
                break;
            }
        }
    }
    let (sent, error) = refused.expect("a runtime that never reads must refuse a line at some point");
    assert!(matches!(error, AgentError::InputBacklog { .. }), "{error:?} after {sent} lines");
    assert!(started.elapsed() < Duration::from_secs(10));
    drop(process);
    std::fs::remove_dir_all(&base).unwrap();
}

/// Dropping the supervisor kills the runtime and what it started; a runtime that ends on its own
/// takes what it left running with it.
#[test]
fn the_whole_process_group_is_killed_on_drop_and_on_exit() {
    let base = scratch("group");
    let (runtime, folder) = claude(&base, "/bin/sleep 300 & echo \"$!\"; /bin/sleep 300");
    let process = spawn(&runtime, &folder, SpawnRequest::default()).unwrap();
    let grandchild: i32 = next_line(&process).parse().unwrap();
    let child = i32::try_from(process.pid()).unwrap();
    assert!(alive(grandchild) && alive(child));
    drop(process);
    let deadline = Instant::now() + Duration::from_secs(10);
    while (alive(grandchild) || alive(child)) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(!alive(grandchild) && !alive(child), "a process outlived the drop");
    // The foreground sleep too, even when the drop came while the shell was starting it.
    assert!(group_gone(child), "a process of the group outlived the drop");

    let (runtime, folder) = claude(&base, "/bin/sleep 300 & echo \"$!\"; read line; exit 0");
    let mut process = spawn(&runtime, &folder, SpawnRequest::default()).unwrap();
    let grandchild: i32 = next_line(&process).parse().unwrap();
    let child = i32::try_from(process.pid()).unwrap();
    process.close_input();
    assert_eq!(wait_exit(&mut process).code, Some(0));
    let deadline = Instant::now() + Duration::from_secs(10);
    while alive(grandchild) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(!alive(grandchild), "what the runtime left running outlived it");
    assert!(group_gone(child), "a process of the group outlived the runtime");
    std::fs::remove_dir_all(&base).unwrap();
}
