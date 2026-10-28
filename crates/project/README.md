# archforge-project

**Purpose.** A project on disk: `<name>.archforge`, the data folder of a Dolt server with one
database, `project`, whose `settings` table says what the project is and whose `er_` tables hold
the ER model. Creates it, opens it (upgrading an older format), reads its settings back, stores and
loads the ER model, and closes it.

**Layer.** Host services, layer 8, above `archforge-dolt`: the application uses it, the interface
never depends on it.

**Binding rules.** A name follows `archforge-names`. A creation validates first, keeps a marker
file until it is finished, and undoes itself on failure, removing the new folder only while it is
still the one it made (same identity, marker or empty). A folder from elsewhere is never trusted:
others may not write to it, its own `.dolt` may hold nothing but a server lock, and its settings are
read with a cap on rows, keys and values; anything unexpected is refused, never changed.

The ER model is stored in the working set only, so Save stays a Dolt commit: a store checks the
model first, rewrites the five tables in one transaction and rolls back past its time budget
(10 s). A load checks the tables' columns, reads every table row by row with a cap on rows and on all
text (what the largest valid model can hold), and refuses anything that does not make a valid model as damaged. Unsaved means an `er_`
table differs from HEAD, or a setting other than `er_next` does (owner decision). Opening refuses
a project whose database holds triggers, stored procedures or events, since Archforge's writes
would set them off. Opening a format-1 project upgrades it: anything staged that is not Archforge's refuses the opening, then the
tables are made, the settings say format 2 in one transaction, and only Archforge's tables are
staged by name and committed; a crash at any step is finished by the next opening.

Measured with dolt 2.6: a DOUBLE comes back exact as its own text, but cast to text it is written
out in full decimals, so the place columns are read as they are, after the column check. Negative
zero comes back as zero, which compares equal.

**Public API.**

| Item | Signature |
|---|---|
| `Project::create` | `pub fn create(dolt: &Dolt, opts: &ServerOptions, folder: &Path, name: &str, created_by: &str, now: &str) -> Result<Project, Error>` |
| `Project::create_with` | the same with `Hooks` (a step after the server starts, a cancel flag) |
| `Project::open` | `pub fn open(dolt: &Dolt, opts: &ServerOptions, path: &Path) -> Result<Project, Error>` |
| `Project::open_with` | the same with `Hooks` (a step of the upgrade to fail at) |
| `Project::store_er` | `pub fn store_er(&self, model: &ErModel, counter: u64) -> Result<(), Error>` |
| `Project::store_er_within` | the same with its own time budget |
| `Project::load_er` | `pub fn load_er(&self) -> Result<(ErModel, u64), Error>` |
| `Project::model_uncommitted` | `pub fn model_uncommitted(&self) -> Result<bool, Error>` |
| `UpgradeStep` | the points of an upgrade a test can stop at |
| `NEXT`, `STORE_BUDGET` | the id counter's setting, a store's time budget |
| `Project::{settings, path, server, close}` | the open project's facts, its server, and closing it |
| `Settings` | format, name, created by, created at |
| `timestamp` | `pub fn timestamp(unix_seconds: u64) -> String` (RFC 3339, UTC) |
| `FORMAT`, `EXTENSION`, `DATABASE`, `MARKER` | the format written and read, the folder's extension, the database, the unfinished marker |
| `Error` | one enum; every variant names the value it is about |

**Tests.** `tests/project.rs` and `tests/er.rs` against a real dolt 2.6 or later, measured on macOS; the Linux and
Windows runs are still to be done.

**Non-goals.** Save itself, the Open screen and the recent list.
