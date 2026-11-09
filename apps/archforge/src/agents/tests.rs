//! The real agents host against fake `claude` and `codex` programs: shell scripts that answer
//! `--version` as the tested versions do and then replay the agents crate's transcripts
//! (`crates/agents/tests/fixtures/`) or answer as a test needs, recording their arguments,
//! environment, tool server configuration and process id. Keys are canaries in a `MemoryStore`;
//! no test runs an installed program, uses a real key or reads `~/.claude` or `~/.codex`. The
//! window's side is a real shell over a fresh project, answering the control channel's requests
//! as a frame does.
#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use archforge_agents::HostEnv;
use archforge_control::{CallError, Client};
use archforge_secrets::{MemoryStore, Secret, SecretError, SecretName, SecretStore};
use archforge_ui_egui::Shell;
use archforge_ui_egui::ai_agents::{AccountError, AgentHost, CodexAccount, CodexSignIn, LoginEvent, LoginWay, Provider};
use archforge_ui_egui::chat::{ChatEvent, ChatRefusal, ChatStart, EventSink, HostEvent, MAX_RUNNING, TurnEnd};
use archforge_ui_egui::control::{ControlRequest, TAKEN, WAITING};
use serde_json::{Value, json};

use super::{Agents, Window};

/// Keys no output may ever show: the store holds them, the programs get them.
const CLAUDE_CANARY: &str = "sk-ant-api03-CANARY-host-7c1d9e2f_canary-canary-canary-canary-xyzAA";
const CODEX_CANARY: &str = "sk-proj-CANARY-host-3b8a6f0e_canary-canary-canary-xyzBB";
/// The conversation of the Claude transcripts: its UUID is the session id they carry.
const CHAT: &str = "7f3c2a10-0000-4000-8000-0000000000d1";
/// The bridge's command; it need not exist, as no fake starts it.
const CLI: &str = "/opt/archforge-test/bin/archforge-cli";
/// How long a test waits for anything the host does.
const WAIT: Duration = Duration::from_secs(15);

// --- the log ---------------------------------------------------------------------------------------

/// Every log line any test of this binary wrote, to look for a key in.
static LOG: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// The logger that keeps every line in [`LOG`].
struct Capture;

impl log::Log for Capture {
    /// Every level is kept.
    fn enabled(&self, _: &log::Metadata<'_>) -> bool {
        true
    }
    /// Keeps the line.
    fn log(&self, record: &log::Record<'_>) {
        LOG.lock().unwrap_or_else(PoisonError::into_inner).push(format!("{} {}", record.level(), record.args()));
    }
    /// Nothing is buffered.
    fn flush(&self) {}
}

/// Sends every log line of this test binary to [`LOG`].
fn capture_logs() {
    static LOGGER: Capture = Capture;
    if log::set_logger(&LOGGER).is_ok() {
        log::set_max_level(log::LevelFilter::Trace);
    }
}

/// Whether any log line holds `text`.
fn logged(text: &str) -> bool {
    LOG.lock().unwrap_or_else(PoisonError::into_inner).iter().any(|l| l.contains(text))
}

// --- the fakes -------------------------------------------------------------------------------------

/// One line of a transcript.
struct Record {
    dir: String,
    line: String,
}

/// The transcript `name` under the agents crate's `tests/fixtures/`.
fn fixture(name: &str) -> Vec<Record> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../crates/agents/tests/fixtures").join(name);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let v: Value = serde_json::from_str(l).unwrap();
            Record { dir: v["dir"].as_str().unwrap().to_owned(), line: v["line"].as_str().unwrap().to_owned() }
        })
        .collect()
}

/// `text` quoted for the shell.
fn quoted(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\\''"))
}

/// A Claude script body that replays transcript `name`, then reads its input to the end.
fn claude_replay(name: &str) -> String {
    let mut body = String::new();
    for record in fixture(name) {
        match record.dir.as_str() {
            "out" => body.push_str(&format!("printf '%s\\n' {}\n", quoted(&record.line))),
            "in" => body.push_str("input\n"),
            _ => {}
        }
    }
    body.push_str("drain\n");
    body
}

/// [`claude_replay`] with the text piece `piece` of the transcript written `times` times.
fn claude_flood(name: &str, piece: &str, times: usize) -> String {
    let mut body = String::new();
    for record in fixture(name) {
        match record.dir.as_str() {
            "out" if record.line.contains(piece) => {
                body.push_str(&format!("i=0; while [ $i -lt {times} ]; do printf '%s\\n' {}; i=$((i+1)); done\n", quoted(&record.line)));
            }
            "out" => body.push_str(&format!("printf '%s\\n' {}\n", quoted(&record.line))),
            "in" => body.push_str("input\n"),
            _ => {}
        }
    }
    body.push_str("drain\n");
    body
}

/// A Claude script body that answers nothing and runs until its input closes.
const CLAUDE_IDLE: &str = "drain\n";

/// A Codex script body for the account calls: the handshake, then one request answered by its kind
/// (the account, a device-code or browser sign-in that completes and is read back, a sign-out).
const CODEX_ACCOUNT: &str = r#"
hello
req
account='{"account":{"type":"chatgpt","email":"person@example.com","planType":"plus","workspaceName":null},"requiresOpenaiAuth":true}'
case "$line" in
  *'"account/read"'*) ok "$id" "$account" ;;
  *'"account/logout"'*) ok "$id" '{}' ;;
  *chatgptDeviceCode*)
    ok "$id" '{"type":"chatgptDeviceCode","loginId":"019a0000-0000-7000-8000-00000000c002","verificationUrl":"https://auth.openai.com/codex/device","userCode":"ABCD-EFGH"}'
    say '{"method":"account/login/completed","params":{"loginId":"019a0000-0000-7000-8000-00000000c002","success":true,"error":null,"onboardingEntrypoint":null}}'
    req; ok "$id" "$account" ;;
  *'"chatgpt"'*)
    ok "$id" '{"type":"chatgpt","loginId":"019a0000-0000-7000-8000-00000000c001","authUrl":"https://auth.openai.com/oauth/authorize?PLACEHOLDER"}'
    say '{"method":"account/login/completed","params":{"loginId":"019a0000-0000-7000-8000-00000000c001","success":true,"error":null,"onboardingEntrypoint":null}}'
    req; ok "$id" "$account" ;;
esac
drain
"#;

/// The shell functions every fake has: `input` reads one line (recorded); `req` reads one JSON-RPC
/// line into `$line` and its number into `$id`; `ok`, `say` and `hello` answer as Codex does;
/// `drain` reads to the end and exits.
const PRELUDE: &str = r#"
input() { IFS= read -r line || exit 0; printf '%s\n' "$line" >> "$IN"; }
req() { IFS= read -r line || exit 0; printf '%s\n' "$line" >> "$IN"; id=$(printf '%s\n' "$line" | sed -n 's/^{"id":\([0-9][0-9]*\),.*/\1/p'); }
ok() { printf '{"id":%s,"result":%s}\n' "$1" "$2"; }
say() { printf '%s\n' "$1"; }
hello() { req; ok "$id" '{"userAgent":"archforge/0.166.0 (test)","codexHome":"/tmp/archforge-agent/codex-home","platformFamily":"unix","platformOs":"macos"}'; req; }
drain() { while IFS= read -r line; do printf '%s\n' "$line" >> "$IN"; done; exit 0; }
"#;

/// A fake `program` at `base/bin/<program>`: `--version` answers as the tested version and is
/// logged in `base/<program>.log`; any other start logs its first argument there, writes its
/// arguments to `base/<program>.args`, its environment to `base/<program>.env`, its process id to
/// `base/<program>.pids` and, for Claude, its tool server configuration to `base/mcp-<session>.json`,
/// then runs `body`.
fn fake(base: &Path, program: &str, version: &str, body: &str) -> PathBuf {
    let path = base.join("bin").join(program);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let file = |suffix: &str| quoted(&base.join(format!("{program}{suffix}")).display().to_string());
    let script = format!(
        "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then echo '--version' >> {log}; echo {version}; exit 0; fi\n\
         echo \"$1\" >> {log}\nprintf '%s\\n' \"$@\" > {args}\n/usr/bin/env > {env}\necho $$ >> {pids}\n\
         prev=; session=; mcp=\nfor a in \"$@\"; do case \"$prev\" in --session-id|--resume) session=$a;; --mcp-config) mcp=$a;; esac; prev=$a; done\n\
         if [ -n \"$mcp\" ]; then cat \"$mcp\" > {base}/mcp-$session.json; fi\nIN={stdin}\n{PRELUDE}\n{body}\n",
        log = file(".log"),
        args = file(".args"),
        env = file(".env"),
        pids = file(".pids"),
        stdin = file(".stdin"),
        base = quoted(&base.display().to_string()),
        version = quoted(version),
    );
    std::fs::write(&path, script).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    path
}

/// The memory store behind an `Arc`, so a test keeps a hand on it.
struct Shared(Arc<MemoryStore>);

impl SecretStore for Shared {
    /// The store's copy.
    fn get(&self, name: &SecretName) -> Result<Secret, SecretError> {
        self.0.get(name)
    }
    /// Kept in the store.
    fn set(&self, name: &SecretName, secret: &Secret) -> Result<(), SecretError> {
        self.0.set(name, secret)
    }
    /// Dropped from the store.
    fn delete(&self, name: &SecretName) -> Result<archforge_secrets::Deleted, SecretError> {
        self.0.delete(name)
    }
}

/// A host under test: the real host over the fakes, its events, the window's requests and its
/// folders.
struct Host {
    agents: Agents,
    base: PathBuf,
    settings: PathBuf,
    events: Receiver<HostEvent>,
    wakes: Arc<AtomicUsize>,
    requests: Receiver<ControlRequest>,
}

/// How a test's host is made.
struct Setup<'a> {
    claude: &'a str,
    codex: &'a str,
    /// Whether the secure store opens.
    store: bool,
    /// Where the bridge's command is found; a test may make it fail.
    cli: super::CliPath,
}

impl Default for Setup<'_> {
    /// Idle programs, a store that opens, a bridge command that need not exist.
    fn default() -> Self {
        Setup { claude: CLAUDE_IDLE, codex: "hello\ndrain\n", store: true, cli: Arc::new(|| Ok(PathBuf::from(CLI))) }
    }
}

impl Host {
    /// The host of the test `name`.
    fn new(name: &str, setup: Setup<'_>) -> Host {
        capture_logs();
        let base = std::env::temp_dir().join(format!("archforge-host-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        let base = base.canonicalize().unwrap();
        let settings = base.join("settings");
        std::fs::create_dir_all(&settings).unwrap();
        std::fs::set_permissions(&settings, std::fs::Permissions::from_mode(0o700)).unwrap();
        let claude = fake(&base, "claude", "2.1.317 (Claude Code)", setup.claude);
        let codex = fake(&base, "codex", "codex-cli 0.166.0", setup.codex);
        let store = Arc::new(MemoryStore::new());
        store.set(&SecretName::ANTHROPIC_API_KEY, &Secret::new(CLAUDE_CANARY.into()).unwrap()).unwrap();
        store.set(&SecretName::OPENAI_API_KEY, &Secret::new(CODEX_CANARY.into()).unwrap()).unwrap();
        let opener: super::Opener = if setup.store {
            Arc::new(move || Ok(Box::new(Shared(store.clone())) as Box<dyn SecretStore>))
        } else {
            Arc::new(|| Err(SecretError::NoSecureStore { reason: "the test's system has none".into() }))
        };
        let env = HostEnv {
            claude_override: Some(claude.into()),
            codex_override: Some(codex.into()),
            path: Some(std::env::join_paths([Path::new("/usr/bin"), Path::new("/bin")]).unwrap()),
            locale: vec![("LANG".into(), "C.UTF-8".into())],
            ..HostEnv::default()
        };
        let (window, requests) = Window::new();
        let agents = Agents::build(&settings, opener, Some(env), setup.cli, window);
        let (tx, events) = std::sync::mpsc::channel();
        let wakes = Arc::new(AtomicUsize::new(0));
        let counted = wakes.clone();
        agents.connect(EventSink::new(
            tx,
            Arc::new(move || {
                counted.fetch_add(1, Ordering::SeqCst);
            }),
        ));
        Host { agents, base, settings, events, wakes, requests }
    }

    /// The host's events until `done` holds for one, within [`WAIT`].
    fn until(&self, what: &str, mut done: impl FnMut(&HostEvent) -> bool) -> Vec<HostEvent> {
        let deadline = Instant::now() + WAIT;
        let mut seen = Vec::new();
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match self.events.recv_timeout(left) {
                Ok(e) => {
                    let stop = done(&e);
                    seen.push(e);
                    if stop {
                        return seen;
                    }
                }
                Err(_) => panic!("waited too long for {what}; seen {seen:?}"),
            }
        }
    }

    /// The events of `chat` until its turn ended.
    fn turn(&self, chat: &str) -> Vec<ChatEvent> {
        chat_events(self.until("the turn's end", |e| matches!(e, HostEvent::Chat { event: ChatEvent::TurnEnded(_), .. })), chat)
    }

    /// The tool server configuration the fake Claude of `chat` was started with, within [`WAIT`].
    fn mcp(&self, chat: &str) -> Value {
        let path = self.base.join(format!("mcp-{chat}.json"));
        let deadline = Instant::now() + WAIT;
        loop {
            if let Some(v) = std::fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str(&t).ok()) {
                return v;
            }
            assert!(Instant::now() < deadline, "no tool server configuration at {}", path.display());
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// The lines the fake `program` logged: `--version` for each look, the first argument of each
    /// start.
    fn log(&self, program: &str) -> Vec<String> {
        std::fs::read_to_string(self.base.join(format!("{program}.log"))).unwrap_or_default().lines().map(str::to_owned).collect()
    }

    /// The token files in the settings folder.
    fn token_files(&self) -> Vec<PathBuf> {
        std::fs::read_dir(&self.settings)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("control-")))
            .collect()
    }

    /// Answers the window's requests with `shell`, as frames do, until `done` holds.
    fn window(&self, shell: &mut Shell, done: impl Fn() -> bool) {
        let deadline = Instant::now() + WAIT;
        while !done() {
            assert!(Instant::now() < deadline, "the window waited too long");
            match self.requests.recv_timeout(Duration::from_millis(10)) {
                Ok(req) => {
                    if req.state.compare_exchange(WAITING, TAKEN, Ordering::SeqCst, Ordering::SeqCst).is_ok() {
                        let answer = archforge_ui_egui::control::answer_request(shell, &req);
                        let _ = req.reply.send(answer);
                    }
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return,
            }
        }
    }
}

impl Drop for Host {
    /// Quits the host and removes the test's folder.
    fn drop(&mut self) {
        self.agents.shutdown();
        let _ = std::fs::remove_dir_all(&self.base);
    }
}

/// The events of `chat` among `events`, in order.
fn chat_events(events: Vec<HostEvent>, chat: &str) -> Vec<ChatEvent> {
    events
        .into_iter()
        .filter_map(|e| match e {
            HostEvent::Chat { chat: c, event, .. } if c == chat => Some(event),
            _ => None,
        })
        .collect()
}

/// Whether `events` hold agent access's change to `held`.
fn access(events: &[HostEvent], held: bool) -> bool {
    events.iter().any(|e| matches!(e, HostEvent::Access { held: h } if *h == held))
}

/// A conversation `chat` with Claude, labelled `origin`, read-only unless `allow_changes`.
fn claude(chat: &str, origin: &str, allow_changes: bool) -> ChatStart {
    ChatStart {
        chat: chat.into(),
        provider: Provider::Claude,
        model: "sonnet".into(),
        origin: origin.into(),
        allow_changes,
        resume: false,
        codex_sign_in: CodexSignIn::ChatGpt,
    }
}

/// A conversation id numbered `n`.
fn id(n: u64) -> String {
    format!("7f3c2a10-0000-4000-8000-{n:012x}")
}

/// The port and the token file the bridge of a configuration is given.
fn bridge_of(mcp: &Value) -> (u16, PathBuf) {
    let args: Vec<&str> = mcp["mcpServers"]["archforge"]["args"].as_array().unwrap().iter().map(|a| a.as_str().unwrap()).collect();
    let after = |flag: &str| args.iter().position(|a| *a == flag).and_then(|i| args.get(i + 1)).copied().unwrap();
    (after("--bridge").parse().unwrap(), PathBuf::from(after("--token-file")))
}

/// Whether Chat's agent access still listens, as the host itself knows it (AR-229): never judged by
/// the port, which another test may bind as soon as it is free.
fn host_listens(host: &Host) -> bool {
    super::locked(&host.agents.inner.access).listening()
}

/// Whether anything listens on `port` of the loopback interface.
fn listens(port: u16) -> bool {
    std::net::TcpStream::connect_timeout(&std::net::SocketAddr::from((std::net::Ipv4Addr::LOCALHOST, port)), Duration::from_millis(300)).is_ok()
}

/// A shell over a fresh project, as the window has one.
fn shell() -> Shell {
    let mut n: u128 = 0;
    let ids = Box::new(move || {
        n += 1;
        archforge_mof::Uuid(n.to_le_bytes())
    });
    let mut shell = Shell::new(archforge_ui_egui::Services::default());
    shell.session = Some(archforge_notations::new_project_session("Shop", ids).unwrap());
    shell
}

/// Whether the process `pid` still runs.
fn alive(pid: &str) -> bool {
    std::process::Command::new("/bin/kill").args(["-0", pid]).stderr(std::process::Stdio::null()).status().is_ok_and(|s| s.success())
}

// --- the tests -------------------------------------------------------------------------------------

/// A whole conversation with agent access: its own read-only token in a private file, the bridge
/// configured with it, a real client answered with its origin, a turn with a tool call over the
/// channel with the window woken, stop, a refused duplicate, and the end giving everything back.
#[test]
fn a_conversation_holds_its_own_read_only_token_until_it_ends() {
    let host = Host::new("session", Setup { claude: &claude_replay("claude/doc-turn-with-mcp-tool.jsonl"), ..Setup::default() });
    host.agents.start(claude(CHAT, "Chat 1", false)).unwrap();
    let opened = host.until("access held", |e| matches!(e, HostEvent::Access { held: true }));
    assert!(access(&opened, true));
    // The bridge: the conversation's token file, read-only, its origin; the key blanked for it.
    let mcp = host.mcp(CHAT);
    let server = &mcp["mcpServers"]["archforge"];
    assert_eq!(server["command"], CLI);
    let (port, token_file) = bridge_of(&mcp);
    let args = server["args"].to_string();
    assert!(args.contains(r#""--read-only","--origin","Chat 1""#), "{args}");
    assert_eq!(server["env"], json!({ "ANTHROPIC_API_KEY": "" }));
    assert!(!mcp.to_string().contains(CLAUDE_CANARY));
    // While it runs: the port listens, the token file is this user's alone, named for the port.
    assert!(listens(port) && host_listens(&host));
    assert_eq!(std::fs::metadata(&token_file).unwrap().permissions().mode() & 0o777, 0o600);
    assert_eq!(token_file.parent(), Some(host.settings.as_path()));
    let name = token_file.file_name().unwrap().to_string_lossy().into_owned();
    assert!(
        name.starts_with(&format!("control-{port}-")) && name.ends_with(".token") && name.len() == format!("control-{port}-").len() + 22,
        "{name}"
    );
    // A real client with the conversation's token is answered by the window, with its origin.
    let client = Client::from_token_file(&token_file, port).unwrap().with_origin(Some("Chat 1".into())).unwrap();
    let answered = std::thread::spawn(move || client.call("engine.execute", json!({ "command": "er.schema.list" })));
    let mut window = shell();
    host.window(&mut window, || answered.is_finished());
    let answer = answered.join().unwrap().unwrap();
    assert_eq!(answer["origin"], "Chat 1", "{answer}");
    // A turn with a tool call: its events over the channel, each waking the window.
    let wakes = host.wakes.load(Ordering::SeqCst);
    host.agents.send(CHAT, "Which entities are in Shop?".into()).unwrap();
    let events = host.turn(CHAT);
    assert!(matches!(events.first(), Some(ChatEvent::Started { access: true, .. })), "{events:?}");
    assert!(events.iter().any(|e| matches!(e, ChatEvent::ToolStarted { tool, .. } if tool.contains("er_entity_list"))), "{events:?}");
    assert!(events.iter().any(|e| matches!(e, ChatEvent::ToolFinished { ok: true, .. })), "{events:?}");
    assert!(events.iter().any(|e| matches!(e, ChatEvent::Message { text } if text.contains("Customer and Order"))), "{events:?}");
    assert_eq!(events.last(), Some(&ChatEvent::TurnEnded(TurnEnd::Completed)));
    assert!(host.wakes.load(Ordering::SeqCst) >= wakes + events.len(), "every event woke the window");
    // Stop, with no turn running, and a second start of it: refused.
    host.agents.stop(CHAT).unwrap();
    assert_eq!(host.agents.start(claude(CHAT, "Chat 1", false)), Err(ChatRefusal::AlreadyRunning));
    // The key reached the program in its environment alone.
    let env = std::fs::read_to_string(host.base.join("claude.env")).unwrap();
    assert!(env.contains(&format!("ANTHROPIC_API_KEY={CLAUDE_CANARY}")));
    assert!(!std::fs::read_to_string(host.base.join("claude.args")).unwrap().contains(CLAUDE_CANARY));
    // The end gives everything back: the token file is gone, and nothing listens.
    host.agents.end(CHAT).unwrap();
    let ended = host.until("the end", |e| matches!(e, HostEvent::Chat { event: ChatEvent::Ended { .. }, .. }));
    assert!(access(&ended, false), "access closed before the end was told: {ended:?}");
    assert_eq!(chat_events(ended, CHAT).last(), Some(&ChatEvent::Ended { failure: None }));
    assert!(!token_file.exists(), "the token file went with the conversation");
    assert!(!host_listens(&host), "nothing listens once the last conversation ended");
    // The freed port taken again, as another test may take it, fools no check (AR-229).
    if let Ok(_squatter) = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port)) {
        assert!(listens(port), "something listens on the port again");
        assert!(!host_listens(&host), "yet the host's own listener is closed");
    }
    assert!(matches!(Client::with_token(port, "0".repeat(64)).call("document.inspect", json!({})), Err(CallError::Unreachable(_))));
    assert_eq!(host.agents.send(CHAT, "again".into()), Err(ChatRefusal::NotRunning));
    assert!(!logged(CLAUDE_CANARY) && !logged(CODEX_CANARY), "no key reached the log");
}

/// Quitting closes agent access, a held connection answered "revoked", ends every conversation and
/// waits for its program.
#[test]
fn quitting_closes_access_and_ends_every_conversation() {
    let host = Host::new("close", Setup::default());
    host.agents.start(claude(CHAT, "Chat 1", false)).unwrap();
    host.until("access held", |e| matches!(e, HostEvent::Access { held: true }));
    let (port, token_file) = bridge_of(&host.mcp(CHAT));
    let token = archforge_control::read_token(&token_file).unwrap();
    // A connection that showed the token and holds on.
    let held = |token: &str, port: u16, host: &Host| {
        let mut s = std::net::TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, port)).unwrap();
        s.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
        std::io::Write::write_all(&mut s, format!("{{\"id\":1,\"method\":\"document.inspect\",\"token\":\"{token}\"}}\n").as_bytes()).unwrap();
        let mut r = std::io::BufReader::new(s);
        let reader = std::thread::spawn(move || {
            let first = archforge_control::read_line(&mut r, archforge_control::MAX_REPLY).unwrap();
            (first, r)
        });
        let mut window = shell();
        host.window(&mut window, || reader.is_finished());
        let (first, r) = reader.join().unwrap();
        assert!(first.is_some_and(|l| serde_json::from_slice::<Value>(&l).unwrap()["ok"] == true));
        r
    };
    // A second conversation shares the opening; quitting closes it and ends both.
    let second = id(2);
    host.agents.start(claude(&second, "Chat 2", true)).unwrap();
    let (same_port, second_file) = bridge_of(&host.mcp(&second));
    assert_eq!(same_port, port, "one opening serves both");
    let mut r = held(&token, port, &host);
    host.agents.shutdown();
    let last: Value = serde_json::from_slice(&archforge_control::read_line(&mut r, archforge_control::MAX_REPLY).unwrap().unwrap()).unwrap();
    assert_eq!(last["error"], archforge_control::REVOKED);
    assert_eq!(archforge_control::read_line(&mut r, archforge_control::MAX_REPLY).unwrap(), None, "then closed");
    assert!(!host_listens(&host) && !token_file.exists() && !second_file.exists());
    // And it stays closed: no conversation is granted access after quitting.
    let bound = archforge_control::Bound { key: id(4), origin: "Chat 4".into() };
    let refused = super::locked(&host.agents.inner.access).grant(false, bound, None).err();
    assert_eq!(refused.as_deref(), Some("Archforge is closing"));
    assert!(!host_listens(&host), "nothing opened again");
    let pids = std::fs::read_to_string(host.base.join("claude.pids")).unwrap();
    assert_eq!(pids.lines().count(), 2);
    for pid in pids.lines() {
        assert!(!alive(pid), "the program {pid} ended before quitting returned");
    }
    let events: Vec<HostEvent> = host.events.try_iter().collect();
    assert_eq!(events.iter().filter(|e| matches!(e, HostEvent::Chat { event: ChatEvent::Ended { .. }, .. })).count(), 2, "{events:?}");
    assert_eq!(host.agents.start(claude(&id(3), "Chat 3", false)), Err(ChatRefusal::Closing), "nothing starts after quitting");
}

/// The control channel holds the read-only promise for the conversation's token: reads are
/// answered; a change, a batch with one, undo and save are refused. A conversation that allows
/// changes writes. An ended conversation's token opens nothing.
#[test]
fn a_read_only_token_only_reads_and_an_ended_conversations_token_is_refused() {
    let host = Host::new("roles", Setup::default());
    let (reader, writer) = (id(1), id(2));
    host.agents.start(claude(&reader, "Chat 1", false)).unwrap();
    host.agents.start(claude(&writer, "Chat 2", true)).unwrap();
    let (port, read_file) = bridge_of(&host.mcp(&reader));
    let (same_port, write_file) = bridge_of(&host.mcp(&writer));
    assert_eq!(port, same_port, "one opening serves both");
    assert!(!host.mcp(&writer)["mcpServers"]["archforge"]["args"].to_string().contains("--read-only"));
    let read_token = archforge_control::read_token(&read_file).unwrap();
    let calls = std::thread::spawn(move || {
        let reads = Client::from_token_file(&read_file, port).unwrap().with_origin(Some("Chat 1".into())).unwrap();
        let writes = Client::from_token_file(&write_file, port).unwrap().with_origin(Some("Chat 2".into())).unwrap();
        let read = |m: &str, p: Value| reads.call(m, p);
        let answers = vec![
            ("inspect", read("document.inspect", json!({}))),
            ("query", read("engine.execute", json!({ "command": "er.schema.list" }))),
            ("query batch", read("engine.batch", json!({ "steps": [{ "command": "er.schema.list" }] }))),
            ("execute", read("engine.execute", json!({ "command": "er.schema.add", "params": { "name": "x" } }))),
            (
                "batch",
                read(
                    "engine.batch",
                    json!({ "steps": [{ "command": "er.schema.list" }, { "command": "er.schema.add", "params": { "name": "y" } }] }),
                ),
            ),
            ("undo", read("engine.undo", json!({}))),
            ("save", read("app.save", json!({}))),
            ("write", writes.call("engine.execute", json!({ "command": "er.schema.add", "params": { "name": "shop" } }))),
        ];
        answers
    });
    let mut window = shell();
    host.window(&mut window, || calls.is_finished());
    for (what, answer) in calls.join().unwrap() {
        match what {
            "inspect" | "query" | "query batch" => assert!(answer.is_ok(), "{what}: {answer:?}"),
            "write" => assert_eq!(answer.unwrap()["origin"], "Chat 2"),
            _ => assert!(matches!(&answer, Err(CallError::Refused(e)) if e.contains("may only read")), "{what}: {answer:?}"),
        }
    }
    let schemas = archforge_ui_egui::control::answer(&mut window, "engine.execute", &json!({ "command": "er.schema.list" })).unwrap();
    assert_eq!(schemas["result"].as_array().map(Vec::len), Some(1), "only the writer's schema: {schemas}");
    // The reader's conversation ends: its token opens nothing, the writer's goes on.
    host.agents.end(&reader).unwrap();
    host.until("the reader's end", |e| matches!(e, HostEvent::Chat { chat, event: ChatEvent::Ended { .. }, .. } if *chat == reader));
    assert!(matches!(Client::with_token(port, read_token).call("document.inspect", json!({})), Err(CallError::Unreachable(_))));
    assert!(listens(port) && host_listens(&host), "the writer still holds access");
}

/// A conversation asked to end has its token revoked as the end is asked, before its program has
/// shut down (AR-228): a call with it while the program still ends, here a change from a
/// conversation that allowed changes, is refused and changes nothing.
#[test]
fn an_ending_conversations_token_is_revoked_before_its_program_shuts_down() {
    // The program says when its input closed, that is when it was asked to end, then takes most of
    // the shutdown's grace to go.
    let host = Host::new(
        "revoke-on-end",
        Setup { claude: "while IFS= read -r line; do :; done\nprintf 'closed\\n' >> \"$IN\"\nsleep 1.5\n", ..Setup::default() },
    );
    host.agents.start(claude(CHAT, "Chat 1", true)).unwrap();
    host.until("access held", |e| matches!(e, HostEvent::Access { held: true }));
    let (port, file) = bridge_of(&host.mcp(CHAT));
    let client = Client::from_token_file(&file, port).unwrap().with_origin(Some("Chat 1".into())).unwrap();
    host.agents.end(CHAT).unwrap();
    let closed = || std::fs::read_to_string(host.base.join("claude.stdin")).unwrap_or_default().contains("closed");
    let deadline = Instant::now() + WAIT;
    while !closed() {
        assert!(Instant::now() < deadline, "its program was asked to end");
        std::thread::sleep(Duration::from_millis(10));
    }
    // The program still ends: a change with its token.
    let call = std::thread::spawn(move || client.call("engine.execute", json!({ "command": "er.schema.add", "params": { "name": "late" } })));
    let mut window = shell();
    host.window(&mut window, || call.is_finished());
    let answer = call.join().unwrap();
    assert!(
        matches!(&answer, Err(CallError::Unreachable(_))) || answer == Err(CallError::Refused(archforge_control::REVOKED.into())),
        "refused while the program ends: {answer:?}"
    );
    let schemas = archforge_ui_egui::control::answer(&mut window, "engine.execute", &json!({ "command": "er.schema.list" })).unwrap();
    assert_eq!(schemas["result"].as_array().map(Vec::len), Some(0), "nothing changed: {schemas}");
    host.until("the end", |e| matches!(e, HostEvent::Chat { event: ChatEvent::Ended { .. }, .. }));
}

/// Asking a conversation to end revokes its token before the asking call returns (AR-228, review):
/// a call with that full-access token right after, with no origin or in another conversation's
/// name, is refused and changes nothing; another conversation's own token still works. The window
/// here shows no conversation, so its gate lets every origin through: what refuses them is the
/// revocation alone.
#[test]
fn a_conversation_asked_to_end_has_its_token_revoked_before_the_call_returns() {
    let host = Host::new("revoke-at-ask", Setup::default());
    let (ending, other) = (id(1), id(2));
    host.agents.start(claude(&ending, "Chat 1", true)).unwrap();
    host.agents.start(claude(&other, "Chat 2", false)).unwrap();
    host.until("access held", |e| matches!(e, HostEvent::Access { held: true }));
    let (port, file) = bridge_of(&host.mcp(&ending));
    let (_, other_file) = bridge_of(&host.mcp(&other));
    let token = archforge_control::read_token(&file).unwrap();
    host.agents.end(&ending).unwrap();
    assert!(!file.exists(), "its token file went as the end was asked");
    let calls = std::thread::spawn(move || {
        let change = |c: Client| c.call("engine.execute", json!({ "command": "er.schema.add", "params": { "name": "late" } }));
        let other_reads = Client::from_token_file(&other_file, port).unwrap().with_origin(Some("Chat 2".into())).unwrap();
        vec![
            ("no origin", change(Client::with_token(port, token.clone()))),
            ("another conversation's origin", change(Client::with_token(port, token).with_origin(Some("Chat 2".into())).unwrap())),
            ("the other conversation's own token", other_reads.call("document.inspect", json!({}))),
        ]
    });
    let mut window = shell();
    host.window(&mut window, || calls.is_finished());
    for (what, answer) in calls.join().unwrap() {
        if what == "the other conversation's own token" {
            assert!(answer.is_ok(), "{what}: {answer:?}");
        } else {
            assert!(
                matches!(&answer, Err(CallError::Unreachable(_))) || answer == Err(CallError::Refused(archforge_control::REVOKED.into())),
                "{what}: refused: {answer:?}"
            );
        }
    }
    let schemas = archforge_ui_egui::control::answer(&mut window, "engine.execute", &json!({ "command": "er.schema.list" })).unwrap();
    assert_eq!(schemas["result"].as_array().map(Vec::len), Some(0), "nothing changed: {schemas}");
    host.until("its end", |e| matches!(e, HostEvent::Chat { chat, event: ChatEvent::Ended { .. }, .. } if *chat == ending));
    host.agents.end(&other).unwrap();
}

/// A conversation's token speaks only for that conversation (AR-230): the channel bound it to the
/// conversation when it granted it, so a request on it that names no origin is still that
/// conversation's, its change labelled with its name, and one naming another conversation is
/// refused and changes nothing.
#[test]
fn a_conversations_token_speaks_only_for_its_conversation() {
    let host = Host::new("bound-origin", Setup::default());
    let (first, second) = (id(1), id(2));
    host.agents.start(claude(&first, "Chat 1", true)).unwrap();
    host.agents.start(claude(&second, "Chat 2", true)).unwrap();
    host.until("access held", |e| matches!(e, HostEvent::Access { held: true }));
    let (port, file) = bridge_of(&host.mcp(&first));
    let token = archforge_control::read_token(&file).unwrap();
    let calls = std::thread::spawn(move || {
        let add = |c: Client, name: &str| c.call("engine.execute", json!({ "command": "er.schema.add", "params": { "name": name } }));
        vec![
            ("no origin", add(Client::with_token(port, token.clone()), "shop")),
            ("another conversation's origin", add(Client::with_token(port, token.clone()).with_origin(Some("Chat 2".into())).unwrap(), "stock")),
            ("its own origin", add(Client::with_token(port, token).with_origin(Some("Chat 1".into())).unwrap(), "sales")),
        ]
    });
    let mut window = shell();
    host.window(&mut window, || calls.is_finished());
    for (what, answer) in calls.join().unwrap() {
        match what {
            "another conversation's origin" => {
                assert!(matches!(&answer, Err(CallError::Refused(e)) if e.contains("speaks for \"Chat 1\"")), "{what}: {answer:?}");
            }
            _ => assert_eq!(answer.as_ref().map(|a| a["origin"].clone()), Ok(json!("Chat 1")), "{what}: {answer:?}"),
        }
    }
    let schemas = archforge_ui_egui::control::answer(&mut window, "engine.execute", &json!({ "command": "er.schema.list" })).unwrap();
    assert_eq!(schemas["result"].as_array().map(Vec::len), Some(2), "the refused one changed nothing: {schemas}");
    for chat in [&first, &second] {
        host.agents.end(chat).unwrap();
    }
}

/// The window gets, with each request, the conversation its token is bound to, as the channel says
/// it, so its gate can know the conversation by key (AR-230).
#[test]
fn the_window_gets_the_conversation_a_token_is_bound_to() {
    let (window, requests) = Window::new();
    let handler = super::access::handler(window);
    let call = std::thread::spawn(move || {
        let live = Arc::new(std::sync::atomic::AtomicBool::new(true));
        handler(archforge_control::Call {
            method: "document.inspect",
            params: json!({}),
            origin: Some("Chat 1"),
            conversation: Some(CHAT),
            role: archforge_control::Role::Full,
            live: &live,
        })
    });
    let request = requests.recv_timeout(WAIT).unwrap();
    assert_eq!((request.origin.as_deref(), request.conversation.as_deref()), (Some("Chat 1"), Some(CHAT)));
    request.reply.send(Ok(json!(null))).unwrap();
    assert_eq!(call.join().unwrap(), Ok(json!(null)));
}

/// The generation a request carries is the window's when the channel accepted it, whatever the
/// window does while the request waits.
#[test]
fn a_request_carries_the_generation_it_was_accepted_under() {
    use std::sync::atomic::AtomicBool;
    let (window, requests) = Window::new();
    let generation = window.generation();
    generation.store(7, Ordering::SeqCst);
    let handler = super::access::handler(window);
    let call = std::thread::spawn(move || {
        let live = Arc::new(AtomicBool::new(true));
        let role = archforge_control::Role::Full;
        // A field of the request has no say in it.
        handler(archforge_control::Call {
            method: "document.inspect",
            params: json!({ "generation": 99 }),
            origin: None,
            conversation: None,
            role,
            live: &live,
        })
    });
    let request = requests.recv_timeout(WAIT).unwrap();
    generation.store(8, Ordering::SeqCst);
    assert_eq!(request.generation, 7);
    request.reply.send(Ok(json!(null))).unwrap();
    assert_eq!(call.join().unwrap(), Ok(json!(null)));
}

/// A request of a token revoked while it waits in the window's queue is given up at once, answered
/// "revoked", and never runs when the window comes to it: the token revoked by its conversation's
/// end, or the whole opening closed by quitting.
#[test]
fn a_queued_request_of_a_revoked_token_never_runs() {
    for quit in [false, true] {
        let host = Host::new(if quit { "queued-quit" } else { "queued" }, Setup::default());
        host.agents.start(claude(CHAT, "Chat 1", true)).unwrap();
        host.until("access held", |e| matches!(e, HostEvent::Access { held: true }));
        let (port, file) = bridge_of(&host.mcp(CHAT));
        let client = Client::from_token_file(&file, port).unwrap();
        let call = std::thread::spawn(move || client.call("engine.execute", json!({ "command": "er.schema.add", "params": { "name": "late" } })));
        // The request waits in the window's queue, not taken.
        let queued = host.requests.recv_timeout(WAIT).unwrap();
        let revoked = Instant::now();
        // Quitting runs on a thread of its own, as it waits for the programs; an end revokes the
        // token on this thread, before the call returns.
        let quitting = if quit {
            let agents = Agents { inner: host.agents.inner.clone() };
            Some(std::thread::spawn(move || agents.shutdown()))
        } else {
            host.agents.end(CHAT).unwrap();
            None
        };
        let answer = call.join().unwrap();
        assert!(revoked.elapsed() < Duration::from_secs(2), "quit {quit}: given up at once: {:?}", revoked.elapsed());
        assert_eq!(answer, Err(CallError::Refused(archforge_control::REVOKED.into())), "quit {quit}");
        // When the window comes to it, it is not run.
        assert_eq!(queued.state.load(Ordering::SeqCst), archforge_ui_egui::control::ABANDONED, "quit {quit}");
        let mut window = shell();
        let before = window.session.as_ref().unwrap().revision();
        assert!(queued.state.compare_exchange(WAITING, TAKEN, Ordering::SeqCst, Ordering::SeqCst).is_err(), "the window does not take it");
        assert_eq!(archforge_ui_egui::control::answer_request(&mut window, &queued), Err(archforge_control::REVOKED.into()), "nor would it run");
        assert_eq!(window.session.as_ref().unwrap().revision(), before);
        if let Some(quitting) = quitting {
            quitting.join().unwrap();
        }
    }
}

/// A conversation whose events nobody takes keeps at most `MAX_WAITING` of them waiting, the drop
/// said once in the log, and its end still comes.
#[test]
fn a_conversation_whose_events_wait_is_capped_and_its_end_still_comes() {
    use archforge_ui_egui::chat::MAX_WAITING;
    let flood = claude_flood("claude/doc-turn-with-mcp-tool.jsonl", "Customer and Order.", MAX_WAITING + 500);
    let host = Host::new("flood", Setup { claude: &flood, ..Setup::default() });
    host.agents.start(claude(CHAT, "Chat 1", false)).unwrap();
    host.agents.send(CHAT, "Which entities are in Shop?".into()).unwrap();
    // Everything that comes, kept (so its places stay taken), until the program falls silent.
    let mut kept = Vec::new();
    while let Ok(e) = host.events.recv_timeout(Duration::from_secs(2)) {
        kept.push(e);
    }
    let waiting = kept.iter().filter(|e| matches!(e, HostEvent::Chat { .. })).count();
    assert_eq!(waiting, MAX_WAITING, "the cap holds while nothing is taken");
    let said =
        || LOG.lock().unwrap_or_else(PoisonError::into_inner).iter().filter(|l| l.contains("too many events waiting") && l.contains(CHAT)).count();
    assert_eq!(said(), 1, "the drop is said once");
    host.agents.end(CHAT).unwrap();
    let rest = host.until("the end", |e| matches!(e, HostEvent::Chat { event: ChatEvent::Ended { .. }, .. }));
    assert!(matches!(rest.last(), Some(HostEvent::Chat { event: ChatEvent::Ended { .. }, .. })), "the end is never dropped");
    assert_eq!(said(), 1);
    drop(kept);
}

/// The test message runs with no tool server, in a scratch folder removed afterwards; the Codex
/// account is read, signed in with a device code and in the browser, and signed out, each through a
/// short-lived codex program, with the ChatGPT sign-in in Codex's own files although the secure
/// store opens (AR-236).
#[test]
fn the_test_message_and_codex_account_calls_run_through_their_programs() {
    let host =
        Host::new("accounts", Setup { claude: &claude_replay("claude/doc-turn-with-mcp-tool.jsonl"), codex: CODEX_ACCOUNT, ..Setup::default() });
    assert_eq!(host.agents.test_message(Provider::Claude, "sonnet", CodexSignIn::ChatGpt).recv_timeout(WAIT).unwrap(), Ok(()));
    let args = std::fs::read_to_string(host.base.join("claude.args")).unwrap();
    let session = args.lines().skip_while(|l| *l != "--session-id").nth(1).unwrap().to_string();
    assert_eq!(std::fs::read_to_string(host.base.join(format!("mcp-{session}.json"))).unwrap(), r#"{"mcpServers":{}}"#, "no tool server");
    let workspace = host.settings.join("agents");
    let left: Vec<String> = std::fs::read_dir(&workspace)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.contains("test-"))
        .collect();
    assert!(left.is_empty(), "the scratch folder was removed: {left:?}");
    assert!(host.token_files().is_empty(), "the test message holds no access");
    // Codex's account.
    let account = host.agents.codex_account_read().recv_timeout(WAIT).unwrap();
    assert_eq!(account, Ok(CodexAccount::SignedIn { email: "person@example.com".into(), plan: Some("plus".into()) }));
    assert!(host.agents.inner.store().is_ok(), "the secure store opens");
    assert_eq!(codex_store(&host), "\"file\"", "the account read");
    // A device-code sign-in: the code, then who is signed in.
    let events: Vec<LoginEvent> = host.agents.codex_login_start(LoginWay::DeviceCode).iter().collect();
    assert_eq!(
        events,
        vec![
            LoginEvent::DeviceCode { code: "ABCD-EFGH".into(), url: "https://auth.openai.com/codex/device".into(), minutes: 15 },
            LoginEvent::Done(CodexAccount::SignedIn { email: "person@example.com".into(), plan: Some("plus".into()) }),
        ]
    );
    assert_eq!(codex_store(&host), "\"file\"", "the device-code sign-in");
    // In the browser: the page to open, then who is signed in.
    let events: Vec<LoginEvent> = host.agents.codex_login_start(LoginWay::Browser).iter().collect();
    assert!(
        matches!(&events[..], [LoginEvent::OpenBrowser { url }, LoginEvent::Done(CodexAccount::SignedIn { .. })] if url.starts_with("https://auth.openai.com/oauth/authorize")),
        "{events:?}"
    );
    assert!(archforge_ui_egui::ai_agents::approved_browser_page("https://auth.openai.com/oauth/authorize?PLACEHOLDER").is_some());
    assert_eq!(codex_store(&host), "\"file\"", "the browser sign-in");
    assert_eq!(host.agents.codex_logout().recv_timeout(WAIT).unwrap(), Ok(()));
    assert_eq!(codex_store(&host), "\"file\"", "the sign-out");
    assert!(!logged("person@example.com") && !logged("ABCD-EFGH"), "no sign-in value reached the log");
    assert!(!format!("{events:?}").contains("person@example.com"), "Debug shows no email");
}

/// A Codex script body for a device-code sign-in that never completes: the code, then only reading.
const CODEX_DEVICE_WAITS: &str = r#"
hello
req
ok "$id" '{"type":"chatgptDeviceCode","loginId":"019a0000-0000-7000-8000-00000000c002","verificationUrl":"https://auth.openai.com/codex/device","userCode":"ABCD-EFGH"}'
drain
"#;

/// The device code every sign-in test is given.
fn device_code() -> LoginEvent {
    LoginEvent::DeviceCode { code: "ABCD-EFGH".into(), url: "https://auth.openai.com/codex/device".into(), minutes: 15 }
}

/// The lines the fake `program` read on its input.
fn stdin_of(host: &Host, program: &str) -> String {
    std::fs::read_to_string(host.base.join(format!("{program}.stdin"))).unwrap_or_default()
}

/// Waits until the fake `program` has read a line holding `text`, within [`WAIT`].
fn read_by(host: &Host, program: &str, text: &str) {
    let deadline = Instant::now() + WAIT;
    while !stdin_of(host, program).contains(text) {
        assert!(Instant::now() < deadline, "{program} never read {text:?}: {}", stdin_of(host, program));
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// A sign-in nobody finishes in its time is cancelled with the codex program and the window is told
/// it ran out of time, never left waiting.
#[test]
fn a_sign_in_that_runs_out_of_time_says_so() {
    let host = Host::new("sign-in-timeout", Setup { codex: CODEX_DEVICE_WAITS, ..Setup::default() });
    host.agents.inner.sign_in_millis.store(300, Ordering::SeqCst);
    let events: Vec<LoginEvent> = host.agents.codex_login_start(LoginWay::DeviceCode).iter().collect();
    assert_eq!(events, vec![device_code(), LoginEvent::Failed(AccountError::SignInTimedOut)]);
    read_by(&host, "codex", "account/login/cancel");
}

/// A sign-in the codex program cancels on its own is a failure the window shows, naming the cancel.
#[test]
fn a_sign_in_the_codex_program_cancels_says_so() {
    let body = r#"
hello
req
ok "$id" '{"type":"chatgptDeviceCode","loginId":"019a0000-0000-7000-8000-00000000c002","verificationUrl":"https://auth.openai.com/codex/device","userCode":"ABCD-EFGH"}'
say '{"method":"account/login/completed","params":{"loginId":"019a0000-0000-7000-8000-00000000c002","success":false,"error":"Login was not completed","onboardingEntrypoint":null}}'
drain
"#;
    let host = Host::new("sign-in-codex-cancel", Setup { codex: body, ..Setup::default() });
    let events: Vec<LoginEvent> = host.agents.codex_login_start(LoginWay::DeviceCode).iter().collect();
    assert_eq!(events, vec![device_code(), LoginEvent::Failed(AccountError::SignInCancelled)]);
}

/// The person's own cancel, and quitting, end a sign-in without a word to a window that no longer
/// waits for it; the codex program is told to cancel either way.
#[test]
fn the_persons_cancel_or_quitting_ends_a_sign_in_quietly() {
    let host = Host::new("sign-in-cancel", Setup { codex: CODEX_DEVICE_WAITS, ..Setup::default() });
    let events = host.agents.codex_login_start(LoginWay::DeviceCode);
    assert_eq!(events.recv_timeout(WAIT).unwrap(), device_code());
    host.agents.codex_login_cancel();
    assert_eq!(events.iter().collect::<Vec<_>>(), vec![], "nothing after the person's cancel");
    read_by(&host, "codex", "account/login/cancel");
    let quit = Host::new("sign-in-quit", Setup { codex: CODEX_DEVICE_WAITS, ..Setup::default() });
    let events = quit.agents.codex_login_start(LoginWay::DeviceCode);
    assert_eq!(events.recv_timeout(WAIT).unwrap(), device_code());
    quit.agents.shutdown();
    assert_eq!(events.iter().collect::<Vec<_>>(), vec![], "nothing after quitting");
    read_by(&quit, "codex", "account/login/cancel");
}

/// The test message of a provider without a key says so, and sends nothing.
#[test]
fn a_test_message_without_a_key_needs_one() {
    let host = Host::new("no-key", Setup::default());
    let store: Box<dyn SecretStore> = Box::new(MemoryStore::new());
    let _ = host.agents.inner.store.set(Ok(store));
    let answer = host.agents.test_message(Provider::Claude, "sonnet", CodexSignIn::ChatGpt).recv_timeout(WAIT).unwrap();
    assert_eq!(answer, Err(AccountError::NoCredential));
    assert!(!host.log("claude").iter().any(|l| l != "--version"), "nothing was started");
}

/// The `cli_auth_credentials_store` value the codex program's latest start was given, taken: the
/// arguments' file is removed, so the next call fails unless another start wrote it again.
fn codex_store(host: &Host) -> String {
    let path = host.base.join("codex.args");
    let args = std::fs::read_to_string(&path).unwrap();
    std::fs::remove_file(&path).unwrap();
    let values: Vec<&str> = args.lines().filter_map(|l| l.strip_prefix("cli_auth_credentials_store=")).collect();
    assert_eq!(values.len(), 1, "one store: {args}");
    values[0].to_owned()
}

/// A codex body that makes the handshake and ends, so a start is recorded and nothing waits.
const CODEX_HELLO_ENDS: &str = "hello\nexit 0\n";

/// Every Codex start with a ChatGPT sign-in (the account, a conversation, the test message) keeps
/// the sign-in in Codex's own files in the private `CODEX_HOME` (mode 0700), also where the secure
/// store opens: the codex program runs with a private `HOME`, where no keychain is found (AR-236).
/// An API key still goes with the ephemeral store.
#[test]
fn a_codex_chatgpt_start_keeps_the_sign_in_in_codex_files_even_where_the_secure_store_opens() {
    for opens in [true, false] {
        let host = Host::new(&format!("chatgpt-files-{opens}"), Setup { codex: CODEX_HELLO_ENDS, store: opens, ..Setup::default() });
        assert_eq!(host.agents.inner.store().is_ok(), opens, "the secure store opens: {opens}");
        let _ = host.agents.codex_account_read().recv_timeout(WAIT).unwrap();
        assert_eq!(codex_store(&host), "\"file\"", "the account, store {opens}");
        let _ = host.agents.test_message(Provider::Codex, "default", CodexSignIn::ChatGpt).recv_timeout(WAIT).unwrap();
        assert_eq!(codex_store(&host), "\"file\"", "the test message, store {opens}");
        let chat = ChatStart { provider: Provider::Codex, model: "default".into(), ..claude(CHAT, "Chat 1", false) };
        host.agents.start(chat).unwrap();
        host.until("the conversation's end", |e| matches!(e, HostEvent::Chat { event: ChatEvent::Ended { .. }, .. }));
        assert_eq!(codex_store(&host), "\"file\"", "the conversation, store {opens}");
        let env = std::fs::read_to_string(host.base.join("codex.env")).unwrap();
        let home = host.settings.join("agents").join("codex-home");
        assert!(env.contains(&format!("CODEX_HOME={}\n", home.display())), "the private CODEX_HOME: {env}");
        let private = format!("HOME={}/", host.settings.join("agents").display());
        assert!(env.lines().any(|l| l.starts_with(&private)), "Codex keeps its private HOME: {env}");
        let meta = std::fs::symlink_metadata(&home).unwrap();
        assert!(meta.is_dir() && meta.permissions().mode() & 0o777 == 0o700, "CODEX_HOME is 0700: {:o}", meta.permissions().mode());
    }
    // An API key goes with the ephemeral store, never into Codex's files.
    let host = Host::new("chatgpt-files-key", Setup { codex: CODEX_HELLO_ENDS, ..Setup::default() });
    let _ = host.agents.test_message(Provider::Codex, "default", CodexSignIn::ApiKey).recv_timeout(WAIT).unwrap();
    assert_eq!(codex_store(&host), "\"ephemeral\"", "the API key's test message");
}

/// At most eight conversations run at once, and an id that is not a conversation's UUID is refused
/// before anything starts.
#[test]
fn at_most_eight_conversations_run_and_a_bad_id_is_refused() {
    let host = Host::new("cap", Setup::default());
    for n in 1..=MAX_RUNNING as u64 {
        host.agents.start(claude(&id(n), &format!("Chat {n}"), false)).unwrap();
    }
    assert_eq!(host.agents.start(claude(&id(99), "Chat 99", false)), Err(ChatRefusal::TooMany { most: MAX_RUNNING }));
    for bad in ["", "Chat 1", "7F3C2A10-0000-4000-8000-0000000000D1", "../conversation-x", "7f3c2a10-0000-4000-8000-0000000000d1/..", &"a".repeat(65)]
    {
        assert_eq!(host.agents.start(claude(bad, "Chat", false)), Err(ChatRefusal::BadId { bytes: bad.len() }), "{bad:?}");
        assert_eq!(host.agents.send(bad, "hi".into()), Err(ChatRefusal::BadId { bytes: bad.len() }), "{bad:?}");
    }
    assert_eq!(host.agents.send(&id(99), "hi".into()), Err(ChatRefusal::NotRunning));
    let refused = host.agents.start(ChatStart { origin: "Chat\n1".into(), ..claude(&id(98), "x", false) });
    assert!(matches!(refused, Err(ChatRefusal::Refused { .. })), "a bad origin label: {refused:?}");
    // One ends: room for another.
    host.agents.end(&id(1)).unwrap();
    host.until("an end", |e| matches!(e, HostEvent::Chat { event: ChatEvent::Ended { .. }, .. }));
    let deadline = Instant::now() + WAIT;
    while host.agents.start(claude(&id(99), "Chat 99", false)).is_err() {
        assert!(Instant::now() < deadline, "the ended conversation's place came back");
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// A conversation's thread that fails inside Archforge still gives its access back: the token is
/// revoked, the opening closes and the window is told the conversation ended.
#[test]
fn a_failing_conversation_thread_still_gives_its_access_back() {
    let host = Host::new("panic", Setup { cli: Arc::new(|| panic!("a failure inside the host")), ..Setup::default() });
    host.agents.start(claude(CHAT, "Chat 1", false)).unwrap();
    let events = host.until("the end", |e| matches!(e, HostEvent::Chat { event: ChatEvent::Ended { .. }, .. }));
    assert!(access(&events, true), "access was granted first: {events:?}");
    let closed = if access(&events, false) { events } else { host.until("access closed", |e| matches!(e, HostEvent::Access { held: false })) };
    assert!(access(&closed, false));
    assert!(host.token_files().is_empty(), "the token was revoked and its file removed");
    let deadline = Instant::now() + WAIT;
    while host.agents.start(claude(CHAT, "Chat 1", false)) == Err(ChatRefusal::AlreadyRunning) {
        assert!(Instant::now() < deadline, "the failed conversation left the running list");
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Codex's account is read once the AI Agents page shows (and again on each later showing, AR-222)
/// or a Codex conversation starts, never at launch: a plain launch starts no `codex app-server`
/// (only the version look), and a Claude conversation starts none either.
#[test]
fn codex_account_is_read_on_first_use_never_at_launch() {
    let host = Host::new("first-use", Setup { codex: CODEX_ACCOUNT, ..Setup::default() });
    let Host { agents, .. } = &host;
    let agents: Arc<dyn AgentHost> = Arc::new(Agents { inner: agents.inner.clone() });
    archforge_ui_egui::i18n::set_language("en");
    let mut h = egui_kittest::Harness::builder().with_size(egui::vec2(1600.0, 1400.0)).build_ui_state(
        |ui, shell: &mut Shell| {
            shell.logic(ui.ctx());
            shell.ui(ui);
        },
        Shell::new(archforge_ui_egui::Services { agents: Some(agents), ..archforge_ui_egui::Services::default() }),
    );
    let frames_until = |h: &mut egui_kittest::Harness<'_, Shell>, what: &str, done: &dyn Fn(&Shell) -> bool| {
        let deadline = Instant::now() + WAIT;
        while !done(h.state()) {
            assert!(Instant::now() < deadline, "waited too long for {what}");
            std::thread::sleep(Duration::from_millis(10));
            h.run();
        }
    };
    frames_until(&mut h, "the look at launch", &|s| !s.agents.looking() && s.agents.look.codex.found());
    for _ in 0..20 {
        h.run();
    }
    assert!(!h.state().agents.reading_account() && h.state().agents.codex.is_none(), "no account read was asked at launch");
    // Time for a read, had one been asked, to start its program.
    std::thread::sleep(Duration::from_millis(500));
    assert!(host.log("codex").iter().all(|l| l == "--version"), "a plain launch starts no app-server: {:?}", host.log("codex"));
    // A Claude conversation reads no Codex account.
    archforge_ui_egui::chat::start(h.state_mut(), claude(CHAT, "Chat 1", false)).unwrap();
    for _ in 0..20 {
        h.run();
    }
    assert!(host.log("codex").iter().all(|l| l == "--version"), "{:?}", host.log("codex"));
    // Opening AI Agents reads it, once.
    archforge_ui_egui::settings::open_at(h.state_mut(), archforge_ui_egui::settings::Section::Agents);
    frames_until(&mut h, "the account", &|s| s.agents.codex.is_some());
    assert_eq!(host.log("codex").iter().filter(|l| *l == "app-server").count(), 1);
    assert_eq!(h.state().agents.codex, Some(Ok(CodexAccount::SignedIn { email: "person@example.com".into(), plan: Some("plus".into()) })));
}

/// Removing a deleted conversation's private folder (AR-217): its running program is ended first
/// and has ended when the answer comes, its folder goes and every other conversation's stays; one
/// waiting for the project goes only when the answer says deleted, never when it says listed or
/// does not come; one asked just before quitting has finished when quitting returns; an id that is
/// not a conversation's is refused and one without a folder says so.
#[test]
fn forgetting_a_deleted_conversation_ends_its_program_and_removes_its_folder_alone_even_at_quit() {
    let host = Host::new("forget", Setup::default());
    let (gone, kept, last) = (CHAT.to_string(), id(2), id(3));
    let folder = |chat: &str| host.settings.join("agents").join(format!("conversation-{chat}"));
    for chat in [&gone, &kept, &last] {
        host.agents.start(claude(chat, "Chat 1", false)).unwrap();
    }
    let deadline = Instant::now() + WAIT;
    let pids = || std::fs::read_to_string(host.base.join("claude.pids")).unwrap_or_default();
    while pids().lines().count() < 3 {
        assert!(Instant::now() < deadline, "the three programs started");
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!([&gone, &kept, &last].iter().all(|c| folder(c).is_dir()));
    let answer = host.agents.forget(&gone).recv_timeout(WAIT).unwrap();
    assert_eq!(answer, Ok(true));
    assert!(!folder(&gone).exists(), "its folder is gone");
    assert!(folder(&kept).is_dir() && folder(&last).is_dir(), "the others' stay");
    // It left the running list as its program ended; the window is told so too.
    host.until("its end", |e| matches!(e, HostEvent::Chat { chat, event: ChatEvent::Ended { .. }, .. } if *chat == gone));
    assert_eq!(host.agents.send(&gone, "still there?".into()), Err(ChatRefusal::NotRunning));
    assert!(host.agents.send(&kept, "still there?".into()).is_ok(), "another conversation runs on");
    // Nothing to remove, and an id that is not a conversation's.
    assert_eq!(host.agents.forget(&id(9)).recv_timeout(WAIT).unwrap(), Ok(false));
    assert!(host.agents.forget("../settings").recv_timeout(WAIT).unwrap().is_err());
    assert!(host.settings.is_dir());
    // Once the project answered: a deletion lets the folder go; a restoration refused because
    // the conversation is listed, or no answer at all, keeps it.
    use archforge_ui_egui::ai_agents::FolderGoes;
    use archforge_ui_egui::chat_store::{Answer, Refusal};
    let answer = |a: Option<Result<Answer, Refusal>>| {
        let (tx, rx) = std::sync::mpsc::channel();
        if let Some(a) = a {
            tx.send(a).unwrap();
        }
        rx
    };
    let made = |chat: &str| {
        let id = archforge_agents::ConversationId::new(chat).unwrap();
        archforge_agents::Workspace::open(&host.settings.join("agents")).unwrap().conversation(&id).unwrap();
    };
    let (listed, unanswered, deleted) = (id(4), id(5), id(6));
    for chat in [&listed, &unanswered, &deleted] {
        made(chat);
    }
    let kept_listed = host.agents.forget_after(&listed, vec![(answer(Some(Err(Refusal::Listed))), FolderGoes::IfNotRestored)]);
    let kept_silent = host.agents.forget_after(&unanswered, vec![(answer(None), FolderGoes::IfDeleted)]);
    let removed = host.agents.forget_after(&deleted, vec![(answer(Some(Ok(Answer::Deleted { before: "0".repeat(32) }))), FolderGoes::IfDeleted)]);
    assert_eq!(
        (kept_listed.recv_timeout(WAIT).unwrap(), kept_silent.recv_timeout(WAIT).unwrap(), removed.recv_timeout(WAIT).unwrap()),
        (Ok(false), Ok(false), Ok(true))
    );
    assert!(folder(&listed).is_dir() && folder(&unanswered).is_dir() && !folder(&deleted).exists());
    // Asked just before quitting: done when quitting returns.
    let at_quit = host.agents.forget(&last);
    host.agents.shutdown();
    assert!(!folder(&last).exists(), "removed by the time quitting returned");
    assert_eq!(at_quit.try_recv(), Ok(Ok(true)));
    assert!(folder(&kept).is_dir(), "a conversation not deleted keeps its folder at quit");
}

/// A channel already holding the project's answer `a`, or closed without one.
fn project_answer(
    a: Option<Result<archforge_ui_egui::chat_store::Answer, archforge_ui_egui::chat_store::Refusal>>,
) -> Receiver<Result<archforge_ui_egui::chat_store::Answer, archforge_ui_egui::chat_store::Refusal>> {
    let (tx, rx) = std::sync::mpsc::channel();
    if let Some(a) = a {
        tx.send(a).unwrap();
    }
    rx
}

/// Makes the private folder of the conversation `chat` in `host`'s workspace, as its program would,
/// and gives its path.
fn conversation_folder(host: &Host, chat: &str) -> PathBuf {
    let id = archforge_agents::ConversationId::new(chat).unwrap();
    archforge_agents::Workspace::open(&host.settings.join("agents")).unwrap().conversation(&id).unwrap();
    host.settings.join("agents").join(format!("conversation-{chat}"))
}

/// An Undo pressed before its deletion answered, its project then closed (AR-218): the host gets
/// both answers and removes the folder only when the deletion said deleted and the restoration did
/// not bring the conversation back. A deletion that failed, or whose answer never came, keeps the
/// folder even when the restoration after it failed too (as when the project could not read its
/// list), and so does a restoration that brought it back or found it listed.
#[test]
fn an_undo_before_its_deletion_answered_lets_the_folder_go_only_once_deleted_and_not_brought_back() {
    use archforge_ui_egui::ai_agents::FolderGoes;
    use archforge_ui_egui::chat_store::{Answer, Refusal};
    let host = Host::new("forget-both", Setup::default());
    let deleted = || Some(Ok(Answer::Deleted { before: "0".repeat(32) }));
    let failed = |why: &str| Some(Err(Refusal::Failed(why.into())));
    let cases = [
        ("deletion failed, restoration failed", failed("the project is busy"), failed("no deletion of it was asked in this project"), false),
        ("deletion unanswered, restoration failed", None, failed("no deletion of it was asked in this project"), false),
        ("deleted, brought back", deleted(), Some(Ok(Answer::Done)), false),
        ("deleted, listed", deleted(), Some(Err(Refusal::Listed)), false),
        ("deleted, restoration failed", deleted(), failed("another conversation has its label now"), true),
    ];
    for (n, (case, deletion, restoration, goes)) in cases.into_iter().enumerate() {
        let chat = id(40 + n as u64);
        let folder = conversation_folder(&host, &chat);
        let answers = vec![(project_answer(deletion), FolderGoes::IfDeleted), (project_answer(restoration), FolderGoes::IfNotRestored)];
        let answer = host.agents.forget_after(&chat, answers).recv_timeout(WAIT).unwrap();
        assert_eq!(answer, Ok(goes), "{case}");
        assert_eq!(!folder.exists(), goes, "{case}: the folder {}", if goes { "goes" } else { "stays" });
    }
}

/// While a conversation's private folder is being removed (AR-218), the host refuses to start it,
/// saying so in a refusal of its own, both while the removal waits for the project's answers and
/// while it waits for the conversation's program to end; other conversations start meanwhile. A
/// start refused meanwhile keeps the folder, as a panel (a copy of the project, say) shows the
/// conversation and will start it again; without one, the folder goes when the answers let it.
/// Once the removal is done, whatever it decided, the conversation starts again.
#[test]
fn a_conversation_does_not_start_while_its_folder_is_being_removed_and_starts_once_it_is_done() {
    use archforge_ui_egui::ai_agents::FolderGoes;
    use archforge_ui_egui::chat_store::{Answer, Refusal};
    let host = Host::new("forget-start", Setup::default());
    let deleted = || Ok(Answer::Deleted { before: "0".repeat(32) });
    let cases = [
        ("deleted, a start refused meanwhile", deleted(), true, false),
        ("deleted, no start asked", deleted(), false, true),
        ("not deleted, a start refused meanwhile", Err(Refusal::Failed("the project is busy".into())), true, false),
    ];
    for (n, (case, answer, ask, goes)) in cases.into_iter().enumerate() {
        let chat = id(50 + n as u64);
        let folder = conversation_folder(&host, &chat);
        let (tx, rx) = std::sync::mpsc::channel();
        let removal = host.agents.forget_after(&chat, vec![(rx, FolderGoes::IfDeleted)]);
        if ask {
            assert_eq!(host.agents.start(claude(&chat, "Chat 1", false)), Err(ChatRefusal::Forgetting), "{case}: refused while the removal waits");
            assert_eq!(host.agents.start(claude(&chat, "Chat 1", false)), Err(ChatRefusal::Forgetting), "{case}: and asked again");
        }
        assert!(folder.is_dir(), "{case}: its folder is untouched meanwhile");
        let other = id(60 + n as u64);
        host.agents.start(claude(&other, "Chat 2", false)).unwrap();
        tx.send(answer).unwrap();
        assert_eq!(removal.recv_timeout(WAIT).unwrap(), Ok(goes), "{case}");
        assert_eq!(!folder.exists(), goes, "{case}");
        let aside: Vec<_> = std::fs::read_dir(host.settings.join("agents"))
            .unwrap()
            .filter_map(|e| e.ok().map(|e| e.file_name().to_string_lossy().into_owned()))
            .filter(|n| n.starts_with(&format!("conversation-{chat}-rm-")))
            .collect();
        assert!(aside.is_empty(), "{case}: nothing is left moved aside: {aside:?}");
        host.agents.start(claude(&chat, "Chat 1", false)).unwrap();
        for c in [&chat, &other] {
            host.agents.end(c).unwrap();
            host.until("its end", |e| matches!(e, HostEvent::Chat { chat, event: ChatEvent::Ended { .. }, .. } if chat.as_str() == c.as_str()));
        }
    }
    // A program slow to end: the removal waits for it, and a start while it ends is refused too
    // and keeps the folder. The program says when its input closed, that is when it was asked to
    // end, then takes a second.
    let slow = Host::new(
        "forget-slow",
        Setup { claude: "while IFS= read -r line; do :; done\nprintf 'closed\\n' >> \"$IN\"\nsleep 1\n", ..Setup::default() },
    );
    slow.agents.start(claude(CHAT, "Chat 1", false)).unwrap();
    let folder = slow.settings.join("agents").join(format!("conversation-{CHAT}"));
    // Its program has started (a conversation asked to end before then starts none, AR-228).
    let started = || slow.base.join("claude.pids").exists();
    let deadline = Instant::now() + WAIT;
    while !folder.is_dir() || !started() {
        assert!(Instant::now() < deadline, "its program started in its folder");
        std::thread::sleep(Duration::from_millis(10));
    }
    let removal = slow.agents.forget(CHAT);
    let closed = || std::fs::read_to_string(slow.base.join("claude.stdin")).unwrap_or_default().contains("closed");
    let deadline = Instant::now() + WAIT;
    while !closed() {
        assert!(Instant::now() < deadline, "the removal asked its program to end");
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(slow.agents.start(claude(CHAT, "Chat 1", false)), Err(ChatRefusal::Forgetting), "refused while its program ends");
    assert_eq!(removal.recv_timeout(WAIT).unwrap(), Ok(false), "kept for the start refused");
    assert!(folder.is_dir());
    slow.agents.start(claude(CHAT, "Chat 1", false)).unwrap();
}

/// A start that comes as a removal has decided and its folder goes (CodeRabbit, AR-218): the
/// removal decided and moved the folder aside in one step under the lock a start takes, so that
/// start is taken, and makes a new folder, rather than refused while the folder it would keep is
/// removed. The start is put in that exact place by the host's test hook, on the removal's thread,
/// with no timing involved; nothing is left moved aside.
#[test]
fn a_start_as_the_folder_goes_is_taken_and_never_refused_while_its_folder_is_lost() {
    let host = Host::new("forget-between", Setup::default());
    let chat = id(70);
    let folder = conversation_folder(&host, &chat);
    std::fs::write(folder.join("config").join("transcript.jsonl"), "{}").unwrap();
    let seen: Arc<Mutex<Option<Result<(), ChatRefusal>>>> = Arc::new(Mutex::new(None));
    {
        let (inner, seen, chat) = (host.agents.inner.clone(), seen.clone(), chat.clone());
        *host.agents.inner.as_folder_goes.lock().unwrap() = Some(Box::new(move || {
            *seen.lock().unwrap() = Some(super::sessions::start(&inner, claude(&chat, "Chat 1", false)));
        }));
    }
    assert_eq!(host.agents.forget(&chat).recv_timeout(WAIT).unwrap(), Ok(true), "the deleted conversation's folder went");
    let started = seen.lock().unwrap().take().expect("a start was put as the folder goes");
    assert_eq!(started, Ok(()), "taken, not refused while its folder is removed");
    assert!(!folder.join("config").join("transcript.jsonl").exists(), "the old folder's contents went with it");
    let aside: Vec<_> = std::fs::read_dir(host.settings.join("agents"))
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.file_name().to_string_lossy().into_owned()))
        .filter(|n| n.starts_with(&format!("conversation-{chat}-rm-")))
        .collect();
    assert!(aside.is_empty(), "nothing is left moved aside: {aside:?}");
    host.agents.end(&chat).unwrap();
    host.until("its end", |e| matches!(e, HostEvent::Chat { chat: c, event: ChatEvent::Ended { .. }, .. } if *c == chat));
}
