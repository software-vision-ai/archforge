# archforge-core

**Purpose.** The shared core metamodel: a project's one root (`Model`), folders that organise
elements of any notation, comments and tags on any element, trace links between elements of
any notations, which make the notations viewpoints of one model (decision), and skills:
text the user writes on an element to guide the AI. Generated from
`core.xmi` by `cargo xtask generate core`; `lib.rs` holds the rules the MOF core cannot know and
`skills.rs` the skill rules and commands.

**Layer.** 5, above `archforge-mof` and `archforge-engine`; pure, builds for the portable target.
Depends on no notation: everything that refers to another notation's element is typed Any, and what
a skill may sit on or aim at comes from the notation's view through `skills::SkillHost`.

**Binding rules.** The metamodel is data compiled in; the XMI is ours and lives in the
repository, so the gate's `generated` step checks it everywhere. The MOF core enforces types,
bounds, containment and the derived union `ownedElement`; this crate adds: a tag's name is unique
among the tags on one element (`tag_name_free`), a link's ends differ (`link`), links die with
their ends and skills with their subjects (`destroy_with_links`: the MOF core alone refuses to
destroy an element a link requires, and would null a skill's optional subject, turning a table's
skill into a model-wide one; every notation removes through it), and a project has one Model and nothing else without a
container (`problems`). Skills (`skills`): a name is plain and unique among the skills on one
subject; a skill aims at a kind the notation declares, and names a type only for the typed kind; a
dialect belongs to an export skill; a body has at most 16,000 characters; the effective skills of an
element (`skills::effective_skills`) are those of its levels outermost first, then its own (its
containers, or the level the view names instead: a diagram's is the element it shows),
the least specific first within a level (everything below the subject, then a kind, then a kind
with a type), filtered by trigger, with a skill named like an earlier one marked as overriding it.

**The metaclasses.** `Model` (member, comment, tag, link, skill), `Folder` (member, tag), `Comment`
(body, annotatedElement), `Tag` (name, value, element), `Link` (kind, source, target, note), `Skill`
(name, description, body, trigger, dialect, subject, appliesTo, typePattern) and the enumerations
`LinkKind` (trace, realization, persistence, derivation, satisfaction, refinement) and
`SkillTrigger` (modelling, drawing, naming, export); `Element` and `NamedElement` abstract above
them. A skill's `appliesTo` holds a class key such as `er.Entity` (`skills::class_key`).

**Public API.**

| Item | Signature |
|---|---|
| `metamodel`, `universe_with` | the metamodel; a universe of the core and the metamodels given |
| `CoreSlabs` | the slabs, combined with other metamodels' through `archforge_mof::CompositeSlabs` |
| `link` | `pub fn link(store, owner, kind, source, target, note) -> Result<Obj, Invalid>` |
| `destroy_with_links`, `links_of` | destroy with the links on the subtree first; those links |
| `tag_name_free`, `problems`, `CoreProblem` | the rules |
| `skills::SkillHost` | what the view answers: `selection_mut`, `skill_subjects`, `skill_kinds`, `typed_classes`, `skill_problem` (an element of a subject class that still carries none), `level_above` (the level above an element when it is not its container) |
| `skills::specs::<V>()` | the commands `skill.add`, `skill.set`, `skill.set-body`, `skill.rename`, `skill.remove`, `skill.move` and the queries `skill.list`, `skill.effective`, for a notation's registry |
| `skills::{effective_skills, EffectiveSkill, levels, aims_at, skills, skills_on, subject_of, triggers_of, name_of, model_of, skill_name_free, name_problem, text_problem, Problem, hidden, trigger_named, trigger_words, class_key, kind_by_key, class_by_key, category_of, type_matches, skill_json, problems, SkillProblem}` | the skill rules and lookups |
| `skills::{MAX_NAME, MAX_DESCRIPTION, MAX_BODY, MAX_DIALECT, MAX_TYPE_PATTERN, MAX_TRIGGERS, CATEGORIES, NO_MODEL, NO_SKILLS}` | the limits, the type categories, the reasons a command is unavailable |
| the generated constants, structs, `LinkKind`, `TABLES`, `JUNCTIONS` | as every generated metamodel |

**Tests.** `crates/er/tests/skills.rs` (skills over ER: every command's round trip, refusals, the
effective skills over four levels with kinds, types, triggers and an override, the cascade with a
subject), `crates/uml/tests/skills.rs` (skills over UML's packages, classes and members). `crates/uml/tests/core_over_uml.rs` (with the notation, since a dependency only points
down a layer), over the core and UML in one store: a Model with a Folder holding a UML
Package (containment across metamodels), a comment and a tag on elements of two notations, a
realization link between two classes; the rules refuse and change nothing; destroy with links and
an exact undo; the sweep over a session of generic commands on the combined store.

**Known lacks.** The rules run on demand (`problems`) and in the helpers; nothing calls them before
a Save yet. The generator marks `Tag::name` as the class's name property, so the MOF
core's sibling-name rule (`validate`) would report two tags of one name under one Model even when
they tag different elements, which the core allows; left for an owner decision. No
commands or UI.

**Non-goals.** Issues and chats (own tables), diagrams, ER and C4, the Model's
creation in a project.
