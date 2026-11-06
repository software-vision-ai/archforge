//! The Claude adapter against the AR-196 transcripts and against fake `claude` programs: shell
//! scripts that answer `--version` as Claude Code 2.1.317 does and then replay a transcript or do
//! what a test needs. No test starts an installed `claude` or uses a real key, but the one ignored
//! round trip at the end, which needs the owner's OK.
//!
//! The transcripts in `tests/fixtures/` come from the spike (`docs/agent-runtimes/README.md` on the
//! AR-196 branch): one JSON object a line, `{"t", "dir": in|out|err|sig|exit|note, "line"}`.
//! `observed-*` were recorded from Claude Code 2.1.317 and sanitised (paths under
//! `/tmp/archforge-agent/`, the host name `host.example`, the installation id zeroed, colour escapes
//! removed); `doc-*` were written from the documentation and start with a `note` line saying so;
//! `synthetic-hostile-lines.jsonl` holds made-up lines every parser must survive.
#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use archforge_agents::claude::{
    Bridge, BridgeHome, ClaudeConfig, ClaudeParser, ClaudeSession, KEY_VAR, MCP_CONFIG_FILE, Model, SessionId, Start, command_line, mcp_config,
};
use archforge_agents::{
    AgentError, AgentSession, ConversationFolder, ConversationId, Dropped, ErrorKind, HostEnv, Limits, Runtime, RuntimeKind, SessionEvent, Skipped,
    TurnOutcome, Usage, Workspace, find,
};
use archforge_secrets::Secret;
use serde_json::Value;

/// A key no output of the adapter may ever show, as long as a real one and of its alphabet, so no
/// cap of the adapter is longer than it.
const CANARY: &str = "sk-ant-api03-CANARY-9e4b1c7a0d2f_canary-canary-canary-canary-canary-canary-canary-canary-canary-canary-xyzAA";
/// The conversation's UUID in the tests.
const SESSION: &str = "7f3c2a10-0000-4000-8000-0000000000d1";

// --- helpers -------------------------------------------------------------------------------------

/// Every log line any test wrote, to look for the key in.
static LOG: Mutex<Vec<String>> = Mutex::new(Vec::new());

struct Capture;

impl log::Log for Capture {
    fn enabled(&self, _: &log::Metadata<'_>) -> bool {
        true
    }
    fn log(&self, record: &log::Record<'_>) {
        LOG.lock().unwrap_or_else(PoisonError::into_inner).push(format!("{} {}", record.level(), record.args()));
    }
    fn flush(&self) {}
}

/// Sends every log line of this test binary to [`LOG`].
fn capture_logs() {
    static LOGGER: Capture = Capture;
    if log::set_logger(&LOGGER).is_ok() {
        log::set_max_level(log::LevelFilter::Trace);
    }
}

/// One line of a transcript.
struct Record {
    dir: String,
    line: String,
}

/// The transcript `name` under `tests/fixtures/`.
fn fixture(name: &str) -> Vec<Record> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let v: Value = serde_json::from_str(l).unwrap();
            Record { dir: v["dir"].as_str().unwrap().to_owned(), line: v["line"].as_str().unwrap().to_owned() }
        })
        .collect()
}

/// Every Claude transcript.
const CLAUDE_FIXTURES: [&str; 10] = [
    "claude/doc-api-errors.jsonl",
    "claude/doc-permission-denied.jsonl",
    "claude/doc-turn-with-mcp-tool.jsonl",
    "claude/observed-keyless-turn.jsonl",
    "claude/observed-network-retries-exhausted.jsonl",
    "claude/observed-network-retry-then-interrupt.jsonl",
    "claude/observed-network-retry-then-sigint.jsonl",
    "claude/observed-no-session-persistence.jsonl",
    "claude/observed-resume-keyless.jsonl",
    "claude/observed-resume-unknown-session.jsonl",
];

/// The events the parser makes of a transcript's output; a stop request or a signal on the input
/// side is told to the parser as the session would.
fn parse(name: &str) -> (Vec<SessionEvent>, u64) {
    let mut parser = ClaudeParser::new(Limits::default().max_line_bytes);
    let mut events = Vec::new();
    for record in fixture(name) {
        match record.dir.as_str() {
            "out" => events.extend(parser.line(&record.line)),
            "sig" => parser.stop_requested(),
            "in" if record.line.contains("control_request") => parser.stop_requested(),
            _ => {}
        }
    }
    (events, parser.skipped())
}

/// The outcomes of every finished turn among `events`.
fn outcomes(events: &[SessionEvent]) -> Vec<&TurnOutcome> {
    events
        .iter()
        .filter_map(|e| match e {
            SessionEvent::TurnFinished { outcome, .. } => Some(outcome),
            _ => None,
        })
        .collect()
}

/// The kind of a failed outcome.
fn failed_kind(outcome: &TurnOutcome) -> Option<ErrorKind> {
    match outcome {
        TurnOutcome::Failed(e) => Some(e.kind()),
        _ => None,
    }
}

/// A fresh folder in the temporary directory for one test, named with the process id.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("archforge-claude-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir.canonicalize().unwrap()
}

/// `text` quoted for the shell.
fn quoted(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\\''"))
}

/// A fake `claude` at `base/bin/claude` that answers `--version`, writes its arguments to
/// `base/args`, its environment to `base/env` and every input line it reads to `base/stdin`, and
/// otherwise runs `body`; found through the override, with a fresh conversation folder.
fn fake_claude(base: &Path, body: &str) -> (Runtime, ConversationFolder) {
    let program = base.join("bin/claude");
    std::fs::create_dir_all(program.parent().unwrap()).unwrap();
    let script = format!(
        "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then echo '2.1.317 (Claude Code)'; exit 0; fi\n\
         printf '%s\\n' \"$@\" > {args}\n/usr/bin/env > {env}\nIN={stdin}\n\
         input() {{ IFS= read -r line || exit 0; printf '%s\\n' \"$line\" >> \"$IN\"; }}\n{body}\n",
        args = quoted(&base.join("args").display().to_string()),
        env = quoted(&base.join("env").display().to_string()),
        stdin = quoted(&base.join("stdin").display().to_string()),
    );
    std::fs::write(&program, script).unwrap();
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
    let env = HostEnv {
        claude_override: Some(program.into()),
        path: Some(std::env::join_paths([Path::new("/usr/bin"), Path::new("/bin")]).unwrap()),
        locale: vec![("LANG".into(), "C.UTF-8".into())],
        ..HostEnv::default()
    };
    let ws = Workspace::open(&base.join("workspace")).unwrap();
    let runtime = find(RuntimeKind::Claude, &env, &ws).unwrap().into_result().unwrap();
    let folder = ws.conversation(&ConversationId::new(SESSION).unwrap()).unwrap();
    (runtime, folder)
}

/// A shell body that replays a transcript: output lines are printed, the error stream's lines
/// written to it, each input line waited for (and recorded), an end of input waited for, a signal
/// waited for, and the exit code kept.
fn replay(name: &str) -> String {
    let mut body = String::from("got=\ntrap 'got=1' INT\n");
    for record in fixture(name) {
        match record.dir.as_str() {
            "out" => body.push_str(&format!("printf '%s\\n' {}\n", quoted(&record.line))),
            "err" => body.push_str(&format!("printf '%s\\n' {} >&2\n", quoted(&record.line))),
            "in" if record.line == "<EOF>" => body.push_str("while IFS= read -r line; do :; done\n"),
            "in" => body.push_str("input\n"),
            "sig" => body.push_str("while [ -z \"$got\" ]; do sleep 0.02; done\n"),
            "exit" => body.push_str(&format!("exit {}\n", record.line)),
            _ => {}
        }
    }
    body
}

/// The bridge of the tests: an `archforge-cli` that need not exist, read-only unless `allow`.
fn bridge(allow: bool) -> Bridge {
    Bridge {
        cli: PathBuf::from("/opt/archforge/bin/archforge-cli"),
        port: 47000,
        origin: "Chat 1".to_owned(),
        allow_changes: allow,
        home: BridgeHome { home: Some(PathBuf::from("/Users/person")), xdg_config_home: None },
    }
}

/// A configuration for a new conversation with `sonnet`.
fn config(allow: bool) -> ClaudeConfig {
    ClaudeConfig {
        session: SessionId::new(SESSION).unwrap(),
        start: Start::New,
        model: Model::new("sonnet").unwrap(),
        bridge: bridge(allow),
        limits: Limits::default(),
    }
}

/// The canary as a secret.
fn canary() -> Secret {
    Secret::new(CANARY.to_owned()).unwrap()
}

/// Events until (and with) the first that `stop` accepts, within five seconds.
fn events_until(session: &mut ClaudeSession, stop: impl Fn(&SessionEvent) -> bool) -> Vec<SessionEvent> {
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut events = Vec::new();
    while Instant::now() < deadline {
        if let Some(event) = session.next_event(Duration::from_millis(100)) {
            let done = stop(&event);
            events.push(event);
            if done {
                return events;
            }
        }
    }
    panic!("the awaited event did not come within five seconds: {events:?}");
}

/// Whether `event` ends a turn.
fn turn_end(event: &SessionEvent) -> bool {
    matches!(event, SessionEvent::TurnFinished { .. })
}

/// The lines the fake read from its input, waiting up to five seconds for `count` of them.
fn stdin_lines(base: &Path, count: usize) -> Vec<String> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let text = std::fs::read_to_string(base.join("stdin")).unwrap_or_default();
        let lines: Vec<String> = text.lines().map(str::to_owned).collect();
        if lines.len() >= count || Instant::now() >= deadline {
            return lines;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

// --- the parser over the transcripts -------------------------------------------------------------

/// A read-only turn: live text, one Archforge tool call and its result, the answer, and the token
/// counts without a cost.
#[test]
fn a_turn_with_a_tool_call_reads_in_order() {
    let (events, skipped) = parse("claude/doc-turn-with-mcp-tool.jsonl");
    assert_eq!(skipped, 0, "{events:?}");
    let SessionEvent::Started { session_id, model, tools, servers } = &events[0] else { panic!("{events:?}") };
    assert_eq!((session_id.as_str(), model.as_str()), (SESSION, "claude-sonnet-5-5"));
    assert_eq!(tools.len(), 3);
    assert!(tools.iter().all(|t| t.starts_with("mcp__archforge__")));
    assert_eq!((servers[0].name.as_str(), servers[0].status.as_str()), ("archforge", "connected"));
    let rest: Vec<&SessionEvent> = events.iter().skip(1).collect();
    assert_eq!(*rest[0], SessionEvent::TextDelta { text: "Let me look at the schema.".into() });
    assert_eq!(*rest[1], SessionEvent::MessageDone { text: "Let me look at the schema.".into() });
    let SessionEvent::ToolStarted { id, server, tool, input } = rest[2] else { panic!("{rest:?}") };
    assert_eq!((id.as_str(), server.as_deref(), tool.as_str()), ("toolu_01ARCHFORGE000000000001", Some("archforge"), "er_entity_list"));
    assert_eq!(serde_json::from_str::<Value>(input).unwrap()["schema"], "0b6c1d2e-4f3a-4b5c-8d9e-0a1b2c3d4e5f");
    let SessionEvent::ToolFinished { id, tool, ok, summary } = rest[3] else { panic!("{rest:?}") };
    assert_eq!((id.as_str(), tool.as_deref(), *ok), ("toolu_01ARCHFORGE000000000001", Some("er_entity_list"), true));
    assert!(summary.contains("Customer") && summary.contains("Order"), "{summary}");
    assert_eq!(*rest[4], SessionEvent::TextDelta { text: "Shop has two entities: ".into() });
    assert_eq!(*rest[5], SessionEvent::TextDelta { text: "Customer and Order.".into() });
    assert_eq!(*rest[6], SessionEvent::MessageDone { text: "Shop has two entities: Customer and Order.".into() });
    assert_eq!(
        *rest[7],
        SessionEvent::TurnFinished {
            outcome: TurnOutcome::Completed,
            usage: Usage { input_tokens: 4300, output_tokens: 120, cache_read_input_tokens: 0, cache_creation_input_tokens: 0 },
        }
    );
    assert_eq!(rest.len(), 8);
}

/// A tool `dontAsk` denied finishes as not ok, and the turn still completes.
#[test]
fn a_denied_tool_finishes_not_ok() {
    let (events, skipped) = parse("claude/doc-permission-denied.jsonl");
    assert_eq!(skipped, 0);
    assert!(matches!(&events[0], SessionEvent::ToolStarted { tool, .. } if tool == "er_entity_add"));
    assert!(
        matches!(&events[1], SessionEvent::ToolFinished { ok: false, tool: Some(t), summary, .. } if t == "er_entity_add" && summary.contains("denied"))
    );
    assert!(matches!(&events[2], SessionEvent::MessageDone { .. }));
    assert_eq!(outcomes(&events), vec![&TurnOutcome::Completed]);
}

/// Each API failure lands in its category from the category fields, although every one of those
/// results says `subtype: success`; retries are neutral progress with their attempt count.
#[test]
fn api_failures_map_by_category_never_by_subtype() {
    let (events, skipped) = parse("claude/doc-api-errors.jsonl");
    assert_eq!(skipped, 0);
    let kinds: Vec<Option<ErrorKind>> = outcomes(&events).into_iter().map(failed_kind).collect();
    assert_eq!(
        kinds,
        vec![
            Some(ErrorKind::InvalidCredential),
            Some(ErrorKind::RateLimited),
            Some(ErrorKind::RateLimited),
            Some(ErrorKind::ModelUnavailable),
            Some(ErrorKind::ModelUnavailable),
        ]
    );
    let retries: Vec<(u32, Option<u32>, ErrorKind)> = events
        .iter()
        .filter_map(|e| match e {
            SessionEvent::Retrying { attempt, max_attempts, kind, .. } => Some((*attempt, *max_attempts, *kind)),
            _ => None,
        })
        .collect();
    assert_eq!(
        retries,
        vec![(1, Some(10), ErrorKind::RateLimited), (2, Some(10), ErrorKind::RateLimited), (1, Some(10), ErrorKind::ModelUnavailable)]
    );
    // The synthetic error messages are not shown as answers.
    assert!(!events.iter().any(|e| matches!(e, SessionEvent::MessageDone { .. })));
}

/// The observed runs: a keyless turn is an invalid credential, retries without a network are
/// network progress and their end a network failure, both stops end as stopped, an unknown resume
/// is Other, and every observed line is understood.
#[test]
fn the_observed_runs_map_as_the_spike_found() {
    let expect: [(&str, &[Option<ErrorKind>], usize); 7] = [
        ("claude/observed-keyless-turn.jsonl", &[Some(ErrorKind::InvalidCredential)], 0),
        ("claude/observed-network-retries-exhausted.jsonl", &[Some(ErrorKind::Network)], 10),
        ("claude/observed-network-retry-then-interrupt.jsonl", &[None], 4),
        ("claude/observed-network-retry-then-sigint.jsonl", &[None], 4),
        ("claude/observed-no-session-persistence.jsonl", &[Some(ErrorKind::InvalidCredential)], 0),
        ("claude/observed-resume-keyless.jsonl", &[Some(ErrorKind::InvalidCredential)], 0),
        ("claude/observed-resume-unknown-session.jsonl", &[Some(ErrorKind::Other)], 0),
    ];
    for (name, kinds, retries) in expect {
        let (events, skipped) = parse(name);
        assert_eq!(skipped, 0, "{name}: {events:?}");
        let found: Vec<Option<ErrorKind>> = outcomes(&events).into_iter().map(failed_kind).collect();
        assert_eq!(found, kinds, "{name}: {events:?}");
        let retrying: Vec<&SessionEvent> = events.iter().filter(|e| matches!(e, SessionEvent::Retrying { .. })).collect();
        assert_eq!(retrying.len(), retries, "{name}");
        assert!(retrying.iter().all(|e| matches!(e, SessionEvent::Retrying { kind: ErrorKind::Network, max_attempts: Some(10), .. })), "{name}");
    }
    for name in ["claude/observed-network-retry-then-interrupt.jsonl", "claude/observed-network-retry-then-sigint.jsonl"] {
        assert_eq!(outcomes(&parse(name).0), vec![&TurnOutcome::Stopped], "{name}");
    }
    let (events, _) = parse("claude/observed-resume-unknown-session.jsonl");
    let Some(TurnOutcome::Failed(error)) = outcomes(&events).first().copied() else { panic!() };
    assert!(error.to_string().contains("No conversation found"), "{error}");
    let (events, _) = parse("claude/observed-network-retries-exhausted.jsonl");
    let Some(TurnOutcome::Failed(error)) = outcomes(&events).first().copied() else { panic!() };
    assert!(error.to_string().contains("connection_failed"), "{error}");
}

/// Every transcript parses without a panic, and every observed one without a skipped line.
#[test]
fn every_transcript_parses() {
    for name in CLAUDE_FIXTURES {
        let (events, skipped) = parse(name);
        assert!(!events.is_empty(), "{name}");
        assert_eq!(skipped, 0, "{name}");
        assert_eq!(
            outcomes(&events).len(),
            fixture(name).iter().filter(|r| r.dir == "out" && r.line.contains("\"type\":\"result\"")).count(),
            "{name}"
        );
    }
}

/// Hostile lines become typed events or are skipped and counted; nothing panics, nothing is held
/// past the cap.
#[test]
fn hostile_lines_are_typed_or_skipped_and_counted() {
    let lines: Vec<String> = fixture("synthetic-hostile-lines.jsonl").into_iter().filter(|r| r.dir == "out").map(|r| r.line).collect();
    let mut parser = ClaudeParser::new(Limits::default().max_line_bytes);
    let events: Vec<Vec<SessionEvent>> = lines.iter().map(|l| parser.line(l)).collect();
    assert_eq!(events[0], vec![SessionEvent::Ignored(Skipped::Empty)]);
    assert_eq!(events[1], vec![SessionEvent::Ignored(Skipped::NotJson { bytes: 15 })]);
    assert_eq!(events[2], vec![SessionEvent::Ignored(Skipped::NotAnObject)]);
    assert_eq!(events[3], vec![SessionEvent::Ignored(Skipped::NotAnObject)]);
    assert_eq!(events[4], vec![SessionEvent::Ignored(Skipped::Unknown { kind: "brand_new_event".into() })]);
    // A result with no fields still ends the turn, as a failure that says so.
    assert!(
        matches!(&events[5][..], [SessionEvent::TurnFinished { outcome: TurnOutcome::Failed(e), usage }] if e.kind() == ErrorKind::Other && *usage == Usage::default())
    );
    assert_eq!(events[6], vec![SessionEvent::Ignored(Skipped::Malformed { kind: "assistant", reason: "it has no message object" })]);
    assert!(matches!(&events[7][..], [SessionEvent::Ignored(Skipped::NotJson { .. })]), "deep nesting is refused, not recursed into");
    assert!(matches!(&events[8][..], [SessionEvent::TextDelta { text }] if text.len() > 4000));
    assert_eq!(events[9], vec![SessionEvent::Ignored(Skipped::Untyped)]);
    assert_eq!(events[10], Vec::new(), "a stray control response that is not ours changes nothing");
    assert_eq!(parser.skipped(), 8);

    // The oversized text, repeated past the cap, is skipped by its length alone.
    let mut small = ClaudeParser::new(1024);
    let big = lines[8].replace("AAAA", &"A".repeat(400));
    assert!(big.len() > 1024);
    assert_eq!(small.line(&big), vec![SessionEvent::Ignored(Skipped::TooLong { bytes: big.len() as u64 })]);
    // Lines the reader dropped (past its cap, or the raw bytes 0xFF 0xFE) are counted too.
    assert_eq!(small.dropped(Dropped::NotText { bytes: 2 }), SessionEvent::Ignored(Skipped::NotText { bytes: 2 }));
    assert_eq!(small.dropped(Dropped::TooLong { bytes: 9_000_000 }), SessionEvent::Ignored(Skipped::TooLong { bytes: 9_000_000 }));
    assert_eq!(small.skipped(), 3);

    // Odd shapes inside known types never panic.
    let mut parser = ClaudeParser::new(1 << 20);
    for line in [
        r#"{"type":"system"}"#,
        r#"{"type":"system","subtype":"api_retry","attempt":-1,"max_retries":99999999999,"retry_delay_ms":"x","error_status":"429"}"#,
        r#"{"type":"system","subtype":"init","tools":[1,null,{"x":1}],"mcp_servers":"none"}"#,
        r#"{"type":"stream_event","event":7}"#,
        r#"{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":5}}}"#,
        r#"{"type":"assistant","message":{"content":[{"type":"tool_use"},{"type":"text","text":null},7]}}"#,
        r#"{"type":"user","message":{"content":[{"type":"tool_result","content":[{"type":"image"},{"text":"x"}]}]}}"#,
        r#"{"type":"result","is_error":"yes","usage":{"input_tokens":-5,"output_tokens":1e40}}"#,
        r#"{"type":"rate_limit_event","rate_limit_info":null}"#,
        r#"{"type":"control_response","response":{"subtype":"error","request_id":7}}"#,
        "{\"type\":\"\\u202e\\u0000\"}",
    ] {
        let _ = parser.line(line);
    }
}

// --- arguments and the tool server configuration -------------------------------------------------

/// The command line is the spike's, with the model on every start and the session id or resume.
#[test]
fn the_command_line_names_the_model_and_the_session() {
    let mcp = Path::new("/tmp/archforge-agent/conversation-x/claude-mcp.json");
    let expected = |last: [&str; 2]| -> Vec<String> {
        let mut args: Vec<String> = [
            "-p",
            "--bare",
            "--input-format",
            "stream-json",
            "--output-format",
            "stream-json",
            "--verbose",
            "--include-partial-messages",
            "--mcp-config",
            "/tmp/archforge-agent/conversation-x/claude-mcp.json",
            "--strict-mcp-config",
            "--tools",
            "",
            "--allowedTools",
            "mcp__archforge__*",
            "--permission-mode",
            "dontAsk",
            "--model",
            "sonnet",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        args.extend(last.iter().map(|s| s.to_string()));
        args
    };
    let text = |args: Vec<std::ffi::OsString>| args.into_iter().map(|a| a.into_string().unwrap()).collect::<Vec<_>>();
    assert_eq!(text(command_line(&config(false), mcp)), expected(["--session-id", SESSION]));
    let resume = ClaudeConfig { start: Start::Resume, ..config(true) };
    assert_eq!(text(command_line(&resume, mcp)), expected(["--resume", SESSION]), "a resume names the model again");
}

/// The configuration starts the bridge read-only unless changes are allowed, blanks the key for it,
/// gives it the person's home back for the token, and refuses what `archforge-cli` would refuse.
#[test]
fn the_tool_server_configuration_is_read_only_unless_allowed() {
    let read_only: Value = serde_json::from_str(&mcp_config(&bridge(false)).unwrap()).unwrap();
    let server = &read_only["mcpServers"]["archforge"];
    assert_eq!(server["command"], "/opt/archforge/bin/archforge-cli");
    assert_eq!(server["args"], serde_json::json!(["mcp", "--bridge", "47000", "--read-only", "--origin", "Chat 1"]));
    assert_eq!(server["env"], serde_json::json!({ "ANTHROPIC_API_KEY": "", "HOME": "/Users/person" }));
    assert_eq!(read_only["mcpServers"].as_object().unwrap().len(), 1, "the bridge is the only server");

    let allowed: Value = serde_json::from_str(&mcp_config(&bridge(true)).unwrap()).unwrap();
    assert_eq!(allowed["mcpServers"]["archforge"]["args"], serde_json::json!(["mcp", "--bridge", "47000", "--origin", "Chat 1"]));
    assert_eq!(allowed["mcpServers"]["archforge"]["env"]["ANTHROPIC_API_KEY"], "");

    let linux = Bridge { home: BridgeHome { home: Some("/home/p".into()), xdg_config_home: Some("/home/p/.cfg".into()) }, ..bridge(false) };
    let linux: Value = serde_json::from_str(&mcp_config(&linux).unwrap()).unwrap();
    assert_eq!(linux["mcpServers"]["archforge"]["env"]["XDG_CONFIG_HOME"], "/home/p/.cfg");

    for bad in [
        Bridge { cli: "archforge-cli".into(), ..bridge(false) },
        Bridge { port: 0, ..bridge(false) },
        Bridge { origin: String::new(), ..bridge(false) },
        Bridge { origin: "--read-only".into(), ..bridge(false) },
        Bridge { origin: "Chat\u{202E}1".into(), ..bridge(false) },
        Bridge { origin: "x".repeat(65), ..bridge(false) },
        Bridge { home: BridgeHome { home: Some("relative".into()), xdg_config_home: None }, ..bridge(false) },
        Bridge { home: BridgeHome::default(), ..bridge(false) },
    ] {
        assert!(matches!(mcp_config(&bad), Err(AgentError::InvalidSetting { .. })), "{bad:?}");
    }
}

// --- sessions against fake runtimes --------------------------------------------------------------

/// The key reaches `claude` only as `ANTHROPIC_API_KEY` in its environment: never on its command
/// line, in the tool server configuration (where it is blanked), an event, an error, a `Debug`
/// output or a log line, even when the runtime echoes it back on every stream.
#[test]
fn the_key_reaches_only_the_runtime_environment() {
    capture_logs();
    let base = scratch("canary");
    let body = r#"input
printf '%s\n' "{\"type\":\"system\",\"subtype\":\"init\",\"session_id\":\"x\",\"model\":\"$ANTHROPIC_API_KEY\",\"tools\":[],\"mcp_servers\":[]}"
printf '%s\n' "{\"type\":\"$ANTHROPIC_API_KEY\"}"
printf '%s\n' "{\"type\":\"system\",\"subtype\":\"$ANTHROPIC_API_KEY\"}"
printf '%s\n' "{\"type\":\"stream_event\",\"event\":{\"type\":\"content_block_delta\",\"delta\":{\"type\":\"text_delta\",\"text\":\"key $ANTHROPIC_API_KEY\"}}}"
pad=$(printf '%01000d' 0)
printf '%s\n' "{\"type\":\"assistant\",\"message\":{\"content\":[{\"type\":\"text\",\"text\":\"$pad Invalid key $ANTHROPIC_API_KEY\"}]},\"error\":\"authentication_failed\",\"is_api_error_message\":true}"
printf '%s\n' '{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":"marker"}}}'
input
printf '%s\n' "{\"type\":\"result\",\"subtype\":\"success\",\"is_error\":true,\"result\":\"bad $ANTHROPIC_API_KEY\",\"api_error_status\":401}"
echo "auth failed for $ANTHROPIC_API_KEY" >&2
exit 3"#;
    let (runtime, folder) = fake_claude(&base, body);
    let cfg = config(false);
    assert!(!format!("{cfg:?}").contains(CANARY));
    let mut session = ClaudeSession::start(&runtime, &folder, cfg, canary()).unwrap();
    assert!(!format!("{session:?}").contains(CANARY));
    session.send("hello").unwrap();
    // Between the error line and the result, the turn's state holds the error's words.
    let mut events = events_until(&mut session, |e| matches!(e, SessionEvent::TextDelta { text } if text == "marker"));
    assert!(!format!("{session:?}").contains(&CANARY[..20]), "{session:?}");
    session.stop().unwrap();
    events.extend(events_until(&mut session, turn_end));
    events.extend(events_until(&mut session, |e| matches!(e, SessionEvent::Ended { .. })));

    let SessionEvent::TurnFinished { outcome: TurnOutcome::Failed(error), .. } = events.iter().find(|e| turn_end(e)).unwrap() else {
        panic!("{events:?}")
    };
    assert_eq!(error.kind(), ErrorKind::InvalidCredential, "a real category during a stop is still shown");
    assert!(events.iter().any(|e| matches!(e, SessionEvent::Ignored(Skipped::Unknown { kind }) if kind == "redacted")), "{events:?}");
    assert!(events.iter().any(|e| matches!(e, SessionEvent::Ignored(Skipped::Unknown { kind }) if kind == "system/redacted")), "{events:?}");
    assert!(error.to_string().contains("<redacted>"), "{error}");
    let SessionEvent::Ended { error: Some(ended), .. } = events.last().unwrap() else { panic!("{events:?}") };
    for (what, text) in
        [("events", format!("{events:?}")), ("error", error.to_string()), ("end", ended.to_string()), ("debug", format!("{session:?}"))]
    {
        assert!(!text.contains(&CANARY[..20]), "{what} shows a part of the key: {text}");
    }
    assert!(!session.process().stderr_tail().contains(CANARY));

    let args = std::fs::read_to_string(base.join("args")).unwrap();
    assert!(!args.contains(CANARY), "{args}");
    let env = std::fs::read_to_string(base.join("env")).unwrap();
    let holding: Vec<&str> = env.lines().filter(|l| l.contains(CANARY)).collect();
    assert_eq!(holding, vec![format!("{KEY_VAR}={CANARY}").as_str()], "the key is in its variable and nowhere else");

    let config_path = folder.path().join(MCP_CONFIG_FILE);
    let written = std::fs::read_to_string(&config_path).unwrap();
    assert!(!written.contains(CANARY), "{written}");
    let written: Value = serde_json::from_str(&written).unwrap();
    assert_eq!(written["mcpServers"]["archforge"]["env"][KEY_VAR], "", "the key is blanked for the tool server");
    assert_eq!(std::fs::metadata(&config_path).unwrap().permissions().mode() & 0o777, 0o600);
    assert!(args.contains(&config_path.display().to_string()));

    let logs = LOG.lock().unwrap_or_else(PoisonError::into_inner).join("\n");
    assert!(!logs.contains(&CANARY[..20]), "{logs}");
    std::fs::remove_dir_all(&base).unwrap();
}

/// A replayed turn end to end through `spawn`: the message goes out as one stream-json line, the
/// events come back in order, and the runtime stays up until shut down.
#[test]
fn a_replayed_turn_runs_end_to_end() {
    let base = scratch("replay");
    let (runtime, folder) = fake_claude(&base, &replay("claude/doc-turn-with-mcp-tool.jsonl"));
    let mut session = ClaudeSession::start(&runtime, &folder, config(false), canary()).unwrap();
    session.send("Which entities are in Shop?\nAnd \"why\"?").unwrap();
    assert!(matches!(session.send("again"), Err(AgentError::TurnRunning { .. })), "one turn at a time");
    let events = events_until(&mut session, turn_end);
    assert!(matches!(&events[0], SessionEvent::Started { .. }));
    assert!(events.iter().any(|e| matches!(e, SessionEvent::ToolStarted { tool, .. } if tool == "er_entity_list")));
    assert!(events.iter().any(|e| matches!(e, SessionEvent::ToolFinished { ok: true, .. })));
    assert!(matches!(events.last(), Some(SessionEvent::TurnFinished { outcome: TurnOutcome::Completed, .. })));
    assert!(!session.turn_running());

    let sent = stdin_lines(&base, 1);
    let line: Value = serde_json::from_str(&sent[0]).unwrap();
    assert_eq!(line["type"], "user");
    assert_eq!(line["message"]["role"], "user");
    assert_eq!(line["message"]["content"], "Which entities are in Shop?\nAnd \"why\"?");
    assert_eq!(line["parent_tool_use_id"], Value::Null);
    let exit = session.shutdown(Duration::from_secs(2)).unwrap();
    assert_eq!(exit.code, Some(0));
    assert_eq!(session.skipped(), 0);
    std::fs::remove_dir_all(&base).unwrap();
}

/// Stop sends the `interrupt` control request: the turn ends as stopped, the runtime stays up.
#[test]
fn stop_sends_the_interrupt_control_request() {
    let base = scratch("stop");
    let (runtime, folder) = fake_claude(&base, &replay("claude/observed-network-retry-then-interrupt.jsonl"));
    let mut session = ClaudeSession::start(&runtime, &folder, config(false), canary()).unwrap();
    session.stop().unwrap();
    assert!(!base.join("stdin").exists(), "no stop is sent while no turn runs");
    session.send("List the entities in the open model.").unwrap();
    let before = events_until(&mut session, |e| matches!(e, SessionEvent::Retrying { attempt: 4, .. }));
    assert!(before.iter().all(|e| !turn_end(e)));
    session.stop().unwrap();
    let after = events_until(&mut session, turn_end);
    assert!(matches!(after.last(), Some(SessionEvent::TurnFinished { outcome: TurnOutcome::Stopped, .. })), "{after:?}");
    let sent = stdin_lines(&base, 2);
    let request: Value = serde_json::from_str(&sent[1]).unwrap();
    assert_eq!(request["type"], "control_request");
    assert_eq!(request["request"]["subtype"], "interrupt");
    assert!(request["request_id"].as_str().unwrap().starts_with("archforge-stop-"));
    assert!(session.process().pid() > 0 && !session.turn_running());
    session.shutdown(Duration::from_secs(2)).unwrap();
    std::fs::remove_dir_all(&base).unwrap();
}

/// A refused stop request falls back to SIGINT to the runtime, which ends the turn.
#[test]
fn a_refused_stop_request_falls_back_to_sigint() {
    let base = scratch("sigint");
    let result = r#"{"type":"result","subtype":"error_during_execution","is_error":true,"terminal_reason":"aborted_streaming"}"#;
    let body = format!(
        "stopped() {{ printf '%s\\n' {result}; }}\ntrap stopped INT\ninput\n\
         printf '%s\\n' '{{\"type\":\"system\",\"subtype\":\"api_retry\",\"attempt\":1,\"max_retries\":10,\"error_status\":null,\"error\":\"unknown\"}}'\n\
         input\nprintf '%s\\n' '{{\"type\":\"control_response\",\"response\":{{\"subtype\":\"error\",\"request_id\":\"archforge-stop-1\",\"error\":\"no\"}}}}'\n\
         while IFS= read -r line; do :; done",
        result = quoted(result)
    );
    let (runtime, folder) = fake_claude(&base, &body);
    let mut session = ClaudeSession::start(&runtime, &folder, config(false), canary()).unwrap();
    session.send("go").unwrap();
    events_until(&mut session, |e| matches!(e, SessionEvent::Retrying { .. }));
    session.stop().unwrap();
    let events = events_until(&mut session, turn_end);
    assert!(matches!(events.last(), Some(SessionEvent::TurnFinished { outcome: TurnOutcome::Stopped, .. })), "{events:?}");
    session.shutdown(Duration::from_secs(2)).unwrap();
    std::fs::remove_dir_all(&base).unwrap();
}

/// A runtime that ends during a turn fails the turn and ends the session with the reason; one too
/// old for a flag is a missing runtime.
#[test]
fn an_end_during_a_turn_fails_it() {
    let base = scratch("crash");
    let (runtime, folder) = fake_claude(&base, "input\necho 'something broke' >&2\nexit 1");
    let mut session = ClaudeSession::start(&runtime, &folder, config(false), canary()).unwrap();
    session.send("hi").unwrap();
    let events = events_until(&mut session, |e| matches!(e, SessionEvent::Ended { .. }));
    assert!(
        matches!(&events[0], SessionEvent::TurnFinished { outcome: TurnOutcome::Failed(e), .. } if e.kind() == ErrorKind::Other && e.to_string().contains("something broke")),
        "{events:?}"
    );
    assert!(matches!(&events[1], SessionEvent::Ended { exit: Some(exit), error: Some(_) } if exit.code == Some(1)));
    assert_eq!(session.next_event(Duration::from_millis(10)), None);
    assert!(matches!(session.send("again"), Err(AgentError::InputClosed { .. })));
    std::fs::remove_dir_all(&base).unwrap();

    let base = scratch("old");
    let (runtime, folder) = fake_claude(&base, "echo \"error: unknown option '--bare'\" >&2\nexit 1");
    let mut session = ClaudeSession::start(&runtime, &folder, config(false), canary()).unwrap();
    let events = events_until(&mut session, |e| matches!(e, SessionEvent::Ended { .. }));
    assert!(matches!(events.last(), Some(SessionEvent::Ended { error: Some(e), .. }) if e.kind() == ErrorKind::MissingRuntime), "{events:?}");
    std::fs::remove_dir_all(&base).unwrap();
}

/// Only a Claude runtime is driven by the Claude adapter.
#[test]
fn settings_are_refused_before_anything_starts() {
    let base = scratch("refuse");
    let (runtime, folder) = fake_claude(&base, "exit 0");
    let bad = ClaudeConfig { bridge: Bridge { origin: " padded".into(), ..bridge(false) }, ..config(false) };
    assert!(matches!(ClaudeSession::start(&runtime, &folder, bad, canary()), Err(AgentError::InvalidSetting { .. })));
    assert!(!base.join("args").exists(), "nothing was started");
    assert!(!folder.path().join(MCP_CONFIG_FILE).exists());
    std::fs::remove_dir_all(&base).unwrap();
}

/// A key in the bridge's settings (pasted into the origin label) is refused before anything is
/// written, as it would stand on the bridge's command line.
#[test]
fn a_key_in_the_tool_server_configuration_is_refused() {
    let base = scratch("origin-key");
    let (runtime, folder) = fake_claude(&base, "exit 0");
    let short = || Secret::new("sk-short-key-1".to_owned()).unwrap();
    let cfg = ClaudeConfig { bridge: Bridge { origin: "sk-short-key-1".into(), ..bridge(false) }, ..config(false) };
    let error = ClaudeSession::start(&runtime, &folder, cfg, short()).unwrap_err();
    assert!(matches!(&error, AgentError::SecretExposed { variable, place: "tool server configuration" } if variable == KEY_VAR), "{error:?}");
    assert!(!error.to_string().contains("sk-short-key-1"));
    assert!(!folder.path().join(MCP_CONFIG_FILE).exists() && !base.join("args").exists());
    std::fs::remove_dir_all(&base).unwrap();
}

/// A link planted where the configuration goes is replaced, never written through.
#[test]
fn the_configuration_never_follows_a_link() {
    let base = scratch("link");
    let (runtime, folder) = fake_claude(&base, "input");
    let outside = base.join("outside.txt");
    std::fs::write(&outside, "keep me").unwrap();
    std::fs::set_permissions(&outside, std::fs::Permissions::from_mode(0o644)).unwrap();
    let place = folder.path().join(MCP_CONFIG_FILE);
    std::os::unix::fs::symlink(&outside, &place).unwrap();
    let mut session = ClaudeSession::start(&runtime, &folder, config(false), canary()).unwrap();
    assert_eq!(std::fs::read_to_string(&outside).unwrap(), "keep me");
    assert_eq!(std::fs::metadata(&outside).unwrap().permissions().mode() & 0o777, 0o644);
    let meta = std::fs::symlink_metadata(&place).unwrap();
    assert!(meta.file_type().is_file() && meta.permissions().mode() & 0o777 == 0o600);
    session.shutdown(Duration::from_secs(2)).unwrap();
    std::fs::remove_dir_all(&base).unwrap();
}

/// A runtime writing only lines that mean nothing cannot hold `next_event` past its timeout, so
/// `stop` and `shutdown` stay reachable.
#[test]
fn lines_that_mean_nothing_cannot_hold_next_event() {
    let base = scratch("noise");
    let (runtime, folder) = fake_claude(&base, "input\nwhile :; do printf '%s\\n' '{\"type\":\"system\",\"subtype\":\"status\"}'; done");
    let mut session = ClaudeSession::start(&runtime, &folder, config(false), canary()).unwrap();
    session.send("hi").unwrap();
    let started = Instant::now();
    assert_eq!(session.next_event(Duration::from_millis(100)), None);
    assert!(started.elapsed() < Duration::from_millis(1000), "{:?}", started.elapsed());
    let started = Instant::now();
    for _ in 0..5 {
        let _ = session.next_event(Duration::ZERO);
    }
    assert!(started.elapsed() < Duration::from_millis(1000), "a zero timeout returns at once: {:?}", started.elapsed());
    session.shutdown(Duration::from_millis(200)).unwrap();
    std::fs::remove_dir_all(&base).unwrap();
}

// --- the real round trip -------------------------------------------------------------------------

/// One real turn with an installed Claude Code, a real key and a running Archforge. Ignored: it
/// costs a little and needs the owner's OK. To run it, start Archforge with `--control <port>`,
/// build `archforge-cli`, and run
/// `read -rs ARCHFORGE_LIVE_KEY && export ARCHFORGE_LIVE_KEY` (so the key stays out of the shell's
/// history), then `ARCHFORGE_LIVE_PORT=<port> ARCHFORGE_LIVE_CLI=<abs path to archforge-cli>
/// cargo test -p archforge-agents --test claude -- --ignored --nocapture`.
/// It checks the spike's open round-trip items 1, 2, 4 and 5: the event order with a real tool
/// call, whether `init` comes once per process or per turn, that the bridge does not see the key
/// (look at its environment by hand), and a resume with "Allow changes" switched on.
#[test]
#[ignore = "a real round trip: needs a real key, a running Archforge and the owner's OK"]
fn a_real_round_trip() {
    let var = |k: &str| std::env::var(k).unwrap_or_else(|_| panic!("{k} is not set"));
    let base = scratch("live");
    let ws = Workspace::open(&base.join("workspace")).unwrap();
    let runtime = find(RuntimeKind::Claude, &HostEnv::from_process(), &ws).unwrap().into_result().unwrap();
    let folder = ws.conversation(&ConversationId::new(SESSION).unwrap()).unwrap();
    let live_bridge = |allow| Bridge {
        cli: PathBuf::from(var("ARCHFORGE_LIVE_CLI")),
        port: var("ARCHFORGE_LIVE_PORT").parse().unwrap(),
        origin: "Live test".into(),
        allow_changes: allow,
        home: BridgeHome::from_process(),
    };
    let start = |start, allow| ClaudeConfig { start, bridge: live_bridge(allow), ..config(allow) };
    let key = || Secret::new(var("ARCHFORGE_LIVE_KEY")).unwrap();
    let mut session = ClaudeSession::start(&runtime, &folder, start(Start::New, false), key()).unwrap();
    for message in ["Which schemas are in the open model? Use the tools.", "And how many entities in all?"] {
        session.send(message).unwrap();
        let deadline = Instant::now() + Duration::from_secs(300);
        while Instant::now() < deadline {
            if let Some(event) = session.next_event(Duration::from_secs(1)) {
                println!("{event:?}");
                if turn_end(&event) {
                    break;
                }
            }
        }
    }
    session.shutdown(Duration::from_secs(5)).unwrap();
    let mut resumed = ClaudeSession::start(&runtime, &folder, start(Start::Resume, true), key()).unwrap();
    resumed.send("Add an entity named LiveCheck to the first schema.").unwrap();
    let deadline = Instant::now() + Duration::from_secs(300);
    while Instant::now() < deadline {
        if let Some(event) = resumed.next_event(Duration::from_secs(1)) {
            println!("{event:?}");
            if turn_end(&event) {
                break;
            }
        }
    }
    resumed.shutdown(Duration::from_secs(5)).unwrap();
    std::fs::remove_dir_all(&base).unwrap();
}
