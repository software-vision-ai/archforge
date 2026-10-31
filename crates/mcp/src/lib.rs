//! The MCP server (agent-control-surfaces §10.5, BP-CS-4): JSON-RPC 2.0 over the standard
//! streams, one message per line, speaking protocol versions 2024-11-05, 2025-03-26 and 2025-06-18.
//! It runs in bridge mode: every tool is a request to the running Archforge through its control
//! channel ([`archforge_control::Client`]), so the window shows each change and undo covers it.
//! The headless mode is not built and not advertised (CS-3).
//!
//! Tools: `list_commands`, `describe_command`, `execute_command`, `batch`, `inspect_document`,
//! `undo`, `redo` and `call_app`. Only the reads say they are read-only (CS-13); unknown tools,
//! methods and argument keys are refused, naming the accepted ones (CS-3).
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unreachable, clippy::todo, clippy::unimplemented)]

use std::io::{BufRead, Write};

use archforge_control::{CallError, MAX_LINE, read_line};
use serde_json::{Value, json};

/// The protocol versions spoken, the newest last.
pub const VERSIONS: [&str; 3] = ["2024-11-05", "2025-03-26", "2025-06-18"];

/// What the tools reach: the running application's control channel, or a stand-in in tests.
pub trait Backend {
    fn call(&self, method: &str, params: Value) -> Result<Value, CallError>;
}

impl Backend for archforge_control::Client {
    fn call(&self, method: &str, params: Value) -> Result<Value, CallError> {
        archforge_control::Client::call(self, method, params)
    }
}

/// One tool: its name, what it does, its arguments' schema, the control method it calls, whether
/// it only reads, and whether its changes can remove things.
struct Tool {
    name: &'static str,
    description: &'static str,
    schema: fn() -> Value,
    method: Option<&'static str>,
    read_only: bool,
    destructive: bool,
}

const TOOLS: [Tool; 8] = [
    Tool {
        name: "list_commands",
        description: "List Archforge's commands for the open project, compactly: id, label, whether it is enabled now and whether it changes the model. Use describe_command for one command's parameters.",
        schema: || object(json!({ "filter": { "type": "string", "description": "Only commands whose id or label contains this text" } }), &[]),
        method: Some("engine.commands"),
        read_only: true,
        destructive: false,
    },
    Tool {
        name: "describe_command",
        description: "One command's documentation: its parameters with their types, defaults, ranges and meaning, and why it is disabled when it is.",
        schema: || object(json!({ "command": { "type": "string", "description": "The command id, such as er.entity.add" } }), &["command"]),
        method: Some("engine.describe"),
        read_only: true,
        destructive: false,
    },
    Tool {
        name: "execute_command",
        description: "Run one command on the open project. The window shows the change at once and the user can undo it.",
        schema: || {
            object(
                json!({
                    "command": { "type": "string", "description": "The command id" },
                    "params": { "type": "object", "description": "The command's parameters, as describe_command lists them" },
                }),
                &["command"],
            )
        },
        method: Some("engine.execute"),
        read_only: false,
        destructive: true,
    },
    Tool {
        name: "batch",
        description: "Run several commands as one undo step. A step's params may refer to an earlier step's result as \"$N\" or \"$N.path\" (for example \"$0.id\"); a string starting \"$$\" stands for itself with one \"$\". If a step fails, the whole batch is rolled back.",
        schema: || {
            object(
                json!({
                    "label": { "type": "string", "description": "The undo step's name" },
                    "steps": {
                        "type": "array",
                        "minItems": 1,
                        "maxItems": 256,
                        "items": object(json!({ "command": { "type": "string" }, "params": { "type": "object" } }), &["command"]),
                    },
                }),
                &["steps"],
            )
        },
        method: Some("engine.batch"),
        read_only: false,
        destructive: true,
    },
    Tool {
        name: "inspect_document",
        description: "The open project: its name and path, revision, whether it has unsaved changes, the undo and redo steps, the selection and the open diagram.",
        schema: || object(json!({}), &[]),
        method: Some("document.inspect"),
        read_only: true,
        destructive: false,
    },
    Tool {
        name: "undo",
        description: "Undo the last change, as Edit > Undo does.",
        schema: || object(json!({}), &[]),
        method: Some("engine.undo"),
        read_only: false,
        destructive: true,
    },
    Tool {
        name: "redo",
        description: "Redo the last undone change.",
        schema: || object(json!({}), &[]),
        method: Some("engine.redo"),
        read_only: false,
        destructive: true,
    },
    Tool {
        name: "call_app",
        description: "Send any control-channel request to the running Archforge: engine.commands, engine.describe, engine.execute, engine.batch, engine.undo, engine.redo, document.inspect or ui.inspect (the controls drawn in the window).",
        schema: || object(json!({ "method": { "type": "string" }, "params": { "type": "object" } }), &["method"]),
        method: None,
        read_only: false,
        destructive: true,
    },
];

fn object(properties: Value, required: &[&str]) -> Value {
    json!({ "type": "object", "properties": properties, "required": required, "additionalProperties": false })
}

/// The control methods the tools call with their argument keys, `call_app` aside, and the text
/// that tells an agent which methods `call_app` reaches: a test beside the application keeps both
/// in step with its methods.
pub fn tool_methods() -> (Vec<(&'static str, Vec<String>)>, &'static str) {
    let methods = TOOLS
        .iter()
        .filter_map(|t| {
            let keys = (t.schema)()["properties"].as_object().map(|p| p.keys().cloned().collect()).unwrap_or_default();
            t.method.map(|m| (m, keys))
        })
        .collect();
    let call_app = TOOLS.iter().find(|t| t.method.is_none()).map(|t| t.description).unwrap_or_default();
    (methods, call_app)
}

/// The tools as `tools/list` gives them.
pub fn tool_list() -> Value {
    let tools: Vec<Value> = TOOLS
        .iter()
        .map(|t| {
            json!({
                "name": t.name,
                "description": t.description,
                "inputSchema": (t.schema)(),
                "annotations": { "readOnlyHint": t.read_only, "destructiveHint": t.destructive, "openWorldHint": false },
            })
        })
        .collect();
    json!({ "tools": tools })
}

/// Serves `input` until it ends, writing a reply line to `output` for every request.
pub fn serve(input: &mut impl BufRead, output: &mut impl Write, backend: &dyn Backend) -> std::io::Result<()> {
    loop {
        let reply = match read_line(input, MAX_LINE) {
            Ok(None) => return Ok(()),
            Ok(Some(line)) => handle(&line, backend),
            Err(e) if e.kind() == std::io::ErrorKind::InvalidData => {
                skip_line(input)?;
                Some(error(Value::Null, -32700, "the message is longer than 4 MiB"))
            }
            Err(e) => return Err(e),
        };
        if let Some(reply) = reply {
            let mut text = serde_json::to_vec(&reply).unwrap_or_default();
            text.push(b'\n');
            output.write_all(&text)?;
            output.flush()?;
        }
    }
}

/// Reads past the rest of a line that was too long.
fn skip_line(input: &mut impl BufRead) -> std::io::Result<()> {
    loop {
        let buf = input.fill_buf()?;
        if buf.is_empty() {
            return Ok(());
        }
        match buf.iter().position(|b| *b == b'\n') {
            Some(i) => {
                input.consume(i + 1);
                return Ok(());
            }
            None => {
                let n = buf.len();
                input.consume(n);
            }
        }
    }
}

/// The reply to one message, none for a notification.
pub fn handle(line: &[u8], backend: &dyn Backend) -> Option<Value> {
    let Ok(message) = serde_json::from_slice::<Value>(line) else { return Some(error(Value::Null, -32700, "the message is not JSON")) };
    let Some(object) = message.as_object() else { return Some(error(Value::Null, -32600, "a message is a JSON object")) };
    let id = object.get("id").cloned();
    let Some(method) = object.get("method").and_then(Value::as_str) else {
        // A response or something else without a method: nothing to answer unless it has an id.
        return id.map(|id| error(id, -32600, "a request needs a method"));
    };
    // A notification (no id) is never answered.
    let id = id?;
    let params = object.get("params").cloned().unwrap_or_else(|| json!({}));
    Some(match method {
        "initialize" => {
            let asked = params.get("protocolVersion").and_then(Value::as_str).unwrap_or_default();
            let version = if VERSIONS.contains(&asked) { asked } else { VERSIONS[VERSIONS.len() - 1] };
            result(
                id,
                json!({
                    "protocolVersion": version,
                    "capabilities": { "tools": { "listChanged": false } },
                    "serverInfo": { "name": "archforge", "version": env!("CARGO_PKG_VERSION") },
                    "instructions": "Archforge models databases as ER diagrams. Every tool works on the project open in the running Archforge window: list_commands and describe_command show what can be done, execute_command and batch change the model (the user sees it and can undo it), inspect_document tells what is open and selected.",
                }),
            )
        }
        "ping" => result(id, json!({})),
        "tools/list" => result(id, tool_list()),
        "tools/call" => call(id, &params, backend),
        other => error(id, -32601, &format!("unknown method {other:?}; the server answers initialize, ping, tools/list and tools/call")),
    })
}

/// Runs a tool: its arguments checked against its schema's keys, then its control method.
fn call(id: Value, params: &Value, backend: &dyn Backend) -> Value {
    let Some(name) = params.get("name").and_then(Value::as_str) else { return error(id, -32602, "tools/call needs name") };
    let Some(tool) = TOOLS.iter().find(|t| t.name == name) else {
        let names: Vec<&str> = TOOLS.iter().map(|t| t.name).collect();
        return error(id, -32602, &format!("unknown tool {name:?}; the tools are {}", names.join(", ")));
    };
    let args = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
    let Some(given) = args.as_object() else { return tool_error(id, "arguments must be an object") };
    let schema = (tool.schema)();
    let keys: Vec<&str> = schema["properties"].as_object().map(|p| p.keys().map(String::as_str).collect()).unwrap_or_default();
    if let Some(key) = given.keys().find(|k| !keys.contains(&k.as_str())) {
        return tool_error(
            id,
            &format!("{name} has no argument {key:?}; it takes {}", if keys.is_empty() { "none".to_string() } else { keys.join(", ") }),
        );
    }
    let required = schema["required"].as_array().cloned().unwrap_or_default();
    if let Some(missing) = required.iter().filter_map(Value::as_str).find(|k| !given.contains_key(*k)) {
        return tool_error(id, &format!("{name} needs {missing:?}"));
    }
    let (method, request) = match tool.method {
        Some(m) => (m.to_string(), args.clone()),
        None => {
            (given.get("method").and_then(Value::as_str).unwrap_or_default().to_string(), given.get("params").cloned().unwrap_or_else(|| json!({})))
        }
    };
    match backend.call(&method, request) {
        Ok(value) => result(id, json!({ "content": [{ "type": "text", "text": value.to_string() }], "isError": false })),
        Err(CallError::Unreachable(e)) => tool_error(id, &format!("Archforge is not reachable: {e}")),
        Err(CallError::Refused(e)) => tool_error(id, &e),
    }
}

fn result(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn error(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

/// A tool that ran and failed: the agent reads why.
fn tool_error(id: Value, message: &str) -> Value {
    result(id, json!({ "content": [{ "type": "text", "text": message }], "isError": true }))
}

#[cfg(test)]
mod tests;
