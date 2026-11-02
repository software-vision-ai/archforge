# archforge-mof

**Purpose.** The MOF core: the meta-metamodel a metamodel is described in, the store that holds
objects of every metamodel in one id space, change sets with exact inverses, the semantics the
store enforces on every change, the indexes it keeps, and the reflective access generic tools use.
It knows no metamodel of its own: generated code (one crate per metamodel) brings the typed slabs
and a `Reflect` per metaclass.

**Layer.** 4, above the engine; pure, builds for the portable target. The shared core
metamodel comes at 5, the notations' generated metamodels at 6.

**Binding rules.** MOF 2.5.1, EMOF plus subsets,
derived unions, read-only redefinition resolution and association-owned ends; no operations,
package merge, OCL or profiles. Every change is an op with an exact inverse, recorded between
`begin` and `end` as the engine's delta; applying a delta is all or nothing. Before anything
changes the store refuses: a wrong kind of value, a reference that does not conform, an upper
bound, a duplicate in a unique list, a write to a derived or read-only property, a dead handle, a
containment cycle, a non-finite real, a string over 65,535 characters. On every change it keeps:
one container per object (setting a composite reference moves the child, MOF 12.5.5), opposites
consistent in the same change set (12.5.6), a stored subsetting property's values also in its
stored subsetted properties, derived unions computed on read. Every semantic operation is atomic: a refusal half way
is taken back by the inverse ops, inside or outside the engine; if taking back fails the store
marks itself corrupt and refuses every change until the project is opened again. Lower bounds are
not enforced per change (a half-built object is legal while editing): `validate` reports them, with
duplicate sibling names. Reference lists must be unique (the builder refuses `is_unique = false` on
a reference), since the referrer index is a set. Ids are UUIDv7 from the host's clock and randomness; handles never leave the
process. Caps: 10 million live objects, 1 million ops per change set, 1 million values per list.

**Public API.**

| Item | Signature |
|---|---|
| `Builder` | describes one metamodel: `class`, `data_type`, `property(PropertySpec)`, `finish() -> Arc<Metamodel>` |
| `Universe::new` | `pub fn new(metamodels: Vec<Arc<Metamodel>>) -> Result<Universe, MetaError>`: one id space, inheritance flattened, subsets and opposites checked |
| `Universe::{class, prop, data_type, info, conforms, has}` | lookups and type conformance |
| `Store::new` | `pub fn new(universe: Arc<Universe>, slabs: Box<dyn Slabs>, ids: Box<dyn FnMut() -> Uuid + Send>) -> Store` |
| `Store::{create, set, push, insert, remove_at, remove_value, move_item, destroy}` | the semantic level; every refusal is an `Invalid` naming its cause |
| `Store::{get, list, owned_elements, container, contains, referrers, by_name, by_prefix, by_uuid, uuid, class_of, reflect}` | reading, including derived unions and the indexes |
| `Store::{validate, validate_all}` | lower bounds and sibling names |
| `Store::apply_delta` | a delta outside a recording, all or nothing |
| `Reflect`, `Slabs` | what generated code implements |
| `Op`, `Delta`, `Invalid`, `Problem`, `Value`, `Uuid` | the data |
| `schema::{Table, Column, Junction}` | the storage schema a metamodel's generated code describes, as data for the project crate |
| `toy` | feature `testing`: a twenty-metaclass metamodel in the generated shape, for tests; `tests/toy.xmi` is the same metamodel as XMI, and the generator's fidelity test proves they agree |

**Engine.** `Store` implements `archforge_engine::Document`: `begin`, `end`, `apply`, `invert`,
`compose`, `is_empty` (ops that cancel are dropped), `weight`.

**Measured** (release, M4 Max): a million objects in 190 MB; 10,000 random edits with undo, edit
p99 186 µs; undoing all 10,000 in 59 ms.

**Tests.** `tests/mof.rs`: the universe's resolution and its refusals, every op and its inverse
back to the identical store, each refusal naming its cause and changing nothing, containment moves
and cycles, opposites both ways, subsets and derived unions, destroy with required and optional
referrers, indexes exact against a rebuild after random sequences with undo back to empty, and the
store as the engine's document (rollback on panic and on failure, batch, gesture, sweep).

**Known lacks.** A destroyed object keeps its slot until the store is dropped: the engine does not
yet tell the document when a history step is gone. `validate_all` over many
same-named siblings in one container is quadratic; `first_problem` is the linear check.
A value set into a single-valued subset (such as
`InterfaceRealization::contract`) does not reach its stored superset, though it leaves the subset when it
leaves the superset. Redefinitions resolve the property table; a redefining property with a different type is not
checked further.

**Non-goals.** The generator, XMI, storage in Dolt, any real metamodel, OCL,
operations, profiles, package merge, user-defined metamodels (the `Reflect` trait leaves the door
open).
