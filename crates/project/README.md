# archforge-project

**Purpose.** A project on disk: `<name>.archforge`, the data folder of a Dolt server with one
database, `project`, whose `settings` table says what the project is. Creates it, opens it, reads
its settings back, and closes it.

**Layer.** Host services, layer 10, above `archforge-dolt`: the application uses it, the interface
never depends on it.

**Binding rules.** A name follows `archforge-names`. A creation validates first, keeps a marker
file until it is finished, and undoes itself on failure, removing the new folder only while it is
still the one it made (same identity, marker or empty). A folder from elsewhere is never trusted:
others may not write to it, its own `.dolt` may hold nothing but a server lock, and its settings are
read with a cap on rows, keys and values; anything unexpected is refused, never changed.


The model's tables are the notations' generated storage, composed by `archforge-notations`;
a project's database holds only the tables rows have gone to, made with the first rows, and a table
it lacks reads as empty, so a notation the project does not use costs it nothing. The table
`model_tables` names every model table rows have gone to, written with those rows: one of them
missing is a loss, and the project is refused rather than read short. A project of an earlier
Archforge records the model tables it holds when it is brought up to date. Every table's shape
is looked at in two queries, whatever the number of tables. A project whose model fingerprint is an
earlier Archforge's is brought up to date when it opens, but only by adding the columns that come
after the ones a table has, empty or holding what a new object holds. A table
Archforge does not know, columns it does not write, or a list table of another shape keep the
project refused, unchanged. Opening refuses a project whose database holds triggers,
stored procedures or events, since Archforge's writes would set them off.

**Public API.**

| Item | Signature |
|---|---|
| `Project::create` | `pub fn create(dolt: &Dolt, opts: &ServerOptions, folder: &Path, name: &str, created_by: &str, now: &str) -> Result<Project, Error>` |
| `Project::create_with` | the same with `Hooks` (a step after the server starts, a cancel flag) |
| `Project::open` | `pub fn open(dolt: &Dolt, opts: &ServerOptions, path: &Path) -> Result<Project, Error>` |
| `Project::{settings, path, server, close}` | the open project's facts, its server, and closing it |
| `Settings` | format, name, created by, created at |
| `timestamp` | `pub fn timestamp(unix_seconds: u64) -> String` (RFC 3339, UTC) |
| `FORMAT`, `EXTENSION`, `DATABASE`, `MARKER` | the format written and read, the folder's extension, the database, the unfinished marker |
| `Error` | one enum; every variant names the value it is about |

**Tests.** `tests/project.rs` against a real dolt 2.6 or later, measured on macOS; the Linux and
Windows runs are still to be done.

**Non-goals.** The model's own tables and Save, the Open screen and the recent list.
