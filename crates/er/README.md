# archforge-er

**Purpose.** The ER notation: entities with ordered attributes, relationships with a cardinality at
each end, and diagrams that place entities. A document, a view (the open diagram and the selection)
and the commands that edit them, for the engine.

**Layer.** 4, above the engine; pure, builds for the portable target. The project crate stores the
model; the canvas comes later. The model records its changes by copying (`Snapshotted<ErModel>`)
until it moves onto the MOF core (gm2.28).

**Binding rules.** Every edit is a registered command with documented parameters, refused centrally
when unknown or out of range and refused by the handler, naming the key, when an id is missing or of
the wrong kind. Ids are a kind letter and a number from the session's counter, never given twice.
`ErModel::check` holds every invariant: ids canonical, unique and below the counter; names plain,
at most 128 characters and unique ignoring case (entities, attributes within their entity,
diagrams); a key is never nullable; relationship ends and places name entities that exist; places
within range; caps on every collection. Removing an entity takes its relationships and its places
in the same step.

**Public API.**

| Item | Signature |
|---|---|
| `registry` | `pub fn registry() -> Result<Arc<Registry<ErModel, ErView>>, String>`: the edit area and the ER commands |
| `Session` | `archforge_engine::Session<ErModel, ErView>` |
| `ErModel::check` | `pub fn check(&self, counter: u64) -> Result<(), Invalid>` |
| `ErModel`, `Entity`, `Attribute`, `Relationship`, `Cardinality`, `Diagram`, `Place` | the model |
| `ErView` | the open diagram and the selection, kept valid after every change |
| `model::{make_id, parse_id, MAX_*}` | ids and the caps |
| `NO_ENTITIES`, `NO_DIAGRAMS` | the shared reasons a command is disabled |

Commands: `er.entity.{add, rename, remove}`, `er.attribute.{add, rename, set-kind, set-flags, move,
remove}`, `er.relationship.{add, set-ends, rename, remove}`, `er.remove`, `er.diagram.{add, rename,
remove, place, move, resize, unplace}`, `view.{open-diagram, select}`, and the queries
`er.{entity, diagram}.{list, get}` and `er.model.get`.

**Tests.** `tests/er.rs`: each broken invariant, every command's round trip, refusals that change
nothing, cascades, a multi-entity drag as one step, the view after undo, the adversarial sweep,
seeded random sequences with journal replay, and a model at every cap.

**Non-goals.** Comments on elements and foreign-key pairing of attributes (decided with the next
format), the canvas, and the class model.
