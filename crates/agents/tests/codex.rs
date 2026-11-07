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

use archforge_agents::codex::{AccountEvent, CancelStatus, CodexAccount, CodexHome, CredentialStore, LoginOutcome};
use archforge_agents::{AgentError, ErrorKind, HostEnv, Limits, Runtime, RuntimeKind, Workspace, find};
use archforge_secrets::Secret;
use serde_json::{Value, json};

/// A key no output of the adapter may ever show but the one line that signs in.
const CANARY: &str = "sk-proj-CANARY-5b2e9d0c7a41_canary-canary-canary-canary-canary-canary-canary-xyz";

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
/// through the override; its workspace and Archforge's private `CODEX_HOME`.
struct World {
    base: PathBuf,
    runtime: Runtime,
    workspace: Workspace,
    home: CodexHome,
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
        World { base, runtime, workspace, home }
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

    /// Removes the world's folder.
    fn remove(self) {
        let base = self.base.clone();
        drop(self);
        std::fs::remove_dir_all(&base).unwrap();
    }
}

/// The canary as a secret.
fn canary() -> Secret {
    Secret::new(CANARY.to_owned()).unwrap()
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
/// the end stays queued as an event, and later calls are refused.
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
    assert!(matches!(account.read(false, Duration::from_secs(1)), Err(AgentError::InputClosed { .. })));
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
