# archforge-notations

**Purpose.** The notations a project holds, composed. Each metamodel brings what it adds to the
store as a `Part`: its metamodel, its index and slabs, and the tables and list tables its objects
are stored in (`PARTS`: the shared core and the diagrams first, then ER and UML). Each notation
brings what it answers in the project's view as a `Notation` (`NOTATIONS`: ER, and UML's class
diagrams).
So one universe, one store, one database, one view and one command table hold the notations side
by side. The shell, the host and the agent's tools make projects here and name no
notation for it.

**Layer.** 8, the one crate above the notations (7) that names them all; below the shell (9).
Pure, builds for the portable target.

**Binding rules.** The tables are stored in the order of `PARTS`, and the project's fingerprint
is taken over them in that order: a metamodel joins at the end, and the fingerprint the tables had
goes to the project crate's `EARLIER_MODELS`, so a project of the earlier list is brought up to date.
A metamodel that gains a table changes the fingerprint the same way: the diagrams' table of packages'
default diagrams (`diagram_default_diagram_binding`, AR-552) put 0.14.0's on that list. Such an
upgrade only adds the tables and columns that came since, as storage always has; it migrates no
diagram kind and rewrites no row. A project's database holds only the tables rows have gone to, so a
notation's tables cost nothing until the project uses it. A session starts with `view()`, never the
empty default view, which holds no notation and so knows no diagram kind. A model holding a diagram
of a kind none of `NOTATIONS` knows, such as one an earlier version stored, is refused as it opens,
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
| `Part`, `PARTS` | what a metamodel brings to the store, its name rule among it; the list a project holds |
| `name_kind` | the kind an element is told apart within by name in its owner, by its metamodel's rule (UML's is `archforge_uml::names::kind`, ER's `archforge_er::names::kind`): the check as a project opens refuses only two of one kind named alike (AR-208, AR-209) |
| `NOTATIONS`, `view` | the notations a project's view asks; a `ProjectView` of them |
| `universe`, `store` | the universe of every notation; an empty store over it with the host's ids |
| `storage` | every metamodel's tables and list tables, in the order of `PARTS` |
| `registry`, `Session` | the edit area, the shared element commands, every notation's own, the diagrams' and the skills'; a project's session, `Session<Store, ProjectView>` |
| `registry_checked` | the commands of `registry` with a commit check run on every change before it is kept, whose refusal takes the change back whole; for tests of an operation's atomicity (AR-641) |
| `new_project_session` | a session with the project's Model root named `name`; the host supplies the ids |
| `session_from_rows`, `session_from_rows_repaired` | a session rebuilt from a project's rows and checked (by `check_kinds_in_rows`, `check_bindings_in_rows`, then `name_kind`), saved as it is; the second runs a repair on the store first; refused with an `OpenError` |
| `session_from_rows_diagnosed`, `Opened` | the second, with the binding problems of the store the session gets, after the repair (`archforge_uml::defaults::problems`), beside it |
| `OpenError` | why rows do not open: `UnknownKinds` or `BrokenBindings`, which callers match, or any other reason in English |
| `check_bindings_in_rows`, `BrokenBinding`, `BindingEnd` | the default-diagram bindings in rows whose owner or diagram is no row of any table, or none |
| `check_kinds`, `check_kinds_in_rows`, `UnknownKinds`, `UnknownDiagram` | the diagrams of kinds no notation knows, in a store or in rows of any shape whose diagram table has its `id` and `kind` columns, read by name alone, so a host checks a project before it brings its tables up to date |
