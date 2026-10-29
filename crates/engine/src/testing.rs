//! Helpers for the tests of crates that bring commands (the `testing` feature; never shipped).

use serde_json::Value;

use crate::{Document, Session, View};

/// Runs a command, then undoes and redoes it, and says whether each step gave the identical
/// document: before the command after the undo, and after it after the redo.
pub fn round_trip<D: Document + Clone + PartialEq + std::fmt::Debug, V: View<D>>(
    session: &mut Session<D, V>,
    id: &str,
    params: &Value,
) -> Result<(), String> {
    let before = session.doc().clone();
    session.execute(id, params).map_err(|e| format!("{id}: {e}"))?;
    let after = session.doc().clone();
    if after == before {
        return Ok(());
    }
    session.undo().map_err(|e| format!("undo of {id}: {e}"))?;
    if *session.doc() != before {
        return Err(format!("undo of {id} gave {:?}, not {before:?}", session.doc()));
    }
    session.redo().map_err(|e| format!("redo of {id}: {e}"))?;
    if *session.doc() != after {
        return Err(format!("redo of {id} gave {:?}, not {after:?}", session.doc()));
    }
    Ok(())
}

/// The longest a swept call may take.
const SLOW: std::time::Duration = std::time::Duration::from_secs(4);

/// The clock the sweep times calls with; the portable target has none, so calls go untimed there.
#[cfg(not(target_arch = "wasm32"))]
fn clock() -> Option<std::time::Instant> {
    Some(std::time::Instant::now())
}

#[cfg(target_arch = "wasm32")]
fn clock() -> Option<std::time::Instant> {
    None
}

/// The adversarial sweep: every command of the session's registry with hostile parameter sets
/// (nothing, wrong shapes, unknown keys, each documented key given every wrong type, huge and
/// odd numbers, long and control-character strings, deep arrays, and each of `ids`, the ids of every
/// kind the notation has, for every string key). No call may panic (a panic the session contains
/// still fails the sweep) or take longer than 4 s (timed, not interrupted); every refused call
/// must leave the document, the view, the counter, the history and the journal as they were. Gives
/// how many calls were made.
pub fn sweep<D: Document + Clone + PartialEq + std::fmt::Debug, V: View<D> + std::fmt::Debug>(
    session: &mut Session<D, V>,
    ids: &[&str],
) -> Result<usize, String> {
    sweep_with(session, ids, Clone::clone)
}

/// [`sweep`] for a document that is not cloned: `picture` gives what a refused call must leave as
/// it was.
pub fn sweep_with<D: Document, V: View<D> + std::fmt::Debug, P: PartialEq + std::fmt::Debug>(
    session: &mut Session<D, V>,
    ids: &[&str],
    picture: impl Fn(&D) -> P,
) -> Result<usize, String> {
    use serde_json::json;
    let commands: Vec<(String, Vec<String>)> = {
        let facts = session.facts();
        session.registry().infos(&facts).into_iter().map(|i| (i.id.to_string(), i.params.into_iter().map(|p| p.key).collect())).collect()
    };
    let long = "x".repeat(70 * 1024);
    let mut deep = json!(1);
    for _ in 0..40 {
        deep = json!([deep]);
    }
    let odd: Vec<serde_json::Value> = vec![
        json!(null),
        json!(true),
        json!(-1),
        json!(0),
        json!(1.5),
        json!(1e308),
        json!(-1e308),
        json!(u64::MAX),
        json!(""),
        json!("\u{0}\u{7}\n"),
        json!(long),
        json!([]),
        json!({}),
        deep.clone(),
    ];
    let mut calls = 0usize;
    for (id, keys) in &commands {
        let mut sets: Vec<serde_json::Value> = vec![json!(null), json!([]), json!("x"), json!(7), json!({ "zzz": 1 }), deep.clone()];
        for key in keys {
            for v in odd.iter().chain(ids.iter().map(|i| json!(i)).collect::<Vec<_>>().iter()) {
                sets.push(json!({ key.as_str(): v }));
                // Every other key filled with an id too, so a call can get past its first check.
                let mut all = serde_json::Map::new();
                for k in keys {
                    all.insert(k.clone(), if k == key { v.clone() } else { json!(ids.first().copied().unwrap_or("x")) });
                }
                sets.push(serde_json::Value::Object(all));
            }
        }
        for params in sets {
            let before = (
                picture(session.doc()),
                session.view().clone(),
                session.counter(),
                session.can_undo(),
                session.can_redo(),
                session.journal().count(),
            );
            calls += 1;
            let started = clock();
            let result = session.execute(id, &params);
            if let Some(took) = started.map(|s| s.elapsed())
                && took > SLOW
            {
                return Err(format!("{id} with {params} took {took:?}"));
            }
            if let Err(e @ crate::EngineError::Internal { .. }) = &result {
                return Err(format!("{id} with {params} panicked: {e}"));
            }
            if result.is_err() {
                let after = (
                    picture(session.doc()),
                    session.view().clone(),
                    session.counter(),
                    session.can_undo(),
                    session.can_redo(),
                    session.journal().count(),
                );
                if after != before {
                    return Err(format!("{id} with {params} was refused but changed the session"));
                }
            }
        }
    }
    Ok(calls)
}
