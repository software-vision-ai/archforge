//! The MCP server (agent-control-surfaces §10.5, BP-CS-4): JSON-RPC 2.0 over the standard
//! streams, one message per line, speaking protocol versions 2024-11-05, 2025-03-26 and 2025-06-18.
//! It runs in bridge mode: every tool is a request to the running Archforge through its control
//! channel ([`archforge_control::Client`]), so the window shows each change and undo covers it.
//! The headless mode is not built and not advertised (CS-3).
//!
//! Tools: `list_commands`, `describe_command`, `execute_command`, `batch`, `inspect_document`,
//! `undo`, `redo`, `save_project` and `call_app`, and one tool per command of the table (`er_entity_add` for
//! `er.entity.add`), its schema generated from the command's parameter documentation (BP-CS-4).
//! With `compact` the command tools are not listed and `search_tools` finds them, each to run
//! through `execute_command` (CS-14). The model and its skills are read-only resources too
//! (`resources`), in both modes. Only the reads say they are read-only (CS-13); unknown tools, methods and
//! argument keys are refused, naming the accepted ones (CS-3).
//!
//! With `read_only` the server offers only the tools whose `readOnlyHint` is true, and refuses a
//! call of any other with the typed error [`READ_ONLY`] naming the tool. Both the listing and the
//! refusal read the one hint each tool carries ([`Catalog`]'s entries), so they cannot drift apart.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unreachable, clippy::todo, clippy::unimplemented)]

mod resources;

use std::io::{BufRead, Write};

use archforge_control::{CallError, MAX_LINE, read_line};
use serde_json::{Value, json};

/// The protocol versions spoken, the newest last.
pub const VERSIONS: [&str; 3] = ["2024-11-05", "2025-03-26", "2025-06-18"];

/// The JSON-RPC error code of a tool refused because the session is read-only; the error's data
/// names the tool (`{"tool": name, "readOnly": true}`).
pub const READ_ONLY: i64 = -32004;

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

const TOOLS: [Tool; 9] = [
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
        name: "save_project",
        description: "Save the open project, as the Save button does: its changes become a version in the project's history. Waits until the save is done (up to 30 seconds) and gives the project and the revision saved.",
        schema: || object(json!({}), &[]),
        method: Some("app.save"),
        read_only: false,
        destructive: false,
    },
    Tool {
        name: "call_app",
        description: "Send any control-channel request to the running Archforge: engine.commands, engine.describe, engine.execute, engine.batch, engine.undo, engine.redo, document.inspect, ui.inspect (the controls drawn in the window), app.save (gives a ticket) or app.saved {ticket}.",
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

/// The fixed tools, `call_app` among them: each one's name, the control method it calls (none for
/// `call_app`, which calls any) and whether it says it only reads, for a test beside the
/// application to hold each hint to what its method can do.
pub fn fixed_tools() -> Vec<(&'static str, Option<&'static str>, bool)> {
    TOOLS.iter().map(|t| (t.name, t.method, t.read_only)).collect()
}

/// One command of the table as a tool: its tool name, the command it runs, what it does, its
/// arguments' schema, whether it only reads and whether it can remove anything (a change of the
/// view alone cannot).
#[derive(Debug, Clone, PartialEq)]
struct CommandTool {
    name: String,
    id: String,
    description: String,
    schema: Value,
    read_only: bool,
    destructive: bool,
}

/// Commands the general tools already offer, left out as tools of their own.
const COVERED: [&str; 2] = ["edit.undo", "edit.redo"];
/// The longest search, in bytes and in words.
const MAX_QUERY: usize = 256;
const MAX_WORDS: usize = 16;
/// The most tools one search gives.
const MAX_FOUND: usize = 20;

/// The tools a server offers: the fixed ones and one per command of the table, all of them or,
/// read-only, only those that read.
#[derive(Debug, Clone, Default)]
pub struct Catalog {
    commands: Vec<CommandTool>,
    compact: bool,
    read_only: bool,
}

/// Which kind of tool an entry is, and what it runs.
enum Kind<'a> {
    Fixed(&'a Tool),
    Search,
    Command(&'a CommandTool),
}

/// One tool as the server knows it: the one place its name, description, schema and hints are
/// read from, by `tools/list` and by `tools/call` alike. `listed` is false for a command's tool in
/// the compact mode, which runs when called but is found through `search_tools`, unless the
/// session is read-only: then the reads are few, and listed, since a client may call only what
/// it was offered.
struct Entry<'a> {
    kind: Kind<'a>,
    listed: bool,
}

impl Entry<'_> {
    fn name(&self) -> &str {
        match &self.kind {
            Kind::Fixed(t) => t.name,
            Kind::Search => SEARCH,
            Kind::Command(c) => &c.name,
        }
    }

    fn read_only(&self) -> bool {
        match &self.kind {
            Kind::Fixed(t) => t.read_only,
            Kind::Search => true,
            Kind::Command(c) => c.read_only,
        }
    }

    fn destructive(&self) -> bool {
        match &self.kind {
            Kind::Fixed(t) => t.destructive,
            Kind::Search => false,
            Kind::Command(c) => c.destructive,
        }
    }

    fn description(&self) -> &str {
        match &self.kind {
            Kind::Fixed(t) => t.description,
            Kind::Search => SEARCH_DOC,
            Kind::Command(c) => &c.description,
        }
    }

    fn schema(&self) -> Value {
        match &self.kind {
            Kind::Fixed(t) => (t.schema)(),
            Kind::Search => object(
                json!({ "query": { "type": "string", "description": "Words of the command's name or label, such as \"add relationship\"" } }),
                &["query"],
            ),
            Kind::Command(c) => c.schema.clone(),
        }
    }

    fn listing(&self) -> Value {
        listed(self.name(), self.description(), self.schema(), self.read_only(), self.destructive())
    }
}

/// A command id as a tool name: `er.foreign-key.add` becomes `er_foreign_key_add`.
pub fn tool_name(command: &str) -> String {
    command.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect()
}

impl Catalog {
    /// The catalogue for the commands `infos` describes (the engine's `CommandInfo`s as JSON).
    /// With `compact`, the command tools are left out of `tools/list` and found through
    /// `search_tools`.
    /// A command whose tool name another tool already has is left out, never shadowing it.
    pub fn new(infos: &[Value], compact: bool) -> Catalog {
        let mut commands: Vec<CommandTool> = Vec::new();
        for tool in infos.iter().filter_map(command_tool) {
            let taken = TOOLS.iter().any(|t| t.name == tool.name) || tool.name == SEARCH || commands.iter().any(|c| c.name == tool.name);
            if !taken {
                commands.push(tool);
            }
        }
        Catalog { commands, compact, read_only: false }
    }

    /// The catalogue offering only the tools that read, when `read_only`: the others are neither
    /// listed nor run.
    pub fn read_only(self, read_only: bool) -> Catalog {
        Catalog { read_only, ..self }
    }

    /// Whether the command tools are left out of the listing.
    pub fn compact(&self) -> bool {
        self.compact
    }

    /// Whether only the tools that read are offered.
    pub fn is_read_only(&self) -> bool {
        self.read_only
    }

    /// Every tool this server knows, offered or not: the fixed ones, `search_tools` in the compact
    /// mode, and one per command.
    fn entries(&self) -> impl Iterator<Item = Entry<'_>> {
        let fixed = TOOLS.iter().map(|t| Entry { kind: Kind::Fixed(t), listed: true });
        let search = self.compact.then_some(Entry { kind: Kind::Search, listed: true });
        let commands = self.commands.iter().map(|c| Entry { kind: Kind::Command(c), listed: !self.compact || self.read_only });
        fixed.chain(search).chain(commands)
    }

    /// Whether a tool with this hint is offered: every tool, or in a read-only session only those
    /// that say they only read.
    fn offers(&self, read_only: bool) -> bool {
        !self.read_only || read_only
    }

    /// The tools as `tools/list` gives them.
    pub fn list(&self) -> Value {
        let tools: Vec<Value> = self.entries().filter(|e| e.listed && self.offers(e.read_only())).map(|e| e.listing()).collect();
        json!({ "tools": tools })
    }
}

const SEARCH: &str = "search_tools";
const SEARCH_DOC: &str = "Find Archforge's commands by words of their name or label: each comes with its command id, its parameters' schema and hints; run one with execute_command {command, params}.";

fn listed(name: &str, description: &str, schema: Value, read_only: bool, destructive: bool) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": schema,
        "annotations": { "readOnlyHint": read_only, "destructiveHint": destructive, "openWorldHint": false },
    })
}

/// A command's tool, its schema from the documented parameters: a parameter without a default
/// is required, unless it is optional; choices, ranges and defaults carry over.
/// A command of a kind of parameter this does not know is left out, never described wrongly.
fn command_tool(info: &Value) -> Option<CommandTool> {
    let id = info.get("id")?.as_str()?;
    if COVERED.contains(&id) {
        return None;
    }
    let label = info.get("label").and_then(Value::as_str).unwrap_or(id);
    let read_only = !info.get("journal").and_then(Value::as_bool).unwrap_or(true);
    let undoable = info.get("undoable").and_then(Value::as_bool).unwrap_or(true);
    let mut properties = serde_json::Map::new();
    let mut required = Vec::new();
    for p in info.get("params").and_then(Value::as_array).into_iter().flatten() {
        let Some(key) = p.get("key").and_then(Value::as_str) else { continue };
        let kind = match p.get("kind").and_then(Value::as_str)? {
            "bool" => "boolean",
            k @ ("number" | "integer" | "string" | "array" | "object") => k,
            _ => return None,
        };
        let mut prop = json!({ "type": kind, "description": p.get("meaning").and_then(Value::as_str).unwrap_or_default() });
        match p.get("default") {
            // An optional key may be left out and has no default.
            _ if p.get("optional").and_then(Value::as_bool) == Some(true) => {}
            Some(Value::Null) | None => required.push(key.to_string()),
            Some(d) => prop["default"] = d.clone(),
        }
        if let Some(choices) = p.get("choices").and_then(Value::as_array).filter(|c| !c.is_empty()) {
            prop["enum"] = Value::Array(choices.clone());
        }
        if let Some([lo, hi]) = p.get("range").and_then(Value::as_array).map(Vec::as_slice) {
            prop["minimum"] = lo.clone();
            prop["maximum"] = hi.clone();
        }
        properties.insert(key.to_string(), prop);
    }
    if undoable && !read_only {
        // The engine's own key on every undoable command: calls with the same value merge into
        // one undo step.
        properties.insert(
            "coalesce".into(),
            json!({ "type": "string", "maxLength": 256, "description": "calls with the same value in a row become one undo step" }),
        );
    }
    let effect = match (read_only, undoable) {
        (true, _) => "Reads the model; changes nothing.",
        (false, true) => "Changes the model in the open project; the window shows it and undo takes it back.",
        (false, false) => "Changes the view (what is selected or open), not the model; not recorded for undo.",
    };
    Some(CommandTool {
        name: tool_name(id),
        id: id.to_string(),
        description: format!("{label} ({id}). {effect}"),
        schema: json!({ "type": "object", "properties": properties, "required": required, "additionalProperties": false }),
        read_only,
        destructive: !read_only && undoable,
    })
}

/// The fixed tools as `tools/list` gives them, without the command tools.
pub fn tool_list() -> Value {
    Catalog::default().list()
}

/// Serves `input` until it ends, writing a reply line to `output` for every request.
pub fn serve(input: &mut impl BufRead, output: &mut impl Write, backend: &dyn Backend, catalog: &Catalog) -> std::io::Result<()> {
    loop {
        let reply = match read_line(input, MAX_LINE) {
            Ok(None) => return Ok(()),
            Ok(Some(line)) => handle(&line, backend, catalog),
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
pub fn handle(line: &[u8], backend: &dyn Backend, catalog: &Catalog) -> Option<Value> {
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
                    "capabilities": { "tools": { "listChanged": false }, "resources": { "subscribe": false, "listChanged": false } },
                    "serverInfo": { "name": "archforge", "version": env!("CARGO_PKG_VERSION") },
                    "instructions": match (catalog.is_read_only(), catalog.compact()) {
                        (false, false) => INSTRUCTIONS,
                        (false, true) => COMPACT_INSTRUCTIONS,
                        (true, false) => READ_ONLY_INSTRUCTIONS,
                        (true, true) => READ_ONLY_COMPACT_INSTRUCTIONS,
                    },
                }),
            )
        }
        "ping" => result(id, json!({})),
        "tools/list" => result(id, catalog.list()),
        "tools/call" => call(id, &params, backend, catalog),
        "resources/list" => result(id, resources::list()),
        "resources/templates/list" => result(id, resources::templates()),
        "resources/read" => match params.get("uri").and_then(Value::as_str) {
            Some(uri) => match resources::read(backend, uri) {
                Ok(read) => result(id, read),
                Err((code, why)) => error(id, code, &why),
            },
            None => error(id, -32602, "resources/read needs uri"),
        },
        other => error(
            id,
            -32601,
            &format!(
                "unknown method {other:?}; the server answers initialize, ping, tools/list, tools/call, resources/list, resources/templates/list and resources/read"
            ),
        ),
    })
}

/// Runs a tool: its arguments checked against its schema's keys, then its control method, or for
/// a command's tool `engine.execute` with that command.
fn call(id: Value, params: &Value, backend: &dyn Backend, catalog: &Catalog) -> Value {
    let Some(name) = params.get("name").and_then(Value::as_str) else { return error(id, -32602, "tools/call needs name") };
    let args = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
    let Some(given) = args.as_object() else { return tool_error(id, "arguments must be an object") };
    let Some(entry) = catalog.entries().find(|e| e.name() == name) else {
        return error(id, -32602, &format!("unknown tool {name:?}; {}", close_tools(name, catalog)));
    };
    // The hint the listing went by decides: a tool not offered is refused before anything runs.
    if !catalog.offers(entry.read_only()) {
        return json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": {
                "code": READ_ONLY,
                "message": format!("{name} can change the model, the view or the project, and this session is read-only: it offers only the tools that read; changes need a session started without --read-only"),
                "data": { "tool": name, "readOnly": true },
            },
        });
    }
    let schema = entry.schema();
    let (method, request) = match entry.kind {
        Kind::Fixed(tool) => match tool.method {
            Some(m) => (m.to_string(), args.clone()),
            None => (
                given.get("method").and_then(Value::as_str).unwrap_or_default().to_string(),
                given.get("params").cloned().unwrap_or_else(|| json!({})),
            ),
        },
        Kind::Command(c) => ("engine.execute".to_string(), json!({ "command": c.id, "params": args })),
        Kind::Search => {
            return match given.get("query").and_then(Value::as_str) {
                Some(q) => search(id, catalog, q),
                None => tool_error(id, "search_tools needs \"query\""),
            };
        }
    };
    if let Err(why) = check(name, &schema, given) {
        return tool_error(id, &why);
    }
    let started = std::time::Instant::now();
    let answer = backend.call(&method, request);
    // A save goes on in the background: the tool waits until it is done.
    let answer = match answer {
        Ok(v) if name == "save_project" && v.get("saving") == Some(&Value::Bool(true)) => wait_for_save(backend, v, started),
        other => other,
    };
    match answer {
        Ok(value) => result(id, json!({ "content": [{ "type": "text", "text": value.to_string() }], "isError": false })),
        Err(CallError::Unreachable(e)) => tool_error(id, &format!("Archforge is not reachable: {e}")),
        Err(CallError::Refused(e)) => tool_error(id, &e),
    }
}

/// What every mode says first: what Archforge models and where the tools work.
macro_rules! scope {
    () => {
        "Archforge models UML class diagrams (packages; classes, interfaces and data types with their attributes and operations; enumerations with their literals; primitive types, beside the built-in Integer, String, Boolean, Real and UnlimitedNatural; associations, generalizations, dependencies and interface realizations; association classes, an association between two classes that is a class with its own attributes and operations; property modifiers on attributes and association ends: a default value read by the type, derived, read only, ordered, unique, and the properties it subsets or redefines, which the attribute commands and the association end's command set, a field left out keeping its value and clearing said explicitly (an empty default, an empty list), and the class read gives back; constraints owned by a classifier or a package, naming the elements they constrain, their expression kept exactly as written and never evaluated, each class diagram showing one as braces or as a linked note; generalization sets grouping generalizations to one general class, complete or incomplete and disjoint or overlapping, a generalization in a set keeping its ends and a set left without members going with them; qualifiers on an association's end, each a name, a type and a multiplicity, told apart by name within the end, drawn as a box at that end's class, the other end's multiplicity counting what one qualifier value picks; n-ary associations joining three or more classes in one association, drawn as a diamond with a leg to each class, a class at several ends when their roles differ, their ends never aggregations, an end added, moved or removed with its legs following and the last of three removed only by saying to make it binary; templates: a class, an interface or a data type with template parameters, each a class, interface, data type, enumeration or primitive type parameter (value and operation parameters refused as not supported yet) with an optional constraint and default, typing the template's own members, drawn as a dashed signature box on its card, a parameter a binding uses never removed; template bindings, a class, interface or data type binding a template of its kind with an argument per parameter (a parameter left out uses its default), drawn as a dashed line to the template labelled «bind» <T -> Order>, neither a dependency nor a realization, a classifier a binding uses as an argument never removed; nested classifiers, a class, an interface, an enumeration or a data type owned by a class, an interface or an association class instead of a package, its owner changed keeping its id, members, relationships and the types that use it, an owner deleted with what it nests; profiles of stereotypes, each extending metaclasses UML defines and Archforge draws and holding typed properties, applied to packages so their elements may use them, each stereotype applied to an element whose metaclass it extends with its property values set by type and multiplicity, a default kept apart from a value set, and a change that would strand a stereotype's uses or lose a value refused) and ER diagrams (schemas, entities, attributes, keys, relationships). Every tool works on the project open in the running Archforge window, which must have agent access on (started with --control <port>)."
    };
}
/// What every mode says of the resources.
macro_rules! resources {
    () => {
        "Resources give read-only context: archforge://project, archforge://skills, archforge://element/{id} and archforge://skills/effective/{id}/export, the skills in effect for generating code such as SQL DDL."
    };
}
const INSTRUCTIONS: &str = concat!(
    scope!(),
    " Each command has its own tool (uml_package_add, uml_class_add, uml_interface_add, uml_enumeration_add, uml_data_type_add, uml_primitive_type_add, uml_literal_add, uml_literal_rename, uml_literal_move, uml_attribute_add, uml_operation_add, uml_association_add, uml_association_add_nary, uml_realization_add, uml_association_class_add, uml_constraint_add, uml_constraint_show, uml_generalization_set_add, uml_qualifier_add, uml_template_parameter_add, uml_nested_add, uml_classifier_set_owner, uml_profile_add, uml_stereotype_add, uml_profile_apply, uml_stereotype_apply, uml_stereotype_set_value, er_schema_add, er_entity_add, er_attribute_add, er_relationship_add, diagram_add with kind uml.class or er, diagram_place, diagram_connect, element_remove, ...; ids come back in the results); batch runs several as one undo step; inspect_document tells what is open and selected; undo and redo step through the history; save_project saves the project, as the Save button does, once the model is done. ",
    resources!(),
    " Every change shows in the window at once and the user can undo it."
);
const COMPACT_INSTRUCTIONS: &str = concat!(
    scope!(),
    " search_tools finds a command (uml.class.add, uml.interface.add, uml.enumeration.add, uml.data-type.add, uml.primitive-type.add, uml.literal.add, uml.association.add, uml.association.add-nary, uml.realization.add, uml.association-class.add, uml.constraint.add, uml.generalization-set.add, uml.qualifier.add, uml.template-parameter.add, uml.nested.add, uml.classifier.set-owner, uml.profile.add, uml.stereotype.add, uml.profile.apply, uml.stereotype.apply, uml.stereotype.set-value, er.entity.add, er.relationship.add, diagram.add with kind uml.class or er, diagram.place, ...) with its parameters; execute_command {command, params} runs it and batch runs several as one undo step; inspect_document tells what is open and selected; undo and redo step through the history; save_project saves the project, as the Save button does, once the model is done. ",
    resources!(),
    " Every change shows in the window at once and the user can undo it."
);
const READ_ONLY_INSTRUCTIONS: &str = concat!(
    scope!(),
    " This session is read-only: it offers only the tools that read, and refuses any other. Each read has its own tool (uml_package_list, uml_class_list, uml_class_get, uml_relationship_list, uml_constraint_list, uml_generalization_set_list, uml_template_parameter_list, uml_nested_list, uml_classifier_owners, uml_profile_list, uml_stereotype_get, uml_stereotype_list_applied, uml_stereotype_applicable, er_schema_list, er_schema_get, er_entity_list, er_entity_get, diagram_list, diagram_get, skill_list, skill_effective, ...); list_commands and describe_command tell what the commands are, and inspect_document what is open and selected. ",
    resources!(),
    " Changing the model needs a session the user starts with changes allowed."
);
const READ_ONLY_COMPACT_INSTRUCTIONS: &str = concat!(
    scope!(),
    " This session is read-only: it offers only the tools that read, and refuses any other. Each read has its own tool (uml_class_list, uml_class_get, uml_relationship_list, er_entity_list, er_entity_get, diagram_get, skill_effective, ...), and search_tools finds one by words of its name with its parameters. list_commands and describe_command tell what the commands are, and inspect_document what is open and selected. ",
    resources!(),
    " Changing the model needs a session the user starts with changes allowed."
);

/// Up to five tool names close to an unknown one: those sharing its first word, else the general
/// tools.
fn close_tools(name: &str, catalog: &Catalog) -> String {
    let head = name.split('_').next().unwrap_or_default();
    let mut close: Vec<&str> = catalog
        .commands
        .iter()
        .filter(|c| catalog.offers(c.read_only))
        .map(|c| c.name.as_str())
        .filter(|n| !head.is_empty() && n.starts_with(head))
        .take(5)
        .collect();
    if close.is_empty() || catalog.compact() {
        close = TOOLS.iter().filter(|t| catalog.offers(t.read_only)).map(|t| t.name).take(5).collect();
    }
    format!("close ones are {}; tools/list names them all", close.join(", "))
}

/// How often `save_project` looks whether its save is done, and how long it waits in all, the
/// calls included, so it answers before an MCP client gives up on it.
const SAVE_POLL: std::time::Duration = std::time::Duration::from_millis(200);
const SAVE_WAIT: std::time::Duration = std::time::Duration::from_secs(30);

/// Waits until the save `begun` (app.save's answer, with its ticket) is done, and says how it went.
fn wait_for_save(backend: &dyn Backend, begun: Value, started: std::time::Instant) -> Result<Value, CallError> {
    let Some(ticket) = begun.get("ticket").and_then(Value::as_u64) else {
        return Err(CallError::Refused("the save started without a ticket to follow it by".into()));
    };
    let project = begun.get("project").cloned().unwrap_or(Value::Null);
    loop {
        if started.elapsed() > SAVE_WAIT {
            return Err(CallError::Refused(format!("save {ticket} is still running after {} seconds; it goes on in Archforge", SAVE_WAIT.as_secs())));
        }
        std::thread::sleep(SAVE_POLL);
        let state = match backend.call("app.saved", json!({ "ticket": ticket })) {
            Err(CallError::Unreachable(e)) => {
                return Err(CallError::Refused(format!(
                    "save {ticket} was started, but whether it finished is unknown: Archforge is not reachable: {e}"
                )));
            }
            other => other?,
        };
        if state.get("saving") != Some(&Value::Bool(true)) {
            return Ok(json!({ "saved": true, "project": project, "revision": state.get("revision").cloned().unwrap_or(Value::Null) }));
        }
    }
}

/// Refuses an argument the schema does not name and a required one missing.
fn check(name: &str, schema: &Value, given: &serde_json::Map<String, Value>) -> Result<(), String> {
    let keys: Vec<&str> = schema["properties"].as_object().map(|p| p.keys().map(String::as_str).collect()).unwrap_or_default();
    if let Some(key) = given.keys().find(|k| !keys.contains(&k.as_str())) {
        return Err(format!("{name} has no argument {key:?}; it takes {}", if keys.is_empty() { "none".to_string() } else { keys.join(", ") }));
    }
    let required = schema["required"].as_array().cloned().unwrap_or_default();
    match required.iter().filter_map(Value::as_str).find(|k| !given.contains_key(*k)) {
        Some(missing) => Err(format!("{name} needs {missing:?}")),
        None => Ok(()),
    }
}

/// The command tools whose name, command or description holds every word of `query`, at most 20.
fn search(id: Value, catalog: &Catalog, query: &str) -> Value {
    let words: Vec<String> = query.to_lowercase().split_whitespace().map(str::to_string).collect();
    if query.len() > MAX_QUERY || words.len() > MAX_WORDS {
        return tool_error(id, &format!("a search is at most {MAX_QUERY} bytes and {MAX_WORDS} words"));
    }
    let matching: Vec<&CommandTool> = catalog
        .commands
        .iter()
        .filter(|c| catalog.offers(c.read_only))
        .filter(|c| {
            let text = format!("{} {} {}", c.name, c.id, c.description).to_lowercase();
            words.iter().all(|w| text.contains(w.as_str()))
        })
        .collect();
    let found: Vec<Value> = matching
        .iter()
        .take(MAX_FOUND)
        .map(|c| {
            json!({
                "command": c.id,
                "tool": c.name,
                "description": c.description,
                "inputSchema": c.schema,
                "annotations": { "readOnlyHint": c.read_only, "destructiveHint": c.destructive },
            })
        })
        .collect();
    let more = matching.len().saturating_sub(MAX_FOUND);
    let run = if catalog.is_read_only() { "call the tool named in \"tool\" with the command's params" } else { "execute_command {command, params}" };
    let answer = json!({ "commands": found, "more": more, "run": run });
    result(id, json!({ "content": [{ "type": "text", "text": answer.to_string() }], "isError": false }))
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
