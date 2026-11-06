//! `archforge-cli`, Archforge's command-line tool (agent-control-surfaces §10.5, BP-CS-3): the MCP
//! server an agent starts, bridged to the running application, and one-off requests to that
//! application. Arguments are parsed by hand; an unknown verb or flag exits 2 before any I/O.
//!
//! Exit codes (BP-CS-6): 0 success, also for `--help`, `--version` and an output pipe closed after
//! the work; 1 a failure the application or a command reported; 2 a usage error; 3 no application
//! to reach (not running with `--control <port>`, or its token unreadable or not the current one).
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unreachable, clippy::todo, clippy::unimplemented)]

use std::io::Write;
use std::path::PathBuf;

use archforge_control::{CallError, Client};
use serde_json::{Value, json};

const USAGE: &str = "usage: archforge-cli <verb>
  mcp --bridge <port> [--compact] [--read-only] [--origin <label>]
                                      serve MCP on the standard streams, bridged to Archforge started with --control <port>;
                                      --compact lists only the general tools and search_tools, not one tool per command;
                                      --read-only offers only the tools that read and refuses every other;
                                      --origin labels every change this server makes, on its undo step and in its result
                                      (1 to 64 characters of printable text)
  app <method> [<json>] --port <port> send one request to the running Archforge and print its result
  commands [<filter>]                 list Archforge's commands
  describe <command>                  print one command's documentation
  --version | --help";

#[derive(Debug, PartialEq)]
enum Verb {
    Help,
    Version,
    Mcp { port: u16, compact: bool, read_only: bool, origin: Option<String> },
    App { method: String, params: Value, port: u16 },
    Commands { filter: String },
    Describe { id: String },
}

fn port(text: Option<&String>) -> Result<u16, String> {
    let text = text.ok_or("a port is needed")?;
    text.parse::<u16>().ok().filter(|p| *p > 0).ok_or_else(|| format!("{text:?} is not a port (1 to 65535)"))
}

fn parse(args: &[String]) -> Result<Verb, String> {
    let Some(verb) = args.first() else { return Err("a verb is needed".into()) };
    let rest = args.get(1..).unwrap_or_default();
    match verb.as_str() {
        "--help" | "-h" if rest.is_empty() => Ok(Verb::Help),
        "--version" | "-V" if rest.is_empty() => Ok(Verb::Version),
        "mcp" => mcp(rest),
        "app" => {
            let mut positional = Vec::new();
            let mut at = None;
            let mut i = 0;
            while let Some(a) = rest.get(i) {
                if a == "--port" {
                    at = Some(port(rest.get(i + 1))?);
                    i += 2;
                } else if a.starts_with("--") {
                    return Err(format!("app has no flag {a:?}"));
                } else {
                    positional.push(a.clone());
                    i += 1;
                }
            }
            let port = at.ok_or("app needs --port <port>")?;
            let (method, params) = match positional.as_slice() {
                [m] => (m.clone(), json!({})),
                [m, p] => (m.clone(), serde_json::from_str(p).map_err(|e| format!("the parameters are not JSON: {e}"))?),
                _ => return Err("app takes a method and optionally its parameters as JSON".into()),
            };
            Ok(Verb::App { method, params, port })
        }
        "commands" => match rest {
            [] => Ok(Verb::Commands { filter: String::new() }),
            [f] if !f.starts_with("--") => Ok(Verb::Commands { filter: f.to_lowercase() }),
            _ => Err("commands takes at most a filter".into()),
        },
        "describe" => match rest {
            [id] if !id.starts_with("--") => Ok(Verb::Describe { id: id.clone() }),
            _ => Err("describe takes a command id".into()),
        },
        other => Err(format!("unknown verb {other:?}")),
    }
}

/// The `mcp` verb's flags, in any order, each at most once.
fn mcp(rest: &[String]) -> Result<Verb, String> {
    const TAKES: &str = "mcp takes --bridge <port> and optionally --compact, --read-only and --origin <label>";
    let (mut at, mut compact, mut read_only, mut origin) = (None, false, false, None);
    let mut i = 0;
    while let Some(flag) = rest.get(i) {
        let twice = || format!("{flag} is given twice");
        match flag.as_str() {
            "--bridge" if at.is_none() => {
                at = Some(port(rest.get(i + 1))?);
                i += 1;
            }
            "--origin" if origin.is_none() => {
                let label = rest.get(i + 1).ok_or("--origin needs a label")?;
                // A flag after --origin is a label left out, never a label: `--origin --read-only`
                // must not start a session that may change the model.
                if label.starts_with("--") {
                    return Err(format!("--origin needs a label, not the flag {label}"));
                }
                archforge_control::check_origin(label).map_err(|e| format!("--origin {label:?}: {e}"))?;
                origin = Some(label.clone());
                i += 1;
            }
            "--compact" if !compact => compact = true,
            "--read-only" if !read_only => read_only = true,
            "--bridge" | "--origin" | "--compact" | "--read-only" => return Err(twice()),
            "--demo" => return Err("the headless MCP server is not built yet; use --bridge <port>".into()),
            other => return Err(format!("mcp has no flag {other:?}; {TAKES}")),
        }
        i += 1;
    }
    let port = at.ok_or(TAKES)?;
    Ok(Verb::Mcp { port, compact, read_only, origin })
}

/// The tools' backend in bridge mode: the token is read for every call, so the server can start
/// before Archforge and goes on across its restarts. The origin, set on the command line by the
/// one who started the server, goes with every request beside its parameters; nothing an agent
/// sends through the tools can set or change it.
struct Bridge {
    settings: PathBuf,
    port: u16,
    origin: Option<String>,
}

impl archforge_mcp::Backend for Bridge {
    fn call(&self, method: &str, params: Value) -> Result<Value, CallError> {
        Client::new(&self.settings, self.port)?.with_origin(self.origin.clone()).map_err(|e| CallError::Refused(e.to_string()))?.call(method, params)
    }
}

/// Writes `text` and a line break to standard output; a closed pipe is not a failure.
fn out(text: &str) -> Result<(), i32> {
    let mut o = std::io::stdout().lock();
    match writeln!(o, "{text}").and_then(|()| o.flush()) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => Err(0),
        Err(_) => Err(1),
    }
}

fn fail(text: &str, code: i32) -> i32 {
    let _ = writeln!(std::io::stderr(), "archforge-cli: {text}");
    code
}

/// The commands as a fresh project lists them, for the offline verbs.
fn infos() -> Result<Vec<Value>, String> {
    let mut n: u128 = 0;
    let ids = Box::new(move || {
        n = n.wrapping_add(1);
        archforge_mof::Uuid(n.to_le_bytes())
    });
    let session = archforge_notations::new_project_session("Model", ids)?;
    session.registry().infos(&session.facts()).into_iter().map(|i| serde_json::to_value(i).map_err(|e| e.to_string())).collect()
}

fn run(args: &[String]) -> i32 {
    let verb = match parse(args) {
        Ok(v) => v,
        Err(e) => return fail(&format!("{e}\n{USAGE}"), 2),
    };
    let done = |r: Result<(), i32>| r.err().unwrap_or(0);
    match verb {
        Verb::Help => done(out(USAGE)),
        Verb::Version => done(out(&format!("archforge-cli {}", env!("CARGO_PKG_VERSION")))),
        Verb::Mcp { port, compact, read_only, origin } => {
            // The tools come from the command table this build carries, so they are listed even
            // before Archforge runs.
            let catalog = match infos() {
                Ok(all) => archforge_mcp::Catalog::new(&all, compact).read_only(read_only),
                Err(e) => return fail(&e, 1),
            };
            let bridge = Bridge { settings: archforge_control::settings_folder(), port, origin };
            let mut input = std::io::stdin().lock();
            let mut output = std::io::stdout().lock();
            match archforge_mcp::serve(&mut input, &mut output, &bridge, &catalog) {
                Ok(()) => 0,
                Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => 0,
                Err(e) => fail(&format!("the MCP server stopped: {e}"), 1),
            }
        }
        Verb::App { method, params, port } => {
            let answer = Client::new(&archforge_control::settings_folder(), port).and_then(|c| c.call(&method, params));
            match answer {
                Ok(v) => done(out(&serde_json::to_string_pretty(&v).unwrap_or_default())),
                Err(CallError::Unreachable(e)) => fail(&e, 3),
                Err(CallError::Refused(e)) => fail(&e, 1),
            }
        }
        Verb::Commands { filter } => match infos() {
            Ok(all) => {
                let lines: Vec<String> = all
                    .iter()
                    .filter(|c| {
                        let (id, label) = (c["id"].as_str().unwrap_or_default(), c["label"].as_str().unwrap_or_default());
                        filter.is_empty() || id.contains(&filter) || label.to_lowercase().contains(&filter)
                    })
                    .map(|c| format!("{:<32} {}", c["id"].as_str().unwrap_or_default(), c["label"].as_str().unwrap_or_default()))
                    .collect();
                done(out(&lines.join("\n")))
            }
            Err(e) => fail(&e, 1),
        },
        Verb::Describe { id } => match infos() {
            Ok(all) => match all.into_iter().find(|c| c["id"] == id.as_str()) {
                Some(c) => done(out(&serde_json::to_string_pretty(&c).unwrap_or_default())),
                None => fail(&format!("there is no command {id:?}; `archforge-cli commands` lists them"), 1),
            },
            Err(e) => fail(&e, 1),
        },
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(run(&args));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }

    /// Every verb and flag parses as documented, and anything else is a usage error.
    #[test]
    fn arguments_are_parsed_by_hand_and_anything_unknown_is_a_usage_error() {
        assert_eq!(parse(&args(&["--help"])), Ok(Verb::Help));
        let mcp = |compact, read_only, origin: Option<&str>| Ok(Verb::Mcp { port: 47000, compact, read_only, origin: origin.map(str::to_string) });
        assert_eq!(parse(&args(&["mcp", "--bridge", "47000"])), mcp(false, false, None));
        assert_eq!(parse(&args(&["mcp", "--bridge", "47000", "--compact"])), mcp(true, false, None));
        assert_eq!(parse(&args(&["mcp", "--compact", "--bridge", "47000"])), mcp(true, false, None));
        assert_eq!(parse(&args(&["mcp", "--read-only", "--bridge", "47000"])), mcp(false, true, None));
        assert_eq!(parse(&args(&["mcp", "--bridge", "47000", "--origin", "Chat 1", "--read-only", "--compact"])), mcp(true, true, Some("Chat 1")));
        assert_eq!(parse(&args(&["mcp", "--origin", "Bestellungen prüfen", "--bridge", "47000"])), mcp(false, false, Some("Bestellungen prüfen")));
        assert_eq!(
            parse(&args(&["app", "engine.execute", "{\"command\":\"x\"}", "--port", "47000"])),
            Ok(Verb::App { method: "engine.execute".into(), params: json!({ "command": "x" }), port: 47000 })
        );
        for bad in [
            &[][..],
            &["mcp"],
            &["mcp", "--bridge", "0"],
            &["mcp", "--bridge", "70000"],
            &["mcp", "--compact"],
            &["mcp", "--bridge", "1", "--compact", "--compact"],
            &["mcp", "--bridge", "1", "--bridge", "2"],
            &["mcp", "--bridge", "1", "--read-only", "--read-only"],
            &["mcp", "--bridge", "1", "--readonly"],
            &["mcp", "--bridge", "1", "--origin"],
            &["mcp", "--bridge", "1", "--origin", ""],
            &["mcp", "--bridge", "1", "--origin", "Chat\n1"],
            &["mcp", "--bridge", "1", "--origin", "Chat\u{202E}1"],
            &["mcp", "--bridge", "1", "--origin", " Chat"],
            &["mcp", "--bridge", "1", "--origin", "a", "--origin", "b"],
            &["mcp", "--bridge", "1", "--origin", "--read-only"],
            &["mcp", "--bridge", "1", "--origin", "--compact"],
            &["mcp", "--demo"],
            &["app", "x"],
            &["app", "x", "--port", "1", "--force"],
            &["serve"],
            &["--help", "x"],
            &["app", "x", "{bad", "--port", "1"],
        ] {
            assert!(parse(&args(bad)).is_err(), "{bad:?}");
        }
        assert!(parse(&args(&["mcp", "--bridge", "1", "--origin", &"x".repeat(65)])).unwrap_err().contains("at most 64"));
        assert!(parse(&args(&["mcp", "--bridge", "1", "--origin", "Chat\u{202E}1"])).unwrap_err().contains("U+202E"));
        // A usage error exits 2 before any I/O.
        assert_eq!(run(&args(&["frobnicate"])), 2);
        assert_eq!(run(&args(&["mcp", "--bridge", "1", "--origin", "Chat\t1"])), 2);
    }

    #[test]
    fn without_a_running_archforge_a_request_exits_3() {
        // A port no Archforge listens on: its token file is missing.
        assert_eq!(run(&args(&["app", "document.inspect", "--port", "1"])), 3);
    }

    /// A stand-in for the application that records the commands the tools ask for.
    struct Recorder(std::cell::RefCell<Vec<Value>>);

    impl archforge_mcp::Backend for Recorder {
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

    #[test]
    fn every_command_of_the_table_is_a_tool_that_runs_it() {
        let all = infos().unwrap();
        let catalog = archforge_mcp::Catalog::new(&all, false);
        let list = catalog.list();
        let tools = list["tools"].as_array().unwrap();
        let mut names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert_eq!(
            tools.len(),
            all.len() + 9 - 2,
            "one tool per command beside the nine general ones, edit.undo and edit.redo left to undo and redo"
        );
        for n in &names {
            assert!(!n.is_empty() && n.len() <= 64 && n.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-'), "{n}");
        }
        names.sort();
        names.dedup();
        assert_eq!(names.len(), tools.len(), "tool names are unique");
        // Drive every command's tool with its required arguments: each reaches its own command, and
        // the real engine takes the arguments the schema calls for (it may refuse their values).
        let rec = Recorder(std::cell::RefCell::new(Vec::new()));
        let mut n: u128 = 0;
        let ids = Box::new(move || {
            n = n.wrapping_add(1);
            archforge_mof::Uuid(n.to_le_bytes())
        });
        let mut engine = archforge_notations::new_project_session("Model", ids).unwrap();
        let mut driven = 0;
        for c in &all {
            let id = c["id"].as_str().unwrap();
            if id == "edit.undo" || id == "edit.redo" {
                continue;
            }
            driven += 1;
            let tool = tools.iter().find(|t| t["name"] == archforge_mcp::tool_name(id)).unwrap();
            let schema = &tool["inputSchema"];
            let args: serde_json::Map<String, Value> = schema["required"]
                .as_array()
                .unwrap()
                .iter()
                .map(|k| (k.as_str().unwrap().to_string(), sample(&schema["properties"][k.as_str().unwrap()])))
                .collect();
            let r = archforge_mcp::handle(
                json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": { "name": tool["name"], "arguments": args } })
                    .to_string()
                    .as_bytes(),
                &rec,
                &catalog,
            )
            .unwrap();
            assert_eq!(r["result"]["isError"], false, "{id}: {r}");
            assert_eq!(rec.0.borrow().last().unwrap()["command"], id);
            use archforge_engine::EngineError as E;
            if let Err(e @ (E::UnknownKey { .. } | E::MissingKey { .. } | E::WrongType { .. })) = engine.execute(id, &Value::Object(args)) {
                panic!("{id}: the schema's arguments do not fit the command: {e}");
            }
        }
        assert_eq!(rec.0.borrow().len(), driven);
    }

    /// A stand-in for the application that records every request and answers it.
    struct Anything(std::cell::RefCell<Vec<(String, Value)>>);

    impl archforge_mcp::Backend for Anything {
        fn call(&self, method: &str, params: Value) -> Result<Value, CallError> {
            self.0.borrow_mut().push((method.to_string(), params));
            Ok(json!({ "result": null }))
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

    /// `--read-only` over the real command table: the listing is the full one less every tool
    /// whose hint is not read-only, and each of those is refused by name, with the arguments its
    /// schema asks for, before anything reaches Archforge; every read still runs.
    #[test]
    fn a_read_only_server_refuses_every_tool_that_can_change_the_model() {
        let all = infos().unwrap();
        for compact in [false, true] {
            let full = archforge_mcp::Catalog::new(&all, false).list();
            let catalog = archforge_mcp::Catalog::new(&all, compact).read_only(true);
            let listed = catalog.list();
            let listed = listed["tools"].as_array().unwrap();
            let reads: Vec<&Value> = full["tools"].as_array().unwrap().iter().filter(|t| t["annotations"]["readOnlyHint"] == true).collect();
            // Compact, the reads stay listed (a client calls only what it was offered) beside
            // search_tools.
            let offered: Vec<&Value> = listed.iter().filter(|t| t["name"] != "search_tools").collect();
            assert_eq!(offered, reads, "compact {compact}: the read-only listing is the full one's reads");
            assert_eq!(listed.iter().any(|t| t["name"] == "search_tools"), compact);
            let mut refused = 0;
            for tool in full["tools"].as_array().unwrap() {
                let name = tool["name"].as_str().unwrap();
                let app = Anything(std::cell::RefCell::new(Vec::new()));
                let r = archforge_mcp::handle(
                    json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": { "name": name, "arguments": required(tool) } })
                        .to_string()
                        .as_bytes(),
                    &app,
                    &catalog,
                )
                .unwrap();
                if tool["annotations"]["readOnlyHint"] == true {
                    assert_eq!(r["result"]["isError"], false, "compact {compact}: the read {name} runs: {r}");
                    assert_eq!(app.0.borrow().len(), 1, "{name}");
                } else {
                    refused += 1;
                    assert_eq!(r["error"]["code"], archforge_mcp::READ_ONLY, "compact {compact}: {name}: {r}");
                    assert_eq!(r["error"]["data"]["tool"], name);
                    assert!(r["error"]["message"].as_str().unwrap().contains(name), "{r}");
                    assert!(app.0.borrow().is_empty(), "compact {compact}: {name} reached Archforge: {:?}", app.0.borrow());
                }
            }
            assert!(refused > 50, "the general tools that change and every command tool that does: {refused}");
        }
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
        let mut engine = archforge_notations::new_project_session("Model", ids).unwrap();
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
                    // A command id, or the kind diagram.add takes for a class diagram.
                    assert!(ids.contains(&word) || word == "uml.class", "compact {compact}: {word} is not a command");
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

    #[test]
    fn the_offline_listing_is_the_registry() {
        let all = infos().unwrap();
        assert!(all.iter().any(|c| c["id"] == "er.entity.add"));
        assert!(all.iter().all(|c| c["params"].is_array()));
    }
}
