//! The model and its skills as MCP resources (AR-64): read-only context an agent reads before it
//! generates something from the model (SQL DDL from a schema, say), beside the tools that read the
//! same. Every read goes through the running Archforge's control channel, as the tools do, and
//! carries the model's revision, so an agent can tell whether what it generated is still current.
//!
//! - `archforge://project`: the open project, its revision, what is selected and shown.
//! - `archforge://skills`: every skill, where it sits and when it applies.
//! - `archforge://element/{id}`: a schema (with its tables, keys and relationships), a table, a
//!   class or a diagram.
//! - `archforge://skills/effective/{id}` and `archforge://skills/effective/{id}/{trigger}`: the
//!   skills in effect on an element or a diagram, outermost first, with where each comes from and
//!   what overrides it, as `skill.effective` gives them.

use archforge_control::CallError;
use serde_json::{Value, json};

use crate::Backend;

/// JSON-RPC's code for a resource that does not exist (or no longer does).
const NOT_FOUND: i64 = -32002;
/// The triggers a skill may have, as `skill.effective` takes them.
const TRIGGERS: [&str; 4] = ["modelling", "drawing", "naming", "export"];
const JSON_TYPE: &str = "application/json";

/// The fixed resources.
pub(crate) fn list() -> Value {
    json!({ "resources": [
        {
            "uri": "archforge://project",
            "name": "project",
            "title": "Open project",
            "description": "The project open in Archforge: its name, model revision, selection and open diagram.",
            "mimeType": JSON_TYPE,
        },
        {
            "uri": "archforge://skills",
            "name": "skills",
            "title": "Skills",
            "description": "Every skill of the model: its name, the element it sits on, the kind it aims at, its triggers and dialect, and its text.",
            "mimeType": JSON_TYPE,
        },
    ] })
}

/// The resources an id or a trigger completes.
pub(crate) fn templates() -> Value {
    json!({ "resourceTemplates": [
        {
            "uriTemplate": "archforge://element/{id}",
            "name": "element",
            "title": "An element",
            "description": "A schema with its tables, a table, a class or a diagram, by id. Packages and other elements are read through the tools.",
            "mimeType": JSON_TYPE,
        },
        {
            "uriTemplate": "archforge://skills/effective/{id}",
            "name": "effective-skills",
            "title": "Skills in effect",
            "description": "The skills in effect on an element or a diagram, outermost first: where each comes from and what overrides it.",
            "mimeType": JSON_TYPE,
        },
        {
            "uriTemplate": "archforge://skills/effective/{id}/{trigger}",
            "name": "effective-skills-for",
            "title": "Skills in effect for a trigger",
            "description": "The same for one trigger: modelling, drawing, naming or export (export guides generating code such as SQL DDL).",
            "mimeType": JSON_TYPE,
        },
    ] })
}

/// What reading a resource failed with: a JSON-RPC error's code and message.
type Failure = (i64, String);

fn unreachable(e: CallError) -> Failure {
    match e {
        CallError::Unreachable(e) => (NOT_FOUND, format!("Archforge is not reachable: {e}")),
        CallError::Refused(e) => (NOT_FOUND, e),
    }
}

/// A query command's result, or why it was refused.
fn query(backend: &dyn Backend, command: &str, params: Value) -> Result<Value, CallError> {
    backend.call("engine.execute", json!({ "command": command, "params": params })).map(|v| v.get("result").cloned().unwrap_or(Value::Null))
}

/// The open project as `document.inspect` gives it; refused when none is open.
fn project(backend: &dyn Backend) -> Result<Value, Failure> {
    let state = backend.call("document.inspect", json!({})).map_err(unreachable)?;
    if state.get("open") != Some(&Value::Bool(true)) {
        return Err((NOT_FOUND, "no project is open in Archforge; open one first".into()));
    }
    Ok(state)
}

/// An element by id: the first of the reads that knows it. A schema's read holds its tables with
/// their columns and keys, and its relationships.
fn element(backend: &dyn Backend, id: &str) -> Result<Value, Failure> {
    for (kind, command, key) in [
        ("schema", "er.schema.get", "schema"),
        ("table", "er.entity.get", "entity"),
        ("class", "uml.class.get", "class"),
        ("diagram", "diagram.get", "diagram"),
    ] {
        if let Ok(found) = query(backend, command, json!({ key: id })) {
            return Ok(json!({ "kind": kind, kind: found }));
        }
    }
    Err((NOT_FOUND, format!("no schema, table, class or diagram has the id {id:?}; packages and other elements are read through the tools")))
}

/// The resource `uri` as a `resources/read` result, or why it cannot be read.
pub(crate) fn read(backend: &dyn Backend, uri: &str) -> Result<Value, Failure> {
    let accepted = "archforge://project, archforge://skills, archforge://element/{id}, archforge://skills/effective/{id} and archforge://skills/effective/{id}/{trigger}";
    let Some(path) = uri.strip_prefix("archforge://") else {
        return Err((-32602, format!("unknown resource {uri:?}; the server reads {accepted}")));
    };
    let state = project(backend)?;
    let revision = state.get("revision").cloned().unwrap_or(Value::Null);
    let parts: Vec<&str> = path.split('/').collect();
    let body = match parts.as_slice() {
        ["project"] => state,
        ["skills"] => json!({ "revision": revision, "skills": query(backend, "skill.list", json!({})).map_err(unreachable)?.get("skills").cloned() }),
        ["element", id] if !id.is_empty() => {
            let mut found = element(backend, id)?;
            if let Value::Object(m) = &mut found {
                m.insert("revision".into(), revision);
            }
            found
        }
        ["skills", "effective", id] | ["skills", "effective", id, ""] if !id.is_empty() => effective(backend, id, "", revision)?,
        ["skills", "effective", id, trigger] if !id.is_empty() => {
            if !TRIGGERS.contains(trigger) {
                return Err((-32602, format!("{trigger:?} is not a trigger; the triggers are {}", TRIGGERS.join(", "))));
            }
            effective(backend, id, trigger, revision)?
        }
        _ => return Err((-32602, format!("unknown resource {uri:?}; the server reads {accepted}"))),
    };
    Ok(json!({ "contents": [{ "uri": uri, "mimeType": JSON_TYPE, "text": body.to_string() }] }))
}

fn effective(backend: &dyn Backend, id: &str, trigger: &str, revision: Value) -> Result<Value, Failure> {
    let skills = query(backend, "skill.effective", json!({ "element": id, "trigger": trigger })).map_err(unreachable)?;
    Ok(
        json!({ "revision": revision, "element": id, "trigger": if trigger.is_empty() { Value::Null } else { json!(trigger) }, "skills": skills.get("skills").cloned() }),
    )
}
