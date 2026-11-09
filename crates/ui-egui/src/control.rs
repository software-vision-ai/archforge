//! The shell's side of the control channel (agent-control-surfaces §10.5, BP-CS-1): the
//! application's loopback server posts each request here, and the shell answers it once per frame
//! through the open project's session, so the window shows every change at once and undo covers
//! it. No network code lives here (CS-17); the server is the application's.
//!
//! Methods: `engine.commands`, `engine.describe`, `engine.execute`, `engine.batch` (steps refer to
//! earlier results as `$N` or `$N.path`, `$$` writing a literal `$`, and a failing step rolls the
//! batch back: CS-12),
//! `engine.undo`, `engine.redo`, `document.inspect`, `ui.inspect`, `app.save` (a version of the
//! open project, as the Save button makes it; the answer, a ticket, comes before the host has
//! finished) and `app.saved` (how the save with that ticket went, whichever tab is shown by then).
//! Unknown methods and
//! parameter keys are refused, naming the accepted ones (CS-3).
//!
//! A request may come with an origin, the label of who sent it (`archforge-cli mcp --origin`), set
//! beside the parameters by the channel and checked there: the undo step an `engine.execute` or an
//! `engine.batch` makes carries it and is labelled `<origin>: <label>`, the result gives it back,
//! and `document.inspect` names the origin of the steps undo and redo would take.
//!
//! Each request also carries its token's role and liveness. A revoked token's request is refused
//! with [`REVOKED`] before it runs. A read-only token (a Chat conversation that does not allow
//! changes) gets only what reads: [`ACCESS`] classifies every method once, and `engine.execute` and
//! `engine.batch` read only when every command they run is a query by the engine's own test
//! (neither journaled nor undoable, run by its own handler). Everything else (a change, undo, redo,
//! save) is refused, so the read-only promise holds at the channel whatever the bridge does.
//!
//! A request can wait its turn after it is accepted, so it carries the window's session generation
//! from that moment: one from an earlier generation is refused and cannot land on a session it was
//! not sent to. While the project is held still, a request that may change anything is refused.
//!
//! While a version is shown or being read, whatever may change anything is refused the same way,
//! starting with [`VIEWING`]; what only reads is answered about what is shown, and
//! `document.inspect` names the version. Nothing is queued for later.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering};

use serde_json::{Value, json};

use crate::i18n::tr;
use crate::{SaveState, Shell};

/// One request from the control channel, where its answer goes, and whether it may still run: the
/// one waiting for the answer gives up after a while, and a request given up is never run, so an
/// agent that tries again never applies a change twice.
pub struct ControlRequest {
    pub method: String,
    pub params: Value,
    /// Who sent it, when the sender gave a label (already checked by the channel); a conversation's
    /// token carries its own label, and a request on it naming another is refused by the channel.
    pub origin: Option<String>,
    /// The key of the conversation the token speaks for, fixed by the channel when it granted the
    /// token, never taken from the request (AR-230); none for the person's own token.
    pub conversation: Option<String>,
    pub reply: std::sync::mpsc::Sender<Result<Value, String>>,
    pub state: Arc<AtomicU8>,
    /// Whether the token the request came with may only read (a Chat conversation that does not
    /// allow changes): a request that may change anything is refused ([`only_reads`]).
    pub read_only: bool,
    /// The liveness of that token: false once it is revoked or its server closed, and a request
    /// whose token is no longer live is refused with [`REVOKED`], never run.
    pub live: Arc<AtomicBool>,
    /// The window's session generation when the channel accepted the request.
    pub generation: u64,
}

/// The window's session generation, moved whenever another session takes the place of the one
/// shown and read by the channel as it accepts each request.
pub type SessionGeneration = Arc<AtomicU64>;

/// The answer to a request whose token was revoked before the window took it: the control
/// channel's own words.
pub const REVOKED: &str = "revoked: the access this token gave has ended";

/// The word a refusal while a version is in view starts with, the same in every language, so that
/// an agent can tell the reason without reading the sentence after it.
pub const VIEWING: &str = "viewing";

/// What a method may do: only read, change something, or run commands, which reads only when every
/// command it runs is a query.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    Reads,
    Writes,
    Commands,
}

/// Every method's access, the one place it is decided; a test holds that each method of
/// [`METHODS`] is here exactly once.
pub const ACCESS: [(&str, Access); 10] = [
    ("engine.commands", Access::Reads),
    ("engine.describe", Access::Reads),
    ("engine.execute", Access::Commands),
    ("engine.batch", Access::Commands),
    ("engine.undo", Access::Writes),
    ("engine.redo", Access::Writes),
    ("document.inspect", Access::Reads),
    ("ui.inspect", Access::Reads),
    ("app.save", Access::Writes),
    ("app.saved", Access::Reads),
];

/// Whether `method` with `params` only reads: a method that reads; `engine.execute` whose command
/// is a query by the engine's own test; `engine.batch` whose every step's command is a query (a
/// batch past [`MAX_STEPS`] is refused before its steps are looked at). An unknown method or
/// command, or a step without one, may change something. Without an open project nothing can be
/// changed, so a command method reads (and fails there as it would).
pub fn only_reads(shell: &Shell, method: &str, params: &Value) -> Result<bool, String> {
    let Some((_, access)) = ACCESS.iter().find(|(m, _)| *m == method) else { return Ok(false) };
    let query = |id: Option<&str>| match (shell.session.as_ref(), id) {
        (None, _) => true,
        (Some(s), Some(id)) => s.registry().is_query(id) == Some(true),
        (Some(_), None) => false,
    };
    match access {
        Access::Reads => Ok(true),
        Access::Writes => Ok(false),
        Access::Commands if method == "engine.batch" => {
            let Some(steps) = params.get("steps").and_then(Value::as_array) else { return Ok(false) };
            if steps.len() > MAX_STEPS {
                return Err(format!("a batch has 1 to {MAX_STEPS} steps"));
            }
            Ok(steps.iter().all(|step| query(step.get("command").and_then(Value::as_str))))
        }
        Access::Commands => Ok(query(params.get("command").and_then(Value::as_str))),
    }
}

/// A request's state: waiting to be answered, taken by the shell, given up by the one waiting.
pub const WAITING: u8 = 0;
pub const TAKEN: u8 = 1;
pub const ABANDONED: u8 = 2;

/// Gives up on a request: true when the shell had not taken it, so it will never run.
pub fn abandon(state: &AtomicU8) -> bool {
    state.compare_exchange(WAITING, ABANDONED, Ordering::SeqCst, Ordering::SeqCst).is_ok()
}

/// The most bytes the references of one batch step may bring into its parameters, and the most
/// a batch's results may hold, so a reference used many times cannot exhaust memory.
pub const MAX_REFERENCED: usize = 1024 * 1024;
pub const MAX_RESULTS: usize = 8 * 1024 * 1024;

/// The most requests answered in one frame; the rest wait for the next.
const PER_FRAME: usize = 32;
/// The most steps in one batch.
pub const MAX_STEPS: usize = 256;

/// Every method, with the parameter keys it takes.
pub const METHODS: [(&str, &[&str]); 10] = [
    ("engine.commands", &["filter"]),
    ("engine.describe", &["command"]),
    ("engine.execute", &["command", "params"]),
    ("engine.batch", &["label", "steps"]),
    ("engine.undo", &[]),
    ("engine.redo", &[]),
    ("document.inspect", &[]),
    ("ui.inspect", &[]),
    ("app.save", &["description"]),
    ("app.saved", &["ticket"]),
];

/// The methods that only read: they change neither the model, its history nor the view. Every
/// other method of [`METHODS`] may change something.
pub const READS: [&str; 5] = ["engine.commands", "engine.describe", "document.inspect", "ui.inspect", "app.saved"];

/// Answers the requests waiting on the channel, at most [`PER_FRAME`] of them.
pub(crate) fn drain(shell: &mut Shell) {
    for _ in 0..PER_FRAME {
        let Some(rx) = shell.control.as_ref() else { return };
        let Ok(req) = rx.try_recv() else { return };
        // A request its sender has given up on is not run.
        if req.state.compare_exchange(WAITING, TAKEN, Ordering::SeqCst, Ordering::SeqCst).is_err() {
            continue;
        }
        let answer = answer_request(shell, &req);
        let _ = req.reply.send(answer);
    }
}

/// Answers a request from the channel as its token allows: a revoked token's request is refused
/// with [`REVOKED`] and a read-only token's request that may change anything is refused naming
/// what it would change; neither runs.
pub fn answer_request(shell: &mut Shell, req: &ControlRequest) -> Result<Value, String> {
    if !req.live.load(Ordering::SeqCst) {
        return Err(REVOKED.to_string());
    }
    if req.generation != shell.session_generation() {
        return Err(tr("The project shown changed after this request was sent, so it was not run. Look at what is shown now and send it again."));
    }
    // A Chat conversation's request runs only while its turn runs in the project shown, and makes
    // the turn's tool rows from what it is answered.
    crate::chat_panel::gate(shell, req.origin.as_deref(), req.conversation.as_deref(), &req.method, &req.params)?;
    let before = crate::chat_panel::before(shell, &req.method, &req.params, req.origin.as_deref());
    // Before the hold's own refusal: a version being read holds the project too.
    if let Some(why) = viewing_refusal(shell, &req.method, &req.params) {
        let refused = Err(why);
        if let Some(b) = before {
            crate::chat_panel::after(shell, &req.method, b, &refused, true);
        }
        return refused;
    }
    if shell.is_held() && !only_reads(shell, &req.method, &req.params)? {
        let refused = Err(tr("The project is held still while it is being replaced, so this request was not run. Try again when that has finished."));
        if let Some(b) = before {
            crate::chat_panel::after(shell, &req.method, b, &refused, true);
        }
        return refused;
    }
    if req.read_only && !only_reads(shell, &req.method, &req.params)? {
        let what = match req.params.get("command").and_then(Value::as_str) {
            Some(id) if req.method == "engine.execute" => format!("the command {id:?} is not a query"),
            _ if req.method == "engine.batch" => "a step of the batch is not a query".to_string(),
            _ => format!("{} changes the project", req.method),
        };
        let refused = Err(format!("this conversation may only read: {what}; Allow changes in the conversation lets it change the model"));
        if let Some(b) = before {
            crate::chat_panel::after(shell, &req.method, b, &refused, true);
        }
        return refused;
    }
    // Every request on the channel is an agent's, with or without a label: the channel decides that.
    let answer = answer_from(shell, &req.method, &req.params, req.origin.as_deref(), true);
    if let Some(b) = before {
        crate::chat_panel::after(shell, &req.method, b, &answer, false);
    }
    answer
}

/// Why `method` with `params` may not run while a version is in view, naming that state and the way
/// back: none when no version is, or when it only reads. A batch over the cap keeps its own reason.
fn viewing_refusal(shell: &Shell, method: &str, params: &Value) -> Option<String> {
    shell.version_in_view()?;
    match only_reads(shell, method, params) {
        Ok(true) => None,
        Ok(false) => Some(format!(
            "{VIEWING}: {}",
            tr("Archforge is viewing an earlier version of this project, so this request was not run. Go back to current work, then send it again.")
        )),
        Err(why) => Some(why),
    }
}

/// Answers one request from the user's own side: its changes have no origin.
pub fn answer(shell: &mut Shell, method: &str, params: &Value) -> Result<Value, String> {
    answer_from(shell, method, params, None, false)
}

/// Answers one request sent on behalf of `origin`: the undo steps it makes carry the origin, and
/// a save it asks for is recorded as an agent's when `agent` says the channel brought it.
pub fn answer_from(shell: &mut Shell, method: &str, params: &Value, origin: Option<&str>, agent: bool) -> Result<Value, String> {
    let Some((_, keys)) = METHODS.iter().find(|(m, _)| *m == method) else {
        let all: Vec<&str> = METHODS.iter().map(|(m, _)| *m).collect();
        return Err(format!("unknown method {method:?}; the methods are {}", all.join(", ")));
    };
    let Some(given) = params.as_object() else { return Err("params must be an object".into()) };
    if let Some(key) = given.keys().find(|k| !keys.contains(&k.as_str())) {
        return Err(if keys.is_empty() {
            format!("{method} takes no parameters, not {key:?}")
        } else {
            format!("{method} has no parameter {key:?}; it takes {}", keys.join(", "))
        });
    }
    if let Some(why) = viewing_refusal(shell, method, params) {
        return Err(why);
    }
    match method {
        "document.inspect" => Ok(inspect(shell)),
        "app.save" => save(shell, params, agent.then_some(origin)),
        "app.saved" => saved(shell, params),
        "ui.inspect" => {
            Ok(json!({ "controls": shell.widgets().iter().map(|w| json!({ "id": w.id, "kind": w.kind, "rect": w.rect })).collect::<Vec<_>>() }))
        }
        _ => engine(shell, method, params, origin),
    }
}

/// The open project, its history and what is selected and shown; `viewing` names the version in
/// view, whether it is still being read (the work still shows then), and whether the work has
/// unsaved changes, which the rest of the answer does not describe once the version shows.
fn inspect(shell: &Shell) -> Value {
    let Some(session) = shell.session.as_ref() else {
        return json!({ "open": false, "projects": shell.tabs.iter().map(|t| t.name.clone()).collect::<Vec<_>>() });
    };
    let store = session.doc();
    let ids = |objs: &[archforge_mof::Obj]| objs.iter().filter_map(|o| store.uuid(*o).ok()).map(|u| u.to_string()).collect::<Vec<_>>();
    json!({
        "open": true,
        "viewing": shell.version_in_view().map(|version| json!({
            "version": version,
            "reading": shell.viewing().is_none(),
            "work_unsaved": shell.active.is_some_and(|i| shell.tab_unsaved(i)),
        })),
        "project": shell.project.as_ref().map(|p| json!({ "name": p.name, "path": p.path.display().to_string() })),
        "revision": session.revision(),
        "unsaved": session.is_dirty() || crate::panels::description_pending(&shell.panels),
        "saving": shell.is_saving(),
        "save_error": shell.save_error,
        "undo": session.undo_label(),
        "redo": session.redo_label(),
        "undo_origin": session.undo_origin(),
        "redo_origin": session.redo_origin(),
        "selection": ids(&session.view().selection),
        "diagram": shell.canvas.active.map(|d| d.to_string()),
        "tabs": shell.canvas.tabs.iter().map(|t| t.to_string()).collect::<Vec<_>>(),
    })
}

/// Saves the open project as the Save button does: its changes are written, then the host makes a
/// version. Answers the save's ticket for `app.saved`, or why none could start, a description
/// being typed that the model refused among the reasons (AR-186). `agent` holds an agent's label
/// (or none) when an agent asked; the version then records that. `description` in `params`, when
/// given, describes the version; what a save refuses about it comes back through `app.saved`.
fn save(shell: &mut Shell, params: &Value, agent: Option<Option<&str>>) -> Result<Value, String> {
    let description = match params.get("description") {
        None => None,
        Some(Value::String(text)) => Some(text.clone()),
        Some(_) => return Err(crate::i18n::tr("A description has to be text, so nothing was saved.")),
    };
    if shell.session.is_none() {
        return Err("no project is open in Archforge; open one first".into());
    }
    if shell.services.projects.is_none() || shell.project.is_none() {
        return Err("this window saves no projects".into());
    }
    // One save at a time: a second request follows the one under way, unless it would lose its text.
    if shell.is_saving() && description.is_some() {
        return Err(crate::i18n::tr("A save is under way, so this description was not used. Try again when it has finished."));
    }
    if !shell.is_saving() {
        let revision = shell.session.as_ref().map(|s| s.revision());
        let by = match agent {
            Some(origin) => crate::SavedBy::Agent(origin.map(str::to_string)),
            None => crate::SavedBy::Person(shell.state.user_name.clone()),
        };
        shell.save_as(by, description);
        if !shell.is_saving() {
            // A description being typed that the model refused stops the save: never reported saved.
            if crate::panels::refused_description(&shell.panels) {
                let why = shell.panels.notice.clone().unwrap_or_default();
                return Err(format!("the description being typed in Properties was refused, so nothing was saved: {why}"));
            }
            if revision.is_some() && shell.refused == revision {
                return Err(shell.save_error.clone().unwrap_or_else(|| "the model could not be written, so it cannot be saved".into()));
            }
            if shell.session.as_ref().is_some_and(|s| !s.is_dirty()) {
                return Ok(json!({ "saving": false, "saved": true, "unsaved": false }));
            }
            return Err("the save could not start: the project's changes have not reached it yet; try again in a moment".into());
        }
    }
    let project = shell.project.as_ref().map(|p| p.name.clone());
    Ok(json!({ "saving": true, "ticket": shell.save_ticket, "project": project }))
}

/// How the save with the given ticket went: still running, saved (the revision written), or why not.
fn saved(shell: &Shell, params: &Value) -> Result<Value, String> {
    let Some(ticket) = params.get("ticket").and_then(Value::as_u64) else { return Err("app.saved needs \"ticket\", a number app.save gave".into()) };
    match shell.save_state(ticket) {
        SaveState::Running => Ok(json!({ "saving": true })),
        SaveState::Saved(revision) => Ok(json!({ "saving": false, "saved": true, "revision": revision })),
        SaveState::Failed(e) => Err(format!("the save failed: {e}")),
        SaveState::Unknown => Err(format!("the project was closed before save {ticket} answered, so whether it was saved is not known")),
        SaveState::Forgotten => Err(format!("no save {ticket} is known: it may be too old to remember")),
    }
}

/// A change's answer: the result under `key`, the undo step now on top, and the origin it was made
/// for when there is one.
fn changed(key: &str, value: Value, undo: Option<&str>, origin: Option<&str>) -> Value {
    let mut answer = json!({ key: value, "undo": undo });
    if let Some(o) = origin {
        answer["origin"] = Value::String(o.to_string());
    }
    answer
}

fn engine(shell: &mut Shell, method: &str, params: &Value, origin: Option<&str>) -> Result<Value, String> {
    let Some(session) = shell.session.as_mut() else { return Err("no project is open in Archforge; open one first".into()) };
    let text = |key: &str| params.get(key).and_then(Value::as_str);
    match method {
        "engine.commands" => {
            let filter = text("filter").unwrap_or_default().to_lowercase();
            let infos = session.registry().infos(&session.facts());
            let listed: Vec<Value> = infos
                .iter()
                .filter(|c| filter.is_empty() || c.id.contains(&filter) || c.label.to_lowercase().contains(&filter))
                .map(|c| json!({ "id": c.id, "label": c.label, "enabled": c.enabled, "changes": c.journal }))
                .collect();
            Ok(json!({ "commands": listed }))
        }
        "engine.describe" => {
            let id = text("command").ok_or("engine.describe needs command, a command id")?;
            let infos = session.registry().infos(&session.facts());
            let info = infos.into_iter().find(|c| c.id == id).ok_or_else(|| format!("there is no command {id:?}; engine.commands lists them"))?;
            serde_json::to_value(info).map_err(|e| e.to_string())
        }
        "engine.execute" => {
            let id = text("command").ok_or("engine.execute needs command, a command id")?;
            let args = params.get("params").cloned().unwrap_or_else(|| json!({}));
            let out = session.execute_from(id, &args, origin).map_err(|e| e.to_string())?;
            Ok(changed("result", out.value, session.undo_label(), origin))
        }
        "engine.batch" => {
            let steps = params.get("steps").and_then(Value::as_array).ok_or("engine.batch needs steps, a list of {command, params}")?;
            if steps.is_empty() || steps.len() > MAX_STEPS {
                return Err(format!("a batch has 1 to {MAX_STEPS} steps"));
            }
            let label = text("label").unwrap_or("Agent changes").to_string();
            let mut results: Vec<Value> = Vec::with_capacity(steps.len());
            let mut held = 0usize;
            // A step's own problem, kept to report once the batch has rolled back.
            let mut problem: Option<String> = None;
            let outcome = session.batch_from(&label, origin, |b| {
                for (n, step) in steps.iter().enumerate() {
                    let mut stop = |why: String| {
                        problem = Some(format!("step {n}: {why}"));
                        archforge_engine::EngineError::NotAnObject
                    };
                    let Some(step) = step.as_object() else { return Err(stop("a step is {command, params}".into())) };
                    if let Some(key) = step.keys().find(|k| *k != "command" && *k != "params") {
                        return Err(stop(format!("a step has no key {key:?}; it has command and params")));
                    }
                    let Some(id) = step.get("command").and_then(Value::as_str) else { return Err(stop("a step needs command".into())) };
                    let mut budget = MAX_REFERENCED;
                    let args = match resolve(step.get("params").cloned().unwrap_or_else(|| json!({})), &results, &mut budget) {
                        Ok(args) => args,
                        Err(why) => return Err(stop(why)),
                    };
                    match b.execute(id, &args) {
                        Ok(v) => {
                            held = held.saturating_add(size(&v));
                            if held > MAX_RESULTS {
                                return Err(stop(format!("the batch's results pass {} MiB; split it", MAX_RESULTS / 1024 / 1024)));
                            }
                            results.push(v);
                        }
                        Err(e) => return Err(stop(e.to_string())),
                    }
                }
                Ok(())
            });
            if let Err(e) = outcome {
                return Err(format!("the batch was rolled back: {}", problem.unwrap_or_else(|| e.to_string())));
            }
            Ok(changed("results", Value::Array(results), session.undo_label(), origin))
        }
        "engine.undo" => session.undo().map(|label| json!({ "undone": label })).map_err(|e| e.to_string()),
        "engine.redo" => session.redo().map(|label| json!({ "redone": label })).map_err(|e| e.to_string()),
        _ => Err(format!("unknown method {method:?}")),
    }
}

/// A value's size as JSON, in bytes.
fn size(v: &Value) -> usize {
    serde_json::to_vec(v).map(|b| b.len()).unwrap_or(usize::MAX)
}

/// `value` with every string `$N` or `$N.path` replaced by step N's result, or the part of it the
/// dotted path names (object keys, list positions); a string starting `$$` keeps one `$` and is
/// otherwise left as it is. What the references bring in is taken from `budget`, refused past it.
fn resolve(value: Value, results: &[Value], budget: &mut usize) -> Result<Value, String> {
    match value {
        Value::String(s) if s.starts_with("$$") => Ok(Value::String(s.get(1..).unwrap_or_default().to_string())),
        Value::String(s) if s.starts_with('$') => {
            let mut parts = s.get(1..).unwrap_or_default().split('.');
            let n: usize = match parts.next().and_then(|n| n.parse().ok()) {
                Some(n) => n,
                None => return Ok(Value::String(s)),
            };
            let mut at = results.get(n).ok_or_else(|| format!("{s} refers to step {n}, which has not run"))?;
            for part in parts {
                at = match at {
                    Value::Object(o) => o.get(part),
                    Value::Array(a) => part.parse::<usize>().ok().and_then(|i| a.get(i)),
                    _ => None,
                }
                .ok_or_else(|| format!("{s}: step {n}'s result has no {part:?}"))?;
            }
            let cost = size(at);
            *budget = budget
                .checked_sub(cost)
                .ok_or_else(|| format!("the references of a step bring in more than {} KiB; refer to less", MAX_REFERENCED / 1024))?;
            Ok(at.clone())
        }
        Value::Array(items) => items.into_iter().map(|v| resolve(v, results, budget)).collect::<Result<Vec<_>, _>>().map(Value::Array),
        Value::Object(map) => {
            map.into_iter().map(|(k, v)| resolve(v, results, budget).map(|v| (k, v))).collect::<Result<serde_json::Map<_, _>, _>>().map(Value::Object)
        }
        other => Ok(other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn references_reach_into_earlier_results_within_a_budget() {
        let results = vec![json!({ "id": "a", "list": [1, { "x": 2 }] })];
        let mut budget = MAX_REFERENCED;
        let got = resolve(json!({ "e": "$0.id", "v": ["$0.list.1.x"], "keep": "$money", "literal": "$$5", "n": 3 }), &results, &mut budget).unwrap();
        assert_eq!(got, json!({ "e": "a", "v": [2], "keep": "$money", "literal": "$5", "n": 3 }));
        assert!(resolve(json!("$1"), &results, &mut budget).is_err());
        assert!(resolve(json!("$0.nope"), &results, &mut budget).is_err());
        // A large result referred to over and over runs out of budget instead of memory.
        let big = vec![json!({ "text": "x".repeat(100_000) })];
        let many = Value::Array(vec![json!("$0"); 1000]);
        let mut budget = MAX_REFERENCED;
        assert!(resolve(many, &big, &mut budget).unwrap_err().contains("KiB"));
    }

    /// A shell with a fresh project, its ids counted from 1.
    fn project() -> Shell {
        let mut n: u128 = 0;
        let ids = Box::new(move || {
            n += 1;
            archforge_mof::Uuid(n.to_le_bytes())
        });
        let mut shell = Shell::new(crate::Services::default());
        shell.session = Some(archforge_notations::new_project_session("Shop", ids).unwrap());
        shell
    }

    /// A request with `read_only` and a token whose liveness is `live`.
    fn request(method: &str, params: Value, read_only: bool, live: bool) -> ControlRequest {
        let (reply, _) = std::sync::mpsc::channel();
        ControlRequest {
            method: method.into(),
            params,
            origin: Some("Chat 1".into()),
            conversation: None,
            reply,
            state: Arc::new(AtomicU8::new(TAKEN)),
            read_only,
            live: Arc::new(AtomicBool::new(live)),
            generation: 0,
        }
    }

    /// Every method has one class, and `READS` lists exactly those that read.
    #[test]
    fn every_method_is_classified_exactly_once() {
        for (method, _) in METHODS {
            assert_eq!(ACCESS.iter().filter(|(m, _)| *m == method).count(), 1, "{method}");
        }
        assert_eq!(ACCESS.len(), METHODS.len(), "no class for a method that does not exist");
        let reads: Vec<&str> = ACCESS.iter().filter(|(_, a)| *a == Access::Reads).map(|(m, _)| *m).collect();
        assert_eq!(reads, READS.to_vec(), "READS lists exactly the methods that read");
    }

    /// Through execute and batch, a command reads exactly when the engine calls it a query; the batch cap comes first.
    #[test]
    fn a_command_reads_exactly_when_the_engine_calls_it_a_query() {
        let shell = project();
        let session = shell.session.as_ref().unwrap();
        let registry = session.registry().clone();
        let mut queries = 0;
        for id in registry.ids() {
            let query = registry.is_query(id).unwrap();
            queries += usize::from(query);
            let execute = only_reads(&shell, "engine.execute", &json!({ "command": id, "params": {} })).unwrap();
            let batch = only_reads(&shell, "engine.batch", &json!({ "steps": [{ "command": id }] })).unwrap();
            assert_eq!((execute, batch), (query, query), "{id}");
        }
        assert!(queries > 0 && queries < registry.len(), "the table has queries and changes");
        assert!(!only_reads(&shell, "engine.execute", &json!({ "command": "no.such" })).unwrap(), "an unknown command may change");
        assert!(!only_reads(&shell, "engine.execute", &json!({})).unwrap());
        let mixed = json!({ "steps": [{ "command": "er.schema.list" }, { "command": "er.schema.add", "params": { "name": "x" } }] });
        assert!(!only_reads(&shell, "engine.batch", &mixed).unwrap(), "one change makes the batch a change");
        // The step cap is checked before any step is looked at.
        let long = json!({ "steps": vec![json!({ "command": "er.schema.list" }); MAX_STEPS + 1] });
        assert!(only_reads(&shell, "engine.batch", &long).unwrap_err().contains("1 to 256"));
    }

    /// A read-only token's reads are answered and its changes refused unrun; a revoked token runs nothing; one that may change writes with its origin.
    #[test]
    fn a_read_only_token_reads_and_every_change_is_refused_unrun() {
        let mut shell = project();
        let revision = |s: &Shell| s.session.as_ref().unwrap().revision();
        let before = revision(&shell);
        for (method, params) in [
            ("document.inspect", json!({})),
            ("engine.commands", json!({ "filter": "er" })),
            ("engine.execute", json!({ "command": "er.schema.list" })),
            ("engine.batch", json!({ "steps": [{ "command": "er.schema.list" }, { "command": "er.schema.list" }] })),
        ] {
            assert!(answer_request(&mut shell, &request(method, params, true, true)).is_ok(), "{method} reads");
        }
        for (method, params) in [
            ("engine.execute", json!({ "command": "er.schema.add", "params": { "name": "x" } })),
            ("engine.batch", json!({ "steps": [{ "command": "er.schema.add", "params": { "name": "x" } }] })),
            ("engine.undo", json!({})),
            ("engine.redo", json!({})),
            ("app.save", json!({})),
        ] {
            let refused = answer_request(&mut shell, &request(method, params, true, true)).unwrap_err();
            assert!(refused.contains("may only read"), "{method}: {refused}");
        }
        assert_eq!(revision(&shell), before, "nothing changed");
        // A token that may change writes; a revoked one runs nothing, however it is allowed.
        let add = json!({ "command": "er.schema.add", "params": { "name": "shop" } });
        assert!(answer_request(&mut shell, &request("engine.execute", add.clone(), false, false)).unwrap_err() == REVOKED);
        assert_eq!(revision(&shell), before);
        let done = answer_request(&mut shell, &request("engine.execute", add, false, true)).unwrap();
        assert_eq!(done["origin"], "Chat 1");
        assert!(revision(&shell) > before);
    }

    /// While held, whatever may change anything is refused by the hold's own answer, whichever
    /// token asks, and what only reads is answered; afterwards the same request runs.
    #[test]
    fn while_held_a_changing_request_is_refused_naming_the_hold_and_reads_are_answered() {
        crate::i18n::set_language("en");
        let mut shell = project();
        let add = json!({ "command": "er.schema.add", "params": { "name": "shop" } });
        answer_request(&mut shell, &request("engine.execute", add.clone(), false, true)).unwrap();
        let before = shell.session.as_ref().unwrap().revision();
        shell.begin_hold().unwrap();
        for (method, params) in [
            ("engine.execute", add.clone()),
            ("engine.batch", json!({ "steps": [{ "command": "er.schema.add", "params": { "name": "x" } }] })),
            ("engine.undo", json!({})),
            ("engine.redo", json!({})),
            ("app.save", json!({})),
        ] {
            let refused = answer_request(&mut shell, &request(method, params, false, true)).unwrap_err();
            assert!(refused.contains("held still"), "{method}: {refused}");
        }
        assert_eq!(shell.session.as_ref().unwrap().revision(), before);
        for (method, params) in [("document.inspect", json!({})), ("engine.execute", json!({ "command": "er.schema.list" }))] {
            assert!(answer_request(&mut shell, &request(method, params, false, true)).is_ok(), "{method} reads");
        }
        shell.end_hold();
        let other = json!({ "command": "er.schema.add", "params": { "name": "billing" } });
        assert!(answer_request(&mut shell, &request("engine.execute", other, false, true)).is_ok(), "afterwards it runs");
    }

    /// A request accepted before the session was replaced is not run, even one that only reads;
    /// one accepted after it is.
    #[test]
    fn a_request_from_an_earlier_session_is_refused_and_a_later_one_runs() {
        crate::i18n::set_language("en");
        let mut shell = project();
        let add = json!({ "command": "er.schema.add", "params": { "name": "shop" } });
        let earlier = request("engine.execute", add.clone(), false, true);
        let reading = request("document.inspect", json!({}), false, true);
        let before = shell.session.as_ref().unwrap().revision();
        shell.session_replaced();
        for req in [&earlier, &reading] {
            let refused = answer_request(&mut shell, req).unwrap_err();
            assert!(refused.contains("after this request was sent"), "{refused}");
        }
        assert_eq!(shell.session.as_ref().unwrap().revision(), before, "nothing ran");
        let mut later = request("engine.execute", add, false, true);
        later.generation = shell.session_generation();
        assert!(answer_request(&mut shell, &later).is_ok());
    }

    #[test]
    fn a_request_given_up_on_is_never_taken() {
        let state = AtomicU8::new(WAITING);
        assert!(abandon(&state));
        assert!(state.compare_exchange(WAITING, TAKEN, Ordering::SeqCst, Ordering::SeqCst).is_err());
        let state = AtomicU8::new(TAKEN);
        assert!(!abandon(&state), "a request already running is not given up");
    }
}
