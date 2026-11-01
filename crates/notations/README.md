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
A project's database holds only the tables rows have gone to, so a notation's tables cost nothing
until the project uses it. A session starts with `view()`, never the empty default view, which
holds no notation and so knows no diagram kind.

**Public API.**

| Item | What it is |
|---|---|
| `Part`, `PARTS` | what a metamodel brings to the store; the list a project holds |
| `NOTATIONS`, `view` | the notations a project's view asks; a `ProjectView` of them |
| `universe`, `store` | the universe of every notation; an empty store over it with the host's ids |
| `storage` | every metamodel's tables and list tables, in the order of `PARTS` |
| `registry`, `Session` | the edit area, the shared element commands, every notation's own, the diagrams' and the skills'; a project's session, `Session<Store, ProjectView>` |
| `new_project_session` | a session with the project's Model root named `name`; the host supplies the ids |
| `session_from_rows`, `session_from_rows_repaired` | a session rebuilt from a project's rows and checked, saved as it is; the second runs a repair on the store first |
