use std::cell::RefCell;

use super::*;

/// A stand-in for the application: records each call and answers from `answer`.
struct Fake {
    calls: RefCell<Vec<(String, Value)>>,
    answer: Result<Value, CallError>,
}

impl Fake {
    fn ok() -> Fake {
        Fake { calls: RefCell::new(Vec::new()), answer: Ok(json!({ "done": true })) }
    }
}

impl Backend for Fake {
    fn call(&self, method: &str, params: Value) -> Result<Value, CallError> {
        self.calls.borrow_mut().push((method.to_string(), params));
        self.answer.clone()
    }
}

fn ask(backend: &dyn Backend, message: Value) -> Value {
    handle(message.to_string().as_bytes(), backend).unwrap()
}

#[test]
fn initialize_answers_the_clients_version_when_spoken_and_the_newest_otherwise() {
    let fake = Fake::ok();
    let r = ask(&fake, json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": { "protocolVersion": "2025-03-26" } }));
    assert_eq!(r["result"]["protocolVersion"], "2025-03-26");
    assert_eq!(r["result"]["capabilities"], json!({ "tools": { "listChanged": false } }));
    let r = ask(&fake, json!({ "jsonrpc": "2.0", "id": 2, "method": "initialize", "params": { "protocolVersion": "1999-01-01" } }));
    assert_eq!(r["result"]["protocolVersion"], "2025-06-18");
    assert_eq!(ask(&fake, json!({ "jsonrpc": "2.0", "id": "p", "method": "ping" }))["result"], json!({}));
    // A notification is never answered.
    assert!(handle(br#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#, &fake).is_none());
    assert_eq!(ask(&fake, json!({ "jsonrpc": "2.0", "id": 3, "method": "resources/list" }))["error"]["code"], -32601);
}

#[test]
fn only_the_reads_say_they_are_read_only() {
    let list = tool_list();
    let tools = list["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 8);
    for t in tools {
        let read_only = t["annotations"]["readOnlyHint"].as_bool().unwrap();
        let name = t["name"].as_str().unwrap();
        assert_eq!(read_only, ["list_commands", "describe_command", "inspect_document"].contains(&name), "{name}");
        assert_eq!(
            t["annotations"]["destructiveHint"].as_bool().unwrap(),
            !read_only,
            "{name}: every change can remove something, undo and redo included"
        );
        assert_eq!(t["inputSchema"]["additionalProperties"], false, "{name}");
    }
}

#[test]
fn a_tool_calls_its_method_and_refuses_unknown_arguments() {
    let fake = Fake::ok();
    let call = |name: &str, args: Value| {
        ask(&fake, json!({ "jsonrpc": "2.0", "id": 7, "method": "tools/call", "params": { "name": name, "arguments": args } }))
    };
    let r = call("execute_command", json!({ "command": "er.entity.add", "params": { "name": "A" } }));
    assert_eq!(r["result"]["isError"], false);
    assert_eq!(fake.calls.borrow()[0], ("engine.execute".into(), json!({ "command": "er.entity.add", "params": { "name": "A" } })));
    call("call_app", json!({ "method": "ui.inspect" }));
    assert_eq!(fake.calls.borrow()[1], ("ui.inspect".into(), json!({})));
    let r = call("execute_command", json!({ "cmd": "x" }));
    assert!(r["result"]["isError"].as_bool().unwrap() && r["result"]["content"][0]["text"].as_str().unwrap().contains("command, params"));
    let r = call("describe_command", json!({}));
    assert!(r["result"]["content"][0]["text"].as_str().unwrap().contains("needs \"command\""));
    assert_eq!(fake.calls.borrow().len(), 2, "refused calls reach nothing");
    let r = call("format_disk", json!({}));
    assert_eq!(r["error"]["code"], -32602);
    // An application that is not running: the agent reads why.
    let down = Fake { calls: RefCell::new(Vec::new()), answer: Err(CallError::Unreachable("no Archforge on port 47000".into())) };
    let r = ask(&down, json!({ "jsonrpc": "2.0", "id": 8, "method": "tools/call", "params": { "name": "inspect_document" } }));
    assert!(r["result"]["isError"].as_bool().unwrap() && r["result"]["content"][0]["text"].as_str().unwrap().contains("not reachable"));
}

#[test]
fn a_line_too_long_is_answered_and_the_next_one_still_served() {
    let fake = Fake::ok();
    let mut input = vec![b'x'; MAX_LINE + 5];
    input.extend_from_slice(b"\n{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}\n");
    let mut out = Vec::new();
    serve(&mut std::io::BufReader::new(&input[..]), &mut out, &fake).unwrap();
    let replies: Vec<Value> = out.split(|b| *b == b'\n').filter(|l| !l.is_empty()).map(|l| serde_json::from_slice(l).unwrap()).collect();
    assert_eq!(replies.len(), 2);
    assert_eq!(replies[0]["error"]["code"], -32700);
    assert_eq!(replies[1]["result"], json!({}));
}

#[test]
fn hostile_messages_never_crash_the_server() {
    let fake = Fake::ok();
    let mut seed: u64 = 0x2545_F491_4F6C_DD1D;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    let shapes = [
        r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"batch","arguments":{"steps":"no"}}}"#,
        r#"{"jsonrpc":"2.0","id":{},"method":"tools/call","params":[]}"#,
        r#"{"jsonrpc":"2.0","id":null,"method":7}"#,
        r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"call_app","arguments":{"method":{"x":1}}}}"#,
        r#"[{"jsonrpc":"2.0","id":1,"method":"ping"}]"#,
        r#"{"id":1e400,"method":"initialize","params":{"protocolVersion":null}}"#,
        "\u{0}\u{1}\u{2}",
    ];
    let mut input = Vec::new();
    for _ in 0..400 {
        match next() % 3 {
            0 => input.extend((0..(next() % 200)).map(|_| (next() % 256) as u8).filter(|b| *b != b'\n')),
            1 => input.extend_from_slice(shapes[(next() % shapes.len() as u64) as usize].as_bytes()),
            _ => {
                let depth = (next() % 4000) as usize;
                input.extend(std::iter::repeat_n(b'{', depth));
            }
        }
        input.push(b'\n');
    }
    let mut out = Vec::new();
    serve(&mut std::io::BufReader::new(&input[..]), &mut out, &fake).unwrap();
    assert!(!out.is_empty());
}
