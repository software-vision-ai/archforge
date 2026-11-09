//! The real `archforge-cli` binary against a live control channel: the MCP server bridged to it,
//! and one-off requests with their exit codes. The child runs with its own home folder, so it
//! finds the token this test's server writes there.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

/// A refusal in the shape the window gives one while a version is in view: its stable word, then
/// a translated sentence. The sentence is this test's own, not the catalogue's.
const REFUSED_WHILE_VIEWING: &str = "viewing: 翻訳された文";

/// A fresh home folder for the child, and the settings folder it will look in.
fn home(name: &str) -> (PathBuf, PathBuf) {
    let home = std::env::temp_dir().join(format!("archforge-cli-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    let settings = if cfg!(target_os = "macos") {
        home.join("Library").join("Application Support").join("Archforge")
    } else {
        home.join(".config").join("archforge")
    };
    std::fs::create_dir_all(&settings).unwrap();
    (home, settings)
}

fn cli(home: &Path) -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_archforge-cli"));
    c.env("HOME", home).env_remove("XDG_CONFIG_HOME");
    c
}

#[cfg(unix)]
#[test]
fn an_agent_drives_the_application_through_the_real_binary() {
    let (home, settings) = home("bridge");
    let seen: Arc<Mutex<Vec<(String, Value)>>> = Arc::default();
    let log = seen.clone();
    let handler: archforge_control::Handler = Arc::new(move |call: archforge_control::Call<'_>| {
        let (method, params) = (call.method, call.params);
        log.lock().unwrap().push((method.to_string(), params.clone()));
        match method {
            "engine.execute" => Ok(json!({ "result": { "id": "e1" }, "undo": "Add entity" })),
            "document.inspect" => Ok(json!({ "open": true })),
            "app.save" => Err(REFUSED_WHILE_VIEWING.to_string()),
            _ => Err(format!("unknown method {method:?}")),
        }
    });
    let server = archforge_control::Server::start(0, &settings, handler).unwrap();
    let port = server.port().to_string();

    // The MCP server over the standard streams.
    let mut child = cli(&home).args(["mcp", "--bridge", &port]).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();
    let mut input = child.stdin.take().unwrap();
    let messages = [
        json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": { "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "test", "version": "0" } } }),
        json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
        json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" }),
        json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": { "name": "execute_command", "arguments": { "command": "er.entity.add", "params": { "name": "Invoice" } } } }),
    ];
    for m in &messages {
        writeln!(input, "{m}").unwrap();
    }
    drop(input);
    let replies: Vec<Value> = BufReader::new(child.stdout.take().unwrap()).lines().map(|l| serde_json::from_str(&l.unwrap()).unwrap()).collect();
    assert!(child.wait().unwrap().success());
    assert_eq!(replies.len(), 3, "the notification is not answered: {replies:?}");
    assert_eq!(replies[0]["result"]["protocolVersion"], "2025-06-18");
    let tools = replies[1]["result"]["tools"].as_array().unwrap();
    assert!(tools.len() > 8 && tools.iter().any(|t| t["name"] == "er_entity_add"), "the general tools and one per command");
    assert_eq!(replies[2]["result"]["isError"], false);
    assert_eq!(seen.lock().unwrap()[0], ("engine.execute".to_string(), json!({ "command": "er.entity.add", "params": { "name": "Invoice" } })));

    // One-off requests: 0 on success, 1 when the application refuses, 3 when nothing listens.
    let ok = cli(&home).args(["app", "document.inspect", "--port", &port]).output().unwrap();
    assert_eq!(ok.status.code(), Some(0));
    assert_eq!(serde_json::from_slice::<Value>(&ok.stdout).unwrap(), json!({ "open": true }));
    assert_eq!(cli(&home).args(["app", "engine.nope", "--port", &port]).output().unwrap().status.code(), Some(1));
    // A refusal shows as the window words it, translated text included.
    let refused = cli(&home).args(["app", "app.save", "--port", &port]).output().unwrap();
    assert_eq!(refused.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&refused.stderr).contains(REFUSED_WHILE_VIEWING), "{refused:?}");
    drop(server);
    let gone = cli(&home).args(["app", "document.inspect", "--port", &port]).output().unwrap();
    assert_eq!(gone.status.code(), Some(3));
    assert!(String::from_utf8_lossy(&gone.stderr).contains("--control"));
    // A usage error exits 2.
    assert_eq!(cli(&home).args(["app"]).output().unwrap().status.code(), Some(2));
    let _ = std::fs::remove_dir_all(&home);
}

/// Runs the real MCP server with `flags` against `port`, sends `messages` and gives the replies.
fn session(home: &Path, port: &str, flags: &[&str], messages: &[Value]) -> Vec<Value> {
    let mut child = cli(home).args(["mcp", "--bridge", port]).args(flags).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();
    let mut input = child.stdin.take().unwrap();
    for m in messages {
        writeln!(input, "{m}").unwrap();
    }
    drop(input);
    let replies = BufReader::new(child.stdout.take().unwrap()).lines().map(|l| serde_json::from_str(&l.unwrap()).unwrap()).collect();
    assert!(child.wait().unwrap().success());
    replies
}

fn call(id: u64, name: &str, arguments: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "method": "tools/call", "params": { "name": name, "arguments": arguments } })
}

/// `--origin` set by the parent goes with every request beside the parameters, and nothing the
/// agent sends changes it; `--read-only` refuses a change before it leaves the server; a bad
/// origin is a usage error before any I/O.
#[cfg(unix)]
#[test]
fn the_origin_comes_from_the_command_line_alone_and_read_only_refuses_changes_in_the_real_binary() {
    let (home, settings) = home("origin");
    let seen: Arc<Mutex<Vec<(String, Value, Option<String>)>>> = Arc::default();
    let log = seen.clone();
    let handler: archforge_control::Handler = Arc::new(move |call: archforge_control::Call<'_>| {
        let (method, params, origin) = (call.method, call.params, call.origin);
        log.lock().unwrap().push((method.to_string(), params.clone(), origin.map(str::to_string)));
        Ok(json!({ "result": { "id": "e1" }, "undo": origin.map(|o| format!("{o}: Add entity")), "origin": origin }))
    });
    let server = archforge_control::Server::start(0, &settings, handler).unwrap();
    let port = server.port().to_string();

    let replies = session(
        &home,
        &port,
        &["--origin", "Chat 1"],
        &[
            call(1, "er_entity_add", json!({ "schema": "s1", "name": "Invoice" })),
            // An origin among the arguments is no argument of the tool.
            call(2, "execute_command", json!({ "command": "er.entity.add", "params": { "schema": "s1" }, "origin": "Spoof" })),
            // Through call_app it stays a parameter, which the window refuses as unknown; the
            // request's own origin is still the command line's.
            call(3, "call_app", json!({ "method": "engine.execute", "params": { "command": "er.entity.add", "origin": "Spoof" } })),
        ],
    );
    assert_eq!(replies.len(), 3);
    assert_eq!(replies[0]["result"]["isError"], false, "{}", replies[0]);
    let text: Value = serde_json::from_str(replies[0]["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!((text["undo"].as_str(), text["origin"].as_str()), (Some("Chat 1: Add entity"), Some("Chat 1")));
    assert!(replies[1]["result"]["content"][0]["text"].as_str().unwrap().contains("no argument \"origin\""), "{}", replies[1]);
    let got = seen.lock().unwrap().clone();
    assert_eq!(got.len(), 2, "the refused call reached nothing: {got:?}");
    assert!(got.iter().all(|(_, _, o)| o.as_deref() == Some("Chat 1")), "{got:?}");
    assert_eq!(got[1].1, json!({ "command": "er.entity.add", "origin": "Spoof" }), "a parameter, never the request's origin");

    // Read-only: only reads are listed, and a change is refused by name before it is sent.
    seen.lock().unwrap().clear();
    let replies = session(
        &home,
        &port,
        &["--read-only"],
        &[
            json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" }),
            call(2, "er_entity_add", json!({ "schema": "s1" })),
            call(3, "inspect_document", json!({})),
        ],
    );
    let tools = replies[0]["result"]["tools"].as_array().unwrap();
    assert!(tools.iter().all(|t| t["annotations"]["readOnlyHint"] == true) && tools.iter().any(|t| t["name"] == "er_entity_list"));
    assert_eq!(replies[1]["error"]["code"], -32004, "{}", replies[1]);
    assert_eq!(replies[1]["error"]["data"]["tool"], "er_entity_add");
    assert_eq!(replies[2]["result"]["isError"], false);
    let got = seen.lock().unwrap().clone();
    assert_eq!(got.iter().map(|(m, _, o)| (m.as_str(), o.clone())).collect::<Vec<_>>(), [("document.inspect", None)]);

    // A bad origin exits 2 and says why.
    for bad in ["", "Chat\n1", " Chat", "Chat\u{202E}1"] {
        let out = cli(&home).args(["mcp", "--bridge", &port, "--origin", bad]).stdin(Stdio::null()).output().unwrap();
        assert_eq!(out.status.code(), Some(2), "{bad:?}");
        assert!(String::from_utf8_lossy(&out.stderr).contains("--origin"), "{bad:?}");
    }
    drop(server);
    let _ = std::fs::remove_dir_all(&home);
}

/// A Chat conversation's token reaches the window through `--token-file`, with its own role; once
/// revoked, its file is gone and the real binary exits 3. A relative path is a usage error.
#[cfg(unix)]
#[test]
fn a_granted_token_file_reaches_the_window_until_it_is_revoked() {
    let (home, settings) = home("token-file");
    let handler: archforge_control::Handler =
        Arc::new(|call: archforge_control::Call<'_>| Ok(json!({ "method": call.method, "role": format!("{:?}", call.role), "origin": call.origin })));
    let server = archforge_control::Server::open(&settings, handler).unwrap();
    let port = server.port().to_string();
    let grant = server.grant(archforge_control::Role::ReadOnly, None).unwrap();
    let file = grant.path().to_str().unwrap().to_string();
    let out = cli(&home).args(["app", "document.inspect", "--port", &port, "--token-file", &file]).output().unwrap();
    assert_eq!(out.status.code(), Some(0), "{}", String::from_utf8_lossy(&out.stderr));
    let answer: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(answer, json!({ "method": "document.inspect", "role": "ReadOnly", "origin": null }));
    // Without the file, the settings folder's own token is looked for, and there is none.
    let out = cli(&home).args(["app", "document.inspect", "--port", &port]).output().unwrap();
    assert_eq!(out.status.code(), Some(3));
    drop(grant);
    let out = cli(&home).args(["app", "document.inspect", "--port", &port, "--token-file", &file]).output().unwrap();
    assert_eq!(out.status.code(), Some(3), "a revoked token's file is gone");
    let out = cli(&home).args(["app", "document.inspect", "--port", &port, "--token-file", "control.token"]).output().unwrap();
    assert_eq!(out.status.code(), Some(2), "a relative path is a usage error");
    drop(server);
    let _ = std::fs::remove_dir_all(&home);
}
