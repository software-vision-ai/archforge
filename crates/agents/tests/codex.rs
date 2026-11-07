//! The Codex adapter against the AR-196 transcripts, the credited schema cut, and fake `codex`
//! programs: shell scripts that answer `--version` as codex-cli 0.166.0 does and then answer the
//! adapter's JSON-RPC lines as a test needs, replaying the spike's message shapes. No test starts an
//! installed `codex`, uses a real key, or reads `~/.codex`.
//!
//! The transcripts in `tests/fixtures/codex/` are copies of the spike's
//! (`docs/agent-runtimes/README.md` on main says how they were recorded and sanitised): one JSON
//! object a line, `{"t", "dir": in|out|err|close|sig|exit|note, "line"}`. Their `note` lines say
//! which captures pipelined the handshake (the recording driver wrote `initialized` and the next
//! request before the `initialize` answer); the replays below answer in the adapter's strict order.
//! `schema/codex-app-server-0.166.0.contract.json` is a cut of Codex's generated schema (Apache-2.0,
//! see its `$comment` and NOTICE).
#![cfg(unix)]

use std::collections::{BTreeSet, HashMap};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use archforge_agents::codex::history::{COPY_FILE, Role};
use archforge_agents::codex::{
    AccountEvent, Auth, Bridge, BridgeHome, CancelStatus, ChatGptStore, CodexAccount, CodexConfig, CodexHome, CodexSession, CredentialStore,
    LoginOutcome, Start,
};
use archforge_agents::{
    AgentError, AgentSession, ConversationFolder, ConversationId, ErrorKind, HostEnv, Limits, Runtime, RuntimeKind, SessionEvent, Skipped,
    TurnOutcome, Workspace, find,
};
use archforge_secrets::Secret;
use serde_json::{Value, json};

/// A key no output of the adapter may ever show but the one line that signs in.
const CANARY: &str = "sk-proj-CANARY-5b2e9d0c7a41_canary-canary-canary-canary-canary-canary-canary-xyz";
/// The thread id the fixtures use.
const THREAD: &str = "019a0000-0000-7000-8000-000000000001";

// --- helpers -------------------------------------------------------------------------------------

/// Every log line any test wrote, to look for the key in.
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

/// One line of a transcript.
#[derive(Debug, Clone)]
struct Record {
    dir: String,
    line: String,
}

/// The transcript `name` under `tests/fixtures/codex/`.
fn fixture(name: &str) -> Vec<Record> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/codex").join(name);
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
    "doc-approval-declined.jsonl",
    "doc-login-api-key.jsonl",
    "doc-login-chatgpt-browser.jsonl",
    "doc-login-chatgpt-device-code-cancelled.jsonl",
    "doc-logout.jsonl",
    "doc-rate-limits.jsonl",
    "doc-turn-errors.jsonl",
    "doc-turn-interrupt.jsonl",
    "doc-turn-with-mcp-tool.jsonl",
    "observed-env-api-key-not-read.jsonl",
    "observed-ephemeral-store-account-read.jsonl",
    "observed-handshake.jsonl",
    "observed-recommended-flags-handshake.jsonl",
    "observed-sigint.jsonl",
    "observed-sigterm.jsonl",
    "observed-strict-config-unknown-key.jsonl",
    "observed-thread-start-with-mcp.jsonl",
];

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

/// The shell functions every fake has: `req` reads one line into `$line` (recorded in `stdin`) and
/// its request number into `$id`; `ok ID JSON` and `err ID CODE MESSAGE` answer; `say JSON` writes
/// a line; `hello` answers the handshake; `drain` reads to the end and exits 0; `hang` never ends.
const PRELUDE: &str = r#"
req() { IFS= read -r line || exit 0; printf '%s\n' "$line" >> "$IN"; id=$(printf '%s\n' "$line" | sed -n 's/^{"id":\([0-9][0-9]*\),.*/\1/p'); }
ok() { printf '{"id":%s,"result":%s}\n' "$1" "$2"; }
err() { printf '{"id":%s,"error":{"code":%s,"message":"%s"}}\n' "$1" "$2" "$3"; }
say() { printf '%s\n' "$1"; }
hello() { req; ok "$id" '{"userAgent":"archforge/0.166.0 (test)","codexHome":"/tmp/archforge-agent/codex-home","platformFamily":"unix","platformOs":"macos"}'; req; }
drain() { while IFS= read -r line; do printf '%s\n' "$line" >> "$IN"; done; exit 0; }
hang() { while :; do /bin/sleep 1; done; }
"#;

/// A test's world: the fake `codex` at `base/bin/codex` running `body` after [`PRELUDE`], found
/// through the override; its workspace, Archforge's private `CODEX_HOME`, and a fresh conversation
/// folder.
struct World {
    base: PathBuf,
    runtime: Runtime,
    workspace: Workspace,
    home: CodexHome,
    folder: ConversationFolder,
}

impl World {
    /// The world of the test `name` with a fake running `body`.
    fn new(name: &str, body: &str) -> World {
        let base = scratch(name);
        let program = base.join("bin/codex");
        std::fs::create_dir_all(program.parent().unwrap()).unwrap();
        let script = format!(
            "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then echo 'codex-cli 0.166.0'; exit 0; fi\n\
             printf '%s\\n' \"$@\" > {args}\n/usr/bin/env > {env}\nIN={stdin}\n{PRELUDE}\n{body}\n",
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
        let workspace = Workspace::open(&base.join("workspace")).unwrap();
        let runtime = find(RuntimeKind::Codex, &env, &workspace).unwrap().into_result().unwrap();
        let home = CodexHome::open(&workspace).unwrap();
        let folder = workspace.conversation(&ConversationId::new("6f1c0a52-9d3e-4b7a-8f00-1234567890ab").unwrap()).unwrap();
        World { base, runtime, workspace, home, folder }
    }

    /// A session of this world with `config` and `auth`.
    fn session(&self, config: CodexConfig, auth: Auth) -> CodexSession {
        CodexSession::start(&self.runtime, &self.folder, &self.home, config, auth).unwrap()
    }

    /// An account process of this world with `store`, a request timeout of `timeout`.
    fn account(&self, store: CredentialStore, timeout: Duration) -> CodexAccount {
        CodexAccount::start(&self.runtime, &self.workspace, &self.home, store, Limits::default(), timeout, Duration::from_secs(5)).unwrap()
    }

    /// The lines the fake read, waiting up to five seconds for `count` of them.
    fn stdin(&self, count: usize) -> Vec<String> {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let text = std::fs::read_to_string(self.base.join("stdin")).unwrap_or_default();
            let lines: Vec<String> = text.lines().map(str::to_owned).collect();
            if lines.len() >= count || Instant::now() >= deadline {
                return lines;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// The methods of the lines the fake read, waiting for `count` of them.
    fn methods(&self, count: usize) -> Vec<String> {
        self.stdin(count)
            .iter()
            .map(|l| serde_json::from_str::<Value>(l).ok().and_then(|v| v["method"].as_str().map(str::to_owned)).unwrap_or_default())
            .collect()
    }

    /// Polls `session` until the fake has read `count` lines, within five seconds; answers the
    /// events that came meanwhile.
    fn pump(&self, session: &mut CodexSession, count: usize) -> Vec<SessionEvent> {
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut events = Vec::new();
        loop {
            let read = std::fs::read_to_string(self.base.join("stdin")).unwrap_or_default().lines().count();
            if read >= count {
                return events;
            }
            assert!(Instant::now() < deadline, "the fake read {read} of {count} lines within five seconds: {events:?}");
            events.extend(session.next_event(Duration::from_millis(10)));
        }
    }

    /// Checks that every request and notification the adapter wrote matches the contract, then
    /// removes the world's folder.
    fn remove(self) {
        let base = self.base.clone();
        drop(self);
        let contract = Contract::load();
        for line in std::fs::read_to_string(base.join("stdin")).unwrap_or_default().lines() {
            let value: Value = serde_json::from_str(line).unwrap_or_else(|e| panic!("the adapter wrote a line that is not JSON ({e}): {line}"));
            match (value.get("method"), value.get("id")) {
                (Some(_), Some(_)) => assert!(contract.fits("ClientRequest", &value), "outside the contract: {line}"),
                (Some(_), None) => assert!(contract.fits("ClientNotification", &value), "outside the contract: {line}"),
                // Answers to Codex's own requests: checked where a test knows the request.
                _ => {}
            }
        }
        std::fs::remove_dir_all(&base).unwrap();
    }
}

/// The bridge of the tests: an `archforge-cli` that need not exist, read-only.
fn bridge() -> Bridge {
    Bridge {
        cli: PathBuf::from("/opt/archforge/bin/archforge-cli"),
        port: 47000,
        origin: "Chat 1".to_owned(),
        allow_changes: false,
        home: BridgeHome { home: Some(PathBuf::from("/Users/person")), xdg_config_home: None },
    }
}

/// A configuration for `start` with a request timeout of `timeout`.
fn config(start: Start, timeout: Duration) -> CodexConfig {
    CodexConfig { start, model: None, bridge: bridge(), instructions: None, limits: Limits::default(), request_timeout: timeout }
}

/// A ChatGPT sign-in in the keyring: no sign-in request in the handshake.
fn chatgpt() -> Auth {
    Auth::ChatGpt(ChatGptStore::Keyring)
}

/// The canary as a secret.
fn canary() -> Secret {
    Secret::new(CANARY.to_owned()).unwrap()
}

/// Events until (and with) the first that `stop` accepts, within `secs` seconds.
fn events_until(session: &mut CodexSession, secs: u64, stop: impl Fn(&SessionEvent) -> bool) -> Vec<SessionEvent> {
    let deadline = Instant::now() + Duration::from_secs(secs);
    let mut events = Vec::new();
    while Instant::now() < deadline {
        if let Some(event) = session.next_event(Duration::from_millis(50)) {
            let done = stop(&event);
            events.push(event);
            if done {
                return events;
            }
        }
    }
    panic!("the awaited event did not come within {secs} seconds: {events:?}");
}

/// Every event that comes within `window`.
fn events_for(session: &mut CodexSession, window: Duration) -> Vec<SessionEvent> {
    let deadline = Instant::now() + window;
    let mut events = Vec::new();
    while Instant::now() < deadline {
        if let Some(event) = session.next_event(Duration::from_millis(20)) {
            events.push(event);
        }
    }
    events
}

/// Whether `event` ends a turn.
fn turn_end(event: &SessionEvent) -> bool {
    matches!(event, SessionEvent::TurnFinished { .. })
}

/// Whether `event` ends the session.
fn ended(event: &SessionEvent) -> bool {
    matches!(event, SessionEvent::Ended { .. })
}

/// The outcome of the last finished turn among `events`.
fn last_outcome(events: &[SessionEvent]) -> &TurnOutcome {
    events
        .iter()
        .rev()
        .find_map(|e| match e {
            SessionEvent::TurnFinished { outcome, .. } => Some(outcome),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no turn finished: {events:?}"))
}

/// A shell body that replays the transcript `name` after `prelude`, answering in the adapter's
/// order: each request the transcript reads is read only when an answer needs it, so a capture that
/// pipelined the handshake does not wait for a line the adapter sends later; answers carry the
/// adapter's own request numbers. Then the input is drained.
fn replay(name: &str, prelude: &str) -> String {
    let mut body = String::from(prelude);
    body.push('\n');
    let mut pending: Vec<Option<u64>> = Vec::new();
    for record in fixture(name) {
        match record.dir.as_str() {
            "in" => {
                let id = serde_json::from_str::<Value>(&record.line).ok().filter(|v| v.get("method").is_some()).and_then(|v| v["id"].as_u64());
                pending.push(id);
            }
            "out" => {
                let value: Value = serde_json::from_str(&record.line).unwrap();
                let answers = if value.get("method").is_none() { value["id"].as_u64() } else { None };
                if let Some(answered) = answers {
                    while let Some(next) = (!pending.is_empty()).then(|| pending.remove(0)) {
                        body.push_str("req\n");
                        if let Some(id) = next {
                            body.push_str(&format!("id_{id}=$id\n"));
                        }
                        if next == Some(answered) {
                            break;
                        }
                    }
                    let prefix = format!("{{\"id\":{answered},");
                    let rest =
                        record.line.strip_prefix(&prefix).unwrap_or_else(|| panic!("{name}: an answer not starting with its id: {}", record.line));
                    body.push_str(&format!("printf '{{\"id\":%s,%s\\n' \"$id_{answered}\" {}\n", quoted(rest)));
                } else {
                    body.push_str(&format!("say {}\n", quoted(&record.line)));
                }
            }
            "err" => body.push_str(&format!("printf '%s\\n' {} >&2\n", quoted(&record.line))),
            _ => {}
        }
    }
    for next in pending {
        body.push_str("req\n");
        if let Some(id) = next {
            body.push_str(&format!("id_{id}=$id\n"));
        }
    }
    body.push_str("drain\n");
    body
}

/// The handshake and a thread named `THREAD`, for replaying a turn.
fn thread_prelude() -> String {
    format!("hello\nreq\nok \"$id\" '{{\"thread\":{{\"id\":\"{THREAD}\"}},\"model\":\"gpt-6.1-sol\"}}'")
}

// --- the contract --------------------------------------------------------------------------------

/// The credited schema cut, and a small validator for the Draft 7 keywords it uses.
struct Contract {
    root: Value,
}

/// The keywords [`Contract::valid`] understands; the cut may use no other.
const KEYWORDS: [&str; 17] = [
    "$comment",
    "$ref",
    "$schema",
    "additionalProperties",
    "allOf",
    "anyOf",
    "definitions",
    "enum",
    "format",
    "items",
    "minLength",
    "minimum",
    "oneOf",
    "properties",
    "required",
    "title",
    "type",
];

impl Contract {
    /// The cut in `schema/`.
    fn load() -> Contract {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("schema/codex-app-server-0.166.0.contract.json");
        Contract { root: serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap() }
    }

    /// The definition a `$ref` names.
    fn resolve(&self, reference: &str) -> &Value {
        let mut node = &self.root;
        for part in reference.strip_prefix("#/").unwrap().split('/') {
            node = &node[part];
        }
        assert!(!node.is_null(), "{reference} does not resolve");
        node
    }

    /// The methods of a `oneOf` of requests or notifications, such as `ClientRequest`.
    fn methods(&self, name: &str) -> BTreeSet<String> {
        self.root["definitions"][name]["oneOf"]
            .as_array()
            .unwrap()
            .iter()
            .map(|o| o["properties"]["method"]["enum"][0].as_str().unwrap().to_owned())
            .collect()
    }

    /// Whether `value` matches `schema`.
    fn valid(&self, schema: &Value, value: &Value) -> bool {
        let object = match schema {
            Value::Bool(b) => return *b,
            Value::Object(object) => object,
            other => panic!("not a schema: {other}"),
        };
        if let Some(reference) = object.get("$ref") {
            return self.valid(self.resolve(reference.as_str().unwrap()), value);
        }
        if let Some(kind) = object.get("type") {
            let kinds: Vec<&str> = match kind {
                Value::String(k) => vec![k.as_str()],
                Value::Array(ks) => ks.iter().map(|k| k.as_str().unwrap()).collect(),
                other => panic!("type {other}"),
            };
            let fits = kinds.iter().any(|k| match *k {
                "null" => value.is_null(),
                "boolean" => value.is_boolean(),
                "string" => value.is_string(),
                "integer" => value.is_i64() || value.is_u64(),
                "number" => value.is_number(),
                "array" => value.is_array(),
                "object" => value.is_object(),
                other => panic!("type {other}"),
            });
            if !fits {
                return false;
            }
        }
        if let Some(choices) = object.get("enum")
            && !choices.as_array().unwrap().contains(value)
        {
            return false;
        }
        if let (Some(min), Some(n)) = (object.get("minimum").and_then(Value::as_f64), value.as_f64())
            && n < min
        {
            return false;
        }
        if let (Some(min), Some(s)) = (object.get("minLength").and_then(Value::as_u64), value.as_str())
            && u64::try_from(s.chars().count()).unwrap() < min
        {
            return false;
        }
        if let Some(fields) = value.as_object() {
            if let Some(required) = object.get("required")
                && !required.as_array().unwrap().iter().all(|r| fields.contains_key(r.as_str().unwrap()))
            {
                return false;
            }
            let properties = object.get("properties").and_then(Value::as_object);
            for (name, field) in fields {
                match properties.and_then(|p| p.get(name)) {
                    Some(schema) => {
                        if !self.valid(schema, field) {
                            return false;
                        }
                    }
                    None => {
                        if let Some(extra) = object.get("additionalProperties")
                            && !self.valid(extra, field)
                        {
                            return false;
                        }
                    }
                }
            }
        }
        if let (Some(items), Some(values)) = (object.get("items"), value.as_array())
            && !values.iter().all(|v| self.valid(items, v))
        {
            return false;
        }
        if let Some(all) = object.get("allOf")
            && !all.as_array().unwrap().iter().all(|s| self.valid(s, value))
        {
            return false;
        }
        if let Some(any) = object.get("anyOf")
            && !any.as_array().unwrap().iter().any(|s| self.valid(s, value))
        {
            return false;
        }
        if let Some(one) = object.get("oneOf")
            && one.as_array().unwrap().iter().filter(|s| self.valid(s, value)).count() != 1
        {
            return false;
        }
        true
    }

    /// Whether `value` matches the definition `name`.
    fn fits(&self, name: &str, value: &Value) -> bool {
        self.valid(&self.root["definitions"][name], value)
    }

    /// Whether `result` matches the result schema of `method`.
    fn fits_result(&self, method: &str, result: &Value) -> bool {
        let schema = &self.root["definitions"]["Responses"]["properties"][method];
        assert!(!schema.is_null(), "no result schema for {method}");
        self.valid(schema, result)
    }
}

/// The methods the Codex adapter sends, reads or answers: the contract holds exactly these.
const CLIENT_REQUESTS: [&str; 11] = [
    "account/login/cancel",
    "account/login/start",
    "account/logout",
    "account/read",
    "initialize",
    "model/list",
    "thread/inject_items",
    "thread/start",
    "thread/unsubscribe",
    "turn/interrupt",
    "turn/start",
];
/// The requests the fixtures hold that the adapter never sends, so the cut leaves out.
const OUTSIDE_THE_CONTRACT: [&str; 4] = ["config/read", "mcpServerStatus/list", "account/rateLimits/read", "no/such/method"];

/// The cut holds every request the adapter sends and nothing it does not; it uses only keywords
/// the validator knows; and it carries its credit.
#[test]
fn the_contract_covers_what_the_adapter_speaks() {
    let contract = Contract::load();
    assert_eq!(contract.methods("ClientRequest"), CLIENT_REQUESTS.iter().map(|m| (*m).to_owned()).collect());
    assert_eq!(contract.methods("ClientNotification"), BTreeSet::from(["initialized".to_owned()]));
    for method in ["turn/completed", "item/agentMessage/delta", "mcpServer/startupStatus/updated", "account/login/completed", "error"] {
        assert!(contract.methods("ServerNotification").contains(method), "{method}");
    }
    let comment = contract.root["$comment"].as_str().unwrap();
    assert!(comment.contains("Apache License") && comment.contains("rust-v0.166.0") && comment.contains("OpenAI"), "{comment}");
    /// Every keyword of `schema` and below, map keys left out.
    fn keywords(schema: &Value, into: &mut BTreeSet<String>) {
        match schema {
            Value::Object(object) => {
                for (key, value) in object {
                    into.insert(key.clone());
                    match key.as_str() {
                        "properties" | "definitions" => {
                            for inner in value.as_object().unwrap().values() {
                                if inner.get("$ref").is_none()
                                    && inner.get("type").is_none()
                                    && inner.as_object().is_some_and(|o| o.values().all(Value::is_object))
                                    && key == "definitions"
                                {
                                    for nested in inner.as_object().unwrap().values() {
                                        keywords(nested, into);
                                    }
                                } else {
                                    keywords(inner, into);
                                }
                            }
                        }
                        _ => keywords(value, into),
                    }
                }
            }
            Value::Array(values) => values.iter().for_each(|v| keywords(v, into)),
            _ => {}
        }
    }
    let mut used = BTreeSet::new();
    keywords(&contract.root, &mut used);
    let unknown: Vec<&String> = used.iter().filter(|k| !KEYWORDS.contains(&k.as_str())).collect();
    assert!(unknown.is_empty(), "keywords the validator does not know: {unknown:?}");
    // The validator says no where it must.
    assert!(!contract.fits("ClientRequest", &json!({"id": 1, "method": "account/read", "params": {"refreshToken": "yes"}})));
    assert!(!contract.fits("ClientRequest", &json!({"id": 1, "method": "no/such/method", "params": {}})));
    assert!(!contract.fits_result("account/read", &json!({"account": null})));
}

/// Every message of every fixture that the contract covers matches it: requests and notifications
/// by kind, answers by the method of the request they answer.
#[test]
fn fixture_messages_match_the_contract_schema() {
    let contract = Contract::load();
    let client_requests = contract.methods("ClientRequest");
    let server_requests = contract.methods("ServerRequest");
    let server_notifications = contract.methods("ServerNotification");
    let mut checked = 0;
    for name in CODEX_FIXTURES {
        // The method of each request in flight, by direction and id.
        let mut asked: HashMap<(bool, u64), String> = HashMap::new();
        for record in fixture(name) {
            let from_codex = match record.dir.as_str() {
                "in" => false,
                "out" => true,
                _ => continue,
            };
            let Ok(value) = serde_json::from_str::<Value>(&record.line) else {
                assert_eq!(record.line, "this is not json", "{name}: an unreadable line");
                continue;
            };
            let method = value.get("method").and_then(Value::as_str);
            let id = value.get("id").and_then(Value::as_u64);
            let fits = match (method, id) {
                (Some(m), Some(id)) => {
                    asked.insert((from_codex, id), m.to_owned());
                    if OUTSIDE_THE_CONTRACT.contains(&m) {
                        continue;
                    }
                    let (kind, known) = if from_codex { ("ServerRequest", &server_requests) } else { ("ClientRequest", &client_requests) };
                    assert!(known.contains(m), "{name}: {m} is outside the contract");
                    contract.fits(kind, &value)
                }
                (Some(m), None) => {
                    if from_codex {
                        assert!(server_notifications.contains(m), "{name}: the notification {m} is outside the contract");
                        contract.fits("ServerNotification", &value)
                    } else {
                        contract.fits("ClientNotification", &value)
                    }
                }
                (None, Some(id)) => {
                    let Some(m) = asked.get(&(!from_codex, id)).cloned() else { panic!("{name}: an answer to no request: {}", record.line) };
                    if OUTSIDE_THE_CONTRACT.contains(&m.as_str()) {
                        continue;
                    }
                    match (value.get("result"), value.get("error")) {
                        (Some(result), _) => contract.fits_result(&m, result),
                        (None, Some(_)) => contract.fits("JSONRPCError", &value),
                        _ => panic!("{name}: neither result nor error"),
                    }
                }
                (None, None) => panic!("{name}: not a message: {}", record.line),
            };
            assert!(fits, "{name}: does not match the contract: {}", record.line);
            checked += 1;
        }
    }
    assert!(checked >= 80, "only {checked} messages were checked");
}

/// Every copy keeps its note line, which says what it is and how it was made.
#[test]
fn every_fixture_keeps_its_note() {
    for name in CODEX_FIXTURES {
        let records = fixture(name);
        let note = records.iter().find(|r| r.dir == "note").unwrap_or_else(|| panic!("{name} has no note"));
        let expected = if name.starts_with("doc-") { "DOC-DERIVED" } else { "OBSERVED" };
        assert!(note.line.starts_with(expected), "{name}: {}", note.line);
        if name.starts_with("observed-") && name != "observed-handshake.jsonl" {
            assert!(note.line.contains("initialized and the next request went out before the initialize result"), "{name}: {}", note.line);
        }
    }
}

// --- sessions over the transcripts ---------------------------------------------------------------

/// A read-only turn replayed from the transcript: the thread is ready with its model, then the live
/// text, the tool call, the answer and the completed turn; the adapter's own lines match the
/// contract.
#[test]
fn a_turn_with_a_tool_call_replays_from_the_fixture() {
    let world = World::new("replay-turn", &replay("doc-turn-with-mcp-tool.jsonl", &thread_prelude()));
    let mut session = world.session(config(Start::New, Duration::from_secs(5)), chatgpt());
    session.send("Which entities are in Shop?").unwrap();
    let events = events_until(&mut session, 10, turn_end);
    let started = events.iter().find_map(|e| match e {
        SessionEvent::Started { session_id, model, .. } => Some((session_id.clone(), model.clone())),
        _ => None,
    });
    assert_eq!(started, Some((THREAD.to_owned(), "gpt-6.1-sol".to_owned())));
    let rest: Vec<&SessionEvent> = events.iter().filter(|e| !matches!(e, SessionEvent::Started { .. })).collect();
    let SessionEvent::ToolStarted { id, server, tool, input } = rest[0] else { panic!("{rest:?}") };
    assert_eq!((id.as_str(), server.as_deref(), tool.as_str()), ("item-2", Some("archforge"), "er_entity_list"));
    assert_eq!(serde_json::from_str::<Value>(input).unwrap()["schema"], "0b6c1d2e-4f3a-4b5c-8d9e-0a1b2c3d4e5f");
    assert!(matches!(rest[1], SessionEvent::ToolFinished { ok: true, summary, .. } if summary.contains("Customer")));
    assert_eq!(*rest[2], SessionEvent::TextDelta { text: "The schema Shop has two entities: ".into() });
    assert_eq!(*rest[3], SessionEvent::TextDelta { text: "Customer and Order.".into() });
    assert_eq!(*rest[4], SessionEvent::MessageDone { text: "The schema Shop has two entities: Customer and Order.".into() });
    assert!(matches!(rest[5], SessionEvent::TurnFinished { outcome: TurnOutcome::Completed, .. }));
    assert_eq!(rest.len(), 6, "{rest:?}");
    assert_eq!(session.skipped(), 0);
    // The copy holds the question and the answer.
    let copy: Value = serde_json::from_slice(&std::fs::read(world.folder.path().join(COPY_FILE)).unwrap()).unwrap();
    assert_eq!(
        copy["messages"],
        json!([{"role": "user", "text": "Which entities are in Shop?"}, {"role": "assistant", "text": "The schema Shop has two entities: Customer and Order."}])
    );
    // Every line the adapter wrote matches the contract.
    let contract = Contract::load();
    let lines = world.stdin(4);
    assert_eq!(world.methods(4), vec!["initialize", "initialized", "thread/start", "turn/start"]);
    for line in &lines {
        let value: Value = serde_json::from_str(line).unwrap();
        let kind = if value.get("id").is_some() { "ClientRequest" } else { "ClientNotification" };
        assert!(contract.fits(kind, &value), "{line}");
    }
    let thread_start: Value = serde_json::from_str(&lines[2]).unwrap();
    assert_eq!(thread_start["params"]["ephemeral"], true);
    assert_eq!(thread_start["params"]["sandbox"], "read-only");
    assert_eq!(thread_start["params"]["approvalPolicy"], "on-request");
    assert_eq!(thread_start["params"]["cwd"], world.folder.work().to_str().unwrap());
    drop(session);
    world.remove();
}

/// The six failed turns of the transcript land in their categories, a retried error first showing as
/// progress.
#[test]
fn turn_errors_map_to_their_categories() {
    // The API key's sign-in comes in the handshake, before the thread.
    let prelude = format!(
        "hello\nreq\nok \"$id\" '{{\"type\":\"apiKey\"}}'\nreq\nok \"$id\" '{{\"thread\":{{\"id\":\"{THREAD}\"}},\"model\":\"gpt-6.1-sol\"}}'"
    );
    let world = World::new("replay-errors", &replay("doc-turn-errors.jsonl", &prelude));
    let mut session = world.session(config(Start::New, Duration::from_secs(5)), Auth::ApiKey(canary()));
    let mut kinds = Vec::new();
    let mut retries = Vec::new();
    for _ in 0..6 {
        session.send("Which entities are in Shop?").unwrap();
        let events = events_until(&mut session, 10, turn_end);
        retries.extend(events.iter().filter_map(|e| match e {
            SessionEvent::Retrying { attempt, kind, .. } => Some((*attempt, *kind)),
            _ => None,
        }));
        match last_outcome(&events) {
            TurnOutcome::Failed(e) => kinds.push(e.kind()),
            other => panic!("{other:?}"),
        }
    }
    assert_eq!(
        kinds,
        vec![
            ErrorKind::Network,
            ErrorKind::InvalidCredential,
            ErrorKind::RateLimited,
            ErrorKind::ModelUnavailable,
            ErrorKind::RateLimited,
            ErrorKind::ModelUnavailable,
        ]
    );
    assert_eq!(retries, vec![(1, ErrorKind::Network)]);
    // The key went in the sign-in line alone, and nowhere else.
    let lines = world.stdin(4);
    assert!(lines[2].contains(CANARY) && lines[2].contains("\"type\":\"apiKey\""), "{}", lines[2]);
    assert_eq!(lines.iter().filter(|l| l.contains(CANARY)).count(), 1);
    for file in ["args", "env"] {
        assert!(!std::fs::read_to_string(world.base.join(file)).unwrap().contains(CANARY), "the key is in the fake's {file}");
    }
    assert!(!format!("{session:?}").contains(CANARY));
    drop(session);
    world.remove();
}

/// Stop during a turn sends one `turn/interrupt` for that turn; the turn ends stopped.
#[test]
fn a_stopped_turn_replays_from_the_fixture() {
    let world = World::new("replay-interrupt", &replay("doc-turn-interrupt.jsonl", &thread_prelude()));
    let mut session = world.session(config(Start::New, Duration::from_secs(5)), chatgpt());
    session.send("Describe every class in detail.").unwrap();
    events_until(&mut session, 10, |e| matches!(e, SessionEvent::TextDelta { .. }));
    session.stop().unwrap();
    session.stop().unwrap();
    let events = events_until(&mut session, 10, turn_end);
    assert_eq!(*last_outcome(&events), TurnOutcome::Stopped);
    let lines = world.stdin(5);
    let interrupt: Value = serde_json::from_str(&lines[4]).unwrap();
    assert_eq!(interrupt["method"], "turn/interrupt");
    assert_eq!(interrupt["params"], json!({"threadId": THREAD, "turnId": "019a0000-0000-7000-8000-0000000000a1"}));
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(world.methods(5).iter().filter(|m| *m == "turn/interrupt").count(), 1, "one stop request a turn");
    drop(session);
    world.remove();
}

/// Approval requests and elicitations are declined with answers the contract accepts, and the turn
/// completes.
#[test]
fn approvals_are_declined() {
    let world = World::new("replay-approvals", &replay("doc-approval-declined.jsonl", &thread_prelude()));
    let mut session = world.session(config(Start::New, Duration::from_secs(5)), chatgpt());
    session.send("What is in the project folder?").unwrap();
    let events = events_until(&mut session, 10, turn_end);
    assert_eq!(*last_outcome(&events), TurnOutcome::Completed);
    let lines = world.stdin(6);
    let contract = Contract::load();
    let answers: Vec<Value> = lines.iter().map(|l| serde_json::from_str::<Value>(l).unwrap()).filter(|v| v.get("result").is_some()).collect();
    assert_eq!(answers.len(), 2, "{lines:?}");
    assert_eq!(answers[0], json!({"id": 0, "result": {"decision": "decline"}}));
    assert!(contract.fits_result("item/commandExecution/requestApproval", &answers[0]["result"]));
    assert_eq!(answers[1], json!({"id": 1, "result": {"action": "decline", "content": null, "_meta": null}}));
    assert!(contract.fits_result("mcpServer/elicitation/request", &answers[1]["result"]));
    for (method, answer) in [
        ("item/permissions/requestApproval", json!({"permissions": {}, "scope": "turn"})),
        ("item/fileChange/requestApproval", json!({"decision": "decline"})),
    ] {
        assert!(contract.fits_result(method, &answer), "{method}");
    }
    drop(session);
    world.remove();
}

// --- accounts ------------------------------------------------------------------------------------

/// An API key reaches Codex in one sign-in line over standard input and nowhere else: not its
/// command line, not its environment, not a log line, not a `Debug`; the account reads back.
#[test]
fn an_api_key_signs_in_over_standard_input_only() {
    capture_logs();
    let body = replay("doc-login-api-key.jsonl", "hello");
    let world = World::new("account-key", &body);
    let mut account = world.account(CredentialStore::Ephemeral, Duration::from_secs(5));
    account.login_api_key(&canary(), Duration::from_secs(5)).unwrap();
    let state = account.read(false, Duration::from_secs(5)).unwrap();
    assert_eq!(state.account, Some(archforge_agents::codex::account::Account::ApiKey));
    assert_eq!(account.next_event(Duration::from_secs(1)), Some(AccountEvent::LoginCompleted { login_id: None, outcome: LoginOutcome::SignedIn }));
    assert_eq!(account.next_event(Duration::from_secs(1)), Some(AccountEvent::AccountUpdated { auth_mode: Some("apikey".into()) }));
    let lines = world.stdin(4);
    assert_eq!(lines.iter().filter(|l| l.contains(CANARY)).count(), 1);
    let args = std::fs::read_to_string(world.base.join("args")).unwrap();
    assert!(args.contains("cli_auth_credentials_store=\"ephemeral\"") && !args.contains(CANARY), "{args}");
    let env = std::fs::read_to_string(world.base.join("env")).unwrap();
    assert!(!env.contains(CANARY));
    assert!(env.contains(&format!("CODEX_HOME={}", world.home.path().display())), "the shared private CODEX_HOME: {env}");
    assert!(!format!("{account:?}{account:#?}").contains(CANARY));
    // A ChatGPT store never takes a key.
    drop(account);
    let world2 = World::new("account-key-store", "hello\ndrain");
    let mut keyring = world2.account(CredentialStore::Keyring, Duration::from_secs(5));
    assert!(matches!(keyring.login_api_key(&canary(), Duration::from_secs(1)), Err(AgentError::InvalidSetting { .. })));
    drop(keyring);
    assert!(!LOG.lock().unwrap_or_else(PoisonError::into_inner).iter().any(|l| l.contains(CANARY)), "the key reached the log");
    world2.remove();
    world.remove();
}

/// A browser sign-in answers its address and login id, and its end comes as an event.
#[test]
fn a_browser_sign_in_returns_its_address() {
    let world = World::new("account-browser", &replay("doc-login-chatgpt-browser.jsonl", "hello"));
    let mut account = world.account(CredentialStore::Keyring, Duration::from_secs(5));
    let login = account.login_chatgpt(Duration::from_secs(5)).unwrap();
    assert_eq!(login.login_id.as_str(), "019a0000-0000-7000-8000-00000000c001");
    assert_eq!(login.auth_url, "https://auth.openai.com/oauth/authorize?PLACEHOLDER");
    assert!(!format!("{login:?}").contains("PLACEHOLDER"));
    let event = account.next_event(Duration::from_secs(5)).unwrap();
    assert!(
        matches!(event, AccountEvent::LoginCompleted { outcome: LoginOutcome::SignedIn, login_id: Some(ref id) } if id == &login.login_id),
        "{event:?}"
    );
    let state = account.read(false, Duration::from_secs(5)).unwrap();
    assert!(matches!(state.account, Some(archforge_agents::codex::account::Account::ChatGpt { .. })));
    assert!(account.login_chatgpt(Duration::from_millis(50)).is_err(), "the fake answers nothing more");
    drop(account);
    let world2 = World::new("account-browser-store", "hello\ndrain");
    let mut ephemeral = world2.account(CredentialStore::Ephemeral, Duration::from_secs(5));
    assert!(matches!(ephemeral.login_chatgpt(Duration::from_secs(1)), Err(AgentError::InvalidSetting { .. })));
    drop(ephemeral);
    world2.remove();
    world.remove();
}

/// A device-code sign-in shows its address and code; cancelling it ends it as cancelled ("Login
/// was not completed").
#[test]
fn a_cancelled_device_code_sign_in_reads_as_cancelled() {
    let world = World::new("account-device", &replay("doc-login-chatgpt-device-code-cancelled.jsonl", "hello"));
    let mut account = world.account(CredentialStore::File, Duration::from_secs(5));
    let login = account.login_device_code(Duration::from_secs(5)).unwrap();
    assert_eq!((login.verification_url.as_str(), login.user_code.as_str()), ("https://auth.openai.com/codex/device", "ABCD-EFGH"));
    assert!(!format!("{login:?}{login:#?}").contains("ABCD"));
    assert_eq!(account.cancel_login(&login.login_id, Duration::from_secs(5)).unwrap(), CancelStatus::Canceled);
    let event = account.next_event(Duration::from_secs(5)).unwrap();
    assert_eq!(event, AccountEvent::LoginCompleted { login_id: Some(login.login_id.clone()), outcome: LoginOutcome::Cancelled });
    let lines = world.stdin(4);
    assert_eq!(serde_json::from_str::<Value>(&lines[3]).unwrap()["params"], json!({"loginId": "019a0000-0000-7000-8000-00000000c002"}));
    let args = std::fs::read_to_string(world.base.join("args")).unwrap();
    assert!(args.contains("cli_auth_credentials_store=\"file\""), "{args}");
    drop(account);
    world.remove();
}

/// A sign-in answer Archforge refuses is cancelled with Codex's own login id, exactly as given, in
/// a second request; a login id that fails the check is never sent back, so the call makes one
/// request.
#[test]
fn a_refused_sign_in_is_cancelled_only_with_a_checked_login_id() {
    let good_id = "019a0000-0000-7000-8000-00000000C0DE";
    let body = format!(
        "hello\n\
         req\nok \"$id\" '{{\"type\":\"chatgpt\",\"loginId\":\"{good_id}\",\"authUrl\":\"http://evil.example/\"}}'\n\
         req\nok \"$id\" '{{\"status\":\"canceled\"}}'\n\
         req\nok \"$id\" '{{\"type\":\"chatgptDeviceCode\",\"loginId\":\"not-a-uuid\",\"verificationUrl\":\"https://auth.openai.com/codex/device\",\"userCode\":\"AB CD\"}}'\n\
         req\nok \"$id\" '{{\"type\":\"chatgptDeviceCode\",\"loginId\":\"{good_id}\",\"verificationUrl\":\"https://auth.openai.com/codex/device\",\"userCode\":\"AB<script>\"}}'\n\
         req\nok \"$id\" '{{\"status\":\"notFound\"}}'\n\
         drain"
    );
    let world = World::new("account-refused", &body);
    let mut account = world.account(CredentialStore::Keyring, Duration::from_secs(5));
    let error = account.login_chatgpt(Duration::from_secs(5)).unwrap_err();
    assert!(matches!(&error, AgentError::Unreadable { reason, .. } if reason.contains("https")), "{error:?}");
    assert_eq!(world.methods(4), vec!["initialize", "initialized", "account/login/start", "account/login/cancel"]);
    assert_eq!(serde_json::from_str::<Value>(&world.stdin(4)[3]).unwrap()["params"]["loginId"], good_id, "the raw id, exactly");
    let error = account.login_device_code(Duration::from_secs(5)).unwrap_err();
    assert!(matches!(&error, AgentError::Unreadable { reason, .. } if reason.contains("UUID")), "{error:?}");
    let error = account.login_device_code(Duration::from_secs(5)).unwrap_err();
    assert!(matches!(&error, AgentError::Unreadable { reason, .. } if reason.contains("device code")), "{error:?}");
    // One request for the unchecked id, two for the checked one: no cancel between the two starts.
    assert_eq!(
        world.methods(7),
        vec![
            "initialize",
            "initialized",
            "account/login/start",
            "account/login/cancel",
            "account/login/start",
            "account/login/start",
            "account/login/cancel"
        ]
    );
    drop(account);
    world.remove();
}

/// A browser sign-in that is cancelled ends as cancelled: Codex 0.166.0 says "Login server error:
/// Login was not completed" for it.
#[test]
fn a_cancelled_browser_sign_in_reads_as_cancelled() {
    let id = "019a0000-0000-7000-8000-00000000c004";
    let body = format!(
        "hello\nreq\nok \"$id\" '{{\"type\":\"chatgpt\",\"loginId\":\"{id}\",\"authUrl\":\"https://auth.openai.com/x\"}}'\n\
         req\nok \"$id\" '{{\"status\":\"canceled\"}}'\n\
         say '{{\"method\":\"account/login/completed\",\"params\":{{\"loginId\":\"{id}\",\"success\":false,\"error\":\"Login server error: Login was not completed\"}}}}'\n\
         say '{{\"method\":\"account/login/completed\",\"params\":{{\"loginId\":\"{id}\",\"success\":false,\"error\":\"Login server error: port 1455 is busy\"}}}}'\ndrain"
    );
    let world = World::new("account-browser-cancel", &body);
    let mut account = world.account(CredentialStore::Keyring, Duration::from_secs(5));
    let login = account.login_chatgpt(Duration::from_secs(5)).unwrap();
    assert_eq!(account.cancel_login(&login.login_id, Duration::from_secs(5)).unwrap(), CancelStatus::Canceled);
    assert_eq!(
        account.next_event(Duration::from_secs(5)),
        Some(AccountEvent::LoginCompleted { login_id: Some(login.login_id.clone()), outcome: LoginOutcome::Cancelled })
    );
    let failed = account.next_event(Duration::from_secs(5));
    assert!(
        matches!(failed, Some(AccountEvent::LoginCompleted { outcome: LoginOutcome::Failed { ref detail }, .. }) if detail.contains("1455")),
        "{failed:?}"
    );
    drop(account);
    world.remove();
}

/// The key an account process sends is scrubbed from what Codex writes back: an error answer
/// echoing it and the error stream's tail show `<redacted>`.
#[test]
fn an_echoed_key_is_scrubbed_from_the_account_process() {
    capture_logs();
    let body = r#"hello
req
key=$(printf '%s\n' "$line" | sed -n 's/.*"apiKey":"\([^"]*\)".*/\1/p')
printf 'rejected key %s\n' "$key" >&2
err "$id" -32600 "invalid api key $key"
req
exit 2"#;
    let world = World::new("account-echo", body);
    let mut account = world.account(CredentialStore::Ephemeral, Duration::from_secs(5));
    let error = account.login_api_key(&canary(), Duration::from_secs(5)).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::InvalidCredential);
    let shown = format!("{error:?}{error:#?}{error}");
    assert!(!shown.contains(CANARY) && shown.contains("<redacted>"), "{shown}");
    let error = account.read(false, Duration::from_secs(5)).unwrap_err();
    let shown = format!("{error:?}{error:#?}{error}");
    assert!(!shown.contains(CANARY) && shown.contains("rejected key <redacted>"), "{shown}");
    assert!(!account.process().stderr_tail().contains(CANARY));
    assert!(!LOG.lock().unwrap_or_else(PoisonError::into_inner).iter().any(|l| l.contains(CANARY)), "the key reached the log");
    drop(account);
    world.remove();
}

/// An API-key sign-in that times out is not taken for a ChatGPT sign-in: its late answer sends no
/// cancel.
#[test]
fn a_late_key_sign_in_sends_no_cancel() {
    let body = "hello\nreq\nlate=$id\n/bin/sleep 0.3\nok \"$late\" '{\"type\":\"apiKey\"}'\nreq\nok \"$id\" '{\"account\":{\"type\":\"apiKey\"},\"requiresOpenaiAuth\":true}'\ndrain";
    capture_logs();
    let world = World::new("account-late-key", body);
    let mut account = world.account(CredentialStore::Ephemeral, Duration::from_millis(100));
    assert!(matches!(account.login_api_key(&canary(), Duration::from_secs(5)), Err(AgentError::TimedOut { .. })));
    std::thread::sleep(Duration::from_millis(400));
    let state = account.read(false, Duration::from_secs(5)).unwrap();
    assert!(state.account.is_some());
    assert_eq!(world.methods(4), vec!["initialize", "initialized", "account/login/start", "account/read"]);
    assert!(
        !LOG.lock().unwrap_or_else(PoisonError::into_inner).iter().any(|l| l.contains("a sign-in started late")),
        "a key's late answer is not handled as a ChatGPT sign-in"
    );
    drop(account);
    world.remove();
}

/// A sign-in whose start timed out but that Codex starts late is cancelled with its login id once
/// the late answer is read.
#[test]
fn a_sign_in_that_starts_late_is_cancelled() {
    let body = "hello\nreq\nlate=$id\n/bin/sleep 0.3\nok \"$late\" '{\"type\":\"chatgpt\",\"loginId\":\"019a0000-0000-7000-8000-00000000c003\",\"authUrl\":\"https://auth.openai.com/x\"}'\nreq\ndrain";
    let world = World::new("account-late-login", body);
    let mut account = world.account(CredentialStore::Keyring, Duration::from_millis(100));
    let error = account.login_chatgpt(Duration::from_secs(5)).unwrap_err();
    assert!(matches!(error, AgentError::TimedOut { request: "account/login/start", .. }), "{error:?}");
    let deadline = Instant::now() + Duration::from_secs(5);
    while world.stdin(0).len() < 4 && Instant::now() < deadline {
        account.next_event(Duration::from_millis(20));
    }
    let lines = world.stdin(4);
    assert_eq!(
        serde_json::from_str::<Value>(&lines[3]).unwrap(),
        json!({"id": 3, "method": "account/login/cancel", "params": {"loginId": "019a0000-0000-7000-8000-00000000c003"}})
    );
    drop(account);
    world.remove();
}

/// The model list follows its cursors to the end, passing each back unchanged; an empty, non-text,
/// repeated or oversized cursor fails the whole call.
#[test]
fn model_list_follows_cursors_and_refuses_bad_ones() {
    let model = |id: &str| format!("{{\"id\":\"{id}\",\"model\":\"{id}\",\"displayName\":\"{id}\",\"hidden\":false,\"isDefault\":false}}");
    let long = "c".repeat(64 * 1024 + 1);
    let page = |models: &str, cursor: &str| format!("req\nok \"$id\" '{{\"data\":[{models}],\"nextCursor\":{cursor}}}'\n");
    let mut body = String::from("hello\n");
    body.push_str(&page(&model("a"), "\"opaque cursor/1 ä\""));
    body.push_str(&page(&model("b"), "\"2\""));
    body.push_str(&page(&model("c"), "null"));
    for bad in ["\"\"", "7", &format!("\"{long}\"")] {
        body.push_str(&page(&model("x"), bad));
    }
    body.push_str(&page(&model("y"), "\"same\""));
    body.push_str(&page(&model("z"), "\"same\""));
    body.push_str("req\nok \"$id\" '{\"data\":[]}'\ndrain");
    let world = World::new("account-models", &body);
    let mut account = world.account(CredentialStore::Keyring, Duration::from_secs(5));
    let models = account.models(Duration::from_secs(5)).unwrap();
    assert_eq!(models.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(), vec!["a", "b", "c"]);
    let lines = world.stdin(5);
    assert_eq!(serde_json::from_str::<Value>(&lines[3]).unwrap()["params"]["cursor"], "opaque cursor/1 ä", "passed back unchanged");
    assert_eq!(serde_json::from_str::<Value>(&lines[4]).unwrap()["params"]["cursor"], "2");
    for why in ["empty", "not text", "longer", "repeats"] {
        let error = account.models(Duration::from_secs(5)).unwrap_err();
        assert!(matches!(&error, AgentError::Unreadable { reason, .. } if reason.contains(why)), "{why}: {error:?}");
    }
    assert!(account.models(Duration::from_secs(5)).unwrap().is_empty(), "a missing cursor ends the list");
    drop(account);
    world.remove();
}

/// A call Codex never answers because it ends gets the end's reason (its exit code and error tail);
/// the end stays queued as an event, and later calls fail with the same reason.
#[test]
fn a_call_codex_never_answers_gets_its_exit_reason() {
    let world = World::new("account-crash", "hello\nreq\nprintf 'fatal: the keyring is locked\\n' >&2\nexit 3");
    let mut account = world.account(CredentialStore::Keyring, Duration::from_secs(5));
    let error = account.read(false, Duration::from_secs(5)).unwrap_err();
    assert!(matches!(&error, AgentError::Exited { code: Some(3), stderr_tail, .. } if stderr_tail.contains("keyring is locked")), "{error:?}");
    let event = account.next_event(Duration::ZERO);
    assert!(
        matches!(&event, Some(AccountEvent::Ended { exit: Some(exit), error: Some(AgentError::Exited { .. }) }) if exit.code == Some(3)),
        "{event:?}"
    );
    assert_eq!(account.next_event(Duration::ZERO), None);
    let later = account.read(false, Duration::from_secs(1)).unwrap_err();
    assert!(matches!(later, AgentError::Exited { code: Some(3), .. }), "a later call keeps the end's reason: {later:?}");
    drop(account);
    world.remove();
}

/// Only Codex's exact refresh sentences, with code -32603, make an expired sign-in.
#[test]
fn an_expired_sign_in_is_read_from_the_exact_sentence() {
    let sentence = archforge_agents::codex::EXPIRED_SIGN_IN_MESSAGES[2];
    let body = format!("hello\nreq\nerr \"$id\" -32603 '{sentence}'\nreq\nerr \"$id\" -32603 'Your refresh token was revoked.'\ndrain");
    let world = World::new("account-expired", &body);
    let mut account = world.account(CredentialStore::Keyring, Duration::from_secs(5));
    assert_eq!(account.read(true, Duration::from_secs(5)).unwrap_err().kind(), ErrorKind::SignInExpired);
    assert_eq!(account.read(true, Duration::from_secs(5)).unwrap_err().kind(), ErrorKind::Other);
    assert_eq!(serde_json::from_str::<Value>(&world.stdin(3)[2]).unwrap()["params"], json!({"refreshToken": true}));
    drop(account);
    world.remove();
}

/// The account process runs in a scratch folder of its own, removed when it is dropped, and also
/// when its start fails.
#[test]
fn the_account_folder_goes_with_the_account() {
    let world = World::new("account-folder", "hello\ndrain");
    let account = world.account(CredentialStore::Keyring, Duration::from_secs(5));
    let folder = account.folder().unwrap().path().to_path_buf();
    assert!(folder.starts_with(world.workspace.root()) && folder.file_name().unwrap().to_string_lossy().starts_with("codex-"));
    let env = std::fs::read_to_string(world.base.join("env")).unwrap();
    assert!(env.contains(&format!("HOME={}", account.folder().unwrap().home().display())), "{env}");
    drop(account);
    assert!(!folder.exists(), "the folder went with the account");
    world.remove();
    // A handshake that fails: the folder goes too.
    let world = World::new("account-folder-fail", "req\nok \"$id\" '{}'\ndrain");
    let error = CodexAccount::start(
        &world.runtime,
        &world.workspace,
        &world.home,
        CredentialStore::Keyring,
        Limits::default(),
        Duration::from_secs(5),
        Duration::from_secs(5),
    )
    .unwrap_err();
    assert!(matches!(error, AgentError::Unreadable { request: "initialize", .. }), "{error:?}");
    let left: Vec<String> = std::fs::read_dir(world.workspace.root())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with("codex-") && n != "codex-home")
        .collect();
    assert!(left.is_empty(), "{left:?}");
    world.remove();
}

/// A zero-timeout poll of the account reads past lines that mean nothing to the event behind them.
#[test]
fn account_zero_timeout_reads_past_lines_without_events() {
    let mut body = String::from("hello\n");
    for _ in 0..10 {
        body.push_str("say '{\"method\":\"remoteControl/status/changed\",\"params\":{\"status\":\"disabled\"}}'\n");
    }
    body.push_str("say '{\"method\":\"account/updated\",\"params\":{\"authMode\":null,\"planType\":null}}'\nhang");
    let world = World::new("account-zero", &body);
    let mut account = world.account(CredentialStore::Keyring, Duration::from_secs(5));
    std::thread::sleep(Duration::from_millis(500));
    assert_eq!(account.next_event(Duration::ZERO), Some(AccountEvent::AccountUpdated { auth_mode: None }));
    drop(account);
    world.remove();
}

// --- sessions ------------------------------------------------------------------------------------

/// A zero-timeout poll of the session reads past lines that mean nothing to the event behind them,
/// and never waits.
#[test]
fn a_zero_timeout_poll_reads_past_lines_without_events() {
    let mut body = String::from("hello\nreq\n/bin/sleep 0.3\n");
    for _ in 0..10 {
        body.push_str("say '{\"method\":\"remoteControl/status/changed\",\"params\":{\"status\":\"disabled\"}}'\n");
    }
    body.push_str(&format!("ok \"$id\" '{{\"thread\":{{\"id\":\"{THREAD}\"}},\"model\":\"m\"}}'\nhang"));
    let world = World::new("session-zero", &body);
    let mut session = world.session(config(Start::New, Duration::from_secs(5)), chatgpt());
    session.send("hello").unwrap();
    assert_eq!(world.pump(&mut session, 3), Vec::new());
    // Ten lines without meaning, then the thread's answer: all wait in the pipe.
    std::thread::sleep(Duration::from_millis(500));
    let started = Instant::now();
    let mut event = None;
    for _ in 0..4 {
        event = session.next_event(Duration::ZERO);
        if event.is_some() {
            break;
        }
    }
    assert!(started.elapsed() < Duration::from_millis(100), "a zero timeout waited {:?}", started.elapsed());
    assert!(matches!(event, Some(SessionEvent::Started { .. })), "{event:?}");
    drop(session);
    world.remove();
}

/// No call waits past its caller's deadline: a silent Codex holds `next_event` for its timeout
/// alone, and an account call for its own timeout even with a long request timeout.
#[test]
fn no_call_waits_past_its_deadline() {
    let world = World::new("deadline", "hang");
    let mut session = world.session(config(Start::New, Duration::from_secs(60)), chatgpt());
    for wait in [Duration::ZERO, Duration::from_millis(30), Duration::from_millis(150)] {
        let started = Instant::now();
        assert_eq!(session.next_event(wait), None);
        assert!(started.elapsed() < wait + Duration::from_millis(60), "{wait:?} took {:?}", started.elapsed());
    }
    drop(session);
    world.remove();
    let world = World::new("deadline-account", "hello\nhang");
    let mut account = world.account(CredentialStore::Keyring, Duration::from_secs(60));
    let started = Instant::now();
    let error = account.read(false, Duration::from_millis(100)).unwrap_err();
    assert!(matches!(error, AgentError::TimedOut { request: "account/read", .. }), "{error:?}");
    assert!(started.elapsed() < Duration::from_millis(250), "{:?}", started.elapsed());
    drop(account);
    world.remove();
}

/// A crash reports its exit code and the error stream's tail, in the open turn's failure and in the
/// end.
#[test]
fn a_crash_reports_its_code_and_error_tail() {
    let body = format!("{}\nreq\nprintf 'thread panicked: out of memory\\n' >&2\nexit 7", thread_prelude());
    let world = World::new("crash", &body);
    let mut session = world.session(config(Start::New, Duration::from_secs(5)), chatgpt());
    session.send("hello").unwrap();
    let events = events_until(&mut session, 10, ended);
    assert!(
        matches!(last_outcome(&events), TurnOutcome::Failed(AgentError::Exited { code: Some(7), stderr_tail, .. }) if stderr_tail.contains("out of memory")),
        "{events:?}"
    );
    let Some(SessionEvent::Ended { exit: Some(exit), error: Some(error) }) = events.last() else { panic!("{events:?}") };
    assert_eq!(exit.code, Some(7));
    assert!(error.to_string().contains("out of memory"), "{error}");
    assert_eq!(session.next_event(Duration::ZERO), None);
    assert!(matches!(session.send("again"), Err(AgentError::InputClosed { .. })));
    // The question was sent, so the copy keeps it.
    let copy: Value = serde_json::from_slice(&std::fs::read(world.folder.path().join(COPY_FILE)).unwrap()).unwrap();
    assert_eq!(copy["messages"], json!([{"role": "user", "text": "hello"}]));
    drop(session);
    world.remove();
}

/// A session ended for a fatal error ends a Codex that ignores its closed input and SIGTERM within
/// a bound, the end carrying the original reason; no poll waits past its own timeout meanwhile.
#[test]
fn a_failed_session_ends_within_a_bound() {
    let world = World::new("fail-bound", "trap '' TERM\nreq\nok \"$id\" '{\"codexHome\":\"/tmp\"}'\nhang");
    let mut session = world.session(config(Start::New, Duration::from_secs(5)), chatgpt());
    let started = Instant::now();
    let mut end = None;
    let mut polls = 0u32;
    while started.elapsed() < Duration::from_secs(5) {
        // Zero-timeout polls between the others: none waits while Codex is being ended.
        let wait = if polls.is_multiple_of(2) { Duration::ZERO } else { Duration::from_millis(50) };
        polls += 1;
        let call = Instant::now();
        let event = session.next_event(wait);
        assert!(call.elapsed() < wait + Duration::from_millis(70), "a poll of {wait:?} waited {:?}", call.elapsed());
        if let Some(SessionEvent::Ended { exit, error }) = event {
            end = Some((exit, error, started.elapsed()));
            break;
        }
    }
    let (exit, error, after) = end.expect("the session did not end");
    assert!(after < Duration::from_millis(1500), "it took {after:?}");
    assert!(matches!(error, Some(AgentError::Unreadable { request: "initialize", .. })), "{error:?}");
    assert_eq!(exit.and_then(|e| e.signal), Some(9), "SIGTERM is ignored, so SIGKILL ended it");
    drop(session);
    world.remove();
}

/// A Codex that closes its output but keeps running is terminated within a bound and the end says
/// so.
#[test]
fn a_codex_that_closes_its_output_but_runs_on_is_terminated() {
    let world = World::new("closed-output", "hello\nexec 1>&-\nhang");
    let mut session = world.session(config(Start::New, Duration::from_secs(5)), chatgpt());
    let started = Instant::now();
    let events = events_until(&mut session, 5, ended);
    assert!(started.elapsed() < Duration::from_millis(1500), "it took {:?}", started.elapsed());
    let Some(SessionEvent::Ended { error: Some(error), .. }) = events.last() else { panic!("{events:?}") };
    assert!(error.to_string().contains("closed its output"), "{error}");
    drop(session);
    world.remove();
}

/// The shell lines of the late-stop scenarios: turn A runs and is stopped; Codex ends it at once
/// without answering the stop; turn B starts and is stopped too, Codex accepting that stop; then
/// `a_stop` happens to A's stop request (a refusal, or nothing); B writes on and is never ended.
fn late_stop_body(a_stop: &str) -> String {
    let delta = |turn: &str, text: &str| {
        format!(
            "say '{{\"method\":\"item/agentMessage/delta\",\"params\":{{\"threadId\":\"{THREAD}\",\"turnId\":\"{turn}\",\"itemId\":\"i\",\"delta\":\"{text}\"}}}}'\n"
        )
    };
    let mut body = thread_prelude();
    body.push_str("\nreq\nok \"$id\" '{\"turn\":{\"id\":\"A\",\"items\":[],\"status\":\"inProgress\"}}'\n");
    body.push_str(&delta("A", "a"));
    body.push_str("req\nstop_a=$id\n");
    body.push_str(&format!("say '{{\"method\":\"turn/completed\",\"params\":{{\"threadId\":\"{THREAD}\",\"turn\":{{\"id\":\"A\",\"items\":[],\"status\":\"interrupted\"}}}}}}'\n"));
    body.push_str("req\nok \"$id\" '{\"turn\":{\"id\":\"B\",\"items\":[],\"status\":\"inProgress\"}}'\n");
    body.push_str(&delta("B", "b"));
    body.push_str("req\nok \"$id\" '{}'\n");
    body.push_str(a_stop);
    body.push_str(&delta("B", "still working"));
    body.push_str("hang");
    body
}

/// Drives the late-stop scenario and checks B: it starts well within A's stop request's time, is
/// stopped with Codex's acceptance, and then runs on past three times the request timeout with no
/// end, as nothing about A's stop belongs to it.
fn late_stop_leaves_b_running(name: &str, a_stop: &str, timeout: Duration) {
    let world = World::new(name, &late_stop_body(a_stop));
    let mut session = world.session(config(Start::New, timeout), chatgpt());
    session.send("A").unwrap();
    events_until(&mut session, 5, |e| matches!(e, SessionEvent::TextDelta { .. }));
    let stopped_at = Instant::now();
    session.stop().unwrap();
    assert_eq!(*last_outcome(&events_until(&mut session, 5, turn_end)), TurnOutcome::Stopped);
    session.send("B").unwrap();
    events_until(&mut session, 5, |e| matches!(e, SessionEvent::TextDelta { .. }));
    session.stop().unwrap();
    events_until(&mut session, 5, |e| matches!(e, SessionEvent::TextDelta { text } if text == "still working"));
    // Everything about A's stop that was written came before this delta, inside A's request time.
    assert!(stopped_at.elapsed() < timeout / 2, "B ran {:?} after A's stop, not well within its request time", stopped_at.elapsed());
    let events = events_for(&mut session, timeout * 3);
    assert!(stopped_at.elapsed() > timeout * 3, "watched past A's expiry and past what would be B's deadline");
    assert!(!events.iter().any(|e| turn_end(e) || ended(e)), "{events:?}");
    assert!(session.turn_running(), "B still runs");
    assert!(matches!(session.send("C"), Err(AgentError::TurnRunning { .. })));
    assert_eq!(world.methods(7).iter().filter(|m| *m == "turn/interrupt").count(), 2, "one stop request a turn");
    drop(session);
    world.remove();
}

/// Turn A is stopped and Codex ends it at once, before it answers the stop. Turn B starts (and is
/// stopped, Codex accepting). Then A's stop is refused, after B started and well before A's stop
/// request expires: B keeps running with no deadline, as the refusal belongs to turn A.
#[test]
fn a_late_stop_refusal_never_touches_the_next_turn() {
    late_stop_leaves_b_running("late-refusal", "err \"$stop_a\" -32600 'no running turn to interrupt'\n", Duration::from_secs(1));
}

/// The same, but A's stop is never answered: its request expires while B runs, and B keeps running
/// with no deadline.
#[test]
fn a_late_stop_timeout_never_touches_the_next_turn() {
    late_stop_leaves_b_running("late-timeout", "", Duration::from_secs(1));
}

/// A shell line writing a piece of turn A's answer, which tells a test that A was named.
fn delta_of_a() -> String {
    format!(
        "say '{{\"method\":\"item/agentMessage/delta\",\"params\":{{\"threadId\":\"{THREAD}\",\"turnId\":\"A\",\"itemId\":\"i\",\"delta\":\"a\"}}}}'\n"
    )
}

/// A refused stop keeps the turn open (send stays refused) for one more request timeout; when Codex
/// does not end it, the session is ended, naming `Start::Resume`.
#[test]
fn a_refused_stop_ends_the_session_naming_resume() {
    let timeout = Duration::from_millis(300);
    let body = format!(
        "{}\nreq\nok \"$id\" '{{\"turn\":{{\"id\":\"A\",\"items\":[],\"status\":\"inProgress\"}}}}'\n{}req\nerr \"$id\" -32600 'cannot interrupt now'\ndrain",
        thread_prelude(),
        delta_of_a()
    );
    let world = World::new("refused-stop", &body);
    let mut session = world.session(config(Start::New, timeout), chatgpt());
    session.send("A").unwrap();
    events_until(&mut session, 5, |e| matches!(e, SessionEvent::TextDelta { .. }));
    let stopped_at = Instant::now();
    session.stop().unwrap();
    let early = events_for(&mut session, timeout / 2);
    assert!(!early.iter().any(|e| turn_end(e) || ended(e)), "{early:?}");
    assert!(matches!(session.send("B"), Err(AgentError::TurnRunning { .. })), "send stays refused while Codex may run the turn");
    let events = events_until(&mut session, 5, ended);
    assert!(stopped_at.elapsed() >= timeout, "{:?}", stopped_at.elapsed());
    let TurnOutcome::Failed(error) = last_outcome(&events) else { panic!("{events:?}") };
    assert!(error.to_string().contains("Start::Resume"), "{error}");
    let Some(SessionEvent::Ended { error: Some(end), .. }) = events.last() else { panic!("{events:?}") };
    assert!(end.to_string().contains("Start::Resume"), "{end}");
    drop(session);
    world.remove();
}

/// A refused stop followed by Codex ending the turn closes it normally, as stopped.
#[test]
fn a_refused_stop_then_a_completed_turn_closes_normally() {
    let timeout = Duration::from_millis(300);
    let body = format!(
        "{}\nreq\nok \"$id\" '{{\"turn\":{{\"id\":\"A\",\"items\":[],\"status\":\"inProgress\"}}}}'\n{}req\nerr \"$id\" -32600 'cannot interrupt now'\n/bin/sleep 0.1\n\
         say '{{\"method\":\"turn/completed\",\"params\":{{\"threadId\":\"{THREAD}\",\"turn\":{{\"id\":\"A\",\"items\":[],\"status\":\"failed\",\"error\":null}}}}}}'\nhang",
        thread_prelude(),
        delta_of_a()
    );
    let world = World::new("refused-then-done", &body);
    let mut session = world.session(config(Start::New, timeout), chatgpt());
    session.send("A").unwrap();
    events_until(&mut session, 5, |e| matches!(e, SessionEvent::TextDelta { .. }));
    session.stop().unwrap();
    assert_eq!(*last_outcome(&events_until(&mut session, 5, turn_end)), TurnOutcome::Stopped);
    let later = events_for(&mut session, timeout * 2);
    assert!(!later.iter().any(ended), "{later:?}");
    assert!(session.send("B").is_ok());
    drop(session);
    world.remove();
}

/// A stop asked for before Codex names the turn is sent once the turn is named; a turn that stays
/// unnamed after being accepted has its thread released after a request timeout and ends stopped.
#[test]
fn a_stop_waits_for_the_turn_s_name_or_releases_its_thread() {
    let timeout = Duration::from_millis(300);
    let body = format!(
        "{}\nreq\n/bin/sleep 0.2\nok \"$id\" '{{\"turn\":{{\"status\":\"inProgress\"}}}}'\n\
         say '{{\"method\":\"turn/started\",\"params\":{{\"threadId\":\"{THREAD}\",\"turn\":{{\"id\":\"N\",\"items\":[],\"status\":\"inProgress\"}}}}}}'\n\
         req\nok \"$id\" '{{}}'\n\
         say '{{\"method\":\"turn/completed\",\"params\":{{\"threadId\":\"{THREAD}\",\"turn\":{{\"id\":\"N\",\"items\":[],\"status\":\"interrupted\"}}}}}}'\n\
         req\nok \"$id\" '{{\"turn\":{{\"status\":\"inProgress\"}}}}'\n\
         req\nok \"$id\" '{{\"status\":\"unsubscribed\"}}'\nhang",
        thread_prelude()
    );
    let world = World::new("unnamed-stop", &body);
    let mut session = world.session(config(Start::New, timeout), chatgpt());
    session.send("A").unwrap();
    world.pump(&mut session, 4);
    session.stop().unwrap();
    assert_eq!(*last_outcome(&events_until(&mut session, 5, turn_end)), TurnOutcome::Stopped);
    let lines = world.stdin(5);
    assert_eq!(serde_json::from_str::<Value>(&lines[4]).unwrap()["params"], json!({"threadId": THREAD, "turnId": "N"}));
    // Turn B is accepted but never named: the stop releases its thread.
    session.send("B").unwrap();
    world.pump(&mut session, 6);
    events_for(&mut session, Duration::from_millis(100));
    session.stop().unwrap();
    let events = events_until(&mut session, 5, turn_end);
    assert_eq!(*last_outcome(&events), TurnOutcome::Stopped);
    assert_eq!(world.methods(7)[6], "thread/unsubscribe");
    assert_eq!(serde_json::from_str::<Value>(&world.stdin(7)[6]).unwrap()["params"], json!({"threadId": THREAD}));
    drop(session);
    world.remove();
}

/// Threads a timeout abandons are released: one a late `thread/start` answer names, one whose copy
/// could not be handed over in time (at once), and one a late `turn/start` turns out to run a turn
/// on (after that turn is stopped).
#[test]
fn timed_out_threads_are_released() {
    let timeout = Duration::from_millis(250);
    // A late thread/start answer.
    let body = "hello\nreq\nfirst=$id\n/bin/sleep 0.5\nok \"$first\" '{\"thread\":{\"id\":\"LATE\"},\"model\":\"m\"}'\nreq\nreq\ndrain";
    let world = World::new("late-thread", body);
    let mut session = world.session(config(Start::New, timeout), chatgpt());
    session.send("A").unwrap();
    let events = events_until(&mut session, 5, turn_end);
    assert!(matches!(last_outcome(&events), TurnOutcome::Failed(AgentError::TimedOut { request: "thread/start", .. })), "{events:?}");
    world.pump(&mut session, 4);
    assert_eq!(world.methods(4)[3], "thread/unsubscribe");
    assert_eq!(serde_json::from_str::<Value>(&world.stdin(4)[3]).unwrap()["params"], json!({"threadId": "LATE"}));
    assert!(session.history().messages().is_empty(), "a turn never sent leaves the copy as it was");
    drop(session);
    world.remove();

    // A copy that cannot be handed over in time: released at once.
    let body = "hello\nreq\nok \"$id\" '{\"thread\":{\"id\":\"T2\"},\"model\":\"m\"}'\nreq\nreq\ndrain";
    let world = World::new("late-inject", body);
    let copy = json!({"messages": [{"role": "user", "text": "earlier"}, {"role": "assistant", "text": "answer"}], "version": 1}).to_string();
    std::fs::write(world.folder.path().join(COPY_FILE), copy).unwrap();
    let mut session = world.session(config(Start::Resume, timeout), chatgpt());
    session.send("again").unwrap();
    let events = events_until(&mut session, 5, turn_end);
    assert!(matches!(last_outcome(&events), TurnOutcome::Failed(AgentError::TimedOut { request: "thread/inject_items", .. })), "{events:?}");
    let lines = world.stdin(5);
    let inject: Value = serde_json::from_str(&lines[3]).unwrap();
    assert_eq!(
        inject["params"]["items"],
        json!([
            {"type": "message", "role": "user", "content": [{"type": "input_text", "text": "earlier"}]},
            {"type": "message", "role": "assistant", "content": [{"type": "output_text", "text": "answer"}]},
        ])
    );
    assert_eq!(
        serde_json::from_str::<Value>(&lines[4]).unwrap(),
        json!({"id": serde_json::from_str::<Value>(&lines[4]).unwrap()["id"], "method": "thread/unsubscribe", "params": {"threadId": "T2"}})
    );
    assert_eq!(session.history().messages().len(), 2, "the question was never sent");
    drop(session);
    world.remove();

    // A late turn/start: its turn is stopped, then its thread released; the question stays.
    let body = format!(
        "{}\nreq\nlate=$id\n/bin/sleep 0.5\nok \"$late\" '{{\"turn\":{{\"id\":\"LT\",\"items\":[],\"status\":\"inProgress\"}}}}'\nreq\nok \"$id\" '{{}}'\nreq\ndrain",
        thread_prelude()
    );
    let world = World::new("late-turn", &body);
    let mut session = world.session(config(Start::New, timeout), chatgpt());
    session.send("question").unwrap();
    let events = events_until(&mut session, 5, turn_end);
    assert!(matches!(last_outcome(&events), TurnOutcome::Failed(AgentError::TimedOut { request: "turn/start", .. })), "{events:?}");
    world.pump(&mut session, 6);
    let lines = world.stdin(6);
    assert_eq!(serde_json::from_str::<Value>(&lines[4]).unwrap()["params"], json!({"threadId": THREAD, "turnId": "LT"}));
    assert_eq!(serde_json::from_str::<Value>(&lines[5]).unwrap()["params"], json!({"threadId": THREAD}));
    assert_eq!(world.methods(6)[5], "thread/unsubscribe");
    assert_eq!(session.history().messages().iter().map(|m| m.text.as_str()).collect::<Vec<_>>(), vec!["question"], "Codex may have it");
    drop(session);
    world.remove();

    // A late turn/start that names no turn: its thread is released at once; one never answered is
    // released when it is forgotten.
    let body = format!("{}\nreq\nlate=$id\n/bin/sleep 0.5\nerr \"$late\" -32600 'too late'\nreq\ndrain", thread_prelude());
    let world = World::new("late-unnamed", &body);
    let mut session = world.session(config(Start::New, timeout), chatgpt());
    session.send("question").unwrap();
    events_until(&mut session, 5, turn_end);
    world.pump(&mut session, 5);
    assert_eq!(world.methods(5)[4], "thread/unsubscribe");
    drop(session);
    world.remove();

    let body = format!("{}\nreq\nreq\ndrain", thread_prelude());
    let world = World::new("late-forgotten", &body);
    let short = Duration::from_millis(50);
    let mut session = world.session(config(Start::New, short), chatgpt());
    session.send("question").unwrap();
    events_until(&mut session, 5, turn_end);
    world.pump(&mut session, 5);
    assert_eq!(world.methods(5)[4], "thread/unsubscribe", "released when forgotten after ten request timeouts");
    drop(session);
    world.remove();
}

/// Tool server states that come before `thread/start`'s answer are held, and only those naming the
/// answered thread are delivered; a later state is reported again with `Started`.
#[test]
fn server_states_before_the_thread_are_held_and_filtered() {
    let status = |thread: &str, name: &str, state: &str| {
        format!(
            "say '{{\"method\":\"mcpServer/startupStatus/updated\",\"params\":{{\"threadId\":{thread},\"name\":\"{name}\",\"status\":\"{state}\",\"error\":null,\"failureReason\":null}}}}'\n"
        )
    };
    let mut body = String::from("hello\nreq\n");
    body.push_str(&status(&format!("\"{THREAD}\""), "archforge", "starting"));
    body.push_str(&status("\"other-thread\"", "intruder", "ready"));
    body.push_str(&status("null", "nobody", "ready"));
    body.push_str(&format!("ok \"$id\" '{{\"thread\":{{\"id\":\"{THREAD}\"}},\"model\":\"m\"}}'\n"));
    body.push_str("req\nok \"$id\" '{\"turn\":{\"id\":\"A\",\"items\":[],\"status\":\"inProgress\"}}'\n");
    body.push_str(&status(&format!("\"{THREAD}\""), "archforge", "ready"));
    body.push_str("hang");
    let world = World::new("held-status", &body);
    let mut session = world.session(config(Start::New, Duration::from_secs(5)), chatgpt());
    session.send("A").unwrap();
    let started: Vec<Vec<(String, String)>> = events_for(&mut session, Duration::from_millis(800))
        .into_iter()
        .filter_map(|e| match e {
            SessionEvent::Started { servers, .. } => Some(servers.into_iter().map(|s| (s.name, s.status)).collect()),
            _ => None,
        })
        .collect();
    assert_eq!(started, vec![vec![("archforge".to_owned(), "starting".to_owned())], vec![("archforge".to_owned(), "ready".to_owned())]]);
    drop(session);
    world.remove();

    // A start that fails drops what was held.
    let mut body = String::from("hello\nreq\n");
    body.push_str(&status(&format!("\"{THREAD}\""), "archforge", "starting"));
    body.push_str("err \"$id\" -32600 'no thread for you'\nreq\n");
    body.push_str(&format!("ok \"$id\" '{{\"thread\":{{\"id\":\"{THREAD}\"}},\"model\":\"m\"}}'\nhang"));
    let world = World::new("held-dropped", &body);
    let mut session = world.session(config(Start::New, Duration::from_secs(5)), chatgpt());
    session.send("A").unwrap();
    let events = events_until(&mut session, 5, turn_end);
    assert!(matches!(last_outcome(&events), TurnOutcome::Failed(AgentError::Refused { request: "thread/start", .. })), "{events:?}");
    session.send("B").unwrap();
    let events = events_until(&mut session, 5, |e| matches!(e, SessionEvent::Started { .. }));
    assert!(matches!(events.last(), Some(SessionEvent::Started { servers, .. }) if servers.is_empty()), "{events:?}");
    drop(session);
    world.remove();
}

/// A new conversation refuses a folder that already holds a copy; a resumed one hands the copy to
/// its new thread before the question, and a refused turn leaves the copy as it was.
#[test]
fn a_new_conversation_refuses_a_folder_with_a_copy_and_a_resume_hands_it_over() {
    let body = "hello\nreq\nok \"$id\" '{\"thread\":{\"id\":\"T\"},\"model\":\"m\"}'\nreq\nok \"$id\" '{}'\nreq\nerr \"$id\" -32600 'busy'\nhang";
    let world = World::new("resume", body);
    let copy = json!({"messages": [{"role": "user", "text": "earlier"}, {"role": "assistant", "text": "answer"}], "version": 1}).to_string();
    let path = world.folder.path().join(COPY_FILE);
    std::fs::write(&path, &copy).unwrap();
    let error = CodexSession::start(&world.runtime, &world.folder, &world.home, config(Start::New, Duration::from_secs(5)), chatgpt()).unwrap_err();
    assert!(matches!(&error, AgentError::Folder { problem, .. } if problem.contains("Start::Resume")), "{error:?}");
    let mut session = world.session(config(Start::Resume, Duration::from_secs(5)), chatgpt());
    assert_eq!(session.history().messages().len(), 2);
    session.send("refused question").unwrap();
    let events = events_until(&mut session, 5, turn_end);
    assert!(matches!(last_outcome(&events), TurnOutcome::Failed(AgentError::Refused { request: "turn/start", .. })), "{events:?}");
    assert_eq!(world.methods(5), vec!["initialize", "initialized", "thread/start", "thread/inject_items", "turn/start"]);
    assert_eq!(session.history().messages().len(), 2, "the refused question is not kept");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), copy, "the copy on disk is unchanged");
    assert_eq!(session.history().messages()[1].role, Role::Assistant);
    drop(session);
    world.remove();
}

/// A copy too long for one batch goes to the new thread in batches, one after another, each
/// waiting for the answer to the one before, in the copy's order.
#[test]
fn a_long_copy_is_handed_over_in_batches() {
    // The fake reads the long lines with `head`, which is safe here: the next line is only sent once
    // the one before is answered. It records each line's length and method.
    let body = r#"hello
req
ok "$id" '{"thread":{"id":"T"},"model":"m"}'
big() { line=$(/usr/bin/head -n 1); printf '%s\n' "$line" | /usr/bin/cut -c1-80 >> "$IN.big"; printf '%s\n' "${#line}" >> "$IN.len"; id=$(printf '%s\n' "$line" | /usr/bin/cut -c1-40 | sed -n 's/^{"id":\([0-9][0-9]*\),.*/\1/p'); }
big
ok "$id" '{}'
big
ok "$id" '{}'
big
hang"#;
    let world = World::new("batches", body);
    // 700 KiB of control characters is 4.2 MiB of JSON, past the half of the 8 MiB line limit a
    // batch takes: it goes alone; the two short messages after it share the second batch.
    let messages = vec![
        json!({"role": "user", "text": "\u{1}".repeat(700 * 1024)}),
        json!({"role": "assistant", "text": "short answer"}),
        json!({"role": "user", "text": "short question"}),
    ];
    std::fs::write(world.folder.path().join(COPY_FILE), json!({"messages": messages, "version": 1}).to_string()).unwrap();
    let mut session = world.session(config(Start::Resume, Duration::from_secs(5)), chatgpt());
    session.send("next").unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    let big = world.base.join("stdin.big");
    while std::fs::read_to_string(&big).unwrap_or_default().lines().count() < 3 && Instant::now() < deadline {
        session.next_event(Duration::from_millis(20));
    }
    let methods: Vec<String> = std::fs::read_to_string(&big)
        .unwrap()
        .lines()
        .map(|l| l.split("\"method\":\"").nth(1).and_then(|m| m.split('"').next()).unwrap_or_default().to_owned())
        .collect();
    assert_eq!(methods, vec!["thread/inject_items", "thread/inject_items", "turn/start"]);
    let sizes: Vec<usize> = std::fs::read_to_string(world.base.join("stdin.len")).unwrap().lines().map(|l| l.trim().parse().unwrap()).collect();
    assert!(sizes[0] > 4 * 1024 * 1024, "the long message alone: {sizes:?}");
    assert!(sizes[1] < 1024, "the two short ones together: {sizes:?}");
    assert!(session.damaged_copy().is_none());
    drop(session);
    world.remove();
}

/// `Debug` of a session never shows the key, while it is held for the handshake or after.
#[test]
fn a_session_s_debug_never_shows_the_key() {
    let world = World::new("session-debug", "hang");
    let session = world.session(config(Start::New, Duration::from_secs(5)), Auth::ApiKey(canary()));
    for shown in [format!("{session:?}"), format!("{session:#?}")] {
        assert!(!shown.contains(CANARY) && shown.contains("key_held: true"), "{shown}");
    }
    drop(session);
    world.remove();
}

/// A key Codex does not accept ends the session as an invalid credential.
#[test]
fn a_refused_key_ends_the_session() {
    let body = "hello\nreq\nok \"$id\" '{\"type\":\"apiKey\"}'\nsay '{\"method\":\"account/login/completed\",\"params\":{\"loginId\":null,\"success\":false,\"error\":\"bad key\"}}'\ndrain";
    let world = World::new("session-refused-key", body);
    let mut session = world.session(config(Start::New, Duration::from_secs(5)), Auth::ApiKey(canary()));
    session.send("hello").unwrap();
    let events = events_until(&mut session, 5, ended);
    assert!(matches!(last_outcome(&events), TurnOutcome::Failed(e) if e.kind() == ErrorKind::InvalidCredential), "{events:?}");
    assert!(matches!(events.last(), Some(SessionEvent::Ended { error: Some(e), .. }) if e.kind() == ErrorKind::InvalidCredential));
    drop(session);
    world.remove();
}

/// A copy that cannot be written is said by `copy_error`, which names no message, and clears once a
/// later write succeeds.
#[test]
fn a_failed_copy_write_is_said_until_one_succeeds() {
    let turn = |id: &str| {
        format!(
            "req\nok \"$id\" '{{\"turn\":{{\"id\":\"{id}\",\"items\":[],\"status\":\"inProgress\"}}}}'\n/bin/sleep 0.2\n\
             say '{{\"method\":\"turn/completed\",\"params\":{{\"threadId\":\"{THREAD}\",\"turn\":{{\"id\":\"{id}\",\"items\":[],\"status\":\"completed\"}}}}}}'\n"
        )
    };
    let body = format!("{}\n{}{}drain", thread_prelude(), turn("A"), turn("B"));
    let world = World::new("copy-error", &body);
    let mut session = world.session(config(Start::New, Duration::from_secs(5)), chatgpt());
    std::fs::set_permissions(world.folder.path(), std::fs::Permissions::from_mode(0o500)).unwrap();
    session.send("secret words").unwrap();
    events_until(&mut session, 5, turn_end);
    let error = session.copy_error().expect("the write into a read-only folder failed").clone();
    assert!(!format!("{error:?}{error}").contains("secret words"), "{error}");
    std::fs::set_permissions(world.folder.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    session.send("next").unwrap();
    events_until(&mut session, 5, turn_end);
    assert!(session.copy_error().is_none(), "a later write succeeded");
    assert!(world.folder.path().join(COPY_FILE).is_file());
    drop(session);
    world.remove();
}

/// A damaged copy is set aside at a resume, and the conversation goes on without it.
#[test]
fn a_damaged_copy_is_set_aside_at_a_resume() {
    let world = World::new("damaged", "hello\ndrain");
    let path = world.folder.path().join(COPY_FILE);
    std::fs::write(&path, b"{not a copy").unwrap();
    std::fs::write(world.folder.path().join(format!("{COPY_FILE}.damaged-1")), b"older").unwrap();
    let session = world.session(config(Start::Resume, Duration::from_secs(5)), chatgpt());
    assert_eq!(session.damaged_copy(), Some(world.folder.path().join(format!("{COPY_FILE}.damaged-2")).as_path()));
    assert!(session.history().messages().is_empty());
    assert_eq!(std::fs::read(world.folder.path().join(format!("{COPY_FILE}.damaged-1"))).unwrap(), b"older");
    drop(session);
    world.remove();
}

/// Unknown notifications are skipped and counted; lines that are not messages are skipped as what
/// they are.
#[test]
fn unknown_lines_are_skipped_and_counted() {
    let body = format!("{}\nsay 'not json'\nsay '{{\"method\":\"brand/new\",\"params\":{{}}}}'\nsay '[]'\nhang", thread_prelude());
    let world = World::new("skipped", &body);
    let mut session = world.session(config(Start::New, Duration::from_secs(5)), chatgpt());
    session.send("A").unwrap();
    let events = events_for(&mut session, Duration::from_millis(500));
    let skipped: Vec<&Skipped> = events
        .iter()
        .filter_map(|e| match e {
            SessionEvent::Ignored(why) => Some(why),
            _ => None,
        })
        .collect();
    assert_eq!(skipped, vec![&Skipped::NotJson { bytes: 8 }, &Skipped::Unknown { kind: "brand/new".into() }, &Skipped::NotAnObject]);
    assert_eq!(session.skipped(), 3);
    drop(session);
    world.remove();
}
