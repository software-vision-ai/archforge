# archforge-notations

**Purpose.** The notations a project holds, composed. Each brings what it adds to a project as a
`Notation`: its metamodel, its index and slabs, and the tables and list tables its objects are
stored in. A project is made from the list `NOTATIONS` (the shared core and the diagrams first,
then ER and UML), so one universe, one store, one database and one command table hold the
notations side by side. The shell, the host and the agent's tools make projects here and name no
notation for it.

**Layer.** 7, the one crate above the notations (6) that names them all; below the shell (8).
Pure, builds for the portable target.

**Binding rules.** The tables are stored in the order of `NOTATIONS`, and the project's fingerprint
is taken over them in that order: a metamodel joins at the end, and the fingerprint the tables had
goes to the project crate's `EARLIER_MODELS`, so a project of the earlier list is brought up to date.
A project's database holds only the tables rows have gone to, so a notation's tables cost nothing
until the project uses it. The session's view is ER's until the project has one view across
notations; UML brings no commands yet.

**Public API.**

| Item | What it is |
|---|---|
| `Notation`, `NOTATIONS` | what a metamodel brings; the list a project holds |
| `universe`, `store` | the universe of every notation; an empty store over it with the host's ids |
| `storage` | every notation's tables and list tables, in the order of `NOTATIONS` |
| `registry`, `Session` | the commands of every notation, the diagrams' and the skills'; a project's session |
| `new_project_session` | a session with the project's Model root named `name`; the host supplies the ids |
| `session_from_rows`, `session_from_rows_repaired` | a session rebuilt from a project's rows and checked, saved as it is; the second runs a repair on the store first |
