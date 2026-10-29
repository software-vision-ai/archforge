# archforge-core

**Purpose.** The shared core metamodel: a project's one root (`Model`), folders that organise
elements of any notation, comments and tags on any element, and trace links between elements of
any notations, which make the notations viewpoints of one model (decision gm2.20). Generated from
`core.xmi` by `cargo xtask generate core`; `lib.rs` holds the rules the MOF core cannot know.

**Layer.** 5, above `archforge-mof`; pure, builds for the portable target. Depends on no notation:
everything that refers to another notation's element is typed Any.

**Binding rules.** The metamodel is data compiled in (gm2.18); the XMI is ours and lives in the
repository, so the gate's `generated` step checks it everywhere. The MOF core enforces types,
bounds, containment and the derived union `ownedElement`; this crate adds: a tag's name is unique
among the tags on one element (`tag_name_free`), a link's ends differ (`link`), links die with
their ends (`destroy_with_links`: the MOF core alone refuses to destroy an element a link
requires), and a project has one Model and nothing else without a container (`problems`).

**The metaclasses.** `Model` (member, comment, tag, link), `Folder` (member, tag), `Comment` (body,
annotatedElement), `Tag` (name, value, element), `Link` (kind, source, target, note) and the
enumeration `LinkKind` (trace, realization, persistence, derivation, satisfaction, refinement);
`Element` and `NamedElement` abstract above them.

**Public API.**

| Item | Signature |
|---|---|
| `metamodel`, `universe_with` | the metamodel; a universe of the core and the metamodels given |
| `CoreSlabs` | the slabs, combined with other metamodels' through `archforge_mof::CompositeSlabs` |
| `link` | `pub fn link(store, owner, kind, source, target, note) -> Result<Obj, Invalid>` |
| `destroy_with_links`, `links_of` | destroy with the links on the subtree first; those links |
| `tag_name_free`, `problems`, `CoreProblem` | the rules |
| the generated constants, structs, `LinkKind`, `TABLES`, `JUNCTIONS` | as every generated metamodel |

**Tests.** `crates/uml/tests/core_over_uml.rs` (with the notation, since a dependency only points
down a layer), over the core and UML in one store: a Model with a Folder holding a UML
Package (containment across metamodels), a comment and a tag on elements of two notations, a
realization link between two classes; the rules refuse and change nothing; destroy with links and
an exact undo; the sweep over a session of generic commands on the combined store.

**Known lacks.** The rules run on demand (`problems`) and in the helpers; nothing calls them before
a Save yet (gm2.31). The generator marks `Tag::name` as the class's name property, so the MOF
core's sibling-name rule (`validate`) would report two tags of one name under one Model even when
they tag different elements, which the core allows; recorded on gm2.31 for an owner decision. No
commands or UI.

**Non-goals.** Issues and chats (own tables, gm2.22), diagrams (gm2.29), ER and C4, the Model's
creation in a project (gm2.31).
