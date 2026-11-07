# archforge-engine

**Purpose.** The command engine. Every edit is a registered command; a session validates each call
before its handler runs, lets the handler change the live document while the document records the
change as a delta, rolls the delta back when the call fails or panics, records one undo step per
command, batch or gesture, undoes and redoes by applying inverse deltas, and knows whether the
project has unsaved changes. A document cheaper to copy than to record wraps itself in
`Snapshotted`.

**Layer.** 3, below the shell; builds for the portable target (no file system, process, clock or
environment). It knows no notation: a notation crate brings its `Document`, its `View` and its
command areas.

**Binding rules.** The forge command-registry standard: dotted area-and-verb ids, parameters
documented one line per key (`key: type = default, range or choices; meaning`, or `key: type?; meaning` for a key that may be left out, which then reaches the handler as absent, so a set command can keep what it is not given) and refused centrally
when unknown or out of bounds, enabled predicates with shared reasons, one undo step per command or
gesture, no step for view-only changes, failures that change nothing. Deviations from the binding:
handlers get `Ctx` (document and view), not the session; `enabled` gets `Facts` and returns a shared
`&'static str`; each notation builds its own `Registry`; parameters have a sixth type, `integer`
(a float with no fraction counts); never-crash: every call into the notation's code runs guarded, a
failure or panic is rolled back by the inverse delta so it changes nothing, a panicking preview
keeps the gesture on its starting state, and a rollback that itself fails poisons the session
(every edit refused, and `mark_saved` too, until the project is opened again) instead of trusting
an unknown state; an undo or redo whose delta does not fit the document leaves the document as it
is and clears the history it belonged to.

**Public API.**

| Item | Signature |
|---|---|
| `Registry::new` | `pub fn new(specs: Vec<CommandSpec<D, V>>) -> Result<Arc<Registry<D, V>>, String>` |
| `cmd!` | registers a command: journaled and undoable, `query`, or `noundo` |
| `cmd::edit::specs` | `edit.undo` and `edit.redo` for every table |
| `Session::new` | `pub fn new(registry: Arc<Registry<D, V>>, doc: D, counter: u64, options: Options) -> Session<D, V>` |
| `Session::counter`, `Ctx::fresh` | the counter new ids come from: outside undo, so an id is never given twice |
| `Session::execute` | `pub fn execute(&mut self, id: &str, params: &Value) -> Result<Outcome, EngineError>` |
| `Session::{undo, redo, batch, replay}` | history, several commands as one step, a journal entry again |
| `Session::{execute_from, batch_from}`, `Session::{undo_origin, redo_origin}` | a call or a batch on behalf of an origin (a chat): its step carries the origin, is labelled `<origin>: <label>` and never merges into another origin's step; the origin of the steps undo and redo would take |
| `Session::{begin_group, end_group, grouping, undo_in_group, redo_in_group}`, `GroupEnd` | an origin's group (a Chat turn): its changes one undo step while that step is on top, another's change in between starting another; the end gives the span, the steps, the commands and each step's own delta; whether the step undo or redo would take is one of the open group's, so a turn undoes or redoes only its own steps |
| `Session::{begin_interaction, preview, commit, cancel}` | a gesture: previews on the state before it, one step at the end |
| `Session::{is_dirty, mark_saved, mark_unsaved, journal, dropped}` | unsaved changes (or none saved yet), the tail log, what the caps dropped |
| `Document` | what undo covers: `begin`, `end` (the recorded delta), `apply` (all or nothing), `invert`, `compose`, `is_empty`, `weight` |
| `Snapshot`, `Snapshotted<T>`, `Replace<T>` | a clone-and-compare state as a document, its delta both states whole |
| `View` | view state, kept valid after every change |
| `Session::poisoned` | why the session refuses edits, once a rollback failed |
| `testing::round_trip` | feature `testing`: a command, its undo and its redo give identical documents |
| `testing::sweep`, `testing::sweep_with` | feature `testing`: every command with hostile parameters; no panic, refusals change nothing (`sweep_with` takes a picture function for documents that are not cloned) |

**Non-goals.** Persistence, dialogs, long jobs, hit-testing: they come with their first users.
