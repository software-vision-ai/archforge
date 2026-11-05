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
  mcp --bridge <port> [--compact]     serve MCP on the standard streams, bridged to Archforge started with --control <port>;
                                      --compact lists only the general tools and search_tools, not one tool per command
  app <method> [<json>] --port <port> send one request to the running Archforge and print its result
  commands [<filter>]                 list Archforge's commands
  describe <command>                  print one command's documentation
  --version | --help";

#[derive(Debug, PartialEq)]
enum Verb {
    Help,
    Version,
    Mcp { port: u16, compact: bool },
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
        "mcp" => match rest {
            [flag, p] if flag == "--bridge" => Ok(Verb::Mcp { port: port(Some(p))?, compact: false }),
            [flag, p, c] | [c, flag, p] if flag == "--bridge" && c == "--compact" => Ok(Verb::Mcp { port: port(Some(p))?, compact: true }),
            [flag] if flag == "--demo" => Err("the headless MCP server is not built yet; use --bridge <port>".into()),
            _ => Err("mcp takes --bridge <port> and optionally --compact".into()),
        },
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

/// The tools' backend in bridge mode: the token is read for every call, so the server can start
/// before Archforge and goes on across its restarts.
struct Bridge {
    settings: PathBuf,
    port: u16,
}

impl archforge_mcp::Backend for Bridge {
    fn call(&self, method: &str, params: Value) -> Result<Value, CallError> {
        Client::new(&self.settings, self.port)?.call(method, params)
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
        Verb::Mcp { port, compact } => {
            // The tools come from the command table this build carries, so they are listed even
            // before Archforge runs.
            let catalog = match infos() {
                Ok(all) => archforge_mcp::Catalog::new(&all, compact),
                Err(e) => return fail(&e, 1),
            };
            let bridge = Bridge { settings: archforge_control::settings_folder(), port };
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
        assert_eq!(parse(&args(&["mcp", "--bridge", "47000"])), Ok(Verb::Mcp { port: 47000, compact: false }));
        assert_eq!(parse(&args(&["mcp", "--bridge", "47000", "--compact"])), Ok(Verb::Mcp { port: 47000, compact: true }));
        assert_eq!(
            parse(&args(&["app", "engine.execute", "{\"command\":\"x\"}", "--port", "47000"])),
            Ok(Verb::App { method: "engine.execute".into(), params: json!({ "command": "x" }), port: 47000 })
        );
        for bad in [
            &[][..],
            &["mcp"],
            &["mcp", "--bridge", "0"],
            &["mcp", "--bridge", "70000"],
            &["app", "x"],
            &["app", "x", "--port", "1", "--force"],
            &["serve"],
            &["--help", "x"],
            &["app", "x", "{bad", "--port", "1"],
        ] {
            assert!(parse(&args(bad)).is_err(), "{bad:?}");
        }
        // A usage error exits 2 before any I/O.
        assert_eq!(run(&args(&["frobnicate"])), 2);
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

    /// The MCP instructions name only real tools and commands, promise what is built, and say the same scope in both modes.
    #[test]
    fn the_onboarding_names_only_what_the_table_has_and_both_modes_say_the_same_scope() {
        let all = infos().unwrap();
        let ids: Vec<&str> = all.iter().map(|c| c["id"].as_str().unwrap()).collect();
        let mut scopes = Vec::new();
        for compact in [false, true] {
            let catalog = archforge_mcp::Catalog::new(&all, compact);
            let list = catalog.list();
            let tools: Vec<&str> = list["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
            let init = archforge_mcp::handle(
                br#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#,
                &Recorder(std::cell::RefCell::new(Vec::new())),
                &catalog,
            )
            .unwrap();
            let text = init["result"]["instructions"].as_str().unwrap().to_string();
            for word in text.split(|c: char| !(c.is_ascii_lowercase() || c == '_' || c == '.')).map(|w| w.trim_matches('.')) {
                if word.contains('_') {
                    assert!(tools.contains(&word), "compact {compact}: {word} is not a listed tool");
                } else if word.contains('.') {
                    // A command id, or the kind diagram.add takes for a class diagram.
                    assert!(ids.contains(&word) || word == "uml.class", "compact {compact}: {word} is not a command");
                }
            }
            // Literals are built (AR-122): an agent is told it can edit them.
            for promise in
                ["UML class diagrams", "interfaces", "enumerations with their literals", "uml.literal.add", "ER diagrams", "agent access", "undo"]
            {
                let promise = if compact { promise.to_string() } else { promise.replace("uml.literal.add", "uml_literal_add") };
                assert!(text.contains(&promise), "compact {compact}: says nothing of {promise}");
            }
            assert!(!text.contains("C4"), "compact {compact}: advertises C4");
            scopes.push(text.split(". ").take(2).collect::<Vec<_>>().join(". "));
        }
        assert_eq!(scopes[0], scopes[1], "both modes describe the same scope and the same access");
    }

    #[test]
    fn the_offline_listing_is_the_registry() {
        let all = infos().unwrap();
        assert!(all.iter().any(|c| c["id"] == "er.entity.add"));
        assert!(all.iter().all(|c| c["params"].is_array()));
    }
}
