//! The model and its skills as MCP resources: read-only context an agent reads before it
//! generates something from the model (SQL DDL from a schema, say), beside the tools that read the
//! same. Every read goes through the running Archforge's control channel, as the tools do, and
//! carries the model's revision, so an agent can tell whether what it generated is still current.
//!
//! - `archforge://project`: the open project, its revision, what is selected and shown.
//! - `archforge://skills`: every skill, where it sits and when it applies.
//! - `archforge://element/{id}`: a schema (with its tables, keys and relationships), a table, a
//!   class, an interface, an enumeration (with its literals), a data type, a primitive type, an
//!   association class (with its ends) or a diagram. The attributes of what has them, and an
//!   association class's ends, come with their modifiers (AR-126), and a UML element with its
//!   attributes, operations and literals with their descriptions (AR-186); an attribute or an end
//!   is not read by its own id.
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
            "description": "A schema with its tables, a table, a class, an interface, an enumeration with its literals, a data type with its attributes and operations, a primitive type, an association class with its ends, attributes and operations, or a diagram, by id; each attribute and association end with its modifiers (default, derived, read_only, ordered, unique, subsets, redefines), an association end with its qualifiers; a UML element, its attributes, operations and literals each with its description; a classifier with its constraints, stored as written and never evaluated, and a template with its template parameters (kind, constraint, default and the bindings that use each). Packages and other elements are read through the tools.",
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
            // The class read gives an interface, an enumeration, a data type, a primitive type and
            // an association class too, and says which it is.
            let kind = match found.get("kind").and_then(Value::as_str) {
                Some("interface") if kind == "class" => "interface",
                Some("enumeration") if kind == "class" => "enumeration",
                Some("data_type") if kind == "class" => "data_type",
                Some("primitive_type") if kind == "class" => "primitive_type",
                Some("association_class") if kind == "class" => "association_class",
                _ => kind,
            };
            return Ok(json!({ "kind": kind, kind: found }));
        }
    }
    Err((
        NOT_FOUND,
        format!(
            "no schema, table, class, interface, enumeration, data type, primitive type, association class or diagram has the id {id:?}; packages and other elements are read through the tools"
        ),
    ))
}

/// The resource `uri` as a `resources/read` result, or why it cannot be read.
pub(crate) fn read(backend: &dyn Backend, uri: &str) -> Result<Value, Failure> {
    let Some(path) = uri.strip_prefix("archforge://") else {
        return Err((-32602, format!("unknown resource {uri:?}; the server reads {ACCEPTED}")));
    };
    // Each read is several control calls, and the user can edit between them: the project is read
    // again after the content, and a read that straddles an edit (or another project) is made
    // again, so the content always carries the revision it was read at.
    for _ in 0..READ_ATTEMPTS {
        let state = project(backend)?;
        let body = content(backend, uri, path, state.clone())?;
        let after = project(backend)?;
        if after.get("revision") == state.get("revision") && after.get("project") == state.get("project") {
            return Ok(json!({ "contents": [{ "uri": uri, "mimeType": JSON_TYPE, "text": body.to_string() }] }));
        }
    }
    Err((CHANGING, format!("the model changed each of the {READ_ATTEMPTS} times {uri} was read; read it again once the edits stop")))
}

/// The resources the server reads, as a refusal names them.
const ACCEPTED: &str = "archforge://project, archforge://skills, archforge://element/{id}, archforge://skills/effective/{id} and archforge://skills/effective/{id}/{trigger}";

/// How often a read straddling an edit is made again before it is refused.
const READ_ATTEMPTS: usize = 3;

/// The error a read gets when the model kept changing under it.
const CHANGING: i64 = -32003;

/// The resource at `path` read against `state`, the project as it was just before.
fn content(backend: &dyn Backend, uri: &str, path: &str, state: Value) -> Result<Value, Failure> {
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
        _ => return Err((-32602, format!("unknown resource {uri:?}; the server reads {ACCEPTED}"))),
    };
    Ok(body)
}

fn effective(backend: &dyn Backend, id: &str, trigger: &str, revision: Value) -> Result<Value, Failure> {
    let skills = query(backend, "skill.effective", json!({ "element": id, "trigger": trigger })).map_err(unreachable)?;
    Ok(
        json!({ "revision": revision, "element": id, "trigger": if trigger.is_empty() { Value::Null } else { json!(trigger) }, "skills": skills.get("skills").cloned() }),
    )
}
