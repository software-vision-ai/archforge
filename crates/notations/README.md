# archforge-notations

**Purpose.** The notations a project holds, composed, as a value (`Notations`). Each metamodel
brings what it adds to the store as a `Part`: its metamodel, its index and slabs, and the tables and
list tables its objects are stored in (the shared core and the diagrams first, then the notations').
Each notation brings what it answers in the project's view as a `Notation`, and its own commands.
So one universe, one store, one database, one view and one command table hold the notations side
by side. The product's composition is `NOTATIONS`: ER, and UML's class diagrams. The app and the
command-line tool hand it to what makes projects (`forge-ux/adr/0003`, "Two registries"), and none
of those names a notation: the shell takes it as a value, `Shell::new(services, notations,
screens)`, from the app (`apps/archforge/src/lib.rs`), with the screens registry; the app's
projects host (`apps/archforge/src/projects.rs`) and the agent's tools in the command-line tool
(`apps/archforge-cli/src/main.rs`) use it.
The gate holds this true: its `layers` step refuses a link from the shell, its interfaces, the
project store or the command-line tool to a notation crate, in production or in tests (rule 3 of
`xtask/src/notation_rules.rs`, `forge-ux/adr/0004`), and a notation's command name in any of their
files (rule 5); only this crate, a notation's own screens crate and the app link a model crate
(rule 1). The shell's and the project store's tests hand them a composition of their own, the fake
notation (`fake`, feature `testing`, `forge-ux/adr/0005`, "Tests follow the plugin").

**Layer.** 8, the one crate above the notations (7) that names them all; below the shell's
interfaces (9) and the shell (11).
Pure, builds for the portable target.

**Binding rules.** The tables are stored in the order of a composition's parts, and the project's fingerprint
is taken over them in that order: a metamodel joins at the end, and the fingerprint the tables had
goes to the project crate's `EARLIER_MODELS`, so a project of the earlier list is brought up to date.
A metamodel that gains a table changes the fingerprint the same way: the diagrams' table of packages'
default diagrams (`diagram_default_diagram_binding`, AR-552) put 0.14.0's on that list. Such an
upgrade only adds the tables and columns that came since, as storage always has; it migrates no
diagram kind and rewrites no row. A project's database holds only the tables rows have gone to, so a
notation's tables cost nothing until the project uses it. A session starts with its composition's `view()`, never
the empty default view, which holds no notation and so knows no diagram kind. A model holding a
diagram of a kind none of its composition's notations knows, such as one an earlier version stored, is refused as it opens,
with the typed `UnknownKinds` naming every such diagram and the kinds known, before anything is
built from its rows; nothing in it is migrated, renamed or removed, and it is never shown under
another kind. A default-diagram binding (Decision 12) that names an owner or diagram the rows do
not hold, or names none, refuses the model the same way, with the typed `BrokenBindings`. One that
names the wrong thing (a package with two, none, or a diagram of another kind or package) opens as
it was stored and is reported, never repaired: the rows read back unchanged, and a repair that
would change a binding, compared by ids, is refused. A version opened to be looked at reports its own.

**Public API.**

| Item | What it is |
|---|---|
| `Notations`, `Notations::new` | a composition: the parts its notations' metamodels bring after the shared core's and the diagrams', the notations its view asks, and their own commands; every item below but `Session`, `OpenError`, the bindings' check and `differences` is one of its methods |
| `NOTATIONS` | the composition the product ships: ER's and UML's parts, notations and commands |
| `Part`, `parts` | what a metamodel brings to the store, its name rule among it; the shared core's, the diagrams' and the notations' parts a project holds, in the order their tables are stored |
| `notations` | the notations a project's view asks |
| `name_kind` | the kind an element is told apart within by name in its owner, by its metamodel's rule (UML's is `archforge_uml::names::kind`, ER's `archforge_er::names::kind`): the check as a project opens refuses only two of one kind named alike (AR-208, AR-209) |
| `view`, `kinds` | a `ProjectView` of the notations; the diagram kinds they know |
| `universe`, `store` | the universe of every notation; an empty store over it with the host's ids |
| `storage` | every metamodel's tables and list tables, in the order of `parts` |
| `registry`, `Session` | the edit area, the shared element commands, every notation's own, the diagrams' and the skills'; a project's session, `Session<Store, ProjectView>` |
| `registry_checked` | the commands of `registry` with a commit check run on every change before it is kept, whose refusal takes the change back whole; for tests of an operation's atomicity (AR-641) |
| `new_project_session` | a session with the project's Model root named `name`; the host supplies the ids |
| `session_from_rows`, `session_from_rows_repaired` | a session rebuilt from a project's rows and checked (by `check_kinds_in_rows`, `check_bindings_in_rows`, then `name_kind`), saved as it is; the second runs a repair on the store first; refused with an `OpenError` |
| `session_from_rows_diagnosed`, `Opened` | the second, with the problems of the store the session gets, after the repair (`open_problems`), beside it |
| `open_problems` | what every notation of the composition reports wrong in a store as a project opens (`Notation::open_problems`), each an `OpenProblem` of the diagram crate, in the order of the notations: UML's default-diagram binding problems today |
| `open_steps` | what every notation of the composition changes in the model of a project just opened, to bring a shape an earlier version saved up to date (`Notation::open_step`, an `OpenStep` of the diagram crate: one of its commands, run as one step written with the next change and undone like any), in the order of the notations; what a refused step could not do, in English: UML's root package today (`uml.model.root`, owner 2026-11-04), which the shell takes as it adopts a project |
| `OpenError` | why rows do not open: `UnknownKinds` or `BrokenBindings`, which callers match, or any other reason in English |
| `check_bindings_in_rows`, `BrokenBinding`, `BindingEnd` | the default-diagram bindings in rows whose owner or diagram is no row of any table, or none |
| `check_kinds`, `check_kinds_in_rows`, `UnknownKinds`, `UnknownDiagram` | the diagrams of kinds no notation knows, in a store or in rows of any shape whose diagram table has its `id` and `kind` columns, read by name alone, so a host checks a project before it brings its tables up to date |
| `fake::{NOTATIONS, KIND, MAX_NAME, Sketch}` | feature `testing`, never shipped: the fake notation, Sketch, on archforge-mof's toy metamodel, composed alone (`fake::NOTATIONS`): packages, classes, their attributes, operations, generalizations and associations, descriptions in owned comments, its one diagram kind (`KIND`, `test.sketch`) and its `sketch.*` commands; for the tests of the shell and the project store, which name no real notation; its rows survive a save and a reopening (`tests/fake.rs`) |
| `differences` | what differs between two states of a project, classified and named (`classified`), and one element's or one diagram's two sides (`element_sides`, `diagram_sides`), each taking the composition the project is of first |
