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

/// Without Let Chat use the model, a conversation runs with no tool server at all: nothing listens,
/// no token is minted, and the window is never told access is held.
#[test]
fn agent_access_never_opens_without_the_opt_in() {
    let host = Host::new("no-opt-in", Setup { claude: &claude_replay("claude/doc-turn-with-mcp-tool.jsonl"), ..Setup::default() });
    host.agents.start(claude(CHAT, "Chat 1", true)).unwrap();
    assert_eq!(host.mcp(CHAT), json!({ "mcpServers": {} }), "no tool server without the opt-in");
    host.agents.send(CHAT, "Which entities are in Shop?".into()).unwrap();
    let events = host.turn(CHAT);
    assert!(events.iter().any(|e| matches!(e, ChatEvent::Started { access: false, .. })), "{events:?}");
    assert!(host.token_files().is_empty(), "no token was minted");
    host.agents.end(CHAT).unwrap();
    let rest = host.until("the end", |e| matches!(e, HostEvent::Chat { event: ChatEvent::Ended { .. }, .. }));
    assert!(!access(&rest, true), "access was never held");
}

/// A whole conversation with agent access: its own read-only token in a private file, the bridge
/// configured with it, a real client answered with its origin, a turn with a tool call over the
/// channel with the window woken, stop, a refused duplicate, and the end giving everything back.
#[test]
fn a_conversation_holds_its_own_read_only_token_until_it_ends() {
    let host = Host::new("session", Setup { claude: &claude_replay("claude/doc-turn-with-mcp-tool.jsonl"), ..Setup::default() });
    host.agents.set_model_access(true);
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
    assert!(listens(port));
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
    assert!(!listens(port), "nothing listens once the last conversation ended");
    assert!(matches!(Client::with_token(port, "0".repeat(64)).call("document.inspect", json!({})), Err(CallError::Unreachable(_))));
    assert_eq!(host.agents.send(CHAT, "again".into()), Err(ChatRefusal::NotRunning));
    assert!(!logged(CLAUDE_CANARY) && !logged(CODEX_CANARY), "no key reached the log");
}

/// Turning the opt-in off closes agent access at once, a held connection answered "revoked"; the
/// conversation goes on without it. Quitting closes it too, ends every conversation and waits for
/// its program.
#[test]
fn turning_the_opt_in_off_or_quitting_closes_access_at_once() {
    let host = Host::new("close", Setup::default());
    host.agents.set_model_access(true);
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
    let mut r = held(&token, port, &host);
    let started = Instant::now();
    host.agents.set_model_access(false);
    assert!(started.elapsed() < Duration::from_secs(3), "closed at once: {:?}", started.elapsed());
    let last: Value = serde_json::from_slice(&archforge_control::read_line(&mut r, archforge_control::MAX_REPLY).unwrap().unwrap()).unwrap();
    assert_eq!(last["error"], archforge_control::REVOKED);
    assert_eq!(archforge_control::read_line(&mut r, archforge_control::MAX_REPLY).unwrap(), None, "then closed");
    assert!(!listens(port) && !token_file.exists());
    host.until("access closed", |e| matches!(e, HostEvent::Access { held: false }));
    assert!(host.agents.send(CHAT, "still here?".into()).is_ok(), "the conversation goes on without access");
    // On again, a second conversation holds it; quitting closes it and ends both.
    host.agents.set_model_access(true);
    let second = id(2);
    host.agents.start(claude(&second, "Chat 2", true)).unwrap();
    host.until("access held again", |e| matches!(e, HostEvent::Access { held: true }));
    let (port, token_file) = bridge_of(&host.mcp(&second));
    let mut r = held(&archforge_control::read_token(&token_file).unwrap(), port, &host);
    host.agents.shutdown();
    let last: Value = serde_json::from_slice(&archforge_control::read_line(&mut r, archforge_control::MAX_REPLY).unwrap().unwrap()).unwrap();
    assert_eq!(last["error"], archforge_control::REVOKED);
    assert!(!listens(port) && !token_file.exists());
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
    host.agents.set_model_access(true);
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
    assert!(listens(port), "the writer still holds access");
}

/// The test message runs with no tool server, in a scratch folder removed afterwards; the Codex
/// account is read, signed in with a device code and in the browser, and signed out, each through a
/// short-lived codex program, with the ChatGPT sign-in in the keyring where the store opens.
#[test]
fn the_test_message_and_codex_account_calls_run_through_their_programs() {
    let host =
        Host::new("accounts", Setup { claude: &claude_replay("claude/doc-turn-with-mcp-tool.jsonl"), codex: CODEX_ACCOUNT, ..Setup::default() });
    host.agents.set_model_access(true);
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
    assert!(std::fs::read_to_string(host.base.join("codex.args")).unwrap().contains("cli_auth_credentials_store=\"keyring\""));
    // A device-code sign-in: the code, then who is signed in.
    let events: Vec<LoginEvent> = host.agents.codex_login_start(LoginWay::DeviceCode).iter().collect();
    assert_eq!(
        events,
        vec![
            LoginEvent::DeviceCode { code: "ABCD-EFGH".into(), url: "https://auth.openai.com/codex/device".into(), minutes: 15 },
            LoginEvent::Done(CodexAccount::SignedIn { email: "person@example.com".into(), plan: Some("plus".into()) }),
        ]
    );
    // In the browser: the page to open, then who is signed in.
    let events: Vec<LoginEvent> = host.agents.codex_login_start(LoginWay::Browser).iter().collect();
    assert!(
        matches!(&events[..], [LoginEvent::OpenBrowser { url }, LoginEvent::Done(CodexAccount::SignedIn { .. })] if url.starts_with("https://auth.openai.com/oauth/authorize")),
        "{events:?}"
    );
    assert!(archforge_ui_egui::ai_agents::approved_browser_page("https://auth.openai.com/oauth/authorize?PLACEHOLDER").is_some());
    assert_eq!(host.agents.codex_logout().recv_timeout(WAIT).unwrap(), Ok(()));
    assert!(!logged("person@example.com") && !logged("ABCD-EFGH"), "no sign-in value reached the log");
    assert!(!format!("{events:?}").contains("person@example.com"), "Debug shows no email");
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

/// The ChatGPT sign-in is kept in the system keyring where the secure store opens, and in the codex
/// program's own files in Archforge's private `CODEX_HOME` where it does not.
#[test]
fn the_chatgpt_sign_in_store_follows_whether_the_secure_store_opens() {
    for (opens, store) in [(true, "keyring"), (false, "file")] {
        let host = Host::new(&format!("store-{store}"), Setup { codex: CODEX_ACCOUNT, store: opens, ..Setup::default() });
        assert!(host.agents.codex_account_read().recv_timeout(WAIT).unwrap().is_ok());
        let args = std::fs::read_to_string(host.base.join("codex.args")).unwrap();
        assert!(args.contains(&format!("cli_auth_credentials_store=\"{store}\"")), "{opens}: {args}");
        let env = std::fs::read_to_string(host.base.join("codex.env")).unwrap();
        assert!(env.contains(&format!("CODEX_HOME={}", host.settings.join("agents").join("codex-home").display())), "the private CODEX_HOME: {env}");
    }
    assert_eq!(super::chatgpt_store(&Ok(Box::new(MemoryStore::new()))), archforge_agents::codex::ChatGptStore::Keyring);
    assert_eq!(super::chatgpt_store(&Err(SecretError::NoSecureStore { reason: "none".into() })), archforge_agents::codex::ChatGptStore::File);
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
    host.agents.set_model_access(true);
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

/// Codex's account is read the first time the AI Agents page shows or a Codex conversation starts,
/// never at launch: a plain launch starts no `codex app-server` (only the version look), and a
/// Claude conversation starts none either.
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
