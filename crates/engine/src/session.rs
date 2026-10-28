//! The session: the document and the view, the commands that change them, and the history.
//!
//! Every call is validated first and runs on a copy that is kept only when it succeeds, so a
//! refused or failed call changes nothing. The notation's own code (its enabled predicates,
//! handlers, `weight`, `retain_valid`, equality and cloning) only ever runs inside the guard and on
//! values the session has not yet taken: a panic there changes nothing either. The document's
//! states carry ids: a change takes a fresh one, undo and redo restore the one stored with the
//! snapshot, and the project is unsaved exactly when the current id is not the saved one.

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
    /// The total weight of the undo and redo snapshots, in bytes as `Document::weight` estimates it.
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

/// A document state kept for undo or redo.
struct Snapshot<D> {
    doc: D,
    id: u64,
    label: String,
    weight: usize,
}

/// A gesture in progress: the command it runs, the state before it, and its last good preview.
struct Interaction<D, V> {
    id: &'static str,
    doc: D,
    view: V,
    next: u64,
    last: Option<(Params, Value)>,
}

/// A change computed on copies, ready to be taken over by plain moves.
struct Change<D, V> {
    value: Value,
    doc: D,
    view: V,
    next: u64,
    doc_changed: bool,
    view_changed: bool,
    next_changed: bool,
    /// The weight of the state before, when the document changed.
    before_weight: usize,
    /// Whether the new document equals the "before" of the step on top of the undo stack.
    back_to_top: bool,
}

pub struct Session<D, V> {
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
    undo: Vec<Snapshot<D>>,
    redo: Vec<Snapshot<D>>,
    weight: usize,
    /// The command and coalescing key of the step the immediately preceding execute pushed.
    chain: Option<(&'static str, String)>,
    /// Each entry with its size and the counter before it.
    journal: VecDeque<(JournalEntry, usize, u64)>,
    journal_bytes: usize,
    dropped: Dropped,
    interaction: Option<Interaction<D, V>>,
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
            log::error!("{id} panicked: {message}; nothing was changed");
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
        let mut view = V::default();
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

    /// Whether the document differs from the state last marked saved.
    pub fn is_dirty(&self) -> bool {
        self.saved != Some(self.id)
    }

    /// Makes the session unsaved until the next save, through any undo: for a session opened over
    /// changes that were never saved, such as edits a crash left uncommitted.
    pub fn mark_unsaved(&mut self) {
        self.saved = None;
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
        self.saved = Some(self.id);
        self.chain = None;
        Ok(())
    }

    fn no_interaction(&self) -> Result<(), EngineError> {
        if self.interaction.is_some() { Err(EngineError::InteractionRunning) } else { Ok(()) }
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

    /// Runs a handler on copies of the state, inside the guard, and works out everything the
    /// session needs to take the result over by plain moves.
    fn compute(
        &self,
        spec: &CommandSpec<D, V>,
        run: crate::cmd::RunFn<D, V>,
        params: &Params,
        from: (&D, &V, u64),
    ) -> Result<Change<D, V>, EngineError> {
        let top = self.undo.last().map(|s| &s.doc);
        guard(spec.id, || {
            let (mut doc, mut view, mut next) = (from.0.clone(), from.1.clone(), from.2);
            let value = run(&mut Ctx { doc: &mut doc, view: &mut view, next: &mut next }, params)
                .map_err(|error| EngineError::Failed { id: spec.id.to_string(), error })?;
            let doc_changed = doc != *from.0;
            let view_changed = view != *from.1;
            view.retain_valid(&doc);
            let before_weight = if doc_changed { from.0.weight() } else { 0 };
            let back_to_top = doc_changed && top.is_some_and(|t| *t == doc);
            Ok(Change { value, doc, view, next, doc_changed, view_changed, next_changed: next != from.2, before_weight, back_to_top })
        })
    }

    /// Runs the command `id` with `params`.
    pub fn execute(&mut self, id: &str, params: &Value) -> Result<Outcome, EngineError> {
        let (spec, docs) = self.spec(id)?;
        let is_query = !spec.journal && !spec.undoable && matches!(spec.handler, Handler::Run(_));
        if self.interaction.is_some() && !is_query {
            return Err(EngineError::InteractionRunning);
        }
        self.check_enabled(&spec)?;
        let params = params::validate(&docs, params, spec.undoable)?;
        let run = match spec.handler {
            Handler::Undo => return self.undo().map(|label| Outcome { value: json!({ "undone": label }), history_cleared: false }),
            Handler::Redo => return self.redo().map(|label| Outcome { value: json!({ "redone": label }), history_cleared: false }),
            Handler::Run(run) => run,
        };
        let c = self.compute(&spec, run, &params, (&self.doc, &self.view, self.counter))?;
        if is_query && (c.doc_changed || c.view_changed || c.next_changed) {
            return Err(failed(spec.id, QUERY_CHANGED));
        }
        if c.doc_changed && !spec.undoable {
            return Err(failed(spec.id, NO_UNDO_STEP));
        }
        if c.next_changed && !c.doc_changed {
            return Err(failed(spec.id, COUNTER_ONLY));
        }
        // From here on only moves and bookkeeping: nothing of the notation runs.
        let counter_before = self.counter;
        self.counter = c.next;
        let key = params.coalesce().map(str::to_string);
        let chain = self.chain.take();
        let same_chain = matches!((&chain, &key), (Some((cid, ck)), Some(k)) if *cid == spec.id && ck == k);
        let mut cleared = false;
        if c.doc_changed {
            let before = std::mem::replace(&mut self.doc, c.doc);
            self.clear_redo();
            if same_chain && !self.undo.is_empty() {
                // The older "before" stays; a merge back to it removes the step and its state.
                if c.back_to_top {
                    if let Some(s) = self.undo.pop() {
                        self.weight = self.weight.saturating_sub(s.weight);
                        self.id = s.id;
                    }
                } else {
                    self.id = self.fresh_id();
                    self.chain = chain;
                }
            } else {
                let before_id = self.id;
                self.id = self.fresh_id();
                cleared = self.push_undo(Snapshot { weight: c.before_weight, doc: before, id: before_id, label: spec.label.to_string() });
                self.chain = if cleared { None } else { key.map(|k| (spec.id, k)) };
            }
        } else if same_chain {
            // A call of the same gesture that changed nothing keeps the chain open.
            self.chain = chain;
        }
        if c.doc_changed || c.view_changed {
            self.view = c.view;
            if spec.journal {
                self.log(JournalEntry::Command { id: spec.id.to_string(), params: params.to_value() }, counter_before);
            }
        }
        Ok(Outcome { value: c.value, history_cleared: cleared })
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
        let (from, id) = if undo { (&self.undo, "edit.undo") } else { (&self.redo, "edit.redo") };
        let reason = if undo { crate::cmd::edit::NOTHING_TO_UNDO } else { crate::cmd::edit::NOTHING_TO_REDO };
        let s = from.last().ok_or_else(|| disabled(id, reason))?;
        // The notation's code first, on what the session has not taken yet.
        let (current_weight, view) = guard(id, || {
            let mut view = self.view.clone();
            view.retain_valid(&s.doc);
            Ok((self.doc.weight(), view))
        })?;
        let s = if undo { self.undo.pop() } else { self.redo.pop() }.ok_or_else(|| disabled(id, reason))?;
        let current = Snapshot { weight: current_weight, doc: std::mem::replace(&mut self.doc, s.doc), id: self.id, label: s.label.clone() };
        self.weight = self.weight.saturating_sub(s.weight).saturating_add(current.weight);
        self.id = s.id;
        if undo {
            self.redo.push(current)
        } else {
            self.undo.push(current)
        }
        self.view = view;
        self.chain = None;
        self.enforce_caps();
        let counter = self.counter;
        self.log(JournalEntry::Command { id: id.to_string(), params: json!({}) }, counter);
        Ok(s.label)
    }

    /// Several commands as one undo step and one journal entry, or nothing when any fails. Gives the
    /// commands' results in order.
    pub fn batch(&mut self, label: &str, f: impl FnOnce(&mut Batch<'_, D, V>) -> Result<(), EngineError>) -> Result<Outcome, EngineError> {
        self.no_interaction()?;
        if label.len() > MAX_LABEL {
            return Err(EngineError::TooLong { key: "label".into(), max: MAX_LABEL });
        }
        let can = (self.can_undo(), self.can_redo());
        let (current, view0, next0) = (&self.doc, &self.view, self.counter);
        let (b, doc_changed, view_changed, before_weight) = guard("a batch", || {
            let mut b = Batch {
                registry: &self.registry,
                doc: current.clone(),
                view: view0.clone(),
                next: next0,
                can_undo: can.0,
                can_redo: can.1,
                commands: Vec::new(),
                results: Vec::new(),
            };
            f(&mut b)?;
            let doc_changed = b.doc != *current;
            let view_changed = b.view != *view0;
            b.view.retain_valid(&b.doc);
            let before_weight = if doc_changed { current.weight() } else { 0 };
            Ok((b.into_parts(), doc_changed, view_changed, before_weight))
        })?;
        let (doc, view, next, commands, results) = b;
        let counter_before = self.counter;
        self.counter = next;
        self.chain = None;
        let mut cleared = false;
        if doc_changed {
            let before = std::mem::replace(&mut self.doc, doc);
            self.clear_redo();
            let before_id = self.id;
            self.id = self.fresh_id();
            cleared = self.push_undo(Snapshot { weight: before_weight, doc: before, id: before_id, label: label.to_string() });
        }
        if doc_changed || view_changed {
            self.view = view;
            self.log(JournalEntry::Batch { label: label.to_string(), commands }, counter_before);
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
        let (spec, _) = self.spec(id)?;
        if !spec.undoable || !matches!(spec.handler, Handler::Run(_)) {
            return Err(failed(spec.id, NOT_A_GESTURE));
        }
        self.check_enabled(&spec)?;
        let (doc, view) = guard(spec.id, || Ok((self.doc.clone(), self.view.clone())))?;
        self.chain = None;
        self.interaction = Some(Interaction { id: spec.id, doc, view, next: self.counter, last: None });
        Ok(())
    }

    /// Shows the gesture's command run on the state before the gesture with `params`; on an error
    /// the state before is shown again.
    pub fn preview(&mut self, params: &Value) -> Result<Value, EngineError> {
        let id = self.interaction.as_ref().ok_or(EngineError::NoInteraction)?.id;
        let (spec, docs) = self.spec(id)?;
        let Handler::Run(run) = spec.handler else { return Err(EngineError::NoInteraction) };
        let it = self.interaction.as_ref().ok_or(EngineError::NoInteraction)?;
        let result = params::validate(&docs, params, true).and_then(|p| self.compute(&spec, run, &p, (&it.doc, &it.view, it.next)).map(|c| (c, p)));
        match result {
            Ok((c, p)) => {
                self.doc = c.doc;
                self.view = c.view;
                self.counter = c.next;
                if let Some(it) = self.interaction.as_mut() {
                    it.last = Some((p, c.value.clone()));
                }
                Ok(c.value)
            }
            Err(e) => {
                // Back to the state before the gesture: a copy of it, or, if even copying fails,
                // the snapshot itself, which ends the gesture.
                let it = self.interaction.as_ref().ok_or(EngineError::NoInteraction)?;
                match guard(spec.id, || Ok((it.doc.clone(), it.view.clone()))) {
                    Ok((doc, view)) => {
                        self.doc = doc;
                        self.view = view;
                        if let Some(it) = self.interaction.as_mut() {
                            self.counter = it.next;
                            it.last = None;
                        }
                    }
                    Err(_) => {
                        if let Some(it) = self.interaction.take() {
                            self.doc = it.doc;
                            self.view = it.view;
                            self.counter = it.next;
                        }
                    }
                }
                Err(e)
            }
        }
    }

    /// Ends the gesture with one undo step and one journal entry from its last good preview, or
    /// with nothing when that equals the state before. Never merges into the step before.
    pub fn commit(&mut self) -> Result<Outcome, EngineError> {
        let id = self.interaction.as_ref().ok_or(EngineError::NoInteraction)?.id;
        let (spec, _) = self.spec(id)?;
        let it = self.interaction.as_ref().ok_or(EngineError::NoInteraction)?;
        let current = &self.doc;
        let (changed, before_weight) = guard(spec.id, || {
            let changed = it.last.is_some() && *current != it.doc;
            Ok((changed, if changed { it.doc.weight() } else { 0 }))
        })?;
        let it = self.interaction.take().ok_or(EngineError::NoInteraction)?;
        self.chain = None;
        match (changed, it.last) {
            (true, Some((params, value))) => {
                self.clear_redo();
                let before_id = self.id;
                self.id = self.fresh_id();
                let cleared = self.push_undo(Snapshot { weight: before_weight, doc: it.doc, id: before_id, label: spec.label.to_string() });
                // A gesture is one step of its own: its coalescing key is not kept.
                let mut p = params.to_value();
                if let Value::Object(m) = &mut p {
                    m.remove(params::COALESCE);
                }
                self.log(JournalEntry::Command { id: spec.id.to_string(), params: p }, it.next);
                Ok(Outcome { value, history_cleared: cleared })
            }
            _ => {
                self.doc = it.doc;
                self.view = it.view;
                self.counter = it.next;
                Ok(Outcome { value: Value::Null, history_cleared: false })
            }
        }
    }

    /// Ends the gesture, restoring the state before it (whose view was valid for it).
    pub fn cancel(&mut self) -> Result<(), EngineError> {
        let it = self.interaction.take().ok_or(EngineError::NoInteraction)?;
        self.doc = it.doc;
        self.view = it.view;
        self.counter = it.next;
        Ok(())
    }

    fn clear_redo(&mut self) {
        for s in self.redo.drain(..) {
            self.weight = self.weight.saturating_sub(s.weight);
        }
    }

    /// Pushes an undo step, then keeps the caps; gives whether the step was so heavy the whole
    /// history had to go.
    fn push_undo(&mut self, s: Snapshot<D>) -> bool {
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

/// The commands of a batch, run on the batch's own copy (inside the batch's guard).
pub struct Batch<'a, D, V> {
    registry: &'a Arc<Registry<D, V>>,
    doc: D,
    view: V,
    next: u64,
    can_undo: bool,
    can_redo: bool,
    commands: Vec<(String, Value)>,
    results: Vec<Value>,
}

impl<D: Document, V: View<D>> Batch<'_, D, V> {
    /// Runs one command on the batch's copy, checked against that copy.
    pub fn execute(&mut self, id: &str, params: &Value) -> Result<Value, EngineError> {
        if self.results.len() >= MAX_BATCH {
            return Err(failed("a batch", BATCH_TOO_LONG));
        }
        let (spec, docs) = self.registry.find(id).map(|(s, d)| (s.clone(), d.clone())).ok_or_else(|| self.registry.unknown(id))?;
        let facts = Facts { doc: &self.doc, view: &self.view, can_undo: self.can_undo, can_redo: self.can_redo };
        (spec.enabled)(&facts).map_err(|reason| disabled(spec.id, reason))?;
        let params = params::validate(&docs, params, spec.undoable)?;
        let Handler::Run(run) = spec.handler else { return Err(EngineError::NotInBatch { id: spec.id.to_string() }) };
        let (mut doc, mut view, mut next) = (self.doc.clone(), self.view.clone(), self.next);
        let value = run(&mut Ctx { doc: &mut doc, view: &mut view, next: &mut next }, &params)
            .map_err(|error| EngineError::Failed { id: spec.id.to_string(), error })?;
        let doc_changed = doc != self.doc;
        let is_query = !spec.journal && !spec.undoable;
        if is_query && (doc_changed || view != self.view || next != self.next) {
            return Err(failed(spec.id, QUERY_CHANGED));
        }
        if doc_changed && !spec.undoable {
            return Err(failed(spec.id, NO_UNDO_STEP));
        }
        if next != self.next && !doc_changed {
            return Err(failed(spec.id, COUNTER_ONLY));
        }
        self.doc = doc;
        self.view = view;
        self.next = next;
        if spec.journal {
            // A batch is one step of its own: a coalescing key inside it is not kept.
            let mut p = params.to_value();
            if let Value::Object(m) = &mut p {
                m.remove(params::COALESCE);
            }
            self.commands.push((spec.id.to_string(), p));
        }
        self.results.push(value.clone());
        Ok(value)
    }

    pub fn doc(&self) -> &D {
        &self.doc
    }

    pub fn view(&self) -> &V {
        &self.view
    }

    #[allow(clippy::type_complexity)]
    fn into_parts(self) -> (D, V, u64, Vec<(String, Value)>, Vec<Value>) {
        (self.doc, self.view, self.next, self.commands, self.results)
    }
}
