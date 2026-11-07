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

The project's conversations (AR-206; the signed-off schema is `docs/design/chat-storage.md`) live in
seven tables of its own, `chat` and `chat_*`, outside the model's tables and fingerprint; no model
table may take such a name. They are made with the first conversation, and a project of format 1 or
2 becomes format 3 only then. They are committed in chat commits of their own (author
`Archforge chat`), staging only the chat tables, at a turn's end and when a conversation is made,
renamed or deleted; Save stages every changed table but them, by name, and they never make the
project unsaved. Don't save commits what of them waits and refuses when it cannot, so its reset never
puts them back. The settings go into a chat commit only when they hold no other change, so a model
fingerprint is never committed without its tables. A question is capped and refused when it holds a
kept API key (`archforge-secrets`); what an agent or tool produced is cut and marked; everything read
back is shape-checked, capped and refused with its reason, and a damaged chat table never blocks the
model.

**Public API.**

| Item | Signature |
|---|---|
| `Project::create` | `pub fn create(dolt: &Dolt, opts: &ServerOptions, folder: &Path, name: &str, created_by: &str, now: &str) -> Result<Project, Error>` |
| `Project::create_with` | the same with `Hooks` (a step after the server starts, a cancel flag) |
| `Project::open` | `pub fn open(dolt: &Dolt, opts: &ServerOptions, path: &Path) -> Result<Project, Error>` |
| `Project::{settings, path, server, close}` | the open project's facts, its server, and closing it |
| `Settings` | format, name, created by, created at |
| `timestamp` | `pub fn timestamp(unix_seconds: u64) -> String` (RFC 3339, UTC) |
| `Project::{prepare_model, write_model, read_model, has_unsaved, commit}` | the model's tables; Save commits every changed table but the conversations' |
| `Project::discard` | `pub fn discard(&self, now: &str) -> Result<(), Error>`: Don't save; the conversations are kept, and this session's change links go back to their state at the last save |
| `Project::{create_chat, start_turn, append_reply, append_tool_call, end_turn}` | make a conversation (its title refused when it holds a kept key); start a turn with its question (refused over 64 KiB or holding a kept key); append replies and tool calls; end it with what it changed, then a chat commit |
| `Project::{set_change_states, rename_chat, set_chat_model, commit_chats, open_chats}` | change links after undo or redo; rename; the next turn's model; commit what waits; on opening, interrupt turns left running |
| `Project::{delete_chat, restore_chat}`, `Deleted` | delete a conversation in one chat-only commit, what waited committed first, giving its label and the commit before (it stays in that commit); bring it back from that commit, every row in one chat-only commit, refused when listed, the commit is not the project's or lacks it, its label is taken, or the project is full |
| `Project::{list_chats, load_chat}` | the conversations newest first with provider and message count (`Listed`); one conversation whole (`Conversation`) |
| `TurnSpan` | a turn's session revisions before and after; `moved(state, revision)` says whether undo or redo undid or applied it |
| `CHAT_TABLES`, `CHAT_AUTHOR`, `limits` | the seven tables, the chat commits' author, the size limits |
| `FORMAT`, `EXTENSION`, `DATABASE`, `MARKER` | the format written and read, the folder's extension, the database, the unfinished marker |
| `Error` | one enum; every variant names the value it is about |

**Tests.** `tests/project.rs`, `tests/model.rs` and `tests/chat.rs` (the conversations) against a real dolt 2.6 or later, measured on macOS; the Linux and
Windows runs are still to be done.

**Non-goals.** The Open screen and the recent list; the chat panel and the agent runtimes' private
folders.
