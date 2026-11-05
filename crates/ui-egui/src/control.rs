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

use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

use serde_json::{Value, json};

use crate::{SaveState, Shell};

/// One request from the control channel, where its answer goes, and whether it may still run: the
/// one waiting for the answer gives up after a while, and a request given up is never run, so an
/// agent that tries again never applies a change twice.
pub struct ControlRequest {
    pub method: String,
    pub params: Value,
    pub reply: std::sync::mpsc::Sender<Result<Value, String>>,
    pub state: Arc<AtomicU8>,
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
    ("app.save", &[]),
    ("app.saved", &["ticket"]),
];

/// Answers the requests waiting on the channel, at most [`PER_FRAME`] of them.
pub(crate) fn drain(shell: &mut Shell) {
    for _ in 0..PER_FRAME {
        let Some(rx) = shell.control.as_ref() else { return };
        let Ok(req) = rx.try_recv() else { return };
        // A request its sender has given up on is not run.
        if req.state.compare_exchange(WAITING, TAKEN, Ordering::SeqCst, Ordering::SeqCst).is_err() {
            continue;
        }
        let answer = answer(shell, &req.method, &req.params);
        let _ = req.reply.send(answer);
    }
}

/// Answers one request.
pub fn answer(shell: &mut Shell, method: &str, params: &Value) -> Result<Value, String> {
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
    match method {
        "document.inspect" => Ok(inspect(shell)),
        "app.save" => save(shell),
        "app.saved" => saved(shell, params),
        "ui.inspect" => {
            Ok(json!({ "controls": shell.widgets().iter().map(|w| json!({ "id": w.id, "kind": w.kind, "rect": w.rect })).collect::<Vec<_>>() }))
        }
        _ => engine(shell, method, params),
    }
}

/// The open project, its history and what is selected and shown.
fn inspect(shell: &Shell) -> Value {
    let Some(session) = shell.session.as_ref() else {
        return json!({ "open": false, "projects": shell.tabs.iter().map(|t| t.name.clone()).collect::<Vec<_>>() });
    };
    let store = session.doc();
    let ids = |objs: &[archforge_mof::Obj]| objs.iter().filter_map(|o| store.uuid(*o).ok()).map(|u| u.to_string()).collect::<Vec<_>>();
    json!({
        "open": true,
        "project": shell.project.as_ref().map(|p| json!({ "name": p.name, "path": p.path.display().to_string() })),
        "revision": session.revision(),
        "unsaved": session.is_dirty() || crate::panels::description_pending(&shell.panels),
        "saving": shell.is_saving(),
        "save_error": shell.save_error,
        "undo": session.undo_label(),
        "redo": session.redo_label(),
        "selection": ids(&session.view().selection),
        "diagram": shell.canvas.active.map(|d| d.to_string()),
        "tabs": shell.canvas.tabs.iter().map(|t| t.to_string()).collect::<Vec<_>>(),
    })
}

/// Saves the open project as the Save button does: its changes are written, then the host makes a
/// version. Answers the save's ticket for `app.saved`, or why none could start, a description
/// being typed that the model refused among the reasons (AR-186).
fn save(shell: &mut Shell) -> Result<Value, String> {
    if shell.session.is_none() {
        return Err("no project is open in Archforge; open one first".into());
    }
    if shell.services.projects.is_none() || shell.project.is_none() {
        return Err("this window saves no projects".into());
    }
    // One save at a time: a second request follows the one under way.
    if !shell.is_saving() {
        let revision = shell.session.as_ref().map(|s| s.revision());
        shell.save();
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

fn engine(shell: &mut Shell, method: &str, params: &Value) -> Result<Value, String> {
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
            let out = session.execute(id, &args).map_err(|e| e.to_string())?;
            Ok(json!({ "result": out.value, "undo": session.undo_label() }))
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
            let outcome = session.batch(&label, |b| {
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
            Ok(json!({ "results": results, "undo": session.undo_label() }))
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

    #[test]
    fn a_request_given_up_on_is_never_taken() {
        let state = AtomicU8::new(WAITING);
        assert!(abandon(&state));
        assert!(state.compare_exchange(WAITING, TAKEN, Ordering::SeqCst, Ordering::SeqCst).is_err());
        let state = AtomicU8::new(TAKEN);
        assert!(!abandon(&state), "a request already running is not given up");
    }
}
