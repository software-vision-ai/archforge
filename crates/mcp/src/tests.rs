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
    handle(message.to_string().as_bytes(), backend, &Catalog::default()).unwrap()
}

#[test]
fn initialize_answers_the_clients_version_when_spoken_and_the_newest_otherwise() {
    let fake = Fake::ok();
    let r = ask(&fake, json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": { "protocolVersion": "2025-03-26" } }));
    assert_eq!(r["result"]["protocolVersion"], "2025-03-26");
    assert_eq!(r["result"]["capabilities"], json!({ "tools": { "listChanged": false }, "resources": { "subscribe": false, "listChanged": false } }));
    let r = ask(&fake, json!({ "jsonrpc": "2.0", "id": 2, "method": "initialize", "params": { "protocolVersion": "1999-01-01" } }));
    assert_eq!(r["result"]["protocolVersion"], "2025-06-18");
    assert_eq!(ask(&fake, json!({ "jsonrpc": "2.0", "id": "p", "method": "ping" }))["result"], json!({}));
    // A notification is never answered.
    assert!(handle(br#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#, &fake, &Catalog::default()).is_none());
    assert_eq!(ask(&fake, json!({ "jsonrpc": "2.0", "id": 3, "method": "prompts/list" }))["error"]["code"], -32601);
}

/// A stand-in that answers as Archforge does with a project open: a schema `s1` with a table `t1`,
/// skills in effect on either; anything else unknown.
struct Model {
    open: bool,
    calls: RefCell<Vec<(String, Value)>>,
    /// Edits the user makes while reads go on: each project read after the first takes one, until
    /// none are left.
    edits: std::cell::Cell<u32>,
    revision: std::cell::Cell<u64>,
}

impl Model {
    fn open() -> Model {
        Model { open: true, calls: RefCell::new(Vec::new()), edits: 0.into(), revision: 7.into() }
    }
}

impl Backend for Model {
    fn call(&self, method: &str, params: Value) -> Result<Value, CallError> {
        self.calls.borrow_mut().push((method.to_string(), params.clone()));
        let refused = |why: &str| Err(CallError::Refused(why.to_string()));
        if method == "document.inspect" {
            let inspected = self.calls.borrow().iter().filter(|(m, _)| m == "document.inspect").count();
            if inspected > 1 && self.edits.get() > 0 {
                self.edits.set(self.edits.get() - 1);
                self.revision.set(self.revision.get() + 1);
            }
            return Ok(if self.open {
                json!({ "open": true, "revision": self.revision.get(), "project": { "name": "Shop" } })
            } else {
                json!({ "open": false })
            });
        }
        let p = &params["params"];
        let answer = match params["command"].as_str().unwrap_or_default() {
            "er.schema.get" if p["schema"] == "s1" => {
                json!({ "id": "s1", "name": "shop", "entities": [{ "id": "t1", "attributes": [{ "name": "id" }] }] })
            }
            "er.entity.get" if p["entity"] == "t1" => json!({ "id": "t1", "name": "customer", "attributes": [{ "name": "id" }] }),
            "skill.list" => json!({ "skills": [{ "id": "k1", "name": "DDL" }] }),
            "skill.effective" if p["element"] == "s1" || p["element"] == "t1" => {
                json!({ "skills": [{ "id": "k1", "name": "DDL", "origin": null, "overridden_by": null, "trigger": p["trigger"] }] })
            }
            "skill.effective" => return refused("there is no element x"),
            _ => return refused("unknown"),
        };
        Ok(json!({ "result": answer }))
    }
}

fn read(backend: &dyn Backend, uri: &str) -> Value {
    ask(backend, json!({ "jsonrpc": "2.0", "id": 9, "method": "resources/read", "params": { "uri": uri } }))
}

/// The text of a read's one content, as JSON.
fn content(r: &Value) -> Value {
    assert_eq!(r["result"]["contents"][0]["mimeType"], "application/json", "{r}");
    serde_json::from_str(r["result"]["contents"][0]["text"].as_str().unwrap()).unwrap()
}

#[test]
fn the_model_and_its_skills_read_as_resources_with_the_revision_and_say_what_they_cannot_read() {
    let model = Model::open();
    let listed = ask(&model, json!({ "jsonrpc": "2.0", "id": 1, "method": "resources/list" }));
    let uris: Vec<&str> = listed["result"]["resources"].as_array().unwrap().iter().map(|r| r["uri"].as_str().unwrap()).collect();
    assert_eq!(uris, ["archforge://project", "archforge://skills"]);
    let templates = ask(&model, json!({ "jsonrpc": "2.0", "id": 2, "method": "resources/templates/list" }));
    assert_eq!(templates["result"]["resourceTemplates"].as_array().unwrap().len(), 3);
    assert_eq!(content(&read(&model, "archforge://project"))["revision"], 7);
    assert_eq!(content(&read(&model, "archforge://skills"))["skills"][0]["name"], "DDL");
    // A schema reads with its tables; a table alone.
    let schema = content(&read(&model, "archforge://element/s1"));
    assert_eq!((schema["kind"].as_str(), schema["revision"].as_i64()), (Some("schema"), Some(7)));
    assert_eq!(schema["schema"]["entities"][0]["attributes"][0]["name"], "id");
    assert_eq!(content(&read(&model, "archforge://element/t1"))["kind"], "table");
    // The skills in effect, for every trigger or for one, as skill.effective gives them.
    let effective = content(&read(&model, "archforge://skills/effective/t1/export"));
    assert_eq!((effective["trigger"].as_str(), effective["skills"][0]["name"].as_str()), (Some("export"), Some("DDL")));
    assert_eq!(content(&read(&model, "archforge://skills/effective/s1"))["trigger"], Value::Null);
    // Every read only reads: inspections and queries, never a command that changes the model.
    for (method, params) in model.calls.borrow().iter() {
        let command = params["command"].as_str().unwrap_or_default();
        assert!(
            method == "document.inspect" || command.ends_with(".get") || command.ends_with(".list") || command == "skill.effective",
            "{method} {params}"
        );
    }
    // What it cannot read is said, with what it can.
    let unknown = read(&model, "archforge://element/zz");
    assert_eq!(unknown["error"]["code"], -32002, "{unknown}");
    assert!(unknown["error"]["message"].as_str().unwrap().contains("packages and other elements"), "{unknown}");
    let bad = read(&model, "archforge://skills/effective/t1/everything");
    assert!(bad["error"]["message"].as_str().unwrap().contains("modelling, drawing, naming, export"), "{bad}");
    let other = read(&model, "file:///etc/passwd");
    assert!(other["error"]["message"].as_str().unwrap().contains("archforge://project"), "{other}");
    assert_eq!(read(&model, "archforge://skills/effective/x")["error"]["code"], -32002);
    assert_eq!(ask(&model, json!({ "jsonrpc": "2.0", "id": 3, "method": "resources/read", "params": {} }))["error"]["code"], -32602);
    // With no project open, nothing reads.
    let closed = Model { open: false, ..Model::open() };
    assert!(read(&closed, "archforge://skills")["error"]["message"].as_str().unwrap().contains("no project is open"));
}

#[test]
fn a_read_that_straddles_an_edit_is_made_again_and_one_that_never_settles_is_refused() {
    // One edit lands while the table is read: the read is made again, at the revision after it.
    let model = Model::open();
    model.edits.set(1);
    let table = content(&read(&model, "archforge://element/t1"));
    assert_eq!(table["revision"], 8, "the content carries the revision it was read at");
    assert_eq!(model.calls.borrow().iter().filter(|(m, _)| m == "document.inspect").count(), 4);
    // Edits that never stop: the read is refused rather than labelled with a revision it was not
    // read at.
    let busy = Model::open();
    busy.edits.set(u32::MAX);
    let refused = read(&busy, "archforge://skills");
    assert_eq!(refused["error"]["code"], -32003, "{refused}");
    assert!(refused["error"]["message"].as_str().unwrap().contains("read it again"), "{refused}");
}

#[test]
fn only_the_reads_say_they_are_read_only() {
    let list = tool_list();
    let tools = list["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 9);
    for t in tools {
        let read_only = t["annotations"]["readOnlyHint"].as_bool().unwrap();
        let name = t["name"].as_str().unwrap();
        assert_eq!(read_only, ["list_commands", "describe_command", "inspect_document"].contains(&name), "{name}");
        assert_eq!(
            t["annotations"]["destructiveHint"].as_bool().unwrap(),
            !read_only && name != "save_project",
            "{name}: every change can remove something, undo and redo included; a save only adds a version"
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
    serve(&mut std::io::BufReader::new(&input[..]), &mut out, &fake, &Catalog::default()).unwrap();
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
    serve(&mut std::io::BufReader::new(&input[..]), &mut out, &fake, &Catalog::default()).unwrap();
    assert!(!out.is_empty());
}

/// Two commands as the engine describes them: one that changes the model, one that reads it.
fn infos() -> Vec<Value> {
    vec![
        json!({ "id": "er.entity.add", "label": "Add entity", "journal": true, "params": [
            { "key": "schema", "kind": "string", "default": null, "range": null, "choices": [], "meaning": "the schema's id" },
            { "key": "name", "kind": "string", "default": "", "range": null, "choices": [], "meaning": "its name" },
            { "key": "width", "kind": "integer", "default": 240, "range": [40.0, 4000.0], "choices": [], "meaning": "its width" },
            { "key": "kind", "kind": "string", "default": "table", "range": null, "choices": ["table", "view"], "meaning": "what it is" },
            { "key": "unique", "kind": "bool", "default": false, "range": null, "choices": [], "meaning": "a flag" },
        ] }),
        json!({ "id": "er.foreign-key.list", "label": "List foreign keys", "journal": false, "params": [] }),
        json!({ "id": "view.select", "label": "Select", "journal": true, "undoable": false, "params": [] }),
        json!({ "id": "edit.undo", "label": "Undo", "journal": true, "params": [] }),
        // Would shadow a general tool, and has a kind this does not know: both left out.
        json!({ "id": "inspect.document", "label": "Shadow", "journal": false, "params": [] }),
        json!({ "id": "er.odd", "label": "Odd", "journal": true, "params": [{ "key": "x", "kind": "colour", "default": null, "range": null, "choices": [], "meaning": "?" }] }),
    ]
}

#[test]
fn every_command_becomes_a_tool_with_a_schema_from_its_documentation() {
    let catalog = Catalog::new(&infos(), false);
    let list = catalog.list();
    let tools = list["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 12, "the general nine, two commands and view.select");
    assert_eq!(tools.iter().filter(|t| t["name"] == "inspect_document").count(), 1, "never shadowed");
    let add = tools.iter().find(|t| t["name"] == "er_entity_add").unwrap();
    assert_eq!(add["inputSchema"]["required"], json!(["schema"]));
    assert_eq!(
        add["inputSchema"]["properties"]["width"],
        json!({ "type": "integer", "description": "its width", "default": 240, "minimum": 40.0, "maximum": 4000.0 })
    );
    assert_eq!(add["inputSchema"]["properties"]["kind"]["enum"], json!(["table", "view"]));
    assert_eq!(add["inputSchema"]["properties"]["unique"]["type"], "boolean");
    assert_eq!((add["annotations"]["readOnlyHint"].clone(), add["annotations"]["destructiveHint"].clone()), (json!(false), json!(true)));
    assert_eq!(add["inputSchema"]["properties"]["coalesce"]["type"], "string", "the engine's own key on undoable commands");
    let list_fk = tools.iter().find(|t| t["name"] == "er_foreign_key_list").unwrap();
    assert_eq!(list_fk["annotations"]["readOnlyHint"], true);
    assert!(list_fk["inputSchema"]["properties"].get("coalesce").is_none());
    // A change of the view alone: neither read-only nor destructive, and said so.
    let select = tools.iter().find(|t| t["name"] == "view_select").unwrap();
    assert_eq!((select["annotations"]["readOnlyHint"].clone(), select["annotations"]["destructiveHint"].clone()), (json!(false), json!(false)));
    assert!(select["description"].as_str().unwrap().contains("not recorded for undo"));
}

#[test]
fn a_command_tool_runs_its_command_and_refuses_what_it_does_not_take() {
    let fake = Fake::ok();
    let catalog = Catalog::new(&infos(), false);
    let call = |name: &str, args: Value| {
        handle(
            json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": { "name": name, "arguments": args } }).to_string().as_bytes(),
            &fake,
            &catalog,
        )
        .unwrap()
    };
    let r = call("er_entity_add", json!({ "schema": "s1", "name": "Invoice" }));
    assert_eq!(r["result"]["isError"], false);
    assert_eq!(
        fake.calls.borrow()[0],
        ("engine.execute".into(), json!({ "command": "er.entity.add", "params": { "schema": "s1", "name": "Invoice" } }))
    );
    assert!(call("er_entity_add", json!({ "name": "x" }))["result"]["content"][0]["text"].as_str().unwrap().contains("needs \"schema\""));
    assert!(
        call("er_entity_add", json!({ "schema": "s", "colour": "red" }))["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("no argument \"colour\"")
    );
    assert_eq!(fake.calls.borrow().len(), 1, "refused calls reach nothing");
    assert_eq!(call("er_entity_fly", json!({}))["error"]["code"], -32602);
}

#[test]
fn the_compact_listing_offers_a_search_and_the_tools_still_answer() {
    let fake = Fake::ok();
    let catalog = Catalog::new(&infos(), true);
    let list = catalog.list();
    let names: Vec<&str> = list["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert!(names.contains(&"search_tools") && !names.contains(&"er_entity_add"));
    let found = handle(
        json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": { "name": "search_tools", "arguments": { "query": "add entity" } } })
            .to_string()
            .as_bytes(),
        &fake,
        &catalog,
    )
    .unwrap();
    let text: Value = serde_json::from_str(found["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(text["commands"][0]["command"], "er.entity.add");
    assert_eq!(text["commands"][0]["annotations"]["destructiveHint"], true);
    assert_eq!(text["more"], 0);
    // What search finds runs through a listed tool.
    let r = handle(json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": { "name": "execute_command", "arguments": { "command": "er.entity.add", "params": { "schema": "s" } } } }).to_string().as_bytes(), &fake, &catalog).unwrap();
    assert_eq!(r["result"]["isError"], false);
    let huge = handle(
        json!({ "jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": { "name": "search_tools", "arguments": { "query": "a ".repeat(40) } } })
            .to_string()
            .as_bytes(),
        &fake,
        &catalog,
    )
    .unwrap();
    assert!(huge["result"]["content"][0]["text"].as_str().unwrap().contains("at most"));
    let init = handle(br#"{"jsonrpc":"2.0","id":5,"method":"initialize","params":{}}"#, &fake, &catalog).unwrap();
    assert!(init["result"]["instructions"].as_str().unwrap().contains("search_tools"));
    let r = handle(
        json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": { "name": "er_entity_add", "arguments": { "schema": "s" } } })
            .to_string()
            .as_bytes(),
        &fake,
        &catalog,
    )
    .unwrap();
    assert_eq!(r["result"]["isError"], false);
}

/// A stand-in that answers each method from a script, in turn.
struct Script(RefCell<Vec<(&'static str, Value, Result<Value, CallError>)>>);

impl Backend for Script {
    fn call(&self, method: &str, params: Value) -> Result<Value, CallError> {
        let mut next = self.0.borrow_mut();
        assert!(!next.is_empty(), "unexpected {method}");
        let (want, want_params, answer) = next.remove(0);
        assert_eq!((method, params), (want, want_params));
        answer
    }
}

fn save(script: &Script) -> Value {
    let r =
        ask(script, json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": { "name": "save_project", "arguments": {} } }))["result"]
            .clone();
    assert!(script.0.borrow().is_empty(), "every scripted call made");
    r
}

#[test]
fn save_project_follows_its_save_by_the_ticket_until_it_is_done() {
    let begun = || ("app.save", json!({}), Ok(json!({ "saving": true, "ticket": 7, "project": "Shop" })));
    let saved = |answer| ("app.saved", json!({ "ticket": 7 }), answer);
    let script = Script(RefCell::new(vec![
        begun(),
        saved(Ok(json!({ "saving": true }))),
        saved(Ok(json!({ "saving": false, "saved": true, "revision": 42 }))),
    ]));
    let r = save(&script);
    assert_eq!(r["isError"], false, "{r}");
    assert_eq!(
        serde_json::from_str::<Value>(r["content"][0]["text"].as_str().unwrap()).unwrap(),
        json!({ "saved": true, "project": "Shop", "revision": 42 })
    );
    // A save that fails says why.
    let script = Script(RefCell::new(vec![begun(), saved(Err(CallError::Refused("the save failed: the disk is full".into())))]));
    let r = save(&script);
    assert!(r["isError"].as_bool().unwrap() && r["content"][0]["text"].as_str().unwrap().contains("the disk is full"), "{r}");
    // Archforge gone while the save runs: started, outcome unknown.
    let script = Script(RefCell::new(vec![begun(), saved(Err(CallError::Unreachable("connection refused".into())))]));
    let text = save(&script)["content"][0]["text"].as_str().unwrap().to_string();
    assert!(text.contains("was started") && text.contains("unknown"), "{text}");
    // Nothing to save: answered at once.
    let script = Script(RefCell::new(vec![("app.save", json!({}), Ok(json!({ "saving": false, "saved": true, "unsaved": false })))]));
    assert_eq!(serde_json::from_str::<Value>(save(&script)["content"][0]["text"].as_str().unwrap()).unwrap()["saved"], true);
}
