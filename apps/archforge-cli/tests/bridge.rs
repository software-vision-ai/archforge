//! The real `archforge-cli` binary against a live control channel: the MCP server bridged to it,
//! and one-off requests with their exit codes. The child runs with its own home folder, so it
//! finds the token this test's server writes there.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

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
    let handler: archforge_control::Handler = Arc::new(move |method, params| {
        log.lock().unwrap().push((method.to_string(), params.clone()));
        match method {
            "engine.execute" => Ok(json!({ "result": { "id": "e1" }, "undo": "Add entity" })),
            "document.inspect" => Ok(json!({ "open": true })),
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
    drop(server);
    let gone = cli(&home).args(["app", "document.inspect", "--port", &port]).output().unwrap();
    assert_eq!(gone.status.code(), Some(3));
    assert!(String::from_utf8_lossy(&gone.stderr).contains("--control"));
    // A usage error exits 2.
    assert_eq!(cli(&home).args(["app"]).output().unwrap().status.code(), Some(2));
    let _ = std::fs::remove_dir_all(&home);
}
