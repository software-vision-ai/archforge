//! The session: the document and the view, the commands that change them, and the history.
//!
//! Every call is validated first. Its handler then changes the live document while the document
//! records the change as a delta; a refused or failed call, and a panic in the notation's code, is
//! rolled back by applying the inverse delta, so it changes nothing. Undo and redo apply the stored
//! deltas backwards and forwards. If a rollback itself fails the session is poisoned: every edit is
//! refused from then on, and the project has to be opened again. The document's states carry ids: a
//! change takes a fresh one, undo and redo restore the one stored with the step, and the project is
//! unsaved exactly when the current id is not the saved one.

use std::collections::VecDeque;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use serde::Serialize;
use serde_json::{Value, json};

use crate::cmd::{CommandSpec, Ctx, Facts, Handler, Registry};
use crate::params::{self, ParamDoc, Params};
use crate::{CommandError, Document, EngineError, View};

/// Why a call that ran was still refused.
pub const QUERY_CHANGED: &str = "a query changed the project";
pub const NO_UNDO_STEP: &str = "changed the document without an undo step";
pub const NOT_A_GESTURE: &str = "only an undoable command can be a gesture";
pub const BATCH_TOO_LONG: &str = "a batch holds at most 10000 commands";
pub const COUNTER_ONLY: &str = "took a new id without changing the document";
/// The most commands one batch may run, and the longest batch label in bytes.
pub const MAX_BATCH: usize = 10_000;
pub const MAX_LABEL: usize = 256;

/// The history's and the journal's caps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Options {
    pub max_steps: usize,
    /// The total weight of the undo and redo deltas, in bytes as `Document::weight` estimates it.
    pub max_weight: usize,
    pub max_journal: usize,
    pub max_journal_bytes: usize,
}

impl Default for Options {
    fn default() -> Self {
        Options { max_steps: 500, max_weight: 256 * 1024 * 1024, max_journal: 10_000, max_journal_bytes: 64 * 1024 * 1024 }
    }
}

/// What an applied call reports: the handler's result, and whether the history had to be cleared
/// because one step was heavier than the whole cap.
#[derive(Debug, Clone, PartialEq)]
pub struct Outcome {
    pub value: Value,
    pub history_cleared: bool,
}

/// One journal entry: what was applied, as it can be run again.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum JournalEntry {
    Command { id: String, params: Value },
    Batch { label: String, commands: Vec<(String, Value)> },
}

/// How many history steps and journal entries the caps dropped or cleared.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Dropped {
    pub steps: u64,
    pub journal: u64,
}

/// One undo or redo step: the forward delta, the state id before it, its label and weight.
struct Step<X> {
    delta: X,
    id: u64,
    label: String,
    weight: usize,
}

/// A gesture in progress: the command it runs, the view and counter before it, and its last good
/// preview with the delta that made it.
struct Interaction<X, V> {
    id: &'static str,
    view: V,
    next: u64,
    last: Option<(Params, Value, X)>,
}

/// What a recorded run produced.
struct Ran<X> {
    value: Value,
    delta: Option<X>,
    view_changed: bool,
    next_changed: bool,
}

pub struct Session<D: Document, V> {
    registry: Arc<Registry<D, V>>,
    options: Options,
    doc: D,
    view: V,
    /// The counter new ids come from: outside the document, never undone.
    counter: u64,
    id: u64,
    next_id: u64,
    /// The state last saved; none when this session holds changes no state of it saved (it was
    /// opened over uncommitted changes).
    saved: Option<u64>,
    undo: Vec<Step<D::Delta>>,
    redo: Vec<Step<D::Delta>>,
    weight: usize,
    /// The command and coalescing key of the step the immediately preceding execute pushed.
    chain: Option<(&'static str, String)>,
    /// Each entry with its size and the counter before it.
    journal: VecDeque<(JournalEntry, usize, u64)>,
    journal_bytes: usize,
    dropped: Dropped,
    interaction: Option<Interaction<D::Delta, V>>,
    /// Why the session can no longer be trusted, once a rollback failed.
    poisoned: Option<String>,
}

/// Runs `f`, turning a panic into an error that names the command; on wasm32 a panic aborts, so
/// this is a plain call there.
pub(crate) fn guard<T>(id: &str, f: impl FnOnce() -> Result<T, EngineError>) -> Result<T, EngineError> {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(r) => r,
        Err(payload) => {
            let message = payload
                .downcast_ref::<&str>()
                .map(|s| (*s).to_string())
                .or_else(|| payload.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "a panic without a message".to_string());
            log::error!("{id} panicked: {message}");
            Err(EngineError::Internal { id: id.to_string(), message })
        }
    }
}

fn failed(id: &str, message: &str) -> EngineError {
    EngineError::Failed { id: id.to_string(), error: CommandError::new(message) }
}

fn disabled(id: &str, reason: &'static str) -> EngineError {
    EngineError::Disabled { id: id.to_string(), reason }
}

impl<D: Document, V: View<D>> Session<D, V> {
    /// A session on `doc` with the id counter at `counter`, and the view the notation's default
    /// gives, made valid for `doc`.
    pub fn new(registry: Arc<Registry<D, V>>, doc: D, counter: u64, options: Options) -> Self {
        Session::with_view(registry, doc, counter, options, V::default())
    }

    /// [`Session::new`] with `view` in place of the default, such as a view that knows which
    /// notations the document holds; made valid for `doc`.
    pub fn with_view(registry: Arc<Registry<D, V>>, doc: D, counter: u64, options: Options, mut view: V) -> Self {
        view.retain_valid(&doc);
        Session {
            registry,
            options,
            doc,
            view,
            counter,
            id: 0,
            next_id: 1,
            saved: Some(0),
            undo: Vec::new(),
            redo: Vec::new(),
            weight: 0,
            chain: None,
            journal: VecDeque::new(),
            journal_bytes: 0,
            dropped: Dropped::default(),
            interaction: None,
            poisoned: None,
        }
    }

    pub fn doc(&self) -> &D {
        &self.doc
    }

    pub fn view(&self) -> &V {
        &self.view
    }

    /// The id counter, for the host to keep with the document.
    pub fn counter(&self) -> u64 {
        self.counter
    }

    pub fn registry(&self) -> &Arc<Registry<D, V>> {
        &self.registry
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn undo_label(&self) -> Option<&str> {
        self.undo.last().map(|s| s.label.as_str())
    }

    pub fn redo_label(&self) -> Option<&str> {
        self.redo.last().map(|s| s.label.as_str())
    }

    /// The document's state: it changes with every change, and undo and redo bring an earlier
    /// state's back, so a host can tell when there is something new to write.
    pub fn revision(&self) -> u64 {
        self.id
    }

    /// Whether the document differs from the state last marked saved.
    pub fn is_dirty(&self) -> bool {
        self.saved != Some(self.id)
    }

    /// Makes the session unsaved until the next save, through any undo: for a session opened over
    /// changes that were never saved, such as edits a crash left uncommitted.
    pub fn mark_unsaved(&mut self) {
        self.saved = None;
    }

    /// Why the session refuses every edit, once a rollback failed.
    pub fn poisoned(&self) -> Option<&str> {
        self.poisoned.as_deref()
    }

    /// The journal, oldest first: a tail log once its caps have dropped old entries.
    pub fn journal(&self) -> impl Iterator<Item = &JournalEntry> {
        self.journal.iter().map(|(e, _, _)| e)
    }

    /// The counter before the oldest journal entry kept: a session started there replays the
    /// journal's tail to the same ids.
    pub fn journal_start_counter(&self) -> u64 {
        self.journal.front().map_or(self.counter, |(_, _, c)| *c)
    }

    pub fn dropped(&self) -> Dropped {
        self.dropped
    }

    pub fn interacting(&self) -> bool {
        self.interaction.is_some()
    }

    /// The facts an enabled predicate sees for the session as it is.
    pub fn facts(&self) -> Facts<'_, D, V> {
        Facts { doc: &self.doc, view: &self.view, can_undo: self.can_undo(), can_redo: self.can_redo() }
    }

    /// Marks the current state as saved.
    pub fn mark_saved(&mut self) -> Result<(), EngineError> {
        self.no_interaction()?;
        self.healthy()?;
        self.saved = Some(self.id);
        self.chain = None;
        Ok(())
    }

    /// Marks an earlier state as saved: the one a save wrote, when edits followed it before it
    /// finished. The session stays dirty until it is back at that state or saved again.
    pub fn mark_saved_at(&mut self, revision: u64) -> Result<(), EngineError> {
        self.healthy()?;
        self.saved = Some(revision);
        if revision == self.id {
            self.chain = None;
        }
        Ok(())
    }

    fn no_interaction(&self) -> Result<(), EngineError> {
        if self.interaction.is_some() { Err(EngineError::InteractionRunning) } else { Ok(()) }
    }

    fn healthy(&self) -> Result<(), EngineError> {
        match &self.poisoned {
            Some(message) => Err(EngineError::Poisoned { message: message.clone() }),
            None => Ok(()),
        }
    }

    fn fresh_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        id
    }

    /// The command `id`, or the error naming close ones.
    fn spec(&self, id: &str) -> Result<(CommandSpec<D, V>, Vec<ParamDoc>), EngineError> {
        self.registry.find(id).map(|(s, d)| (s.clone(), d.clone())).ok_or_else(|| self.registry.unknown(id))
    }

    /// Whether `spec` is enabled now, its predicate run inside the guard.
    fn check_enabled(&self, spec: &CommandSpec<D, V>) -> Result<(), EngineError> {
        let facts = self.facts();
        guard(spec.id, || (spec.enabled)(&facts).map_err(|reason| disabled(spec.id, reason)))
    }

    /// Marks the session as beyond repair; gives the error to report.
    fn poison(&mut self, id: &str, why: String) -> EngineError {
        log::error!("{id}: the session could not be rolled back and is poisoned: {why}");
        self.poisoned = Some(why.clone());
        EngineError::Poisoned { message: why }
    }

    /// Applies `delta` outside a recording, inside the guard; a failure poisons the session, since
    /// the document's state is then unknown.
    fn apply_or_poison(&mut self, id: &str, delta: &D::Delta) -> Result<(), EngineError> {
        let doc = &mut self.doc;
        match guard(id, || doc.apply(delta).map_err(|why| EngineError::Internal { id: id.to_string(), message: why })) {
            Ok(()) => Ok(()),
            Err(e) => Err(self.poison(id, e.to_string())),
        }
    }

    /// Takes the document back by `delta`, and the view and counter to what they were.
    fn roll_back(&mut self, id: &str, delta: Option<D::Delta>, view: V, next: u64) -> Result<(), EngineError> {
        if let Some(d) = delta {
            let inverse = match guard(id, || Ok(D::invert(&d))) {
                Ok(i) => i,
                Err(e) => return Err(self.poison(id, e.to_string())),
            };
            self.apply_or_poison(id, &inverse)?;
        }
        self.view = view;
        self.counter = next;
        Ok(())
    }

    /// Runs a handler on the live document, recording, inside the guard. On any failure the
    /// document, the view and the counter are as before and the error is given back.
    fn run_recorded(&mut self, spec: &CommandSpec<D, V>, run: crate::cmd::RunFn<D, V>, params: &Params) -> Result<Ran<D::Delta>, EngineError> {
        let view0 = self.view.clone();
        let next0 = self.counter;
        let (doc, view, counter) = (&mut self.doc, &mut self.view, &mut self.counter);
        let result = guard(spec.id, || {
            doc.begin();
            run(&mut Ctx { doc, view, next: counter }, params).map_err(|error| EngineError::Failed { id: spec.id.to_string(), error })
        });
        let doc = &mut self.doc;
        let delta = match guard(spec.id, || Ok(doc.end())) {
            Ok(d) => d,
            Err(e) => {
                // The recording itself failed: nothing can say what changed.
                return Err(self.poison(spec.id, e.to_string()));
            }
        };
        match result {
            Ok(value) => {
                let view_changed = self.view != view0;
                let next_changed = self.counter != next0;
                Ok(Ran { value, delta, view_changed, next_changed })
            }
            Err(e) => {
                self.roll_back(spec.id, delta, view0, next0)?;
                Err(e)
            }
        }
    }

    /// Makes the view valid for the document, inside the guard; on a panic the change is rolled
    /// back.
    fn retain_valid_or_roll_back(&mut self, id: &str, delta: &Option<D::Delta>, view0: V, next0: u64) -> Result<(), EngineError> {
        let (doc, view) = (&self.doc, &mut self.view);
        if let Err(e) = guard(id, || {
            view.retain_valid(doc);
            Ok(())
        }) {
            self.roll_back(id, delta.clone(), view0, next0)?;
            return Err(e);
        }
        Ok(())
    }

    fn weigh(&mut self, id: &str, delta: &D::Delta) -> Result<usize, EngineError> {
        guard(id, || Ok(D::weight(delta)))
    }

    /// Runs the command `id` with `params`.
    pub fn execute(&mut self, id: &str, params: &Value) -> Result<Outcome, EngineError> {
        let (spec, docs) = self.spec(id)?;
        let is_query = !spec.journal && !spec.undoable && matches!(spec.handler, Handler::Run(_));
        if self.interaction.is_some() && !is_query {
            return Err(EngineError::InteractionRunning);
        }
        if !is_query {
            self.healthy()?;
        }
        self.check_enabled(&spec)?;
        let params = params::validate(&docs, params, spec.undoable)?;
        let run = match spec.handler {
            Handler::Undo => return self.undo().map(|label| Outcome { value: json!({ "undone": label }), history_cleared: false }),
            Handler::Redo => return self.redo().map(|label| Outcome { value: json!({ "redone": label }), history_cleared: false }),
            Handler::Run(run) => run,
        };
        let view0 = self.view.clone();
        let next0 = self.counter;
        let ran = self.run_recorded(&spec, run, &params)?;
        let doc_changed = ran.delta.is_some();
        let refusal = if is_query && (doc_changed || ran.view_changed || ran.next_changed) {
            Some(QUERY_CHANGED)
        } else if doc_changed && !spec.undoable {
            Some(NO_UNDO_STEP)
        } else if ran.next_changed && !doc_changed {
            Some(COUNTER_ONLY)
        } else {
            None
        };
        if let Some(why) = refusal {
            self.roll_back(spec.id, ran.delta, view0, next0)?;
            return Err(failed(spec.id, why));
        }
        self.retain_valid_or_roll_back(spec.id, &ran.delta, view0.clone(), next0)?;
        let mut cleared = false;
        if let Some(delta) = ran.delta {
            let weight = match self.weigh(spec.id, &delta) {
                Ok(w) => w,
                Err(e) => {
                    self.roll_back(spec.id, Some(delta), view0, next0)?;
                    return Err(e);
                }
            };
            let key = params.coalesce().map(str::to_string);
            let chain = self.chain.take();
            let same_chain = matches!((&chain, &key), (Some((cid, ck)), Some(k)) if *cid == spec.id && ck == k);
            self.clear_redo();
            if same_chain && !self.undo.is_empty() {
                cleared = self.merge_into_top(spec.id, delta, weight)?;
                if !cleared {
                    self.chain = chain;
                }
            } else {
                let before_id = self.id;
                self.id = self.fresh_id();
                cleared = self.push_undo(Step { delta, id: before_id, label: spec.label.to_string(), weight });
                self.chain = if cleared { None } else { key.map(|k| (spec.id, k)) };
            }
        } else if matches!((&self.chain, params.coalesce()), (Some((cid, ck)), Some(k)) if *cid == spec.id && ck == k) {
            // A call of the same gesture that changed nothing keeps the chain open.
        } else {
            self.chain = None;
        }
        if (doc_changed || ran.view_changed) && spec.journal {
            self.log(JournalEntry::Command { id: spec.id.to_string(), params: params.to_value() }, next0);
        }
        Ok(Outcome { value: ran.value, history_cleared: cleared })
    }

    /// Merges `delta` into the step on top of the undo stack (the same gesture continuing). A merge
    /// that ends where the step began removes the step. Gives whether the history was cleared.
    fn merge_into_top(&mut self, id: &str, delta: D::Delta, weight: usize) -> Result<bool, EngineError> {
        let Some(top) = self.undo.pop() else { return Ok(false) };
        let (top_delta, top_id, top_label, top_weight) = (top.delta.clone(), top.id, top.label.clone(), top.weight);
        let composed = match guard(id, || {
            let composed = D::compose(top.delta, delta.clone());
            let empty = D::is_empty(&composed);
            let weight = D::weight(&composed);
            Ok((composed, empty, weight))
        }) {
            Ok(c) => c,
            Err(e) => {
                // The step stays as it was; the new change is taken back.
                self.undo.push(Step { delta: top_delta, id: top_id, label: top_label, weight: top_weight });
                let view = self.view.clone();
                let next = self.counter;
                self.roll_back(id, Some(delta), view, next)?;
                return Err(e);
            }
        };
        self.weight = self.weight.saturating_sub(top_weight);
        let (composed, empty, composed_weight) = composed;
        if empty {
            self.id = top_id;
            return Ok(false);
        }
        let _ = weight;
        self.id = self.fresh_id();
        Ok(self.push_undo(Step { delta: composed, id: top_id, label: top_label, weight: composed_weight }))
    }

    /// Restores the state before the last step and journals it; gives that step's label.
    pub fn undo(&mut self) -> Result<String, EngineError> {
        self.step(true)
    }

    /// Applies the last undone step again and journals it; gives its label.
    pub fn redo(&mut self) -> Result<String, EngineError> {
        self.step(false)
    }

    fn step(&mut self, undo: bool) -> Result<String, EngineError> {
        self.no_interaction()?;
        self.healthy()?;
        let (from, id) = if undo { (&self.undo, "edit.undo") } else { (&self.redo, "edit.redo") };
        let reason = if undo { crate::cmd::edit::NOTHING_TO_UNDO } else { crate::cmd::edit::NOTHING_TO_REDO };
        let s = from.last().ok_or_else(|| disabled(id, reason))?;
        let (forward, back) = guard(id, || Ok((s.delta.clone(), D::invert(&s.delta))))?;
        let (apply, restore) = if undo { (back, forward) } else { (forward, back) };
        // All or nothing: a delta that does not fit changes nothing, but the history it belongs to
        // cannot be trusted any more, so it goes.
        let doc = &mut self.doc;
        if let Err(e) = guard(id, || doc.apply(&apply).map_err(|why| EngineError::Internal { id: id.to_string(), message: why })) {
            let n = u64::try_from(self.undo.len() + self.redo.len()).unwrap_or(u64::MAX);
            self.undo.clear();
            self.redo.clear();
            self.weight = 0;
            self.dropped.steps = self.dropped.steps.saturating_add(n);
            log::error!("{id}: a history step did not fit the document; the history was cleared ({n} steps)");
            return Err(e);
        }
        let (doc, view) = (&self.doc, &mut self.view);
        let view0 = view.clone();
        if let Err(e) = guard(id, || {
            view.retain_valid(doc);
            Ok(())
        }) {
            self.view = view0;
            self.apply_or_poison(id, &restore)?;
            return Err(e);
        }
        let s = if undo { self.undo.pop() } else { self.redo.pop() }.ok_or_else(|| disabled(id, reason))?;
        let current = Step { delta: s.delta, id: self.id, label: s.label.clone(), weight: s.weight };
        self.id = s.id;
        if undo {
            self.redo.push(current)
        } else {
            self.undo.push(current)
        }
        self.chain = None;
        let counter = self.counter;
        self.log(JournalEntry::Command { id: id.to_string(), params: json!({}) }, counter);
        Ok(s.label)
    }

    /// Several commands as one undo step and one journal entry, or nothing when any fails. Gives the
    /// commands' results in order.
    pub fn batch(&mut self, label: &str, f: impl FnOnce(&mut Batch<'_, D, V>) -> Result<(), EngineError>) -> Result<Outcome, EngineError> {
        self.no_interaction()?;
        self.healthy()?;
        if label.len() > MAX_LABEL {
            return Err(EngineError::TooLong { key: "label".into(), max: MAX_LABEL });
        }
        let view0 = self.view.clone();
        let next0 = self.counter;
        let mut b = Batch { session: self, deltas: Vec::new(), commands: Vec::new(), results: Vec::new() };
        let result = guard("a batch", || f(&mut b));
        let Batch { deltas, commands, results, .. } = b;
        if let Err(e) = result {
            // Back over every command that ran, newest first.
            for d in deltas.iter().rev() {
                self.roll_back("a batch", Some(d.clone()), view0.clone(), next0)?;
            }
            self.view = view0;
            self.counter = next0;
            return Err(e);
        }
        let mut whole: Option<D::Delta> = None;
        for d in deltas {
            whole = Some(match whole {
                None => d,
                Some(w) => match guard("a batch", || Ok(D::compose(w, d))) {
                    Ok(c) => c,
                    Err(e) => return Err(self.poison("a batch", e.to_string())),
                },
            });
        }
        let doc_changed = whole.is_some();
        let view_changed = self.view != view0;
        self.retain_valid_or_roll_back("a batch", &whole, view0.clone(), next0)?;
        self.chain = None;
        let mut cleared = false;
        if let Some(delta) = whole {
            let weight = match self.weigh("a batch", &delta) {
                Ok(w) => w,
                Err(e) => {
                    self.roll_back("a batch", Some(delta), view0, next0)?;
                    return Err(e);
                }
            };
            self.clear_redo();
            let before_id = self.id;
            self.id = self.fresh_id();
            cleared = self.push_undo(Step { delta, id: before_id, label: label.to_string(), weight });
        }
        if doc_changed || view_changed {
            self.log(JournalEntry::Batch { label: label.to_string(), commands }, next0);
        }
        Ok(Outcome { value: Value::Array(results), history_cleared: cleared })
    }

    /// Runs a journal entry again.
    pub fn replay(&mut self, entry: &JournalEntry) -> Result<Outcome, EngineError> {
        match entry {
            JournalEntry::Command { id, params } => self.execute(id, params),
            JournalEntry::Batch { label, commands } => self.batch(label, |b| {
                for (id, params) in commands {
                    b.execute(id, params)?;
                }
                Ok(())
            }),
        }
    }

    /// Starts a gesture that runs the undoable command `id` over and over against the state now.
    pub fn begin_interaction(&mut self, id: &str) -> Result<(), EngineError> {
        self.no_interaction()?;
        self.healthy()?;
        let (spec, _) = self.spec(id)?;
        if !spec.undoable || !matches!(spec.handler, Handler::Run(_)) {
            return Err(failed(spec.id, NOT_A_GESTURE));
        }
        self.check_enabled(&spec)?;
        self.chain = None;
        self.interaction = Some(Interaction { id: spec.id, view: self.view.clone(), next: self.counter, last: None });
        Ok(())
    }

    /// Takes the document back to the state before the gesture, undoing the last preview.
    fn back_to_base(&mut self) -> Result<(), EngineError> {
        let Some(it) = self.interaction.as_mut() else { return Err(EngineError::NoInteraction) };
        let id = it.id;
        let (view, next) = (it.view.clone(), it.next);
        let last = it.last.take().map(|(_, _, d)| d);
        self.roll_back(id, last, view, next)
    }

    /// Shows the gesture's command run on the state before the gesture with `params`; on an error
    /// the state before is shown again.
    pub fn preview(&mut self, params: &Value) -> Result<Value, EngineError> {
        let id = self.interaction.as_ref().ok_or(EngineError::NoInteraction)?.id;
        self.healthy()?;
        let (spec, docs) = self.spec(id)?;
        let Handler::Run(run) = spec.handler else { return Err(EngineError::NoInteraction) };
        self.back_to_base()?;
        let p = params::validate(&docs, params, true)?;
        let view0 = self.view.clone();
        let next0 = self.counter;
        let ran = self.run_recorded(&spec, run, &p)?;
        if ran.next_changed && ran.delta.is_none() {
            self.roll_back(spec.id, ran.delta, view0, next0)?;
            return Err(failed(spec.id, COUNTER_ONLY));
        }
        self.retain_valid_or_roll_back(spec.id, &ran.delta, view0, next0)?;
        if let (Some(it), Some(delta)) = (self.interaction.as_mut(), ran.delta) {
            it.last = Some((p, ran.value.clone(), delta));
        }
        Ok(ran.value)
    }

    /// Ends the gesture with one undo step and one journal entry from its last good preview, or
    /// with nothing when that changed nothing. Never merges into the step before.
    pub fn commit(&mut self) -> Result<Outcome, EngineError> {
        let it = self.interaction.take().ok_or(EngineError::NoInteraction)?;
        self.chain = None;
        match it.last {
            Some((params, value, delta)) => {
                let weight = match self.weigh(it.id, &delta) {
                    Ok(w) => w,
                    Err(e) => {
                        self.roll_back(it.id, Some(delta), it.view, it.next)?;
                        return Err(e);
                    }
                };
                let label = self.spec(it.id).map(|(s, _)| s.label.to_string()).unwrap_or_default();
                self.clear_redo();
                let before_id = self.id;
                self.id = self.fresh_id();
                let cleared = self.push_undo(Step { delta, id: before_id, label, weight });
                // A gesture is one step of its own: its coalescing key is not kept.
                let mut p = params.to_value();
                if let Value::Object(m) = &mut p {
                    m.remove(params::COALESCE);
                }
                self.log(JournalEntry::Command { id: it.id.to_string(), params: p }, it.next);
                Ok(Outcome { value, history_cleared: cleared })
            }
            None => {
                self.view = it.view;
                self.counter = it.next;
                Ok(Outcome { value: Value::Null, history_cleared: false })
            }
        }
    }

    /// Ends the gesture, restoring the state before it (whose view was valid for it).
    pub fn cancel(&mut self) -> Result<(), EngineError> {
        if self.interaction.is_none() {
            return Err(EngineError::NoInteraction);
        }
        let r = self.back_to_base();
        self.interaction = None;
        r
    }

    fn clear_redo(&mut self) {
        for s in self.redo.drain(..) {
            self.weight = self.weight.saturating_sub(s.weight);
        }
    }

    /// Pushes an undo step, then keeps the caps; gives whether the step was so heavy the whole
    /// history had to go.
    fn push_undo(&mut self, s: Step<D::Delta>) -> bool {
        if s.weight > self.options.max_weight {
            let n = u64::try_from(self.undo.len() + self.redo.len() + 1).unwrap_or(u64::MAX);
            self.undo.clear();
            self.redo.clear();
            self.weight = 0;
            self.dropped.steps = self.dropped.steps.saturating_add(n);
            log::warn!(
                "an undo step of {} bytes is over the history's {}-byte cap: the history was cleared ({n} steps)",
                s.weight,
                self.options.max_weight
            );
            return true;
        }
        self.weight = self.weight.saturating_add(s.weight);
        self.undo.push(s);
        self.enforce_caps();
        false
    }

    /// Drops the oldest undo steps first, then the far end of redo, until the caps hold.
    fn enforce_caps(&mut self) {
        while self.undo.len() + self.redo.len() > self.options.max_steps || self.weight > self.options.max_weight {
            let gone = if !self.undo.is_empty() {
                self.undo.remove(0)
            } else if !self.redo.is_empty() {
                self.redo.remove(0)
            } else {
                break;
            };
            self.weight = self.weight.saturating_sub(gone.weight);
            self.dropped.steps = self.dropped.steps.saturating_add(1);
            log::info!("the oldest undo step ({}) was dropped to keep the history within its caps", gone.label);
        }
    }

    fn log(&mut self, entry: JournalEntry, counter_before: u64) {
        let size = entry_size(&entry);
        self.journal_bytes = self.journal_bytes.saturating_add(size);
        self.journal.push_back((entry, size, counter_before));
        while self.journal.len() > self.options.max_journal || self.journal_bytes > self.options.max_journal_bytes {
            let Some((_, gone, _)) = self.journal.pop_front() else { break };
            self.journal_bytes = self.journal_bytes.saturating_sub(gone);
            self.dropped.journal = self.dropped.journal.saturating_add(1);
            log::info!("the oldest journal entry was dropped to keep the journal within its caps");
        }
    }
}

/// A journal entry's size, from the bounded measure of its parameters.
fn entry_size(e: &JournalEntry) -> usize {
    let m = |v: &Value| params::measure(v).unwrap_or(params::MAX_BYTES);
    match e {
        JournalEntry::Command { id, params } => id.len().saturating_add(m(params)),
        JournalEntry::Batch { label, commands } => {
            commands.iter().map(|(id, p)| id.len().saturating_add(m(p))).fold(label.len(), usize::saturating_add)
        }
    }
}

/// The commands of a batch, run one after another on the live session; the batch keeps each one's
/// delta so a failure can take them all back.
pub struct Batch<'a, D: Document, V> {
    session: &'a mut Session<D, V>,
    deltas: Vec<D::Delta>,
    commands: Vec<(String, Value)>,
    results: Vec<Value>,
}

impl<D: Document, V: View<D>> Batch<'_, D, V> {
    /// Runs one command, checked against the state as the batch has made it.
    pub fn execute(&mut self, id: &str, params: &Value) -> Result<Value, EngineError> {
        if self.results.len() >= MAX_BATCH {
            return Err(failed("a batch", BATCH_TOO_LONG));
        }
        let (spec, docs) = self.session.spec(id)?;
        let facts = self.session.facts();
        (spec.enabled)(&facts).map_err(|reason| disabled(spec.id, reason))?;
        let params = params::validate(&docs, params, spec.undoable)?;
        let Handler::Run(run) = spec.handler else { return Err(EngineError::NotInBatch { id: spec.id.to_string() }) };
        let view0 = self.session.view.clone();
        let next0 = self.session.counter;
        let ran = self.session.run_recorded(&spec, run, &params)?;
        let doc_changed = ran.delta.is_some();
        let is_query = !spec.journal && !spec.undoable;
        let refusal = if is_query && (doc_changed || ran.view_changed || ran.next_changed) {
            Some(QUERY_CHANGED)
        } else if doc_changed && !spec.undoable {
            Some(NO_UNDO_STEP)
        } else if ran.next_changed && !doc_changed {
            Some(COUNTER_ONLY)
        } else {
            None
        };
        if let Some(why) = refusal {
            self.session.roll_back(spec.id, ran.delta, view0, next0)?;
            return Err(failed(spec.id, why));
        }
        if let Some(d) = ran.delta {
            self.deltas.push(d);
        }
        if spec.journal {
            // A batch is one step of its own: a coalescing key inside it is not kept.
            let mut p = params.to_value();
            if let Value::Object(m) = &mut p {
                m.remove(params::COALESCE);
            }
            self.commands.push((spec.id.to_string(), p));
        }
        self.results.push(ran.value.clone());
        Ok(ran.value)
    }

    pub fn doc(&self) -> &D {
        &self.session.doc
    }

    pub fn view(&self) -> &V {
        &self.session.view
    }
}
