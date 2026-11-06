//! The Codex adapter against the AR-196 transcripts, the generated 0.166.0 schema, and fake `codex`
//! programs: shell scripts that answer `--version` as codex-cli 0.166.0 does and then answer the
//! adapter's JSON-RPC lines, replaying a transcript or doing what a test needs. No test starts an
//! installed `codex`, signs in, or uses a real key, but the one ignored round trip at the end, which
//! needs the owner's OK.
//!
//! The transcripts in `tests/fixtures/codex/` come from the spike (`docs/agent-runtimes/README.md`
//! on the AR-196 branch): one JSON object a line, `{"t", "dir": in|out|err|sig|exit|note, "line"}`.
//! `observed-*` were recorded from codex-cli 0.166.0 and sanitised; `doc-*` were written from the
//! documentation and the schema and start with a `note` line saying so. `schema/` holds what the
//! contract tests read of the schema `codex app-server generate-json-schema` wrote for 0.166.0: the
//! message unions and the shapes of the definitions used (its `$comment` says how it was cut).
#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use archforge_agents::codex::{
    AccountEvent, AccountStatus, Bridge, BridgeHome, CLIENT_METHODS, CancelStatus, CodexAccounts, CodexConfig, CodexParser, CodexSession,
    DECLINED_REQUESTS, DEFAULT_REQUEST_TIMEOUT, HANDLED_NOTIFICATIONS, History, KeyringAccess, LoginStarted, QUIET_NOTIFICATIONS, SignIn, SignInKind,
    Start, command_line, history, rpc,
};
use archforge_agents::{
    AgentError, AgentSession, ConversationFolder, ConversationId, Dropped, ErrorKind, HostEnv, Limits, Runtime, RuntimeKind, SessionEvent, Skipped,
    TurnOutcome, Usage, Workspace, find,
};
use archforge_secrets::Secret;
use serde_json::{Value, json};

/// A key no output of the adapter may ever show, shaped like an OpenAI key.
const CANARY: &str = "sk-proj-CANARY-4f9a1c7e0b2d_canary-canary-canary-canary-canary-canary-canary-canary-canary-xyzQQ";
/// The conversation of the tests.
const CONVERSATION: &str = "7f3c2a10-0000-4000-8000-0000000000d1";
/// The thread the fake starts.
const THREAD: &str = "019a0000-0000-7000-8000-000000000001";

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

/// The transcript `name` under `tests/fixtures/codex/`.
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

/// Every Codex transcript.
const CODEX_FIXTURES: [&str; 17] = [
    "codex/doc-approval-declined.jsonl",
    "codex/doc-login-api-key.jsonl",
    "codex/doc-login-chatgpt-browser.jsonl",
    "codex/doc-login-chatgpt-device-code-cancelled.jsonl",
    "codex/doc-logout.jsonl",
    "codex/doc-rate-limits.jsonl",
    "codex/doc-turn-errors.jsonl",
    "codex/doc-turn-interrupt.jsonl",
    "codex/doc-turn-with-mcp-tool.jsonl",
    "codex/observed-env-api-key-not-read.jsonl",
    "codex/observed-ephemeral-store-account-read.jsonl",
    "codex/observed-handshake.jsonl",
    "codex/observed-recommended-flags-handshake.jsonl",
    "codex/observed-sigint.jsonl",
    "codex/observed-sigterm.jsonl",
    "codex/observed-strict-config-unknown-key.jsonl",
    "codex/observed-thread-start-with-mcp.jsonl",
];

/// The events the parser makes of a transcript's output, for a runtime signed in as `sign_in`; a
/// `turn/interrupt` on the input side is told to the parser as the session would.
fn parse(name: &str, sign_in: SignInKind) -> (Vec<SessionEvent>, u64) {
    let mut parser = CodexParser::new(Limits::default().max_line_bytes, sign_in);
    let mut events = Vec::new();
    for record in fixture(name) {
        match record.dir.as_str() {
            "out" => events.extend(parser.line(&record.line)),
            "in" if record.line.contains("\"turn/interrupt\"") => parser.stop_requested(),
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
    let dir = std::env::temp_dir().join(format!("archforge-codex-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir.canonicalize().unwrap()
}

/// `text` quoted for the shell.
fn quoted(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\\''"))
}

/// The fake's answers to the handshake and the thread, which each test may replace by defining the
/// function again in its body: `send` prints a line with `@ID@` replaced by the id of the request
/// being answered.
const PRELUDE: &str = r#"
send() { printf '%s\n' "$1" | sed "s/@ID@/$id/g"; }
hello() { send '{"id":@ID@,"result":{"userAgent":"archforge/0.166.0 (Mac OS 27.2.0; arm64) unknown (archforge; 0.12.0)","codexHome":"/tmp/archforge-agent/codex-home","platformFamily":"unix","platformOs":"macos"}}'; send '{"method":"remoteControl/status/changed","params":{"status":"disabled","serverName":"host.example","installationId":"00000000-0000-0000-0000-000000000000","environmentId":null},"emittedAtMs":1}'; }
on_login() { send '{"id":@ID@,"result":{"type":"apiKey"}}'; send '{"method":"account/login/completed","params":{"loginId":null,"success":true,"error":null,"onboardingEntrypoint":null}}'; send '{"method":"account/updated","params":{"authMode":"apikey","planType":null}}'; }
on_account() { send '{"id":@ID@,"result":{"account":{"type":"apiKey"},"requiresOpenaiAuth":true}}'; }
on_thread() { send '{"id":@ID@,"result":{"thread":{"id":"019a0000-0000-7000-8000-000000000001","ephemeral":true,"model":"gpt-6.1-sol","status":{"type":"idle"},"path":null},"model":"gpt-6.1-sol","modelProvider":"openai","cwd":"/tmp","approvalPolicy":"on-request","approvalsReviewer":"user","sandbox":{"type":"readOnly","networkAccess":false}}}'; send '{"method":"thread/started","params":{"thread":{"id":"019a0000-0000-7000-8000-000000000001"}}}'; send '{"method":"mcpServer/startupStatus/updated","params":{"threadId":"019a0000-0000-7000-8000-000000000001","name":"archforge","status":"starting","error":null,"failureReason":null}}'; send '{"method":"mcpServer/startupStatus/updated","params":{"threadId":"019a0000-0000-7000-8000-000000000001","name":"archforge","status":"ready","error":null,"failureReason":null}}'; }
on_inject() { send '{"id":@ID@,"result":{}}'; }
on_turn() { :; }
on_interrupt() { send '{"id":@ID@,"result":{}}'; }
on_other() { :; }
"#;

/// The fake's loop, after the test's body: every line read is recorded and answered by method.
const LOOP: &str = r#"
while IFS= read -r line; do
  printf '%s\n' "$line" >> "$IN"
  id=$(printf '%s\n' "$line" | sed -n 's/^{"id":\([0-9]*\),.*/\1/p')
  case "$line" in
    '{"id":'*',"method":"initialize",'*) hello ;;
    '{"method":"initialized"}') ;;
    '{"id":'*',"method":"account/login/start",'*) on_login ;;
    '{"id":'*',"method":"account/read",'*) on_account ;;
    '{"id":'*',"method":"thread/start",'*) on_thread ;;
    '{"id":'*',"method":"thread/inject_items",'*) on_inject ;;
    '{"id":'*',"method":"turn/start",'*) on_turn ;;
    '{"id":'*',"method":"turn/interrupt",'*) on_interrupt ;;
    *) on_other ;;
  esac
done
exit 0
"#;

/// A fake `codex` at `base/bin/codex` that answers `--version`, writes its arguments to
/// `base/args`, its environment to `base/env` and every input line to `base/stdin`, and answers
/// requests as [`PRELUDE`] and `body` say; found through the override, with a fresh workspace.
/// Answers the runtime, the account folder and the conversation's folder.
fn fake_codex(base: &Path, body: &str) -> (Runtime, ConversationFolder, ConversationFolder) {
    let program = base.join("bin/codex");
    std::fs::create_dir_all(program.parent().unwrap()).unwrap();
    let script = format!(
        "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then echo 'codex-cli 0.166.0'; exit 0; fi\n\
         printf '%s\\n' \"$@\" > {args}\n/usr/bin/env > {env}\nIN={stdin}\nid=\n{PRELUDE}\n{body}\n{LOOP}",
        args = quoted(&base.join("args").display().to_string()),
        env = quoted(&base.join("env").display().to_string()),
        stdin = quoted(&base.join("stdin").display().to_string()),
    );
    std::fs::write(&program, script).unwrap();
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
    let env = HostEnv {
        codex_override: Some(program.into()),
        path: Some(std::env::join_paths([Path::new("/usr/bin"), Path::new("/bin")]).unwrap()),
        locale: vec![("LANG".into(), "C.UTF-8".into())],
        ..HostEnv::default()
    };
    let ws = Workspace::open(&base.join("workspace")).unwrap();
    let runtime = find(RuntimeKind::Codex, &env, &ws).unwrap().into_result().unwrap();
    let account = ws.account(RuntimeKind::Codex).unwrap();
    let folder = ws.conversation(&ConversationId::new(CONVERSATION).unwrap()).unwrap();
    (runtime, account, folder)
}

/// A body that replays a transcript's requests after the handshake: each `turn/start` or
/// `turn/interrupt` the adapter sends is answered with the output lines that followed the next
/// request of the transcript, the transcript's id replaced by the adapter's.
fn replay_turns(name: &str) -> String {
    let mut segments: Vec<Vec<String>> = Vec::new();
    let mut fixture_id = String::new();
    for record in fixture(name) {
        match record.dir.as_str() {
            "in" => {
                let v: Value = serde_json::from_str(&record.line).unwrap();
                fixture_id = v["id"].to_string();
                segments.push(Vec::new());
            }
            "out" => {
                let line = record.line.replacen(&format!("{{\"id\":{fixture_id},"), "{\"id\":@ID@,", 1);
                if let Some(segment) = segments.last_mut() {
                    segment.push(line);
                }
            }
            _ => {}
        }
    }
    let mut body = String::from("n=0\nnext_segment() {\n  n=$((n+1))\n  case $n in\n");
    for (i, segment) in segments.iter().enumerate() {
        body.push_str(&format!("    {})\n", i + 1));
        for line in segment {
            body.push_str(&format!("      send {}\n", quoted(line)));
        }
        body.push_str("      ;;\n");
    }
    body.push_str("  esac\n}\non_turn() { next_segment; }\non_interrupt() { next_segment; }\n");
    body
}

/// The bridge of the tests: an `archforge-cli` that need not exist, read-only unless `allow`.
fn bridge(allow: bool) -> Bridge {
    Bridge {
        cli: PathBuf::from("/opt/archforge/bin/archforge-cli"),
        port: 47000,
        origin: "Chat \"1\"".to_owned(),
        allow_changes: allow,
        home: BridgeHome { home: Some(PathBuf::from("/Users/person")), xdg_config_home: None },
    }
}

/// A configuration for a new conversation.
fn config(allow: bool) -> CodexConfig {
    CodexConfig {
        start: Start::New,
        model: None,
        instructions: Some("You help with the open Archforge model.".to_owned()),
        bridge: bridge(allow),
        limits: Limits::default(),
        request_timeout: Duration::from_secs(5),
    }
}

/// The canary as a secret.
fn canary() -> Secret {
    Secret::new(CANARY.to_owned()).unwrap()
}

/// Events until (and with) the first that `stop` accepts, within five seconds.
fn events_until(session: &mut CodexSession, stop: impl Fn(&SessionEvent) -> bool) -> Vec<SessionEvent> {
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

/// The lines the fake read, waiting up to five seconds for `count` of them.
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

/// The request lines among `lines` whose method is `method`.
fn requests<'a>(lines: &'a [String], method: &str) -> Vec<&'a String> {
    lines.iter().filter(|l| serde_json::from_str::<Value>(l).is_ok_and(|v| v["method"] == method)).collect()
}

/// Every regular file under `dir`, with its contents.
fn files_under(dir: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for entry in std::fs::read_dir(&d).unwrap().flatten() {
            let meta = std::fs::symlink_metadata(entry.path()).unwrap();
            if meta.is_dir() {
                stack.push(entry.path());
            } else if meta.is_file() {
                out.push((entry.path(), std::fs::read(entry.path()).unwrap()));
            }
        }
    }
    out
}

// --- the parser over the transcripts -------------------------------------------------------------

/// A read-only turn: a tool call and its result, the answer in pieces and whole, then the end.
#[test]
fn a_turn_with_a_tool_call_reads_in_order() {
    let (events, skipped) = parse("codex/doc-turn-with-mcp-tool.jsonl", SignInKind::ApiKey);
    assert_eq!(skipped, 0, "{events:?}");
    let SessionEvent::ToolStarted { id, server, tool, input } = &events[0] else { panic!("{events:?}") };
    assert_eq!((id.as_str(), server.as_deref(), tool.as_str()), ("item-2", Some("archforge"), "er_entity_list"));
    assert_eq!(serde_json::from_str::<Value>(input).unwrap()["schema"], "0b6c1d2e-4f3a-4b5c-8d9e-0a1b2c3d4e5f");
    let SessionEvent::ToolFinished { id, tool, ok, summary } = &events[1] else { panic!("{events:?}") };
    assert_eq!((id.as_str(), tool.as_deref(), *ok), ("item-2", Some("er_entity_list"), true));
    assert!(summary.contains("Customer") && summary.contains("Order"), "{summary}");
    assert_eq!(events[2], SessionEvent::TextDelta { text: "The schema Shop has two entities: ".into() });
    assert_eq!(events[3], SessionEvent::TextDelta { text: "Customer and Order.".into() });
    assert_eq!(events[4], SessionEvent::MessageDone { text: "The schema Shop has two entities: Customer and Order.".into() });
    assert_eq!(events[5], SessionEvent::TurnFinished { outcome: TurnOutcome::Completed, usage: Usage::default() });
    assert_eq!(events.len(), 6);
}

/// Each failed turn lands in its category from `codexErrorInfo`, never from the message; a retried
/// error is progress. A refused sign-in is an invalid key for an API key and an expired sign-in for
/// ChatGPT.
#[test]
fn failed_turns_map_by_category() {
    let (events, skipped) = parse("codex/doc-turn-errors.jsonl", SignInKind::ApiKey);
    assert_eq!(skipped, 0);
    let kinds: Vec<Option<ErrorKind>> = outcomes(&events).into_iter().map(failed_kind).collect();
    assert_eq!(
        kinds,
        vec![
            Some(ErrorKind::Network),
            Some(ErrorKind::InvalidCredential),
            Some(ErrorKind::RateLimited),
            Some(ErrorKind::ModelUnavailable),
            Some(ErrorKind::RateLimited),
            Some(ErrorKind::ModelUnavailable),
        ]
    );
    let retries: Vec<&SessionEvent> = events.iter().filter(|e| matches!(e, SessionEvent::Retrying { .. })).collect();
    assert_eq!(retries, vec![&SessionEvent::Retrying { attempt: 1, max_attempts: None, delay_ms: None, kind: ErrorKind::Network }]);
    let Some(TurnOutcome::Failed(first)) = outcomes(&events).first().copied() else { panic!() };
    assert!(first.to_string().contains("httpConnectionFailed") && first.to_string().contains("error sending request"), "{first}");

    let (events, _) = parse("codex/doc-turn-errors.jsonl", SignInKind::ChatGpt);
    assert_eq!(failed_kind(outcomes(&events)[1]), Some(ErrorKind::SignInExpired));

    // Other: a category that names no neutral one, and a failure without any.
    let mut parser = CodexParser::new(1 << 20, SignInKind::ApiKey);
    let failed = |info: &str| {
        format!(
            r#"{{"method":"turn/completed","params":{{"threadId":"t","turn":{{"id":"u","items":[],"status":"failed","error":{{"message":"m","codexErrorInfo":{info}}}}}}}}}"#
        )
    };
    for line in [failed("\"badRequest\""), failed("\"contextWindowExceeded\""), failed("null")] {
        assert!(
            matches!(&parser.line(&line)[..], [SessionEvent::TurnFinished { outcome: TurnOutcome::Failed(e), .. }] if e.kind() == ErrorKind::Other),
            "{line}"
        );
    }
    let bare = r#"{"method":"turn/completed","params":{"turn":{"id":"u","status":"failed"}}}"#;
    assert!(matches!(&parser.line(bare)[..], [SessionEvent::TurnFinished { outcome: TurnOutcome::Failed(e), .. }] if e.kind() == ErrorKind::Other));
}

/// The interrupted turn ends as stopped, with the text written so far shown.
#[test]
fn an_interrupted_turn_ends_as_stopped() {
    let (events, skipped) = parse("codex/doc-turn-interrupt.jsonl", SignInKind::ApiKey);
    assert_eq!(skipped, 0);
    assert_eq!(events[0], SessionEvent::TextDelta { text: "Customer holds ".into() });
    assert_eq!(outcomes(&events), vec![&TurnOutcome::Stopped]);
}

/// Every transcript parses without a skipped line: every output line is an answer, a request or a
/// notification the adapter knows, and the handshake, sign-in and thread transcripts mean no turn
/// event at all.
#[test]
fn every_transcript_parses() {
    for name in CODEX_FIXTURES {
        let (events, skipped) = parse(name, SignInKind::ApiKey);
        assert_eq!(skipped, 0, "{name}: {events:?}");
        for record in fixture(name).iter().filter(|r| r.dir == "out") {
            assert!(!matches!(rpc::read_line(&record.line, 1 << 23), rpc::Incoming::Skipped(_)), "{name}: {}", record.line);
        }
        if !name.contains("turn") {
            assert!(events.is_empty(), "{name}: {events:?}");
        }
        assert_eq!(
            outcomes(&events).len(),
            fixture(name).iter().filter(|r| r.dir == "out" && r.line.contains("\"method\":\"turn/completed\"")).count(),
            "{name}"
        );
    }
}

/// Hostile lines are skipped and counted, or mean nothing; odd shapes inside known messages never
/// panic.
#[test]
fn hostile_lines_are_typed_or_skipped_and_counted() {
    let lines: Vec<String> = fixture("synthetic-hostile-lines.jsonl").into_iter().filter(|r| r.dir == "out").map(|r| r.line).collect();
    let mut parser = CodexParser::new(Limits::default().max_line_bytes, SignInKind::ApiKey);
    let events: Vec<Vec<SessionEvent>> = lines.iter().map(|l| parser.line(l)).collect();
    assert_eq!(events[0], vec![SessionEvent::Ignored(Skipped::Empty)]);
    assert_eq!(events[1], vec![SessionEvent::Ignored(Skipped::NotJson { bytes: 15 })]);
    assert_eq!(events[2], vec![SessionEvent::Ignored(Skipped::NotAnObject)]);
    assert_eq!(events[3], vec![SessionEvent::Ignored(Skipped::NotAnObject)]);
    for i in [4, 5, 6, 8, 10] {
        assert_eq!(events[i], vec![SessionEvent::Ignored(Skipped::Untyped)], "line {i}: neither a request, an answer nor a notification");
    }
    assert!(matches!(&events[7][..], [SessionEvent::Ignored(Skipped::NotJson { .. })]), "deep nesting is refused, not recursed into");
    assert_eq!(events[9], Vec::new(), "an answer means no event to the parser alone");
    assert_eq!(parser.skipped(), 10);

    let mut small = CodexParser::new(1024, SignInKind::ApiKey);
    let big = lines[8].replace("AAAA", &"A".repeat(400));
    assert_eq!(small.line(&big), vec![SessionEvent::Ignored(Skipped::TooLong { bytes: big.len() as u64 })]);
    assert_eq!(small.dropped(Dropped::NotText { bytes: 2 }), SessionEvent::Ignored(Skipped::NotText { bytes: 2 }));
    assert_eq!(small.skipped(), 2);

    let mut parser = CodexParser::new(1 << 20, SignInKind::ChatGpt);
    assert_eq!(parser.line(r#"{"method":"brand/new","params":{}}"#), vec![SessionEvent::Ignored(Skipped::Unknown { kind: "brand/new".into() })]);
    for line in [
        r#"{"method":"turn/completed"}"#,
        r#"{"method":"turn/completed","params":{"turn":{"status":7}}}"#,
        r#"{"method":"turn/completed","params":{"turn":{"id":"u","status":"inProgress"}}}"#,
        r#"{"method":"item/started","params":{"item":{"type":"mcpToolCall"}}}"#,
        r#"{"method":"item/started","params":{"item":7}}"#,
        r#"{"method":"item/completed","params":{"item":{"type":"mcpToolCall","status":"failed","error":{"message":5},"result":{"content":[7,{"type":"image"}]}}}}"#,
        r#"{"method":"item/completed","params":{"item":{"type":"agentMessage","text":null}}}"#,
        r#"{"method":"item/agentMessage/delta","params":{"delta":5}}"#,
        r#"{"method":"error","params":{"error":{"codexErrorInfo":{"x":{"httpStatusCode":"401"}}},"willRetry":"yes"}}"#,
        r#"{"method":"thread/tokenUsage/updated","params":{"tokenUsage":{"last":{"inputTokens":-5,"outputTokens":1e40}}}}"#,
        r#"{"method":"mcpServer/startupStatus/updated","params":{"name":7}}"#,
        r#"{"method":"account/updated","params":null}"#,
        r#"{"id":"x","method":"item/tool/call","params":{}}"#,
        "{\"method\":\"\\u202e\\u0000\"}",
    ] {
        let _ = parser.line(line);
    }
    // Token counts of every model response in a turn are summed, a missing one counted as zero.
    let mut parser = CodexParser::new(1 << 20, SignInKind::ApiKey);
    let usage = |input: u64, cached: u64| {
        format!(
            r#"{{"method":"thread/tokenUsage/updated","params":{{"threadId":"t","turnId":"u","tokenUsage":{{"total":{{}},"last":{{"inputTokens":{input},"cachedInputTokens":{cached},"outputTokens":10,"reasoningOutputTokens":0,"totalTokens":0}}}}}}}}"#
        )
    };
    parser.line(&usage(100, 40));
    parser.line(&usage(50, 0));
    let done = parser.line(r#"{"method":"turn/completed","params":{"turn":{"id":"u","status":"completed"}}}"#);
    assert_eq!(
        done,
        vec![SessionEvent::TurnFinished {
            outcome: TurnOutcome::Completed,
            usage: Usage { input_tokens: 150, output_tokens: 20, cache_read_input_tokens: 40, cache_creation_input_tokens: 0 }
        }]
    );
}

// --- the command line ----------------------------------------------------------------------------

/// The command line is the spike's, under `--strict-config`, with the state in the conversation's
/// folder, the bridge read-only unless changes are allowed, the in-memory store for a key and the
/// keyring for ChatGPT; a label with quotes cannot break out of its TOML string.
#[test]
fn the_command_line_is_read_only_unless_allowed() {
    let state = archforge_agents::codex::StateFolders { sqlite_home: "/w/c/config/codex-state".into(), log_dir: "/w/c/config/codex-log".into() };
    let text = |args: Vec<std::ffi::OsString>| args.into_iter().map(|a| a.into_string().unwrap()).collect::<Vec<_>>();
    let read_only = text(command_line(SignInKind::ApiKey, Some(&state), Some(&bridge(false))).unwrap());
    assert_eq!(&read_only[..2], ["app-server", "--strict-config"]);
    let overrides: Vec<&str> = read_only.windows(2).filter(|w| w[0] == "-c").map(|w| w[1].as_str()).collect();
    let disabled: Vec<&str> = read_only.windows(2).filter(|w| w[0] == "--disable").map(|w| w[1].as_str()).collect();
    assert_eq!(
        overrides,
        vec![
            "cli_auth_credentials_store=\"ephemeral\"",
            "forced_login_method=\"api\"",
            "history.persistence=\"none\"",
            "analytics.enabled=false",
            "check_for_update_on_startup=false",
            "web_search=\"disabled\"",
            "approval_policy=\"on-request\"",
            "sandbox_mode=\"read-only\"",
            "sqlite_home=\"/w/c/config/codex-state\"",
            "log_dir=\"/w/c/config/codex-log\"",
            "mcp_servers.archforge.command=\"/opt/archforge/bin/archforge-cli\"",
            "mcp_servers.archforge.args=[\"mcp\",\"--bridge\",\"47000\",\"--read-only\",\"--origin\",\"Chat \\\"1\\\"\"]",
            "mcp_servers.archforge.env={HOME=\"/Users/person\"}",
            "mcp_servers.archforge.startup_timeout_sec=10",
            "mcp_servers.archforge.tool_timeout_sec=60",
            "mcp_servers.archforge.default_tools_approval_mode=\"approve\"",
        ]
    );
    assert_eq!(disabled.len(), 16);
    for feature in ["shell_tool", "unified_exec", "plugins", "memories", "shell_snapshot", "apps", "multi_agent"] {
        assert!(disabled.contains(&feature), "{feature}");
    }
    assert_eq!(read_only.len(), 2 + 2 * overrides.len() + 2 * disabled.len(), "nothing else is on the line");

    let allowed = text(command_line(SignInKind::ChatGpt, Some(&state), Some(&bridge(true))).unwrap());
    assert!(
        allowed.contains(&"cli_auth_credentials_store=\"keyring\"".to_owned()) && allowed.contains(&"forced_login_method=\"chatgpt\"".to_owned())
    );
    assert!(
        allowed.contains(&"mcp_servers.archforge.args=[\"mcp\",\"--bridge\",\"47000\",\"--origin\",\"Chat \\\"1\\\"\"]".to_owned()),
        "{allowed:?}"
    );
    assert!(!allowed.iter().any(|a| a.contains("--read-only")));

    let account = text(command_line(SignInKind::ChatGpt, None, None).unwrap());
    assert!(!account.iter().any(|a| a.contains("mcp_servers") || a.contains("sqlite_home")), "the account process has no tool server");

    for bad in [
        Bridge { cli: "archforge-cli".into(), ..bridge(false) },
        Bridge { port: 0, ..bridge(false) },
        Bridge { origin: "--read-only".into(), ..bridge(false) },
        Bridge { origin: "Chat\n1".into(), ..bridge(false) },
        Bridge { home: BridgeHome::default(), ..bridge(false) },
    ] {
        assert!(matches!(command_line(SignInKind::ApiKey, Some(&state), Some(&bad)), Err(AgentError::InvalidSetting { .. })), "{bad:?}");
    }
    let relative = archforge_agents::codex::StateFolders { sqlite_home: "state".into(), log_dir: "/x".into() };
    assert!(matches!(command_line(SignInKind::ApiKey, Some(&relative), None), Err(AgentError::InvalidSetting { .. })));
}

// --- the contract with the 0.166.0 schema --------------------------------------------------------

/// The generated schema for codex-cli 0.166.0.
fn schema() -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/codex/schema/codex-app-server-0.166.0.contract.json");
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

/// The methods of a `oneOf` of messages in the schema, with each one's params definition.
fn methods(schema: &Value, union: &str) -> Vec<(String, Option<String>)> {
    schema["definitions"][union]["oneOf"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| {
            let method = v["properties"]["method"]["enum"][0].as_str().unwrap().to_owned();
            let params = v["properties"]["params"]["$ref"].as_str().map(|r| r.rsplit('/').next().unwrap().to_owned());
            (method, params)
        })
        .collect()
}

/// A definition by name, from the v1 or v2 part of the schema.
fn definition<'a>(schema: &'a Value, name: &str) -> &'a Value {
    let v2 = &schema["definitions"]["v2"];
    if !v2[name].is_null() { &v2[name] } else { &schema["definitions"][name] }
}

/// Checks `params` against an object definition: every required field present, no field the
/// definition does not list.
fn conforms(schema: &Value, definition_name: &str, params: &Value) -> Result<(), String> {
    let def = definition(schema, definition_name);
    let variants: Vec<&Value> = match def["oneOf"].as_array() {
        Some(list) => list.iter().collect(),
        None => vec![def],
    };
    let object = params.as_object().ok_or("params are not an object")?;
    for variant in variants {
        let properties = variant["properties"].as_object();
        let required: Vec<&str> = variant["required"].as_array().map(|r| r.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
        let known = properties.is_some_and(|p| object.keys().all(|k| p.contains_key(k)));
        let complete = required.iter().all(|r| object.contains_key(*r));
        let type_ok = match variant["properties"]["type"]["enum"].as_array() {
            Some(types) => types.iter().any(|t| Some(t) == object.get("type")),
            None => true,
        };
        if known && complete && type_ok {
            return Ok(());
        }
    }
    Err(format!("{params} does not fit {definition_name}"))
}

/// Every method the adapter sends is a client request of 0.166.0, every notification it reads or
/// drops is a server notification, and every request it declines is a server request: a renamed
/// method fails here, not at run time.
#[test]
fn every_method_is_in_the_schema() {
    let schema = schema();
    let client: Vec<String> = methods(&schema, "ClientRequest").into_iter().map(|(m, _)| m).collect();
    for method in CLIENT_METHODS {
        assert!(client.iter().any(|m| m == method), "{method} is not a client request");
    }
    assert!(methods(&schema, "ClientNotification").iter().any(|(m, _)| m == "initialized"));
    let notifications: Vec<String> = methods(&schema, "ServerNotification").into_iter().map(|(m, _)| m).collect();
    for method in HANDLED_NOTIFICATIONS.iter().chain(QUIET_NOTIFICATIONS.iter()) {
        assert!(notifications.iter().any(|m| m == method), "{method} is not a server notification");
    }
    let server: Vec<String> = methods(&schema, "ServerRequest").into_iter().map(|(m, _)| m).collect();
    for method in DECLINED_REQUESTS {
        assert!(server.iter().any(|m| m == method), "{method} is not a server request");
    }
}

/// Every line the adapter wrote in the end-to-end runs fits the schema's params of its method, and
/// every decline fits the response of the request it answers.
#[test]
fn every_line_sent_fits_the_schema() {
    let schema = schema();
    let client = methods(&schema, "ClientRequest");
    let mut lines = Vec::new();
    // A conversation: handshake with a key, thread, earlier messages, turn, interrupt, declines.
    let base = scratch("contract");
    let body = format!(
        "{}\non_turn() {{ next_segment; send '{{\"id\":90,\"method\":\"item/fileChange/requestApproval\",\"params\":{{}}}}'; send '{{\"id\":91,\"method\":\"mcpServer/elicitation/request\",\"params\":{{}}}}'; send '{{\"id\":92,\"method\":\"execCommandApproval\",\"params\":{{}}}}'; send '{{\"id\":93,\"method\":\"item/permissions/requestApproval\",\"params\":{{}}}}'; send '{{\"id\":94,\"method\":\"item/tool/requestUserInput\",\"params\":{{}}}}'; send '{{\"id\":95,\"method\":\"item/commandExecution/requestApproval\",\"params\":{{}}}}'; send '{{\"id\":96,\"method\":\"applyPatchApproval\",\"params\":{{}}}}'; }}",
        replay_turns("codex/doc-turn-interrupt.jsonl")
    );
    let (runtime, account, folder) = fake_codex(&base, &body);
    let mut earlier = History::default();
    earlier.push(history::Role::User, "Before".into());
    earlier.push(history::Role::Assistant, "Answer".into());
    earlier.write(&folder).unwrap();
    let cfg = CodexConfig { start: Start::Resume, model: Some(archforge_agents::codex::Model::new("gpt-6.1-sol").unwrap()), ..config(false) };
    let mut session = CodexSession::start(&runtime, &account, &folder, cfg, SignIn::ApiKey(canary())).unwrap();
    session.send("Describe every class in detail.").unwrap();
    events_until(&mut session, |e| matches!(e, SessionEvent::TextDelta { .. }));
    session.stop().unwrap();
    events_until(&mut session, turn_end);
    lines.extend(stdin_lines(&base, 15));
    session.shutdown(Duration::from_secs(2)).unwrap();
    std::fs::remove_dir_all(&base).unwrap();

    let mut checked = std::collections::BTreeSet::new();
    let mut declines = 0;
    for line in &lines {
        let v: Value = serde_json::from_str(line).unwrap();
        assert!(v.get("jsonrpc").is_none(), "{line}");
        if let Some(method) = v["method"].as_str() {
            if v.get("id").is_none() {
                assert_eq!(method, "initialized");
                continue;
            }
            let (_, params) = client.iter().find(|(m, _)| m == method).unwrap_or_else(|| panic!("{method} is not a client request"));
            let params_name = params.as_ref().unwrap();
            let sent = v.get("params").cloned().unwrap_or(json!({}));
            let sent = if sent["apiKey"].is_string() { json!({ "type": "apiKey", "apiKey": "x" }) } else { sent };
            conforms(&schema, params_name, &sent).unwrap_or_else(|e| panic!("{method}: {e}"));
            checked.insert(method.to_owned());
        } else if let Some(result) = v.get("result") {
            let id = v["id"].as_u64().unwrap();
            let response = match id {
                90 => "FileChangeRequestApprovalResponse",
                91 => "McpServerElicitationRequestResponse",
                92 => "ExecCommandApprovalResponse",
                93 => "PermissionsRequestApprovalResponse",
                94 => "ToolRequestUserInputResponse",
                95 => "CommandExecutionRequestApprovalResponse",
                96 => "ApplyPatchApprovalResponse",
                other => panic!("an answer to {other}"),
            };
            conforms(&schema, response, result).unwrap_or_else(|e| panic!("{response}: {e}"));
            declines += 1;
        }
    }
    assert_eq!(declines, 7, "every approval was answered with a decline");
    for method in ["initialize", "account/login/start", "account/read", "thread/start", "thread/inject_items", "turn/start", "turn/interrupt"] {
        assert!(checked.contains(method), "{method} was not sent: {lines:?}");
    }
}

// --- sessions against fake runtimes --------------------------------------------------------------

/// A replayed turn end to end through `spawn`: the handshake, the key over standard input, a thread
/// started only for the message, the turn, and the events back in order; Codex's settings folder is
/// the shared account folder and its state the conversation's.
#[test]
fn a_replayed_turn_runs_end_to_end() {
    let base = scratch("replay");
    let (runtime, account, folder) = fake_codex(&base, &replay_turns("codex/doc-turn-with-mcp-tool.jsonl"));
    let mut session = CodexSession::start(&runtime, &account, &folder, config(false), SignIn::ApiKey(canary())).unwrap();
    // The handshake alone starts no thread.
    assert_eq!(session.next_event(Duration::from_millis(300)), None);
    let before = stdin_lines(&base, 4);
    assert!(requests(&before, "thread/start").is_empty(), "no thread before a message: {before:?}");
    session.send("Which entities are in Shop?\nAnd \"why\"?").unwrap();
    assert!(matches!(session.send("again"), Err(AgentError::TurnRunning { .. })), "one turn at a time");
    let events = events_until(&mut session, turn_end);
    let started: Vec<&SessionEvent> = events.iter().filter(|e| matches!(e, SessionEvent::Started { .. })).collect();
    assert!(matches!(started[0], SessionEvent::Started { session_id, model, .. } if session_id == THREAD && model == "gpt-6.1-sol"));
    assert!(
        matches!(started.last(), Some(SessionEvent::Started { servers, .. }) if servers.len() == 1 && servers[0].name == "archforge" && servers[0].status == "ready")
    );
    assert!(events.iter().any(|e| matches!(e, SessionEvent::ToolStarted { tool, .. } if tool == "er_entity_list")));
    assert!(events.iter().any(|e| matches!(e, SessionEvent::ToolFinished { ok: true, .. })));
    assert!(events.iter().any(|e| matches!(e, SessionEvent::MessageDone { text } if text.ends_with("Customer and Order."))));
    assert!(matches!(events.last(), Some(SessionEvent::TurnFinished { outcome: TurnOutcome::Completed, .. })));
    assert!(!session.turn_running());
    assert_eq!(session.thread_id(), Some(THREAD));

    let sent = stdin_lines(&base, 6);
    let start: Value = serde_json::from_str(requests(&sent, "thread/start")[0]).unwrap();
    assert_eq!(start["params"]["ephemeral"], true);
    assert_eq!(start["params"]["sandbox"], "read-only");
    assert_eq!(start["params"]["approvalPolicy"], "on-request");
    assert_eq!(start["params"]["cwd"], folder.work().to_str().unwrap());
    let turn: Value = serde_json::from_str(requests(&sent, "turn/start")[0]).unwrap();
    assert_eq!(turn["params"]["threadId"], THREAD);
    assert_eq!(turn["params"]["input"][0]["text"], "Which entities are in Shop?\nAnd \"why\"?");

    let env = std::fs::read_to_string(base.join("env")).unwrap();
    assert!(env.lines().any(|l| l == format!("CODEX_HOME={}", account.config().display())), "{env}");
    assert!(env.lines().any(|l| l == format!("HOME={}", folder.home().display())));
    for name in ["codex-state", "codex-log"] {
        let meta = std::fs::metadata(folder.config().join(name)).unwrap();
        assert!(meta.is_dir() && meta.permissions().mode() & 0o777 == 0o700, "{name}");
    }
    // Archforge's copy of the conversation is in the conversation's folder.
    let copy = History::read(&folder).unwrap();
    assert_eq!(copy.entries().len(), 2);
    assert_eq!(copy.entries()[1].text, "The schema Shop has two entities: Customer and Order.");
    assert_eq!(std::fs::metadata(history::path(&folder)).unwrap().permissions().mode() & 0o777, 0o600);

    let exit = session.shutdown(Duration::from_secs(2)).unwrap();
    assert_eq!(exit.code, Some(0));
    assert_eq!(session.skipped(), 0);
    std::fs::remove_dir_all(&base).unwrap();
}

/// A resumed conversation starts a new ephemeral thread and gives it Archforge's copy of the earlier
/// messages before the turn, the waiting message not among them.
#[test]
fn a_resume_gives_the_thread_the_earlier_messages() {
    let base = scratch("resume");
    let (runtime, account, folder) = fake_codex(&base, &replay_turns("codex/doc-turn-with-mcp-tool.jsonl"));
    let mut session = CodexSession::start(&runtime, &account, &folder, config(false), SignIn::ApiKey(canary())).unwrap();
    session.send("First question").unwrap();
    events_until(&mut session, turn_end);
    session.shutdown(Duration::from_secs(2)).unwrap();
    std::fs::remove_file(base.join("stdin")).unwrap();

    let cfg = CodexConfig { start: Start::Resume, ..config(true) };
    let mut resumed = CodexSession::start(&runtime, &account, &folder, cfg, SignIn::ApiKey(canary())).unwrap();
    resumed.send("Second question").unwrap();
    events_until(&mut resumed, turn_end);
    let sent = stdin_lines(&base, 7);
    let methods: Vec<String> = sent.iter().filter_map(|l| serde_json::from_str::<Value>(l).ok()?["method"].as_str().map(str::to_owned)).collect();
    let at = |m: &str| methods.iter().position(|x| x == m).unwrap_or_else(|| panic!("{m} not in {methods:?}"));
    assert!(at("thread/start") < at("thread/inject_items") && at("thread/inject_items") < at("turn/start"), "{methods:?}");
    let inject: Value = serde_json::from_str(requests(&sent, "thread/inject_items")[0]).unwrap();
    assert_eq!(inject["params"]["threadId"], THREAD);
    let items = inject["params"]["items"].as_array().unwrap();
    assert_eq!(items.len(), 2, "{items:?}");
    assert_eq!(items[0], json!({"type": "message", "role": "user", "content": [{"type": "input_text", "text": "First question"}]}));
    assert_eq!(items[1]["role"], "assistant");
    assert_eq!(items[1]["content"][0]["type"], "output_text");
    assert_eq!(History::read(&folder).unwrap().entries().len(), 4);
    resumed.shutdown(Duration::from_secs(2)).unwrap();

    // Without the folder's copy (another computer), the caller primes the thread from the
    // project's record, and the folder's copy starts from it.
    std::fs::remove_file(history::path(&folder)).unwrap();
    std::fs::remove_file(base.join("stdin")).unwrap();
    let mut record = History::default();
    record.push(history::Role::User, "From the project".into());
    record.push(history::Role::Assistant, "Its answer".into());
    let cfg = CodexConfig { start: Start::ResumeFrom(record), ..config(false) };
    let mut primed = CodexSession::start(&runtime, &account, &folder, cfg, SignIn::ApiKey(canary())).unwrap();
    primed.send("Third question").unwrap();
    events_until(&mut primed, turn_end);
    let sent = stdin_lines(&base, 7);
    let inject: Value = serde_json::from_str(requests(&sent, "thread/inject_items")[0]).unwrap();
    assert_eq!(inject["params"]["items"][0]["content"][0]["text"], "From the project");
    assert_eq!(inject["params"]["items"].as_array().unwrap().len(), 2);
    assert_eq!(History::read(&folder).unwrap().entries()[0].text, "From the project");
    primed.shutdown(Duration::from_secs(2)).unwrap();
    std::fs::remove_dir_all(&base).unwrap();
}

/// Stop sends `turn/interrupt` with the thread and the turn Codex named; the turn ends as stopped
/// and Codex stays up for the next one.
#[test]
fn stop_interrupts_the_turn() {
    let base = scratch("stop");
    let (runtime, account, folder) = fake_codex(&base, &replay_turns("codex/doc-turn-interrupt.jsonl"));
    let mut session = CodexSession::start(&runtime, &account, &folder, config(false), SignIn::ApiKey(canary())).unwrap();
    session.stop().unwrap();
    session.send("Describe every class in detail.").unwrap();
    events_until(&mut session, |e| matches!(e, SessionEvent::TextDelta { .. }));
    session.stop().unwrap();
    session.stop().unwrap();
    let after = events_until(&mut session, turn_end);
    assert!(matches!(after.last(), Some(SessionEvent::TurnFinished { outcome: TurnOutcome::Stopped, .. })), "{after:?}");
    let sent = stdin_lines(&base, 7);
    let interrupts = requests(&sent, "turn/interrupt");
    assert_eq!(interrupts.len(), 1, "a second stop sends nothing more: {sent:?}");
    let interrupt: Value = serde_json::from_str(interrupts[0]).unwrap();
    assert_eq!(interrupt["params"], json!({ "threadId": THREAD, "turnId": "019a0000-0000-7000-8000-0000000000a1" }));
    assert!(!session.turn_running() && session.process().pid() > 0);
    assert_eq!(session.shutdown(Duration::from_secs(2)).unwrap().code, Some(0));
    std::fs::remove_dir_all(&base).unwrap();
}

/// A message stopped before its thread exists ends as stopped without a turn being started.
#[test]
fn a_stop_before_the_thread_starts_no_turn() {
    let base = scratch("early-stop");
    let (runtime, account, folder) =
        fake_codex(&base, "on_thread() { sleep 0.3; send '{\"id\":@ID@,\"result\":{\"thread\":{\"id\":\"t-1\"},\"model\":\"m\"}}'; }");
    let mut session = CodexSession::start(&runtime, &account, &folder, config(false), SignIn::ApiKey(canary())).unwrap();
    session.send("hi").unwrap();
    session.stop().unwrap();
    let events = events_until(&mut session, turn_end);
    assert!(matches!(events.last(), Some(SessionEvent::TurnFinished { outcome: TurnOutcome::Stopped, .. })), "{events:?}");
    assert!(requests(&stdin_lines(&base, 5), "turn/start").is_empty());
    session.shutdown(Duration::from_secs(2)).unwrap();
    std::fs::remove_dir_all(&base).unwrap();
}

/// Approvals and questions from Codex are declined in the shape each takes, an unknown request gets
/// an error answer, and the turn goes on.
#[test]
fn requests_from_codex_are_declined() {
    capture_logs();
    let base = scratch("decline");
    let mut body: String = fixture("codex/doc-approval-declined.jsonl")
        .iter()
        .filter(|r| r.dir == "out" && r.line.contains("\"id\""))
        .map(|r| format!("send {}; ", quoted(&r.line)))
        .collect();
    body = format!(
        "on_turn() {{ send '{{\"id\":@ID@,\"result\":{{\"turn\":{{\"id\":\"u-1\",\"status\":\"inProgress\",\"items\":[]}}}}}}'; {body} send '{{\"id\":\"weird\",\"method\":\"item/tool/call\",\"params\":{{}}}}'; }}\n\
         on_other() {{ case \"$line\" in *'\"id\":\"weird\"'*) send '{{\"method\":\"turn/completed\",\"params\":{{\"turn\":{{\"id\":\"u-1\",\"status\":\"completed\",\"items\":[]}}}}}}' ;; esac; }}"
    );
    let (runtime, account, folder) = fake_codex(&base, &body);
    let mut session = CodexSession::start(&runtime, &account, &folder, config(false), SignIn::ApiKey(canary())).unwrap();
    session.send("go").unwrap();
    let events = events_until(&mut session, turn_end);
    assert!(matches!(events.last(), Some(SessionEvent::TurnFinished { outcome: TurnOutcome::Completed, .. })), "{events:?}");
    let sent = stdin_lines(&base, 9);
    let answers: Vec<Value> = sent.iter().filter_map(|l| serde_json::from_str::<Value>(l).ok()).filter(|v| v.get("method").is_none()).collect();
    assert_eq!(answers[0], json!({"id": 0, "result": {"decision": "decline"}}));
    assert_eq!(answers[1], json!({"id": 1, "result": {"action": "decline", "content": null, "_meta": null}}));
    assert_eq!(answers[2]["id"], "weird");
    assert_eq!(answers[2]["error"]["code"], -32601);
    let logs = LOG.lock().unwrap_or_else(PoisonError::into_inner).join("\n");
    assert!(logs.contains("codex asked for item/commandExecution/requestApproval; Archforge declined"), "{logs}");
    session.shutdown(Duration::from_secs(2)).unwrap();
    std::fs::remove_dir_all(&base).unwrap();
}

/// A request Codex never answers fails on its own deadline: the turn fails, the session stays up.
#[test]
fn a_request_without_an_answer_times_out() {
    let base = scratch("timeout");
    let (runtime, account, folder) = fake_codex(&base, "on_thread() { :; }");
    let cfg = CodexConfig { request_timeout: Duration::from_millis(300), ..config(false) };
    let mut session = CodexSession::start(&runtime, &account, &folder, cfg, SignIn::ApiKey(canary())).unwrap();
    session.send("hi").unwrap();
    let started = Instant::now();
    let events = events_until(&mut session, turn_end);
    let Some(SessionEvent::TurnFinished { outcome: TurnOutcome::Failed(error), .. }) = events.last() else { panic!("{events:?}") };
    assert_eq!(error.kind(), ErrorKind::Other);
    assert!(error.to_string().contains("did not answer thread/start within 300 ms"), "{error}");
    assert!(started.elapsed() < Duration::from_secs(3));
    assert!(session.send("again").is_ok(), "the session takes the next message");
    session.shutdown(Duration::from_secs(2)).unwrap();
    std::fs::remove_dir_all(&base).unwrap();
}

/// Without a sign-in the turn fails as an invalid credential before any thread is started, so
/// nothing reaches the network; a refused key does the same.
#[test]
fn a_missing_sign_in_fails_the_turn_before_any_thread() {
    let base = scratch("signed-out");
    let (runtime, account, folder) =
        fake_codex(&base, "on_account() { send '{\"id\":@ID@,\"result\":{\"account\":null,\"requiresOpenaiAuth\":true}}'; }");
    let mut session = CodexSession::start(&runtime, &account, &folder, config(false), SignIn::ChatGpt(KeyringAccess::default())).unwrap();
    session.send("hi").unwrap();
    let events = events_until(&mut session, turn_end);
    assert!(
        matches!(events.last(), Some(SessionEvent::TurnFinished { outcome: TurnOutcome::Failed(e), .. }) if e.kind() == ErrorKind::InvalidCredential && e.to_string().contains("not signed in with ChatGPT")),
        "{events:?}"
    );
    let sent = stdin_lines(&base, 3);
    assert!(requests(&sent, "thread/start").is_empty() && requests(&sent, "account/login/start").is_empty(), "{sent:?}");
    let args = std::fs::read_to_string(base.join("args")).unwrap();
    assert!(args.contains("cli_auth_credentials_store=\"keyring\""));
    session.shutdown(Duration::from_secs(2)).unwrap();
    std::fs::remove_dir_all(&base).unwrap();

    let base = scratch("refused-key");
    let (runtime, account, folder) = fake_codex(
        &base,
        "on_login() { send '{\"id\":@ID@,\"error\":{\"code\":-32600,\"message\":\"invalid api key\"}}'; }\non_account() { send '{\"id\":@ID@,\"result\":{\"account\":null,\"requiresOpenaiAuth\":true}}'; }",
    );
    let mut session = CodexSession::start(&runtime, &account, &folder, config(false), SignIn::ApiKey(canary())).unwrap();
    session.send("hi").unwrap();
    let events = events_until(&mut session, turn_end);
    assert!(
        matches!(events.last(), Some(SessionEvent::TurnFinished { outcome: TurnOutcome::Failed(e), .. }) if e.kind() == ErrorKind::InvalidCredential && e.to_string().contains("invalid api key")),
        "{events:?}"
    );
    session.shutdown(Duration::from_secs(2)).unwrap();
    std::fs::remove_dir_all(&base).unwrap();
}

/// A Codex that answers the handshake as a version past the range, or does not know a method or a
/// setting Archforge needs, is a missing runtime.
#[test]
fn a_codex_out_of_range_is_a_missing_runtime() {
    let base = scratch("too-new");
    let (runtime, account, folder) =
        fake_codex(&base, "hello() { send '{\"id\":@ID@,\"result\":{\"userAgent\":\"archforge/1.2.0 (x)\",\"codexHome\":\"/x\"}}'; }");
    let mut session = CodexSession::start(&runtime, &account, &folder, config(false), SignIn::ApiKey(canary())).unwrap();
    let events = events_until(&mut session, |e| matches!(e, SessionEvent::Ended { .. }));
    assert!(matches!(events.last(), Some(SessionEvent::Ended { error: Some(e), .. }) if e.kind() == ErrorKind::MissingRuntime), "{events:?}");
    assert!(matches!(session.send("hi"), Err(AgentError::InputClosed { .. })));
    std::fs::remove_dir_all(&base).unwrap();

    let base = scratch("old-method");
    let (runtime, account, folder) = fake_codex(
        &base,
        "on_thread() { send '{\"id\":@ID@,\"error\":{\"code\":-32600,\"message\":\"Invalid request: unknown variant `thread/start`, expected one of `initialize`\"}}'; }",
    );
    let mut session = CodexSession::start(&runtime, &account, &folder, config(false), SignIn::ApiKey(canary())).unwrap();
    session.send("hi").unwrap();
    let events = events_until(&mut session, turn_end);
    assert!(
        matches!(events.last(), Some(SessionEvent::TurnFinished { outcome: TurnOutcome::Failed(e), .. }) if e.kind() == ErrorKind::MissingRuntime),
        "{events:?}"
    );
    session.shutdown(Duration::from_secs(2)).unwrap();
    std::fs::remove_dir_all(&base).unwrap();

    let base = scratch("old-setting");
    let (runtime, account, folder) = fake_codex(&base, "echo 'Error: unknown configuration field `sqlite_home` in -c/--config override' >&2\nexit 1");
    let mut session = CodexSession::start(&runtime, &account, &folder, config(false), SignIn::ApiKey(canary())).unwrap();
    let events = events_until(&mut session, |e| matches!(e, SessionEvent::Ended { .. }));
    assert!(matches!(events.last(), Some(SessionEvent::Ended { error: Some(e), .. }) if e.kind() == ErrorKind::MissingRuntime), "{events:?}");
    std::fs::remove_dir_all(&base).unwrap();
}

/// A Codex that ends during a turn fails the turn and ends the session with the reason.
#[test]
fn an_end_during_a_turn_fails_it() {
    let base = scratch("crash");
    let (runtime, account, folder) = fake_codex(&base, "on_turn() { echo 'something broke' >&2; exit 1; }");
    let mut session = CodexSession::start(&runtime, &account, &folder, config(false), SignIn::ApiKey(canary())).unwrap();
    session.send("hi").unwrap();
    let events = events_until(&mut session, |e| matches!(e, SessionEvent::Ended { .. }));
    let failed = events.iter().find(|e| turn_end(e)).unwrap();
    assert!(
        matches!(failed, SessionEvent::TurnFinished { outcome: TurnOutcome::Failed(e), .. } if e.to_string().contains("something broke")),
        "{events:?}"
    );
    assert!(matches!(events.last(), Some(SessionEvent::Ended { exit: Some(exit), error: Some(_) }) if exit.code == Some(1)));
    assert_eq!(session.next_event(Duration::from_millis(10)), None);
    std::fs::remove_dir_all(&base).unwrap();
}

/// The key reaches Codex only in the one `account/login/start` line on its standard input: never in
/// its arguments, environment, any file Archforge or Codex writes (no `auth.json`), an event, an
/// error, a `Debug` output or a log line, even when Codex echoes it back on every stream.
#[test]
fn the_key_reaches_only_the_login_line() {
    capture_logs();
    let base = scratch("canary");
    let body = r#"
on_login() {
  key=$(printf '%s\n' "$line" | sed -n 's/.*"apiKey":"\([^"]*\)".*/\1/p')
  send '{"id":@ID@,"result":{"type":"apiKey"}}'
  echo "login with $key" >&2
}
on_turn() {
  send '{"id":@ID@,"result":{"turn":{"id":"u-1","status":"inProgress","items":[]}}}'
  send "{\"method\":\"item/agentMessage/delta\",\"params\":{\"itemId\":\"i\",\"delta\":\"key $key\"}}"
  send "{\"method\":\"$key\",\"params\":{}}"
  send "{\"method\":\"mcpServer/startupStatus/updated\",\"params\":{\"name\":\"$key\",\"status\":\"$key\"}}"
  send "{\"method\":\"item/started\",\"params\":{\"item\":{\"type\":\"mcpToolCall\",\"id\":\"$key\",\"server\":\"archforge\",\"tool\":\"$key\",\"arguments\":{\"k\":\"$key\"},\"status\":\"inProgress\"}}}"
  send "{\"method\":\"item/completed\",\"params\":{\"item\":{\"type\":\"agentMessage\",\"id\":\"i\",\"text\":\"echo $key\"}}}"
  send "{\"id\":\"$key\",\"method\":\"$key\",\"params\":{}}"
  send "{\"method\":\"error\",\"params\":{\"error\":{\"message\":\"bad key $key\",\"codexErrorInfo\":\"unauthorized\"},\"willRetry\":false}}"
  send "{\"method\":\"turn/completed\",\"params\":{\"turn\":{\"id\":\"u-1\",\"status\":\"failed\",\"items\":[],\"error\":{\"message\":\"401 for $key\",\"codexErrorInfo\":\"unauthorized\"}}}}"
  echo "auth failed for $key" >&2
}
"#;
    let (runtime, account, folder) = fake_codex(&base, body);
    let cfg = config(false);
    assert!(!format!("{cfg:?}").contains(CANARY));
    let sign_in = SignIn::ApiKey(canary());
    assert!(!format!("{sign_in:?}").contains(CANARY));
    let mut session = CodexSession::start(&runtime, &account, &folder, cfg, sign_in).unwrap();
    assert!(!format!("{session:?}").contains(CANARY));
    session.send("hello").unwrap();
    let mut events = events_until(&mut session, turn_end);
    let SessionEvent::TurnFinished { outcome: TurnOutcome::Failed(error), .. } = events.last().unwrap().clone() else { panic!("{events:?}") };
    assert_eq!(error.kind(), ErrorKind::InvalidCredential);
    assert!(error.to_string().contains("<redacted>"), "{error}");
    assert!(events.iter().any(|e| matches!(e, SessionEvent::TextDelta { text } if text == "key <redacted>")), "{events:?}");
    let exit = session.shutdown(Duration::from_secs(2)).unwrap();
    events.extend(std::iter::from_fn(|| session.next_event(Duration::from_millis(50))));
    let tail = session.process().stderr_tail();
    assert!(tail.contains("auth failed for <redacted>"), "{tail}");
    for (what, text) in [
        ("events", format!("{events:?}")),
        ("error", error.to_string()),
        ("debug", format!("{session:?}")),
        ("tail", tail),
        ("exit", format!("{exit:?}")),
    ] {
        assert!(!text.contains(&CANARY[..20]), "{what} shows a part of the key: {text}");
    }

    let args = std::fs::read_to_string(base.join("args")).unwrap();
    assert!(!args.contains(CANARY), "{args}");
    assert!(args.contains("cli_auth_credentials_store=\"ephemeral\""), "the key is kept in memory alone");
    let env = std::fs::read_to_string(base.join("env")).unwrap();
    assert!(!env.contains(CANARY), "{env}");
    let sent = std::fs::read_to_string(base.join("stdin")).unwrap();
    let holding: Vec<&str> = sent.lines().filter(|l| l.contains(CANARY)).collect();
    assert_eq!(holding.len(), 1, "{sent}");
    let login: Value = serde_json::from_str(holding[0]).unwrap();
    assert_eq!(login["method"], "account/login/start");
    assert_eq!(login["params"], json!({ "type": "apiKey", "apiKey": CANARY }));

    let workspace = account.path().parent().unwrap();
    for (path, bytes) in files_under(workspace) {
        assert!(!String::from_utf8_lossy(&bytes).contains(CANARY), "{} holds the key", path.display());
        assert_ne!(path.file_name().unwrap(), "auth.json", "{} was written", path.display());
    }
    let copy = std::fs::read_to_string(history::path(&folder)).unwrap();
    assert!(copy.contains("<redacted>"), "{copy}");

    let logs = LOG.lock().unwrap_or_else(PoisonError::into_inner).join("\n");
    assert!(!logs.contains(&CANARY[..20]), "{logs}");
    std::fs::remove_dir_all(&base).unwrap();
}

/// A key pasted into the bridge's label would stand on Codex's command line: refused before
/// anything starts, the key not repeated.
#[test]
fn a_key_on_the_command_line_is_refused() {
    let base = scratch("key-in-label");
    let (runtime, account, folder) = fake_codex(&base, "");
    let cfg = CodexConfig { bridge: Bridge { origin: "sk-short-key-1".into(), ..bridge(false) }, ..config(false) };
    let error = CodexSession::start(&runtime, &account, &folder, cfg, SignIn::ApiKey(Secret::new("sk-short-key-1".to_owned()).unwrap())).unwrap_err();
    assert!(matches!(&error, AgentError::SecretExposed { place: "command line", .. }), "{error:?}");
    assert!(!error.to_string().contains("sk-short-key-1"));
    assert!(!base.join("args").exists(), "nothing was started");
    std::fs::remove_dir_all(&base).unwrap();
}

/// Settings are refused before anything starts; only a Codex runtime is driven, and only with a
/// private settings folder.
#[test]
fn settings_are_refused_before_anything_starts() {
    let base = scratch("refuse");
    let (runtime, account, folder) = fake_codex(&base, "");
    for (cfg, what) in [
        (CodexConfig { request_timeout: Duration::ZERO, ..config(false) }, "zero timeout"),
        (CodexConfig { request_timeout: Duration::from_secs(601), ..config(false) }, "long timeout"),
        (CodexConfig { instructions: Some("x".repeat(64 * 1024 + 1)), ..config(false) }, "long instructions"),
        (CodexConfig { bridge: Bridge { origin: " padded".into(), ..bridge(false) }, ..config(false) }, "padded label"),
    ] {
        assert!(CodexSession::start(&runtime, &account, &folder, cfg, SignIn::ApiKey(canary())).is_err(), "{what}");
    }
    std::fs::set_permissions(account.config(), std::fs::Permissions::from_mode(0o755)).unwrap();
    let error = CodexSession::start(&runtime, &account, &folder, config(false), SignIn::ApiKey(canary())).unwrap_err();
    assert!(matches!(&error, AgentError::Folder { problem, .. } if problem.contains("settings folder")), "{error:?}");
    std::fs::set_permissions(account.config(), std::fs::Permissions::from_mode(0o700)).unwrap();
    assert!(!base.join("args").exists(), "nothing was started");
    // A broken copy of the conversation is refused on a resume, not silently dropped.
    std::fs::write(history::path(&folder), "{\"version\":9}").unwrap();
    let error = CodexSession::start(&runtime, &account, &folder, CodexConfig { start: Start::Resume, ..config(false) }, SignIn::ApiKey(canary()))
        .unwrap_err();
    assert!(matches!(&error, AgentError::Folder { problem, .. } if problem.contains("conversation copy")), "{error:?}");
    assert!(!history::path(&folder).exists(), "the damaged copy was moved");
    let aside = folder.path().join("codex-history.json.damaged-1");
    assert_eq!(std::fs::read_to_string(&aside).unwrap(), "{\"version\":9}", "and kept, so no later write replaces it");
    std::fs::remove_dir_all(&base).unwrap();
}

/// The account folder is the workspace's `account-codex`, never a conversation's, and survives a
/// conversation's removal, which takes the conversation's state, logs and copy with it.
#[test]
fn the_account_folder_outlives_conversations() {
    let base = scratch("folders");
    let (runtime, account, folder) = fake_codex(&base, &replay_turns("codex/doc-turn-with-mcp-tool.jsonl"));
    assert_eq!(account.path().file_name().unwrap(), "account-codex");
    assert_eq!(account.id().as_str(), "account-codex");
    let mut session = CodexSession::start(&runtime, &account, &folder, config(false), SignIn::ApiKey(canary())).unwrap();
    session.send("hi").unwrap();
    events_until(&mut session, turn_end);
    session.shutdown(Duration::from_secs(2)).unwrap();
    assert!(history::path(&folder).is_file() && folder.config().join("codex-state").is_dir());
    let ws = Workspace::open(account.path().parent().unwrap()).unwrap();
    assert!(ws.remove_conversation(folder.id()).unwrap());
    assert!(!folder.path().exists() && account.config().is_dir());
    std::fs::remove_dir_all(&base).unwrap();
}

/// A Codex writing only lines that mean nothing cannot hold `next_event` past its timeout.
#[test]
fn lines_that_mean_nothing_cannot_hold_next_event() {
    let base = scratch("noise");
    let (runtime, account, folder) = fake_codex(
        &base,
        "on_turn() { send '{\"id\":@ID@,\"result\":{\"turn\":{\"id\":\"u\"}}}'; while :; do printf '%s\\n' '{\"method\":\"thread/status/changed\",\"params\":{}}'; done; }",
    );
    let mut session = CodexSession::start(&runtime, &account, &folder, config(false), SignIn::ApiKey(canary())).unwrap();
    session.send("hi").unwrap();
    std::thread::sleep(Duration::from_millis(300));
    let started = Instant::now();
    while session.next_event(Duration::from_millis(100)).is_some() {}
    assert!(started.elapsed() < Duration::from_millis(2000), "{:?}", started.elapsed());
    session.shutdown(Duration::from_millis(200)).unwrap();
    std::fs::remove_dir_all(&base).unwrap();
}

// --- the account operations ----------------------------------------------------------------------

/// The fake's answers for the Settings page's calls.
const ACCOUNT_BODY: &str = r#"
signed_in=
on_account() {
  if [ -n "$signed_in" ]; then send '{"id":@ID@,"result":{"account":{"type":"chatgpt","email":"person@example.com","planType":"plus","workspaceName":null},"requiresOpenaiAuth":true}}';
  else send '{"id":@ID@,"result":{"account":null,"requiresOpenaiAuth":true}}'; fi
}
on_login() {
  case "$line" in
    *'"type":"chatgptDeviceCode"'*) send '{"id":@ID@,"result":{"type":"chatgptDeviceCode","loginId":"login-2","verificationUrl":"https://auth.openai.com/codex/device","userCode":"ABCD-EFGH"}}' ;;
    *'"type":"chatgpt"'*) send '{"id":@ID@,"result":{"type":"chatgpt","loginId":"login-1","authUrl":"https://auth.openai.com/oauth/authorize?PLACEHOLDER"}}'
      signed_in=1
      send '{"method":"account/login/completed","params":{"loginId":"login-1","success":true,"error":null,"onboardingEntrypoint":null}}'
      send '{"method":"account/updated","params":{"authMode":"chatgpt","planType":"plus"}}' ;;
    *) on_key_login ;;
  esac
}
on_key_login() { send '{"id":@ID@,"result":{"type":"apiKey"}}'; }
on_other() {
  case "$line" in
    *'"method":"account/login/cancel"'*) send '{"id":@ID@,"result":{"status":"canceled"}}'; send '{"method":"account/login/completed","params":{"loginId":"login-2","success":false,"error":"login cancelled","onboardingEntrypoint":null}}' ;;
    *'"method":"account/logout"'*) signed_in=; send '{"id":@ID@,"result":{}}'; send '{"method":"account/updated","params":{"authMode":null,"planType":null}}' ;;
    *'"method":"model/list"'*'"cursor"'*) send '{"id":@ID@,"result":{"data":[{"id":"gpt-6.1-mini","model":"gpt-6.1-mini","displayName":"GPT-6.1-Mini","description":"Fixture text.","hidden":false,"isDefault":false,"supportedReasoningEfforts":[{"reasoningEffort":"low","description":"x"}],"defaultReasoningEffort":"low"}],"nextCursor":null}}' ;;
    *'"method":"model/list"'*) send '{"id":@ID@,"result":{"data":[{"id":"gpt-6.1-sol","model":"gpt-6.1-sol","displayName":"GPT-6.1-Sol","description":"Fixture text.","hidden":false,"isDefault":true,"supportedReasoningEfforts":[{"reasoningEffort":"low","description":"x"},{"reasoningEffort":"high","description":"y"}],"defaultReasoningEffort":"medium"},{"model":"no-id"}],"nextCursor":"page-2"}}' ;;
    *'"method":"account/rateLimits/read"'*) send '{"id":@ID@,"result":{"ordinaryUsageAllowed":false,"rateLimits":{"limitId":"codex","primary":{"usedPercent":100,"windowDurationMins":300,"resetsAt":1793940000},"secondary":{"usedPercent":42,"windowDurationMins":10080,"resetsAt":1794400000},"credits":null,"planType":"plus","rateLimitReachedType":"rate_limit_reached"}}}' ;;
  esac
}
"#;

/// Events of an account process until (and with) the first that `stop` accepts.
fn account_events_until(accounts: &mut CodexAccounts, stop: impl Fn(&AccountEvent) -> bool) -> Vec<AccountEvent> {
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut events = Vec::new();
    while Instant::now() < deadline {
        if let Some(event) = accounts.next_event(Duration::from_millis(100)) {
            let done = stop(&event);
            events.push(event);
            if done {
                return events;
            }
        }
    }
    panic!("the awaited account event did not come within five seconds: {events:?}");
}

/// The device-code sign-in: the code and the address come back and as an event for the Settings
/// page; the person cancels, and the sign-in ends as failed.
#[test]
fn a_device_code_sign_in_is_shown_and_cancelled() {
    let base = scratch("device");
    let (runtime, account, _) = fake_codex(&base, ACCOUNT_BODY);
    let mut accounts = CodexAccounts::open(&runtime, &account, SignIn::ChatGpt(KeyringAccess::default()), Default::default()).unwrap();
    assert_eq!(accounts.read(false).unwrap().status, AccountStatus::SignedOut);
    let started = accounts.login_device_code().unwrap();
    let expected = LoginStarted::DeviceCode {
        login_id: "login-2".into(),
        verification_url: "https://auth.openai.com/codex/device".into(),
        user_code: "ABCD-EFGH".into(),
    };
    assert_eq!(started, expected);
    assert_eq!(accounts.next_event(Duration::from_millis(100)), Some(AccountEvent::LoginStarted(expected)));
    assert_eq!(accounts.cancel_login(started.login_id()).unwrap(), CancelStatus::Canceled);
    let events = account_events_until(&mut accounts, |e| matches!(e, AccountEvent::LoginCompleted { .. }));
    assert_eq!(
        events.last(),
        Some(&AccountEvent::LoginCompleted { login_id: Some("login-2".into()), success: false, error: Some("login cancelled".into()) })
    );
    let sent = stdin_lines(&base, 5);
    let login: Value = serde_json::from_str(requests(&sent, "account/login/start")[0]).unwrap();
    assert_eq!(login["params"], json!({ "type": "chatgptDeviceCode" }));
    let cancel: Value = serde_json::from_str(requests(&sent, "account/login/cancel")[0]).unwrap();
    assert_eq!(cancel["params"], json!({ "loginId": "login-2" }));
    assert!(requests(&sent, "thread/start").is_empty(), "the Settings page starts no thread");
    accounts.shutdown(Duration::from_secs(2)).unwrap();
    std::fs::remove_dir_all(&base).unwrap();
}

/// The browser sign-in: the address to open, the completion, who is signed in, the check, the rate
/// limits, the models across pages, and the sign-out.
#[test]
fn a_browser_sign_in_reads_the_account_and_signs_out() {
    let base = scratch("browser");
    let (runtime, account, _) = fake_codex(&base, ACCOUNT_BODY);
    let mut accounts = CodexAccounts::open(&runtime, &account, SignIn::ChatGpt(KeyringAccess::default()), Default::default()).unwrap();
    let started = accounts.login_chatgpt().unwrap();
    assert!(
        matches!(&started, LoginStarted::Browser { login_id, auth_url } if login_id == "login-1" && auth_url.starts_with("https://auth.openai.com/"))
    );
    let events = account_events_until(&mut accounts, |e| matches!(e, AccountEvent::AccountChanged { .. }));
    assert!(events.contains(&AccountEvent::LoginCompleted { login_id: Some("login-1".into()), success: true, error: None }), "{events:?}");
    assert_eq!(events.last(), Some(&AccountEvent::AccountChanged { auth_mode: Some("chatgpt".into()), plan: Some("plus".into()) }));
    let signed_in = AccountStatus::ChatGpt { email: Some("person@example.com".into()), plan: Some("plus".into()), workspace: None };
    assert_eq!(accounts.read(false).unwrap().status, signed_in);
    assert_eq!(accounts.check().unwrap().status, signed_in);
    let limits = accounts.rate_limits().unwrap();
    assert_eq!(limits.ordinary_usage_allowed, Some(false));
    assert_eq!(limits.primary.unwrap().used_percent, Some(100));
    assert_eq!(limits.reached.as_deref(), Some("rate_limit_reached"));
    let models = accounts.models().unwrap();
    assert_eq!(models.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(), vec!["gpt-6.1-sol", "gpt-6.1-mini"], "a model without an id is left out");
    assert!(models[0].is_default && models[0].reasoning_efforts == vec!["low", "high"]);
    accounts.logout().unwrap();
    assert_eq!(accounts.read(false).unwrap().status, AccountStatus::SignedOut);
    let sent = stdin_lines(&base, 10);
    let refresh = requests(&sent, "account/read").into_iter().filter(|l| l.contains("refreshToken")).count();
    assert_eq!(refresh, 1, "the check renews the sign-in once: {sent:?}");
    let args = std::fs::read_to_string(base.join("args")).unwrap();
    assert!(args.contains("cli_auth_credentials_store=\"keyring\"") && !args.contains("mcp_servers"));
    let env = std::fs::read_to_string(base.join("env")).unwrap();
    assert!(env.lines().any(|l| l == format!("CODEX_HOME={}", account.config().display())));
    assert!(args.contains(&format!("sqlite_home=\"{}/codex-", account.tmp().display())), "{args}");
    assert_eq!(std::fs::read_dir(account.tmp()).unwrap().count(), 1, "the process's own state folder");
    accounts.shutdown(Duration::from_secs(2)).unwrap();
    drop(accounts);
    assert_eq!(std::fs::read_dir(account.tmp()).unwrap().count(), 0, "nothing the account process kept outlives it");
    std::fs::remove_dir_all(&base).unwrap();
}

/// A sign-in address on another host is refused, never handed to the browser; a renewal refused
/// for an expired sign-in reads as expired.
#[test]
fn foreign_addresses_are_refused_and_expiry_is_read() {
    let base = scratch("foreign");
    let body = format!(
        "{ACCOUNT_BODY}\non_login() {{ send '{{\"id\":@ID@,\"result\":{{\"type\":\"chatgpt\",\"loginId\":\"l\",\"authUrl\":\"https://auth.openai.com.evil.example/x\"}}}}'; }}\n\
         on_account() {{ case \"$line\" in *refreshToken*) send '{{\"id\":@ID@,\"error\":{{\"code\":-32600,\"message\":\"refresh token was revoked\"}}}}' ;; *) send '{{\"id\":@ID@,\"result\":{{\"account\":{{\"type\":\"chatgpt\",\"email\":\"p@example.com\",\"planType\":\"pro\"}},\"requiresOpenaiAuth\":true}}}}' ;; esac; }}"
    );
    let (runtime, account, _) = fake_codex(&base, &body);
    let mut accounts = CodexAccounts::open(&runtime, &account, SignIn::ChatGpt(KeyringAccess::default()), Default::default()).unwrap();
    let error = accounts.login_chatgpt().unwrap_err();
    assert!(error.to_string().contains("does not open"), "{error}");
    assert_eq!(accounts.next_event(Duration::from_millis(50)), None, "nothing to open was queued");
    assert_eq!(accounts.check().unwrap().status, AccountStatus::Expired { email: Some("p@example.com".into()), plan: Some("pro".into()) });
    accounts.shutdown(Duration::from_secs(2)).unwrap();
    std::fs::remove_dir_all(&base).unwrap();
}

/// The API-key check: the key goes over standard input once, Codex reads as signed in with a key,
/// and nothing is written; a refused key is an invalid credential; ChatGPT calls are refused on a
/// key's process, which keeps nothing in the keyring.
#[test]
fn an_api_key_is_checked_over_standard_input() {
    capture_logs();
    let base = scratch("account-key");
    let body = format!(
        "{ACCOUNT_BODY}\non_account() {{ send '{{\"id\":@ID@,\"result\":{{\"account\":{{\"type\":\"apiKey\"}},\"requiresOpenaiAuth\":true}}}}'; }}"
    );
    let (runtime, account, _) = fake_codex(&base, &body);
    let mut accounts = CodexAccounts::open(&runtime, &account, SignIn::ApiKey(canary()), Default::default()).unwrap();
    assert_eq!(accounts.check().unwrap().status, AccountStatus::ApiKey);
    assert!(matches!(accounts.login_chatgpt(), Err(AgentError::InvalidSetting { .. })));
    assert!(matches!(accounts.login_device_code(), Err(AgentError::InvalidSetting { .. })));
    assert!(!format!("{accounts:?}").contains(CANARY));
    accounts.shutdown(Duration::from_secs(2)).unwrap();
    let sent = std::fs::read_to_string(base.join("stdin")).unwrap();
    assert_eq!(sent.lines().filter(|l| l.contains(CANARY)).count(), 1);
    assert!(sent.lines().next().unwrap().contains("\"initialize\""), "the handshake comes first");
    let args = std::fs::read_to_string(base.join("args")).unwrap();
    assert!(!args.contains(CANARY) && args.contains("cli_auth_credentials_store=\"ephemeral\""));
    for (path, bytes) in files_under(account.path()) {
        assert!(!String::from_utf8_lossy(&bytes).contains(CANARY), "{}", path.display());
    }
    std::fs::remove_dir_all(&base).unwrap();

    let base = scratch("account-bad-key");
    let body = format!("{ACCOUNT_BODY}\non_key_login() {{ send '{{\"id\":@ID@,\"error\":{{\"code\":-32600,\"message\":\"bad key\"}}}}'; }}");
    let (runtime, account, _) = fake_codex(&base, &body);
    let error = CodexAccounts::open(&runtime, &account, SignIn::ApiKey(canary()), Default::default()).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::InvalidCredential, "{error}");
    let logs = LOG.lock().unwrap_or_else(PoisonError::into_inner).join("\n");
    assert!(!logs.contains(&CANARY[..20]));
    std::fs::remove_dir_all(&base).unwrap();
}

/// A call Codex never answers fails on its own deadline instead of waiting forever.
#[test]
fn an_unanswered_account_call_times_out() {
    let base = scratch("account-timeout");
    let (runtime, account, _) = fake_codex(&base, &format!("{ACCOUNT_BODY}\non_account() {{ :; }}"));
    let options = archforge_agents::codex::AccountOptions { request_timeout: Duration::from_millis(200), ..Default::default() };
    let mut accounts = CodexAccounts::open(&runtime, &account, SignIn::ChatGpt(KeyringAccess::default()), options).unwrap();
    let started = Instant::now();
    let error = accounts.read(false).unwrap_err();
    assert!(error.to_string().contains("did not answer account/read within 200 ms"), "{error}");
    assert!(started.elapsed() < Duration::from_secs(2));
    // The late answer to nothing is counted, and the next call still works.
    assert!(accounts.models().is_ok());
    accounts.shutdown(Duration::from_secs(2)).unwrap();
    std::fs::remove_dir_all(&base).unwrap();
}

// --- the real round trip -------------------------------------------------------------------------

/// One real conversation and the account calls with an installed Codex and a running Archforge.
/// Ignored: it signs in and costs a little, so it needs the owner's OK. To run it, start Archforge
/// with `--control <port>`, build `archforge-cli`, run
/// `read -rs ARCHFORGE_LIVE_OPENAI_KEY && export ARCHFORGE_LIVE_OPENAI_KEY` (the key stays out of
/// the shell's history), then `ARCHFORGE_LIVE_PORT=<port> ARCHFORGE_LIVE_CLI=<abs path to
/// archforge-cli> cargo test -p archforge-agents --test codex -- --ignored --nocapture`.
/// It checks the spike's round-trip items 6, 8, 9 and 11 with a key: the turn's events with a real
/// tool call, whether a tool call raises an elicitation, the interrupt, the switched-off tools, that
/// no `auth.json` is written, and a resume that injects the earlier messages. The ChatGPT items (7,
/// 10) are by hand: `CodexAccounts::login_chatgpt` and `login_device_code` with a real account.
#[test]
#[ignore = "a real round trip: needs a real key, a running Archforge and the owner's OK"]
fn a_real_round_trip() {
    let var = |k: &str| std::env::var(k).unwrap_or_else(|_| panic!("{k} is not set"));
    let base = scratch("live");
    let ws = Workspace::open(&base.join("workspace")).unwrap();
    let runtime = find(RuntimeKind::Codex, &HostEnv::from_process(), &ws).unwrap().into_result().unwrap();
    let account = ws.account(RuntimeKind::Codex).unwrap();
    let folder = ws.conversation(&ConversationId::new(CONVERSATION).unwrap()).unwrap();
    let key = || SignIn::ApiKey(Secret::new(var("ARCHFORGE_LIVE_OPENAI_KEY")).unwrap());
    let mut accounts = CodexAccounts::open(&runtime, &account, key(), Default::default()).unwrap();
    println!("{:?}", accounts.check());
    println!("{:?}", accounts.models().map(|m| m.into_iter().map(|m| m.id).collect::<Vec<_>>()));
    accounts.shutdown(Duration::from_secs(5)).unwrap();
    let live = |start, allow| CodexConfig {
        start,
        bridge: Bridge {
            cli: PathBuf::from(var("ARCHFORGE_LIVE_CLI")),
            port: var("ARCHFORGE_LIVE_PORT").parse().unwrap(),
            origin: "Live test".into(),
            allow_changes: allow,
            home: BridgeHome::from_process(),
        },
        request_timeout: DEFAULT_REQUEST_TIMEOUT,
        ..config(allow)
    };
    let run = |session: &mut CodexSession, message: &str, stop_after_text: bool| {
        session.send(message).unwrap();
        let deadline = Instant::now() + Duration::from_secs(300);
        while Instant::now() < deadline {
            if let Some(event) = session.next_event(Duration::from_secs(1)) {
                println!("{event:?}");
                if stop_after_text && matches!(event, SessionEvent::TextDelta { .. }) {
                    session.stop().unwrap();
                }
                if turn_end(&event) {
                    break;
                }
            }
        }
    };
    let mut session = CodexSession::start(&runtime, &account, &folder, live(Start::New, false), key()).unwrap();
    run(&mut session, "Which schemas are in the open model? Use the tools.", false);
    run(&mut session, "Describe every element of the model in great detail.", true);
    session.shutdown(Duration::from_secs(5)).unwrap();
    let mut resumed = CodexSession::start(&runtime, &account, &folder, live(Start::Resume, true), key()).unwrap();
    run(&mut resumed, "What did I ask first?", false);
    resumed.shutdown(Duration::from_secs(5)).unwrap();
    for (path, _) in files_under(&base) {
        assert_ne!(path.file_name().unwrap(), "auth.json", "{} was written", path.display());
    }
    std::fs::remove_dir_all(&base).unwrap();
}
