//! The agent's tools over the app's real command table, ER's and UML's: each command tool's hints
//! are the table's own classification, and the MCP instructions name only what the table has and
//! promise what the notations build. The command-line tool lists and serves the same table; these
//! checks name ER's and UML's commands, so they live in the app, the one crate that holds both
//! notations (ADR 0005), not in the command-line tool, which names no notation command.
#![allow(clippy::unwrap_used, clippy::panic, reason = "test helpers sit outside the #[test] code that clippy.toml exempts")]

use archforge_control::CallError;
use serde_json::{Value, json};

/// The commands as a fresh project of the app's notations lists them, as the command-line tool's
/// offline verbs list them.
fn infos() -> Result<Vec<Value>, String> {
    let mut n: u128 = 0;
    let ids = Box::new(move || {
        n = n.wrapping_add(1);
        archforge_mof::Uuid(n.to_le_bytes())
    });
    let session = archforge_notations::NOTATIONS.new_project_session("Model", ids)?;
    session.registry().infos(&session.facts()).into_iter().map(|i| serde_json::to_value(i).map_err(|e| e.to_string())).collect()
}

/// A stand-in for the application that records the commands the tools ask for.
struct Recorder(std::cell::RefCell<Vec<Value>>);

impl archforge_mcp::Backend for Recorder {
    /// Records the parameters of an `engine.execute` request, the one method the tools send, and
    /// answers with no result.
    fn call(&self, method: &str, params: Value) -> Result<Value, CallError> {
        assert_eq!(method, "engine.execute");
        self.0.borrow_mut().push(params);
        Ok(json!({ "result": null }))
    }
}

/// The smallest value of a schema property's type.
fn sample(p: &Value) -> Value {
    if let Some(first) = p["enum"].as_array().and_then(|e| e.first()) {
        return first.clone();
    }
    match p["type"].as_str() {
        Some("integer" | "number") => p.get("minimum").cloned().unwrap_or(json!(0)),
        Some("boolean") => json!(false),
        Some("array") => json!([]),
        Some("object") => json!({}),
        _ => json!("x"),
    }
}

/// Every tool's arguments as its schema requires them.
fn required(tool: &Value) -> Value {
    let schema = &tool["inputSchema"];
    let args: serde_json::Map<String, Value> = schema["required"]
        .as_array()
        .unwrap()
        .iter()
        .map(|k| (k.as_str().unwrap().to_string(), sample(&schema["properties"][k.as_str().unwrap()])))
        .collect();
    Value::Object(args)
}

/// Each command tool's hints are the command table's own classification: read-only exactly
/// for a query (neither journaled nor undoable), which the engine refuses to let change
/// anything, and destructive exactly for an undoable change. Run on a model, every read leaves
/// the model, its history and its journal as they were.
#[test]
fn every_read_only_hint_is_the_command_tables_own_and_reads_change_nothing() {
    let all = infos().unwrap();
    let catalog = archforge_mcp::Catalog::new(&all, false);
    let list = catalog.list();
    let tools = list["tools"].as_array().unwrap();
    let mut n: u128 = 0;
    let ids = Box::new(move || {
        n = n.wrapping_add(1);
        archforge_mof::Uuid(n.to_le_bytes())
    });
    let mut engine = archforge_notations::NOTATIONS.new_project_session("Model", ids).unwrap();
    let schema = engine.execute("er.schema.add", &json!({ "name": "shop" })).unwrap().value["id"].as_str().unwrap().to_string();
    engine.execute("er.entity.add", &json!({ "schema": schema, "name": "Customer" })).unwrap();
    let package = engine.execute("uml.package.add", &json!({ "name": "sales" })).unwrap().value["id"].as_str().unwrap().to_string();
    engine.execute("uml.class.add", &json!({ "package": package, "name": "Order" })).unwrap();
    let mut reads = 0;
    for c in &all {
        let id = c["id"].as_str().unwrap();
        let Some(tool) = tools.iter().find(|t| t["name"] == archforge_mcp::tool_name(id)) else {
            assert!(id == "edit.undo" || id == "edit.redo", "{id} has no tool");
            continue;
        };
        let (journal, undoable) = (c["journal"].as_bool().unwrap(), c["undoable"].as_bool().unwrap());
        let read_only = tool["annotations"]["readOnlyHint"].as_bool().unwrap();
        assert_eq!(read_only, !journal && !undoable, "{id}: readOnlyHint against the table's journal {journal}, undoable {undoable}");
        assert_eq!(tool["annotations"]["destructiveHint"].as_bool().unwrap(), journal && undoable, "{id}");
        if read_only {
            reads += 1;
            let before = (engine.revision(), engine.undo_label().map(str::to_string), engine.journal().count(), engine.can_redo());
            let _ = engine.execute(id, &required(tool));
            let after = (engine.revision(), engine.undo_label().map(str::to_string), engine.journal().count(), engine.can_redo());
            assert_eq!(before, after, "{id} said it only reads");
        }
    }
    assert!(reads >= 15, "{reads} reads");
}

/// The MCP instructions name only real tools and commands, promise what is built, and say the same scope in both modes.
#[test]
fn the_onboarding_names_only_what_the_table_has_and_both_modes_say_the_same_scope() {
    let all = infos().unwrap();
    let ids: Vec<&str> = all.iter().map(|c| c["id"].as_str().unwrap()).collect();
    let mut scopes = Vec::new();
    let kinds = archforge_notations::NOTATIONS.kinds();
    for (compact, read_only) in [(false, false), (true, false), (false, true), (true, true)] {
        let catalog = archforge_mcp::Catalog::new(&all, compact).read_only(read_only);
        let list = catalog.list();
        let tools: Vec<&str> = list["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
        let init = archforge_mcp::handle(
            br#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#,
            &Recorder(std::cell::RefCell::new(Vec::new())),
            &catalog,
        )
        .unwrap();
        let text = init["result"]["instructions"].as_str().unwrap().to_string();
        // A command id may hold a hyphen (`uml.data-type.add`), so a word runs through one.
        for word in text.split(|c: char| !(c.is_ascii_lowercase() || c == '_' || c == '.' || c == '-')).map(|w| w.trim_matches('.')) {
            if word.contains('_') {
                assert!(tools.contains(&word), "compact {compact}: {word} is not a listed tool");
            } else if word.contains('.') {
                // A command id, or a diagram kind diagram.add takes, such as the class diagram's.
                assert!(ids.contains(&word) || kinds.contains(&word), "compact {compact}: {word} is not a command");
            }
        }
        // Literals are built (AR-122), data types (AR-123), primitive types (AR-124) and
        // association classes (AR-125): an agent is told it can make them.
        for promise in [
            "UML class diagrams",
            "interfaces",
            "data types with their attributes and operations",
            "enumerations with their literals",
            "uml.literal.add",
            "uml.data-type.add",
            "primitive types, beside the built-in Integer, String, Boolean, Real and UnlimitedNatural",
            "uml.primitive-type.add",
            "association classes, an association between two classes that is a class with its own attributes and operations",
            "uml.association-class.add",
            "ER diagrams",
            "agent access",
            "undo",
        ] {
            // A read-only session promises no changes.
            if read_only && (promise.starts_with("uml.") || promise == "undo") {
                continue;
            }
            // A command's id in the compact mode, its tool's name in the other.
            let promise = if compact || !promise.starts_with("uml.") { promise.to_string() } else { archforge_mcp::tool_name(promise) };
            assert!(text.contains(&promise), "compact {compact}: says nothing of {promise}");
        }
        assert!(!text.contains("C4"), "compact {compact}: advertises C4");
        assert_eq!(text.contains("This session is read-only"), read_only, "compact {compact}: says whether it may change the model");
        scopes.push(text.split(". ").take(2).collect::<Vec<_>>().join(". "));
    }
    assert!(scopes.iter().all(|s| *s == scopes[0]), "every mode describes the same scope and the same access");
}
