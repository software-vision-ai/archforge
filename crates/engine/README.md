# archforge-engine

**Purpose.** The command engine. Every edit is a registered command; a session validates each call
before its handler runs, applies it to a copy kept only on success, records one undo step per
command, batch or gesture, undoes and redoes to identical state, and knows whether the project has
unsaved changes.

**Layer.** 3, below the shell; builds for the portable target (no file system, process, clock or
environment). It knows no notation: a notation crate brings its `Document`, its `View` and its
command areas.

**Binding rules.** The forge command-registry standard: dotted area-and-verb ids, parameters
documented one line per key (`key: type = default, range or choices; meaning`) and refused centrally
when unknown or out of bounds, enabled predicates with shared reasons, one undo step per command or
gesture, no step for view-only changes, failures that change nothing. Deviations from the binding:
handlers get `Ctx` (document and view), not the session; `enabled` gets `Facts` and returns a shared
`&'static str`; each notation builds its own `Registry`; parameters have a sixth type, `integer`
(a float with no fraction counts); never-crash: every call into the notation's code runs guarded on
copies, so a panic changes nothing and needs no revision bump, a panicking preview keeps the gesture
on its starting state, and the recovery save comes with persistence.

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
| `Session::{begin_interaction, preview, commit, cancel}` | a gesture: previews on the state before it, one step at the end |
| `Session::{is_dirty, mark_saved, mark_unsaved, journal, dropped}` | unsaved changes (or none saved yet), the tail log, what the caps dropped |
| `Document`, `View` | what undo covers (with a size estimate); view state, kept valid after every change |
| `testing::round_trip` | feature `testing`: a command, its undo and its redo give identical documents |
| `testing::sweep` | feature `testing`: every command with hostile parameters; no panic, refusals change nothing |

**Non-goals.** Persistence, dialogs, long jobs, hit-testing: they come with their first users.
