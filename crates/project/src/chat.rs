//! Conversations kept in the project (AR-206, the first cut of AR-42; the owner signed off the
//! schema in `docs/design/chat-storage.md`): seven tables of the project's own, beside `settings`
//! and `model_tables`, outside the model's generated tables and its fingerprint.
//!
//! - `chat` is a conversation, `chat_turn` one question and everything it caused, `chat_message`
//!   the timeline (user text, agent text, tool activity rows), `chat_tool_call` the details of a
//!   tool row, `chat_anchor` a message's elements, `chat_change` and `chat_change_element` what a
//!   turn changed in the model and the state of those changes.
//! - Ids are UUID v7 in `BINARY(16)`, as every element's; times are RFC 3339 UTC text; order comes
//!   from integer positions, never from times. Anchors point at element UUIDs with a snapshot of the
//!   metaclass and name, so an anchor to a deleted element stays, and comes alive again when undo
//!   brings the element back under its UUID.
//! - The tables are made with the first conversation, all seven together; one missing reads as
//!   empty. A project of format 1 or 2 is raised to format 3 when it first holds a conversation.
//! - Chats are committed in chat commits of their own (author [`CHAT_AUTHOR`]), staging only the
//!   chat tables, at the end of each turn and when a conversation is made, renamed or deleted. A
//!   Save never commits them and they never make the project unsaved; Don't save commits what of
//!   them waits before it resets, so they are never put back with the model.
//! - Not stored: keys, tokens, the control port, runtime transcripts and session ids, raw provider
//!   events, provider error texts, reasoning, usage or cost. A question holding a kept API key is
//!   refused.
//! - Everything read is untrusted: every chat table is shape-checked, codes are checked against
//!   closed lists, rows and bytes are capped, and a damaged conversation is refused with its reason,
//!   never skipped. A damaged chat table never blocks the model.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{MutexGuard, PoisonError};

use archforge_mof::Uuid;
use archforge_secrets::SecretStore;
use mysql::prelude::Queryable;

use crate::model::{changed_tables, columns_of, same_column, sql, stage, tables_held_by};
use crate::{DATABASE, Error, FORMAT, Project};

/// The conversations' tables, all made together with the first conversation.
pub const CHAT_TABLES: [&str; 7] = ["chat", "chat_turn", "chat_message", "chat_tool_call", "chat_anchor", "chat_change", "chat_change_element"];

/// The author of every chat commit, so a history can tell them from Saves without reading messages.
pub const CHAT_AUTHOR: &str = "Archforge chat <chat@localhost>";

/// What the conversations may hold (owner decision): over a limit, what a person asked for is
/// refused, and what an agent or a tool produced is cut and marked.
pub mod limits {
    /// A question, in bytes: a longer one is refused, neither sent nor stored.
    pub const QUESTION_BYTES: usize = 64 << 10;
    /// An agent's message as stored, in bytes: a longer one is cut at a character boundary.
    pub const REPLY_BYTES: usize = 256 << 10;
    /// A tool call's arguments, as JSON, in bytes.
    pub const ARGUMENTS_BYTES: usize = 8 << 10;
    /// The start of a tool call's result kept, in bytes; its full size is kept beside it.
    pub const RESULT_BYTES: usize = 2 << 10;
    /// Archforge's own refusal or failure text of a tool call, in bytes.
    pub const ERROR_BYTES: usize = 1 << 10;
    /// A conversation's title, in characters: refused when renaming, cut when made automatically.
    pub const TITLE_CHARS: usize = 200;
    /// An element's name snapshot, in characters: cut.
    pub const NAME_CHARS: usize = 256;
    /// The anchors kept per message: the rest are not stored, and the message says so.
    pub const ANCHORS_PER_MESSAGE: usize = 500;
    /// The changed elements kept per turn: the rest are not stored; the count stays exact.
    pub const CHANGED_ELEMENTS_PER_TURN: usize = 2_000;
    /// The messages a conversation may hold.
    pub const MESSAGES_PER_CHAT: usize = 5_000;
    /// The conversations a project may hold.
    pub const CHATS_PER_PROJECT: usize = 10_000;
    /// A conversation's origin label, the undo-step label of its changes (AR-198), in characters.
    pub const ORIGIN_CHARS: usize = 64;
    /// A model's name, in characters.
    pub const MODEL_CHARS: usize = 128;
    /// Who started a conversation, in characters: as the project's own `created_by`.
    pub const CREATED_BY_CHARS: usize = 1024;
    /// An element's metaclass, its generated table's name, in characters.
    pub const METACLASS_CHARS: usize = 64;
    /// A bridge tool's name, in characters.
    pub const TOOL_CHARS: usize = 64;
    /// A command id, in characters.
    pub const COMMAND_CHARS: usize = 128;
    /// The property or aspect a tool call was about, in characters: cut.
    pub const DETAIL_CHARS: usize = 64;
    /// The chips removed of a conversation, as stored: `project,diagram,selection` at most.
    pub const REMOVED_CHIPS_CHARS: usize = 32;
    /// What reading one conversation, or the list of them, may take, in bytes.
    pub const READ_BYTES: usize = 64 << 20;
}

use limits::*;

/// One of the conversations' tables: its columns as `information_schema` checks them, in order,
/// and its keys. `first` counts the columns it was first made with; those after them are nullable,
/// added at the end within chat shape 1, so a table made before them lacks them and its rows read
/// them as empty.
struct TableDef {
    name: &'static str,
    columns: &'static [(&'static str, &'static str)],
    first: usize,
    keys: &'static str,
}

impl TableDef {
    /// The statement that makes the table, with every column, when it is missing.
    fn create(&self) -> String {
        let columns: Vec<String> = self.columns.iter().map(|(name, sql)| format!("`{name}` {sql}")).collect();
        format!("CREATE TABLE IF NOT EXISTS `{}` ({}, {})", self.name, columns.join(", "), self.keys)
    }
}

/// The chat tables a project holds: how many, and those made before their later columns, each with
/// the count of the columns it has.
struct Held {
    tables: usize,
    narrow: BTreeMap<&'static str, usize>,
}

/// Where `chat.removed_chips` is among the `chat` table's columns.
const CHIPS_AT: usize = 8;

impl Held {
    /// Whether the `chat` table has the column of the chips removed: all its columns, or as many as
    /// reach past it.
    fn chips_column(&self) -> bool {
        self.narrow.get("chat").is_none_or(|has| *has > CHIPS_AT)
    }
}

/// The seven tables as the signed-off schema gives them, with the columns added since within
/// chat shape 1.
const TABLES: [TableDef; 7] = [
    TableDef {
        name: "chat",
        columns: &[
            ("id", "BINARY(16) NOT NULL"),
            ("origin", "VARCHAR(64) NOT NULL"),
            ("title", "VARCHAR(200) NOT NULL"),
            ("provider", "VARCHAR(16) NOT NULL"),
            ("model", "VARCHAR(128) NOT NULL"),
            ("created_by", "VARCHAR(1024) NOT NULL"),
            ("created_at", "VARCHAR(20) NOT NULL"),
            ("updated_at", "VARCHAR(20) NOT NULL"),
            ("removed_chips", "VARCHAR(32) NULL"),
        ],
        first: 8,
        keys: "PRIMARY KEY (`id`), INDEX `chat_recent` (`updated_at`)",
    },
    TableDef {
        name: "chat_turn",
        columns: &[
            ("id", "BINARY(16) NOT NULL"),
            ("chat", "BINARY(16) NOT NULL"),
            ("number", "INT NOT NULL"),
            ("model", "VARCHAR(128) NOT NULL"),
            ("allow_changes", "TINYINT NOT NULL"),
            ("outcome", "VARCHAR(16) NOT NULL"),
            ("error", "VARCHAR(32) NULL"),
            ("started_at", "VARCHAR(20) NOT NULL"),
            ("ended_at", "VARCHAR(20) NULL"),
        ],
        first: 9,
        keys: "PRIMARY KEY (`id`), UNIQUE INDEX `chat_turn_order` (`chat`, `number`)",
    },
    TableDef {
        name: "chat_message",
        columns: &[
            ("id", "BINARY(16) NOT NULL"),
            ("chat", "BINARY(16) NOT NULL"),
            ("turn", "BINARY(16) NOT NULL"),
            ("position", "INT NOT NULL"),
            ("role", "VARCHAR(16) NOT NULL"),
            ("body", "MEDIUMTEXT NULL"),
            ("complete", "TINYINT NOT NULL"),
            ("shortened", "TINYINT NOT NULL"),
            ("anchors_cut", "TINYINT NOT NULL"),
            ("created_at", "VARCHAR(20) NOT NULL"),
        ],
        first: 10,
        keys: "PRIMARY KEY (`id`), UNIQUE INDEX `chat_message_order` (`chat`, `position`), INDEX `chat_message_turn` (`turn`)",
    },
    TableDef {
        name: "chat_tool_call",
        columns: &[
            ("message", "BINARY(16) NOT NULL"),
            ("tool", "VARCHAR(64) NOT NULL"),
            ("command", "VARCHAR(128) NULL"),
            ("detail", "VARCHAR(64) NULL"),
            ("status", "VARCHAR(16) NOT NULL"),
            ("changed_model", "TINYINT NOT NULL"),
            ("element_count", "INT NULL"),
            ("arguments", "MEDIUMTEXT NULL"),
            ("result", "MEDIUMTEXT NULL"),
            ("result_bytes", "INT NULL"),
            ("error", "VARCHAR(1024) NULL"),
            ("shortened", "TINYINT NOT NULL"),
        ],
        first: 12,
        keys: "PRIMARY KEY (`message`)",
    },
    TableDef {
        name: "chat_anchor",
        columns: &[
            ("message", "BINARY(16) NOT NULL"),
            ("element", "BINARY(16) NOT NULL"),
            ("role", "VARCHAR(16) NOT NULL"),
            ("metaclass", "VARCHAR(64) NOT NULL"),
            ("name", "VARCHAR(256) NULL"),
        ],
        first: 5,
        keys: "PRIMARY KEY (`message`, `element`, `role`), INDEX `chat_anchor_element` (`element`)",
    },
    TableDef {
        name: "chat_change",
        columns: &[
            ("turn", "BINARY(16) NOT NULL"),
            ("changes", "INT NOT NULL"),
            ("elements", "INT NOT NULL"),
            ("state", "VARCHAR(16) NOT NULL"),
            ("updated_at", "VARCHAR(20) NOT NULL"),
        ],
        first: 5,
        keys: "PRIMARY KEY (`turn`)",
    },
    TableDef {
        name: "chat_change_element",
        columns: &[
            ("turn", "BINARY(16) NOT NULL"),
            ("element", "BINARY(16) NOT NULL"),
            ("kind", "VARCHAR(16) NOT NULL"),
            ("metaclass", "VARCHAR(64) NOT NULL"),
            ("name", "VARCHAR(256) NULL"),
        ],
        first: 5,
        keys: "PRIMARY KEY (`turn`, `element`), INDEX `chat_change_element_element` (`element`)",
    },
];

/// Whether `name` is one of the conversations' tables.
pub(crate) fn is_chat_table(name: &str) -> bool {
    CHAT_TABLES.contains(&name)
}

/// A closed list of short codes, stored as text and checked against the list on reading.
macro_rules! codes {
    ($(#[$m:meta])* $name:ident { $($(#[$vm:meta])* $variant:ident = $code:literal),+ $(,)? }) => {
        $(#[$m])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum $name {
            $($(#[$vm])* $variant),+
        }

        impl $name {
            /// Every value, in order.
            pub const ALL: &'static [$name] = &[$($name::$variant),+];

            /// The code stored for it.
            pub fn code(self) -> &'static str {
                match self {
                    $($name::$variant => $code),+
                }
            }

            /// The value a stored code names; any other code is refused, naming the accepted ones.
            pub fn from_code(code: &str) -> Result<$name, String> {
                match code {
                    $($code => Ok($name::$variant),)+
                    other => Err(format!(
                        "{:?} is not one of {}",
                        other.chars().take(32).collect::<String>(),
                        [$($code),+].join(", ")
                    )),
                }
            }
        }
    };
}

codes! {
    /// The agent runtime a conversation talks to; fixed for the conversation's life.
    Provider { Claude = "claude", Codex = "codex" }
}

codes! {
    /// Who a message is from: the person, the agent, or a tool activity row.
    Role { User = "user", Assistant = "assistant", Tool = "tool" }
}

codes! {
    /// How a turn ended, or that it runs; `Interrupted` when Archforge found it running on opening.
    TurnOutcome { Running = "running", Completed = "completed", Stopped = "stopped", Failed = "failed", Interrupted = "interrupted" }
}

codes! {
    /// Why a turn failed, as a neutral category: never the provider's text.
    TurnError {
        MissingRuntime = "missing_runtime",
        InvalidCredential = "invalid_credential",
        SignInExpired = "sign_in_expired",
        RateLimited = "rate_limited",
        Network = "network",
        ModelUnavailable = "model_unavailable",
        Other = "other",
    }
}

codes! {
    /// How a tool call went: `Refused` is Archforge saying no, `Denied` the runtime's own refusal.
    ToolStatus { Running = "running", Done = "done", Refused = "refused", Failed = "failed", Denied = "denied" }
}

codes! {
    /// Why a message names an element: a context chip sent with a question (`Project`, `Diagram`,
    /// `Selection`), an element a tool read, or one a changing tool call created or changed.
    AnchorRole { Project = "project", Diagram = "diagram", Selection = "selection", Read = "read", Target = "target" }
}

codes! {
    /// What Archforge last knew of a turn's changes to the model: a record, not a promise.
    ChangeState { Applied = "applied", Undone = "undone", Discarded = "discarded" }
}

codes! {
    /// What a turn did to an element: added or removed (one both added and removed within the turn
    /// is not kept), else changed, even when the turn changed it back.
    ChangeKind { Added = "added", Changed = "changed", Removed = "removed" }
}

impl AnchorRole {
    /// Whether it is a context chip, which only a question carries.
    fn is_chip(self) -> bool {
        matches!(self, AnchorRole::Project | AnchorRole::Diagram | AnchorRole::Selection)
    }
}

/// A conversation as the `chat` table holds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chat {
    pub id: Uuid,
    /// The undo-step label of its changes (AR-198), such as `Chat 7`: unique in the project.
    pub origin: String,
    pub title: String,
    pub provider: Provider,
    /// The model the next turn asks for.
    pub model: String,
    pub created_by: String,
    pub created_at: String,
    /// Its last activity: the list goes newest first by it.
    pub updated_at: String,
    /// The context chips the person removed from it (AR-227): only `Project`, `Diagram` and
    /// `Selection`; none for a row written before they were kept.
    pub removed_chips: BTreeSet<AnchorRole>,
}

/// A conversation deleted (AR-217): its label, and the commit just before the deletion, which
/// holds it whole and brings it back ([`Project::restore_chat`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Deleted {
    pub origin: String,
    /// The commit's hash: 32 of Dolt's base-32 digits.
    pub before: String,
}

/// A conversation in the list: its row and its count of questions and replies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatSummary {
    pub chat: Chat,
    pub messages: u32,
}

/// One entry of the list: a conversation, or why its row cannot be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listed {
    pub id: Uuid,
    pub entry: Result<ChatSummary, String>,
}

/// A turn as `chat_turn` holds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Turn {
    pub id: Uuid,
    pub chat: Uuid,
    /// 1, 2, 3 … within the conversation.
    pub number: u32,
    /// The model that answered.
    pub model: String,
    /// Whether the model could be changed during this turn; every conversation opens read-only.
    pub allow_changes: bool,
    pub outcome: TurnOutcome,
    pub error: Option<TurnError>,
    pub started_at: String,
    pub ended_at: Option<String>,
}

/// A message as `chat_message` holds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub id: Uuid,
    pub chat: Uuid,
    pub turn: Uuid,
    /// Its place in the conversation, from 1.
    pub position: u32,
    pub role: Role,
    /// The text as written; none for a tool row.
    pub body: Option<String>,
    /// False when Stop, an error or a crash cut the text off.
    pub complete: bool,
    /// True when the text was cut to its limit when stored.
    pub shortened: bool,
    /// True when it named more elements than are anchored.
    pub anchors_cut: bool,
    pub created_at: String,
}

/// A tool activity row's details, as `chat_tool_call` holds them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolCall {
    pub message: Uuid,
    pub tool: String,
    pub command: Option<String>,
    pub detail: Option<String>,
    pub status: ToolStatus,
    pub changed_model: bool,
    pub element_count: Option<u32>,
    pub arguments: Option<String>,
    pub result: Option<String>,
    /// The result's full size, in bytes.
    pub result_bytes: Option<u32>,
    pub error: Option<String>,
    pub shortened: bool,
}

/// An element a message names, with the snapshot taken when it was named.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Anchor {
    pub message: Uuid,
    pub element: Uuid,
    pub role: AnchorRole,
    pub metaclass: String,
    pub name: Option<String>,
}

/// An element a turn changed: given when the turn ends, and read back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangedElement {
    pub element: Uuid,
    pub kind: ChangeKind,
    /// The element's generated table, such as `uml_class`.
    pub metaclass: String,
    /// Its name after the change, or before it when removed.
    pub name: Option<String>,
}

/// What a turn changed in the model, and the state of those changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeRecord {
    pub turn: Uuid,
    /// The commands that changed the model.
    pub changes: u32,
    /// The elements added, changed or removed in all; `changed` may hold fewer.
    pub elements: u32,
    pub state: ChangeState,
    pub updated_at: String,
    pub changed: Vec<ChangedElement>,
}

/// A turn read back, with what it changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnRecord {
    pub turn: Turn,
    pub change: Option<ChangeRecord>,
}

/// A message read back, with its tool call and anchors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageRecord {
    pub message: Message,
    pub tool: Option<ToolCall>,
    pub anchors: Vec<Anchor>,
}

/// A whole conversation: its row, its turns in order and its timeline in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conversation {
    pub chat: Chat,
    pub turns: Vec<TurnRecord>,
    pub messages: Vec<MessageRecord>,
}

/// A conversation to make.
#[derive(Debug, Clone)]
pub struct NewChat {
    pub id: Uuid,
    /// 1 to 64 printable characters, unique in the project.
    pub origin: String,
    /// The first words of the first question: cut to its limit, line breaks made spaces.
    pub title: String,
    pub provider: Provider,
    pub model: String,
    /// Who starts it: from the same source as the project's own `created_by`.
    pub created_by: String,
    pub now: String,
}

/// A turn to start.
#[derive(Debug, Clone)]
pub struct NewTurn {
    pub id: Uuid,
    pub chat: Uuid,
    pub model: String,
    pub allow_changes: bool,
    pub now: String,
}

/// A failed turn to run again in place (owner decision, AR-205): the same turn row and question,
/// the question replaced when it was edited before being sent again.
#[derive(Debug, Clone)]
pub struct Rerun {
    pub turn: Uuid,
    /// The model the turn asks for this time.
    pub model: String,
    pub allow_changes: bool,
    /// The question as edited, with the context chips sent with it this time; none sends the
    /// stored question and its chips again.
    pub question: Option<(String, Vec<NewAnchor>)>,
    pub now: String,
}

/// An element to anchor to a message.
#[derive(Debug, Clone)]
pub struct NewAnchor {
    pub element: Uuid,
    pub role: AnchorRole,
    pub metaclass: String,
    /// Its name now; cut to [`limits::NAME_CHARS`].
    pub name: Option<String>,
}

/// The question that starts a turn, with the context chips sent with it.
#[derive(Debug, Clone)]
pub struct NewQuestion {
    pub id: Uuid,
    pub body: String,
    pub anchors: Vec<NewAnchor>,
}

/// An agent's message, when it completes or when the turn ends with it cut off.
#[derive(Debug, Clone)]
pub struct NewReply {
    pub id: Uuid,
    pub turn: Uuid,
    pub body: String,
    pub complete: bool,
    /// The caller cut it already (to the limit, or to what it may hold): kept marked shortened.
    pub shortened: bool,
    pub now: String,
}

/// A tool call that ended, with the elements it read or changed.
#[derive(Debug, Clone)]
pub struct NewToolCall {
    /// The id of its timeline row.
    pub id: Uuid,
    pub turn: Uuid,
    pub tool: String,
    pub command: Option<String>,
    pub detail: Option<String>,
    pub status: ToolStatus,
    pub changed_model: bool,
    pub element_count: Option<u32>,
    pub arguments: Option<String>,
    /// The whole result: the start is kept, and its size.
    pub result: Option<String>,
    pub error: Option<String>,
    pub anchors: Vec<NewAnchor>,
    pub now: String,
}

/// What a turn changed in the model, given when it ends.
#[derive(Debug, Clone)]
pub struct TurnChanges {
    /// The commands that changed the model: at least one.
    pub changes: u32,
    /// The elements added, changed or removed in all: at least as many as `changed` lists.
    pub elements: u32,
    /// The elements, each once: one added and removed within the turn left out.
    pub changed: Vec<ChangedElement>,
}

/// How a turn ends.
#[derive(Debug, Clone)]
pub struct TurnEnd {
    pub turn: Uuid,
    pub outcome: TurnOutcome,
    /// Why it failed: given exactly when it failed.
    pub error: Option<TurnError>,
    pub now: String,
    pub changes: Option<TurnChanges>,
}

/// A turn's changes as one session's undo history holds them: the session's revision before the
/// turn's first change and after its last (`Session::revision`; the steps carry the conversation's
/// origin, AR-198). Revisions are the session's own, so this lives in memory only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TurnSpan {
    pub before: u64,
    pub after: u64,
}

impl TurnSpan {
    /// The state the turn's changes move to once an undo or redo has brought the session to
    /// `revision`, from `state`: back before the turn's first change they are undone, at the state
    /// after its last change again they are applied. Anywhere else, and once discarded, nothing
    /// moves.
    pub fn moved(&self, state: ChangeState, revision: u64) -> Option<ChangeState> {
        match state {
            ChangeState::Applied if revision == self.before => Some(ChangeState::Undone),
            ChangeState::Undone if revision == self.after => Some(ChangeState::Applied),
            _ => None,
        }
    }
}

/// The state of each change link of this session's turns, now and at the last save, so Don't save
/// can put each back: to the state it had at the save, or discarded when the turn ran since.
#[derive(Debug, Default)]
pub(crate) struct Ledger {
    now: BTreeMap<Uuid, ChangeState>,
    saved: BTreeMap<Uuid, ChangeState>,
}

impl Ledger {
    /// What Don't save moves: each turn whose state differs from the one it goes back to.
    fn discarded(&self) -> Vec<(Uuid, ChangeState)> {
        self.now
            .iter()
            .filter_map(|(turn, state)| {
                let back = self.saved.get(turn).copied().unwrap_or(ChangeState::Discarded);
                (back != *state).then_some((*turn, back))
            })
            .collect()
    }
}

// Values sent to the database.
fn id_value(u: Uuid) -> mysql::Value {
    mysql::Value::Bytes(u.0.to_vec())
}

fn text_value(s: &str) -> mysql::Value {
    mysql::Value::Bytes(s.as_bytes().to_vec())
}

fn opt_text_value(s: Option<&str>) -> mysql::Value {
    s.map_or(mysql::Value::NULL, text_value)
}

fn flag_value(b: bool) -> mysql::Value {
    mysql::Value::Int(i64::from(b))
}

/// The most an `INT` column holds.
const INT_MAX: u32 = i32::MAX.unsigned_abs();

fn int_value(what: &'static str, n: u32) -> Result<mysql::Value, Error> {
    if n > INT_MAX {
        return Err(value(what, format!("{n} is more than the {INT_MAX} stored")));
    }
    Ok(mysql::Value::Int(i64::from(n)))
}

fn value(what: &'static str, why: impl Into<String>) -> Error {
    Error::ChatValue { what, why: why.into() }
}

/// A short text: 1 to `max` characters, no control character.
fn line(what: &'static str, text: &str, max: usize) -> Result<(), Error> {
    if text.is_empty() {
        return Err(value(what, "it is empty"));
    }
    let n = text.chars().count();
    if n > max {
        return Err(Error::TooLarge { what, size: n, max, unit: "characters" });
    }
    if text.chars().any(char::is_control) {
        return Err(value(what, "it holds a control character"));
    }
    Ok(())
}

/// A time as Archforge stores it: RFC 3339 in UTC to the second, `2026-11-06T14:02:11Z`.
fn time(text: &str) -> Result<(), Error> {
    let shape = "dddd-dd-ddTdd:dd:ddZ";
    let fits = text.len() == shape.len() && text.bytes().zip(shape.bytes()).all(|(c, s)| if s == b'd' { c.is_ascii_digit() } else { c == s });
    if fits { Ok(()) } else { Err(value("a time", format!("{:?} is not of the form {shape}", text.chars().take(32).collect::<String>()))) }
}

/// An element's metaclass: its generated table's name, plain lower-case.
fn metaclass(text: &str) -> Result<(), Error> {
    let plain = !text.is_empty() && text.len() <= METACLASS_CHARS && text.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_');
    if plain { Ok(()) } else { Err(value("an element's metaclass", format!("{:?} is not a table name", text.chars().take(32).collect::<String>()))) }
}

/// `text` cut to at most `max` bytes at a character boundary, and whether it was cut.
fn cut_bytes(text: &str, max: usize) -> (&str, bool) {
    if text.len() <= max {
        return (text, false);
    }
    let mut end = max;
    while !text.is_char_boundary(end) {
        end = end.saturating_sub(1);
    }
    (text.get(..end).unwrap_or_default(), true)
}

/// `text` cut to at most `max` characters, and whether it was cut.
fn cut_chars(text: &str, max: usize) -> (String, bool) {
    (text.chars().take(max).collect(), text.chars().nth(max).is_some())
}

/// The anchors of one message, checked, each element and role once, the first
/// [`limits::ANCHORS_PER_MESSAGE`] of them; and whether there were more.
fn anchors(given: &[NewAnchor], chips: bool) -> Result<(Vec<NewAnchor>, bool), Error> {
    let mut seen = BTreeSet::new();
    let mut kept = Vec::new();
    let mut cut = false;
    for a in given {
        if a.role.is_chip() != chips {
            let carried = if chips { "a question carries only context chips" } else { "a tool row carries only elements read or changed" };
            return Err(value("an anchor", format!("{} is not one: {carried}", a.role.code())));
        }
        metaclass(&a.metaclass)?;
        if !seen.insert((a.element, a.role)) {
            continue;
        }
        if kept.len() == ANCHORS_PER_MESSAGE {
            cut = true;
            continue;
        }
        let name = a.name.as_deref().map(|n| cut_chars(n, NAME_CHARS).0);
        kept.push(NewAnchor { name, ..a.clone() });
    }
    Ok((kept, cut))
}

/// Refuses a text that holds one of the API keys in `keys`, naming the key, never showing it.
fn no_key(keys: &dyn SecretStore, text: &str) -> Result<(), Error> {
    match archforge_secrets::held_key_in(keys, text).map_err(Error::KeyCheck)? {
        Some(name) => Err(Error::HeldKey { name: name.to_string() }),
        None => Ok(()),
    }
}

/// A title made from the first question: line breaks and other control characters made spaces,
/// cut to [`limits::TITLE_CHARS`].
fn auto_title(text: &str) -> Result<String, Error> {
    let flat: String = text.chars().map(|c| if c.is_control() { ' ' } else { c }).collect();
    let (title, _) = cut_chars(flat.trim(), TITLE_CHARS);
    let title = title.trim_end().to_string();
    if title.is_empty() {
        return Err(value("the conversation's title", "it is empty"));
    }
    Ok(title)
}

/// A count read back from a query's first cell.
fn first_int(row: Option<mysql::Row>) -> Result<i64, String> {
    row.as_ref().ok_or_else(|| "a count gave no row".to_string()).and_then(|r| int_at(r, 0))
}

/// Whether `text` has the shape of a Dolt commit hash: 32 of the digits and lower-case letters `a`
/// to `v` Dolt writes them in. Only such a text is put into a query's `AS OF` or a `USE`.
pub(crate) fn is_commit(text: &str) -> bool {
    text.len() == 32 && text.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'v').contains(&b))
}

/// The hash of the commit the project's branch is at, checked to be one.
pub(crate) fn head(c: &mut impl Queryable) -> Result<String, String> {
    let row: Option<mysql::Row> =
        c.query_first("SELECT LEFT(CAST(HASHOF('HEAD') AS CHAR), 64)").map_err(|e| format!("its last commit cannot be read: {e}"))?;
    let hash = row.and_then(|r| r.get_opt::<String, usize>(0)).and_then(Result::ok).unwrap_or_default();
    if is_commit(&hash) { Ok(hash) } else { Err("its last commit does not read as one".into()) }
}

/// The next number of a sequence whose highest is `highest`, within what an `INT` holds.
fn next(what: &'static str, highest: i64) -> Result<u32, Error> {
    highest.checked_add(1).and_then(|n| u32::try_from(n).ok()).filter(|n| *n <= INT_MAX).ok_or_else(|| value(what, "the numbering is full"))
}

impl Project {
    fn changes(&self) -> MutexGuard<'_, Ledger> {
        self.changes.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// A save took every turn's state as it is now.
    pub(crate) fn changes_saved(&self) {
        let mut ledger = self.changes();
        ledger.saved = ledger.now.clone();
    }

    fn unreadable(&self, why: impl Into<String>) -> Error {
        Error::ChatsUnreadable { path: self.path().to_path_buf(), why: why.into() }
    }

    /// Which of the chat tables the project holds, each of exactly its shape, or of its first
    /// shape with only later columns missing at its end; one of another shape, or something else
    /// under its name, makes the conversations unreadable.
    fn chat_shapes(&self, c: &mut mysql::Conn) -> Result<Held, Error> {
        let held = tables_held_by(c, &is_chat_table).map_err(|why| self.unreadable(why))?;
        let mut present = Vec::new();
        for name in CHAT_TABLES {
            match held.get(name) {
                None => {}
                Some(false) => return Err(self.unreadable(format!("{name} is not a table"))),
                Some(true) => present.push(name.to_string()),
            }
        }
        let found = columns_of(c, present.clone()).map_err(|why| self.unreadable(why))?;
        let mut narrow = BTreeMap::new();
        for def in &TABLES {
            if let Some(columns) = found.get(def.name) {
                if columns.len() < def.first || columns.len() > def.columns.len() || !columns.iter().zip(def.columns).all(|(f, e)| same_column(f, e))
                {
                    return Err(self.unreadable(format!("the table {} does not have the shape Archforge writes", def.name)));
                }
                if columns.len() < def.columns.len() {
                    narrow.insert(def.name, columns.len());
                }
            }
        }
        Ok(Held { tables: present.len(), narrow })
    }

    /// The chat tables when the project holds them all, each of its shape (or of its first shape);
    /// none when it holds none.
    fn chat_tables(&self, c: &mut mysql::Conn) -> Result<Option<Held>, Error> {
        let held = self.chat_shapes(c)?;
        match held.tables {
            0 => Ok(None),
            n if n == CHAT_TABLES.len() => Ok(Some(held)),
            _ => Err(self.unreadable("some of its chat tables are missing")),
        }
    }

    /// Whether the project holds the conversations' tables, all of them, each of its shape.
    fn chats_held(&self, c: &mut mysql::Conn) -> Result<bool, Error> {
        Ok(self.chat_tables(c)?.is_some())
    }

    /// Adds to each chat table made before its later columns the columns it lacks, at its end,
    /// empty for the rows it holds, before a write that names them. Each addition is a change to the
    /// table that the next chat commit takes. Run outside a transaction: a change of a table's
    /// shape ends one.
    fn widen(&self, c: &mut mysql::Conn, held: &Held) -> Result<(), Error> {
        for def in &TABLES {
            let Some(has) = held.narrow.get(def.name) else { continue };
            for (name, sql_type) in def.columns.iter().skip(*has) {
                c.query_drop(format!("ALTER TABLE `{}` ADD COLUMN `{name}` {sql_type}", def.name))
                    .map_err(sql("cannot add a column to the conversations' tables"))?;
                log::info!("{}: the table {} took the column {name}", self.path().display(), def.name);
            }
        }
        Ok(())
    }

    /// A connection to a project that holds the conversations' tables; `missing` when it holds none.
    fn chat_connection(&self, missing: impl FnOnce() -> Error) -> Result<mysql::Conn, Error> {
        let mut c = self.model_connection()?;
        if !self.chats_held(&mut c)? {
            return Err(missing());
        }
        Ok(c)
    }

    /// The format the project's settings hold now, written or not.
    fn stored_format(&self, c: &mut impl Queryable) -> Result<u32, Error> {
        let row: Option<mysql::Row> = c
            .query_first("SELECT LEFT(CAST(`value` AS CHAR), 16) FROM settings WHERE `key` = 'format'")
            .map_err(sql("cannot read the project's format"))?;
        let text = row.and_then(|r| r.get_opt::<String, usize>(0)).and_then(Result::ok);
        text.as_deref()
            .and_then(|t| t.parse::<u32>().ok())
            .ok_or_else(|| Error::Damaged { path: self.path().to_path_buf(), why: format!("its format {text:?} is not a number") })
    }

    /// Raises a project of an earlier format that holds chat tables to [`FORMAT`] (owner decision:
    /// a project becomes format 3 when it first holds a conversation, never on opening alone). True
    /// when the settings may go into the chat commit: they held no other change, so a model
    /// fingerprint an upgrade wrote on opening is never committed without its tables. Otherwise the
    /// new format waits in the working set for the next Save, and is written again by the next chat
    /// commit should Don't save put it back.
    fn raise_format(&self, c: &mut impl Queryable, changed: &BTreeSet<String>) -> Result<bool, Error> {
        if self.stored_format(c)? >= FORMAT {
            return Ok(false);
        }
        let table: Option<mysql::Row> = c
            .exec_first("SELECT COUNT(*) FROM information_schema.tables WHERE table_schema = ? AND table_name = 'chat'", (DATABASE,))
            .map_err(sql("cannot list the conversations' tables"))?;
        if first_int(table).map_err(|why| self.unreadable(why))? == 0 {
            return Ok(false);
        }
        // A conversation, not only its tables: tables left empty by a failed first write raise nothing.
        let any: Option<mysql::Row> = c.query_first("SELECT 1 FROM `chat` LIMIT 1").map_err(sql("cannot read the conversations"))?;
        if any.is_none() {
            return Ok(false);
        }
        let alone = !changed.contains("settings");
        c.exec_drop("UPDATE settings SET `value` = ? WHERE `key` = 'format'", (FORMAT.to_string(),))
            .map_err(sql("cannot raise the project's format"))?;
        log::info!("{} holds conversations and is now format {FORMAT}", self.path().display());
        Ok(alone)
    }

    /// A chat commit: whatever of the chat tables changed, and the settings when they hold only the
    /// raised format, staged by name and committed with `message`, by [`CHAT_AUTHOR`]. The model's
    /// changes stay unsaved in the working set. False when nothing waited. Run inside a transaction,
    /// the commit takes the transaction's rows with it, and a commit that fails leaves the
    /// transaction to be rolled back: the rows and the commit land together or not at all.
    fn chat_commit(&self, c: &mut impl Queryable, message: &str) -> Result<bool, Error> {
        let changed = changed_tables(c, self.path())?;
        let mut tables: Vec<String> = changed.iter().filter(|name| is_chat_table(name)).cloned().collect();
        if self.raise_format(c, &changed)? {
            tables.push("settings".into());
        }
        if tables.is_empty() {
            return Ok(false);
        }
        // Whatever an interrupted step left staged is unstaged first, so only these tables go in.
        c.query_drop("CALL DOLT_RESET()").map_err(sql("cannot prepare the chat commit"))?;
        stage(c, &tables).map_err(sql("cannot prepare the chat commit"))?;
        // The commit's reply is not read: converting a row that is not one string would panic.
        c.exec_drop("CALL DOLT_COMMIT('-m', ?, '--author', ?)", (message, CHAT_AUTHOR)).map_err(sql("cannot commit the conversations"))?;
        Ok(true)
    }

    /// A chat commit after rows were written: a failure is said in the log, and the rows wait in
    /// the working set for the next chat commit (the next turn's end, closing, Don't save or the
    /// next opening). The write itself has succeeded.
    fn chat_commit_after(&self, c: &mut mysql::Conn, message: &str) {
        if let Err(e) = self.chat_commit(c, message) {
            log::warn!("the conversations of {} wait for the next chat commit: {e}", self.path().display());
        }
    }

    /// Before Don't save resets: commits what of the conversations waits, and refuses when that
    /// fails or leaves any chat table uncommitted, since the reset would put it back.
    pub(crate) fn keep_chats(&self, c: &mut mysql::Conn) -> Result<(), Error> {
        let not_kept = |why: String| Error::ChatsNotKept { path: self.path().to_path_buf(), why };
        self.chat_commit(c, "Conversations kept before Don't save").map_err(|e| not_kept(e.to_string()))?;
        let left: Vec<String> =
            changed_tables(c, self.path()).map_err(|e| not_kept(e.to_string()))?.into_iter().filter(|n| is_chat_table(n)).collect();
        if !left.is_empty() {
            return Err(not_kept(format!("{} still holds changes not committed", left.join(", "))));
        }
        Ok(())
    }

    /// After Don't save reset the model: each of this session's turns goes back to the state its
    /// changes had at the last save, or becomes discarded when it ran since, and the format is
    /// raised again if the reset put it back; all in a chat commit. A failure only goes to the log:
    /// the model is back at its save either way, and what was written waits for the next opening.
    pub(crate) fn chats_after_discard(&self, c: &mut mysql::Conn, now: &str) {
        let moves = self.changes().discarded();
        if !moves.is_empty() {
            match self.write_states(c, &moves, now, false) {
                Ok(()) => {
                    let mut ledger = self.changes();
                    for (turn, state) in &moves {
                        ledger.now.insert(*turn, *state);
                    }
                }
                Err(e) => log::warn!("the change states of {} could not be put back after Don't save: {e}", self.path().display()),
            }
        }
        if let Err(e) = self.chat_commit(c, "Conversations after Don't save") {
            log::warn!("the conversations of {} wait for the next chat commit: {e}", self.path().display());
        }
    }

    /// Writes change states, in one transaction. `strict` refuses a turn without a change link;
    /// otherwise such a turn (a deleted conversation's) is passed.
    fn write_states(&self, c: &mut mysql::Conn, moves: &[(Uuid, ChangeState)], now: &str, strict: bool) -> Result<(), Error> {
        time(now)?;
        if !self.chats_held(c)? {
            return if strict && let Some((turn, _)) = moves.first() {
                Err(Error::ChatMissing { what: "change link of turn", id: turn.to_string() })
            } else {
                Ok(())
            };
        }
        let mut tx = c.start_transaction(mysql::TxOpts::default()).map_err(sql("cannot start writing the conversations"))?;
        for (turn, state) in moves {
            let there: Option<mysql::Row> =
                tx.exec_first("SELECT 1 FROM `chat_change` WHERE `turn` = ?", (id_value(*turn),)).map_err(sql("cannot read a change link"))?;
            if there.is_none() {
                if strict {
                    return Err(Error::ChatMissing { what: "change link of turn", id: turn.to_string() });
                }
                continue;
            }
            tx.exec_drop("UPDATE `chat_change` SET `state` = ?, `updated_at` = ? WHERE `turn` = ?", (state.code(), now, id_value(*turn)))
                .map_err(sql("cannot write a change state"))?;
        }
        tx.commit().map_err(sql("cannot finish writing the conversations"))
    }

    /// Makes a conversation, with the chat tables when it is the project's first, and commits it.
    /// Refused when its title holds one of the API keys in `keys` (it is made from the first
    /// question), the project holds [`limits::CHATS_PER_PROJECT`] already or another conversation
    /// has the same origin.
    pub fn create_chat(&self, new: &NewChat, keys: &dyn SecretStore) -> Result<(), Error> {
        no_key(keys, &new.title)?;
        line("the conversation's origin", &new.origin, ORIGIN_CHARS)?;
        let title = auto_title(&new.title)?;
        line("the model", &new.model, MODEL_CHARS)?;
        if new.created_by.chars().count() > CREATED_BY_CHARS || new.created_by.chars().any(char::is_control) {
            return Err(value("who started the conversation", format!("it is not 0 to {CREATED_BY_CHARS} printable characters")));
        }
        time(&new.now)?;
        let mut c = self.model_connection()?;
        self.chat_shapes(&mut c)?;
        for def in &TABLES {
            c.query_drop(def.create()).map_err(sql("cannot create the conversations' tables"))?;
        }
        let mut tx = c.start_transaction(mysql::TxOpts::default()).map_err(sql("cannot start writing the conversations"))?;
        let held = first_int(tx.query_first("SELECT COUNT(*) FROM `chat`").map_err(sql("cannot count the conversations"))?)
            .map_err(|why| self.unreadable(why))?;
        if usize::try_from(held).unwrap_or(usize::MAX) >= CHATS_PER_PROJECT {
            return Err(Error::Full { what: "the project's list of conversations", max: CHATS_PER_PROJECT });
        }
        let taken: Option<mysql::Row> =
            tx.exec_first("SELECT 1 FROM `chat` WHERE `origin` = ?", (new.origin.as_str(),)).map_err(sql("cannot read the conversations"))?;
        if taken.is_some() {
            return Err(value("the conversation's origin", format!("another conversation is labelled {:?}", new.origin)));
        }
        tx.exec_drop(
            "INSERT INTO `chat` (`id`, `origin`, `title`, `provider`, `model`, `created_by`, `created_at`, `updated_at`) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            vec![
                id_value(new.id),
                text_value(&new.origin),
                text_value(&title),
                text_value(new.provider.code()),
                text_value(&new.model),
                text_value(&new.created_by),
                text_value(&new.now),
                text_value(&new.now),
            ],
        )
        .map_err(sql("cannot write the conversation"))?;
        tx.commit().map_err(sql("cannot finish writing the conversations"))?;
        self.chat_commit_after(&mut c, &format!("{}: started", new.origin));
        Ok(())
    }

    /// Starts a turn with its question and context chips, before the runtime gets the text, so a
    /// crash cannot lose a question that was sent; gives the turn's number. Refused, with nothing
    /// written, when the question is longer than [`limits::QUESTION_BYTES`], holds one of the API
    /// keys in `keys` (compared exactly), the conversation is full, or a turn of it still runs.
    /// Committed with the turn's end.
    pub fn start_turn(&self, turn: &NewTurn, question: &NewQuestion, keys: &dyn SecretStore) -> Result<u32, Error> {
        if question.body.len() > QUESTION_BYTES {
            return Err(Error::TooLarge { what: "a question", size: question.body.len(), max: QUESTION_BYTES, unit: "bytes" });
        }
        no_key(keys, &question.body)?;
        line("the model", &turn.model, MODEL_CHARS)?;
        time(&turn.now)?;
        let (chips, cut) = anchors(&question.anchors, true)?;
        let missing = || Error::ChatMissing { what: "conversation", id: turn.chat.to_string() };
        let mut c = self.chat_connection(missing)?;
        let mut tx = c.start_transaction(mysql::TxOpts::default()).map_err(sql("cannot start writing the conversations"))?;
        let there: Option<mysql::Row> =
            tx.exec_first("SELECT 1 FROM `chat` WHERE `id` = ?", (id_value(turn.chat),)).map_err(sql("cannot read the conversation"))?;
        if there.is_none() {
            return Err(missing());
        }
        let running: Option<mysql::Row> = tx
            .exec_first("SELECT 1 FROM `chat_turn` WHERE `chat` = ? AND `outcome` = 'running'", (id_value(turn.chat),))
            .map_err(sql("cannot read the conversation's turns"))?;
        if running.is_some() {
            return Err(value("a turn", "another turn of this conversation is still running"));
        }
        let position = self.next_position(&mut tx, turn.chat)?;
        let highest = first_int(
            tx.exec_first("SELECT COALESCE(MAX(`number`), 0) FROM `chat_turn` WHERE `chat` = ?", (id_value(turn.chat),))
                .map_err(sql("cannot read the conversation's turns"))?,
        )
        .map_err(|why| self.unreadable(why))?;
        let number = next("the turn's number", highest)?;
        tx.exec_drop(
            "INSERT INTO `chat_turn` (`id`, `chat`, `number`, `model`, `allow_changes`, `outcome`, `error`, `started_at`, `ended_at`) \
             VALUES (?, ?, ?, ?, ?, 'running', NULL, ?, NULL)",
            vec![
                id_value(turn.id),
                id_value(turn.chat),
                int_value("the turn's number", number)?,
                text_value(&turn.model),
                flag_value(turn.allow_changes),
                text_value(&turn.now),
            ],
        )
        .map_err(sql("cannot write the turn"))?;
        insert_message(&mut tx, question.id, turn.chat, turn.id, position, Role::User, Some(&question.body), true, false, cut, &turn.now)?;
        insert_anchors(&mut tx, question.id, &chips)?;
        touch(&mut tx, turn.chat, &turn.now)?;
        tx.commit().map_err(sql("cannot finish writing the conversations"))?;
        Ok(number)
    }

    /// Runs a failed turn again in place (owner decision, AR-205): the same row goes back to
    /// running, with the model and the switch of this attempt and a new start time; an edited
    /// question replaces the stored one and its context chips, so the question is stored once
    /// however often it is sent. What the failed attempt left in the timeline stays as it was.
    /// Refused, with nothing written, when the question is longer than [`limits::QUESTION_BYTES`]
    /// or holds one of the API keys in `keys`, when the turn did not fail, is not its
    /// conversation's last, or changed the model (running it again could make those changes a
    /// second time). Committed with the turn's end.
    pub fn rerun_turn(&self, rerun: &Rerun, keys: &dyn SecretStore) -> Result<(), Error> {
        let chips = match &rerun.question {
            Some((body, anchors_given)) => {
                if body.len() > QUESTION_BYTES {
                    return Err(Error::TooLarge { what: "a question", size: body.len(), max: QUESTION_BYTES, unit: "bytes" });
                }
                no_key(keys, body)?;
                Some(anchors(anchors_given, true)?)
            }
            None => None,
        };
        line("the model", &rerun.model, MODEL_CHARS)?;
        time(&rerun.now)?;
        let missing = || Error::ChatMissing { what: "turn", id: rerun.turn.to_string() };
        let mut c = self.chat_connection(missing)?;
        let mut tx = c.start_transaction(mysql::TxOpts::default()).map_err(sql("cannot start writing the conversations"))?;
        let row: Option<mysql::Row> = tx
            .exec_first("SELECT `chat`, `outcome`, `number` FROM `chat_turn` WHERE `id` = ?", (id_value(rerun.turn),))
            .map_err(sql("cannot read the turn"))?;
        let row = row.ok_or_else(missing)?;
        let chat = id_at(&row, 0).map_err(|why| self.unreadable(why))?;
        let outcome = text_at(&row, 1, 16).and_then(|t| TurnOutcome::from_code(&t)).map_err(|why| self.unreadable(why))?;
        let number = int_at(&row, 2).map_err(|why| self.unreadable(why))?;
        if outcome != TurnOutcome::Failed {
            return Err(value("the turn", format!("only a failed turn runs again, and it {}", outcome.code())));
        }
        let highest = first_int(
            tx.exec_first("SELECT COALESCE(MAX(`number`), 0) FROM `chat_turn` WHERE `chat` = ?", (id_value(chat),))
                .map_err(sql("cannot read the conversation's turns"))?,
        )
        .map_err(|why| self.unreadable(why))?;
        if highest != number {
            return Err(value("the turn", "only the conversation's last turn runs again"));
        }
        let changed: Option<mysql::Row> =
            tx.exec_first("SELECT 1 FROM `chat_change` WHERE `turn` = ?", (id_value(rerun.turn),)).map_err(sql("cannot read the turn's changes"))?;
        if changed.is_some() {
            return Err(value("the turn", "it changed the model, so running it again could make those changes twice"));
        }
        let question: Option<mysql::Row> = tx
            .exec_first("SELECT `id` FROM `chat_message` WHERE `turn` = ? AND `role` = 'user' ORDER BY `position` LIMIT 1", (id_value(rerun.turn),))
            .map_err(sql("cannot read the turn's question"))?;
        let question = id_at(&question.ok_or_else(|| self.unreadable("a turn has no question"))?, 0).map_err(|why| self.unreadable(why))?;
        tx.exec_drop(
            "UPDATE `chat_turn` SET `outcome` = 'running', `error` = NULL, `ended_at` = NULL, `started_at` = ?, `model` = ?, `allow_changes` = ? WHERE `id` = ?",
            vec![text_value(&rerun.now), text_value(&rerun.model), flag_value(rerun.allow_changes), id_value(rerun.turn)],
        )
        .map_err(sql("cannot write the turn"))?;
        if let (Some((body, _)), Some((chips, cut))) = (&rerun.question, &chips) {
            tx.exec_drop(
                "UPDATE `chat_message` SET `body` = ?, `anchors_cut` = ?, `created_at` = ? WHERE `id` = ?",
                vec![text_value(body), flag_value(*cut), text_value(&rerun.now), id_value(question)],
            )
            .map_err(sql("cannot write the question"))?;
            tx.exec_drop("DELETE FROM `chat_anchor` WHERE `message` = ?", (id_value(question),)).map_err(sql("cannot write the question's chips"))?;
            insert_anchors(&mut tx, question, chips)?;
        }
        touch(&mut tx, chat, &rerun.now)?;
        tx.commit().map_err(sql("cannot finish writing the conversations"))
    }

    /// The next position in a conversation's timeline; refused when it is full.
    fn next_position(&self, tx: &mut mysql::Transaction<'_>, chat: Uuid) -> Result<u32, Error> {
        let row: Option<mysql::Row> = tx
            .exec_first("SELECT COUNT(*), COALESCE(MAX(`position`), 0) FROM `chat_message` WHERE `chat` = ?", (id_value(chat),))
            .map_err(sql("cannot read the conversation's messages"))?;
        let row = row.ok_or_else(|| self.unreadable("the messages cannot be counted"))?;
        let held = int_at(&row, 0).map_err(|why| self.unreadable(why))?;
        let highest = int_at(&row, 1).map_err(|why| self.unreadable(why))?;
        if usize::try_from(held).unwrap_or(usize::MAX) >= MESSAGES_PER_CHAT {
            return Err(Error::Full { what: "the conversation", max: MESSAGES_PER_CHAT });
        }
        next("the message's position", highest)
    }

    /// The conversation and number of a running turn; refused when it is missing or has ended.
    fn running_turn(&self, tx: &mut mysql::Transaction<'_>, turn: Uuid) -> Result<(Uuid, String, i64), Error> {
        let row: Option<mysql::Row> = tx
            .exec_first(
                "SELECT t.`chat`, t.`outcome`, t.`number`, c.`origin` FROM `chat_turn` t JOIN `chat` c ON c.`id` = t.`chat` WHERE t.`id` = ?",
                (id_value(turn),),
            )
            .map_err(sql("cannot read the turn"))?;
        let missing = || Error::ChatMissing { what: "turn", id: turn.to_string() };
        let row = row.ok_or_else(missing)?;
        let chat = id_at(&row, 0).map_err(|why| self.unreadable(why))?;
        let outcome = text_at(&row, 1, 16).and_then(|t| TurnOutcome::from_code(&t)).map_err(|why| self.unreadable(why))?;
        let number = int_at(&row, 2).map_err(|why| self.unreadable(why))?;
        let origin = text_at(&row, 3, ORIGIN_CHARS).map_err(|why| self.unreadable(why))?;
        if outcome != TurnOutcome::Running {
            return Err(value("the turn", format!("it has ended ({})", outcome.code())));
        }
        Ok((chat, origin, number))
    }

    /// Appends an agent's message to a running turn, cut to [`limits::REPLY_BYTES`] at a character
    /// boundary and marked when longer; gives its position.
    pub fn append_reply(&self, reply: &NewReply) -> Result<u32, Error> {
        time(&reply.now)?;
        let (body, cut) = cut_bytes(&reply.body, REPLY_BYTES);
        let shortened = cut || reply.shortened;
        let mut c = self.chat_connection(|| Error::ChatMissing { what: "turn", id: reply.turn.to_string() })?;
        let mut tx = c.start_transaction(mysql::TxOpts::default()).map_err(sql("cannot start writing the conversations"))?;
        let (chat, _, _) = self.running_turn(&mut tx, reply.turn)?;
        let position = self.next_position(&mut tx, chat)?;
        insert_message(&mut tx, reply.id, chat, reply.turn, position, Role::Assistant, Some(body), reply.complete, shortened, false, &reply.now)?;
        touch(&mut tx, chat, &reply.now)?;
        tx.commit().map_err(sql("cannot finish writing the conversations"))?;
        Ok(position)
    }

    /// Appends a tool call that ended to a running turn: its timeline row, its details (the
    /// arguments, the start of the result with its full size, and Archforge's error text, each cut
    /// to its limit and marked), and the elements it read or changed; gives its position.
    pub fn append_tool_call(&self, call: &NewToolCall) -> Result<u32, Error> {
        time(&call.now)?;
        line("the tool's name", &call.tool, TOOL_CHARS)?;
        if let Some(command) = &call.command {
            line("the tool's command", command, COMMAND_CHARS)?;
        }
        let detail = call.detail.as_deref().map(|d| cut_chars(d, DETAIL_CHARS).0);
        let (arguments, cut_arguments) = call.arguments.as_deref().map_or((None, false), |a| {
            let (a, cut) = cut_bytes(a, ARGUMENTS_BYTES);
            (Some(a), cut)
        });
        let (result, cut_result) = call.result.as_deref().map_or((None, false), |r| {
            let (r, cut) = cut_bytes(r, RESULT_BYTES);
            (Some(r), cut)
        });
        let result_bytes = call.result.as_deref().map(|r| u32::try_from(r.len()).unwrap_or(INT_MAX).min(INT_MAX));
        let (error, cut_error) = call.error.as_deref().map_or((None, false), |e| {
            let (e, cut) = cut_bytes(e, ERROR_BYTES);
            (Some(e), cut)
        });
        let element_count = call.element_count.map(|n| int_value("the tool's element count", n)).transpose()?;
        let (targets, anchors_cut) = anchors(&call.anchors, false)?;
        let mut c = self.chat_connection(|| Error::ChatMissing { what: "turn", id: call.turn.to_string() })?;
        let mut tx = c.start_transaction(mysql::TxOpts::default()).map_err(sql("cannot start writing the conversations"))?;
        let (chat, _, _) = self.running_turn(&mut tx, call.turn)?;
        let position = self.next_position(&mut tx, chat)?;
        insert_message(&mut tx, call.id, chat, call.turn, position, Role::Tool, None, true, false, anchors_cut, &call.now)?;
        tx.exec_drop(
            "INSERT INTO `chat_tool_call` (`message`, `tool`, `command`, `detail`, `status`, `changed_model`, `element_count`, `arguments`, `result`, \
             `result_bytes`, `error`, `shortened`) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            vec![
                id_value(call.id),
                text_value(&call.tool),
                opt_text_value(call.command.as_deref()),
                opt_text_value(detail.as_deref()),
                text_value(call.status.code()),
                flag_value(call.changed_model),
                element_count.unwrap_or(mysql::Value::NULL),
                opt_text_value(arguments),
                opt_text_value(result),
                result_bytes.map_or(mysql::Value::NULL, |n| mysql::Value::Int(i64::from(n))),
                opt_text_value(error),
                flag_value(cut_arguments || cut_result || cut_error),
            ],
        )
        .map_err(sql("cannot write the tool call"))?;
        insert_anchors(&mut tx, call.id, &targets)?;
        touch(&mut tx, chat, &call.now)?;
        tx.commit().map_err(sql("cannot finish writing the conversations"))?;
        Ok(position)
    }

    /// Ends a running turn with its outcome and, when it changed the model, what it changed (the
    /// first [`limits::CHANGED_ELEMENTS_PER_TURN`] elements; the counts stay exact), then commits
    /// the conversation in a chat commit. The changes are applied, and this session remembers
    /// their state for Don't save.
    pub fn end_turn(&self, end: &TurnEnd) -> Result<(), Error> {
        time(&end.now)?;
        match (end.outcome, end.error) {
            (TurnOutcome::Running, _) => return Err(value("the turn's outcome", "a turn ends completed, stopped, failed or interrupted")),
            (TurnOutcome::Failed, None) => return Err(value("the turn's outcome", "a failed turn names why it failed")),
            (TurnOutcome::Completed | TurnOutcome::Stopped | TurnOutcome::Interrupted, Some(_)) => {
                return Err(value("the turn's error", "only a failed turn names one"));
            }
            _ => {}
        }
        let mut kept = Vec::new();
        if let Some(changes) = &end.changes {
            if changes.changes == 0 {
                return Err(value("the turn's changes", "a turn that changed nothing gives none"));
            }
            if usize::try_from(changes.elements).unwrap_or(usize::MAX) < changes.changed.len() {
                return Err(value("the turn's changes", "they list more elements than they count"));
            }
            let mut seen = BTreeSet::new();
            for e in &changes.changed {
                metaclass(&e.metaclass)?;
                if !seen.insert(e.element) {
                    return Err(value("the turn's changes", format!("they list the element {} twice", e.element)));
                }
                if kept.len() < CHANGED_ELEMENTS_PER_TURN {
                    kept.push(ChangedElement { name: e.name.as_deref().map(|n| cut_chars(n, NAME_CHARS).0), ..e.clone() });
                }
            }
            int_value("the turn's changes", changes.changes)?;
            int_value("the turn's changed elements", changes.elements)?;
        }
        let mut c = self.chat_connection(|| Error::ChatMissing { what: "turn", id: end.turn.to_string() })?;
        let mut tx = c.start_transaction(mysql::TxOpts::default()).map_err(sql("cannot start writing the conversations"))?;
        let (chat, origin, number) = self.running_turn(&mut tx, end.turn)?;
        tx.exec_drop(
            "UPDATE `chat_turn` SET `outcome` = ?, `error` = ?, `ended_at` = ? WHERE `id` = ?",
            vec![text_value(end.outcome.code()), opt_text_value(end.error.map(TurnError::code)), text_value(&end.now), id_value(end.turn)],
        )
        .map_err(sql("cannot write the turn's end"))?;
        if let Some(changes) = &end.changes {
            tx.exec_drop(
                "INSERT INTO `chat_change` (`turn`, `changes`, `elements`, `state`, `updated_at`) VALUES (?, ?, ?, 'applied', ?)",
                vec![
                    id_value(end.turn),
                    int_value("the turn's changes", changes.changes)?,
                    int_value("the turn's changed elements", changes.elements)?,
                    text_value(&end.now),
                ],
            )
            .map_err(sql("cannot write the turn's changes"))?;
            for chunk in kept.chunks(500) {
                let mut params = Vec::with_capacity(chunk.len() * 5);
                for e in chunk {
                    params.extend([
                        id_value(end.turn),
                        id_value(e.element),
                        text_value(e.kind.code()),
                        text_value(&e.metaclass),
                        opt_text_value(e.name.as_deref()),
                    ]);
                }
                tx.exec_drop(
                    format!(
                        "INSERT INTO `chat_change_element` (`turn`, `element`, `kind`, `metaclass`, `name`) VALUES {}",
                        vec!["(?, ?, ?, ?, ?)"; chunk.len()].join(", ")
                    ),
                    params,
                )
                .map_err(sql("cannot write the turn's changed elements"))?;
            }
        }
        touch(&mut tx, chat, &end.now)?;
        tx.commit().map_err(sql("cannot finish writing the conversations"))?;
        if end.changes.is_some() {
            self.changes().now.insert(end.turn, ChangeState::Applied);
        }
        self.chat_commit_after(&mut c, &format!("{origin}: turn {number}"));
        Ok(())
    }

    /// Records that undo or redo moved turns' changes (see [`TurnSpan::moved`]), stamped `now`;
    /// all of them or none, refused when a turn has no change link. Committed with the next chat
    /// commit; Don't save puts them back to what they were at the last save.
    pub fn set_change_states(&self, moves: &[(Uuid, ChangeState)], now: &str) -> Result<(), Error> {
        let mut c = self.model_connection()?;
        self.write_states(&mut c, moves, now, true)?;
        let mut ledger = self.changes();
        for (turn, state) in moves {
            ledger.now.insert(*turn, *state);
        }
        Ok(())
    }

    /// Renames a conversation: refused when the title is empty, longer than
    /// [`limits::TITLE_CHARS`], holds a control character or one of the API keys in `keys`.
    /// Committed at once.
    pub fn rename_chat(&self, chat: Uuid, title: &str, now: &str, keys: &dyn SecretStore) -> Result<(), Error> {
        no_key(keys, title)?;
        line("the conversation's title", title, TITLE_CHARS)?;
        time(now)?;
        let origin = self.update_chat(chat, "`title` = ?", text_value(title), now)?;
        let mut c = self.model_connection()?;
        self.chat_commit_after(&mut c, &format!("{origin}: renamed"));
        Ok(())
    }

    /// Sets the model the conversation's next turn asks for, within its provider, which never
    /// changes. Committed with the next chat commit.
    pub fn set_chat_model(&self, chat: Uuid, model: &str, now: &str) -> Result<(), Error> {
        line("the model", model, MODEL_CHARS)?;
        time(now)?;
        self.update_chat(chat, "`model` = ?", text_value(model), now).map(|_| ())
    }

    /// Keeps the context chips the person removed from a conversation (AR-227), all of them: each
    /// removal and each chip put back writes the whole set, none for every chip put back. Refused
    /// for an anchor role that is not a chip. Its last activity stays, as no message was written;
    /// committed with the next chat commit. A `chat` table made before the column takes it first,
    /// and keeps it should the write then fail (a nullable column, empty for the rows it holds).
    pub fn set_removed_chips(&self, chat: Uuid, removed: &BTreeSet<AnchorRole>) -> Result<(), Error> {
        if let Some(role) = removed.iter().find(|r| !r.is_chip()) {
            return Err(value("the chips removed", format!("{:?} is not a chip; the chips are project, diagram and selection", role.code())));
        }
        let missing = || Error::ChatMissing { what: "conversation", id: chat.to_string() };
        let mut c = self.model_connection()?;
        let held = self.chat_tables(&mut c)?.ok_or_else(missing)?;
        let there: Option<mysql::Row> =
            c.exec_first("SELECT 1 FROM `chat` WHERE `id` = ?", (id_value(chat),)).map_err(sql("cannot read the conversation"))?;
        if there.is_none() {
            return Err(missing());
        }
        self.widen(&mut c, &held)?;
        c.exec_drop("UPDATE `chat` SET `removed_chips` = ? WHERE `id` = ?", vec![chips_value(removed), id_value(chat)])
            .map_err(sql("cannot write the conversation"))
    }

    /// Sets one column of a conversation's row and its last activity; gives its origin.
    fn update_chat(&self, chat: Uuid, set: &str, to: mysql::Value, now: &str) -> Result<String, Error> {
        let missing = || Error::ChatMissing { what: "conversation", id: chat.to_string() };
        let mut c = self.chat_connection(missing)?;
        let mut tx = c.start_transaction(mysql::TxOpts::default()).map_err(sql("cannot start writing the conversations"))?;
        let origin = self.origin_of(&mut tx, chat)?;
        tx.exec_drop(format!("UPDATE `chat` SET {set}, `updated_at` = ? WHERE `id` = ?"), vec![to, text_value(now), id_value(chat)])
            .map_err(sql("cannot write the conversation"))?;
        tx.commit().map_err(sql("cannot finish writing the conversations"))?;
        Ok(origin)
    }

    fn origin_of(&self, tx: &mut mysql::Transaction<'_>, chat: Uuid) -> Result<String, Error> {
        let row: Option<mysql::Row> =
            tx.exec_first("SELECT `origin` FROM `chat` WHERE `id` = ?", (id_value(chat),)).map_err(sql("cannot read the conversation"))?;
        let row = row.ok_or_else(|| Error::ChatMissing { what: "conversation", id: chat.to_string() })?;
        text_at(&row, 0, ORIGIN_CHARS).map_err(|why| self.unreadable(why))
    }

    /// Deletes a conversation (AR-217): whatever of the conversations waits is committed first
    /// ("Conversations kept"), so the commit before the deletion holds this one whole; then its rows
    /// in all seven tables go, in one transaction that ends in exactly one chat commit
    /// (`Chat N: deleted`), the rows and the commit together or not at all. Gives its label and that
    /// commit before, which [`Project::restore_chat`] brings it back from. It stays in the project's
    /// earlier commits (owner decision); no model element changes, and the model's changes not
    /// saved stay unsaved.
    pub fn delete_chat(&self, chat: Uuid) -> Result<Deleted, Error> {
        let missing = || Error::ChatMissing { what: "conversation", id: chat.to_string() };
        let not_deleted = |why: String| Error::ChatNotDeleted { id: chat.to_string(), why };
        let mut c = self.chat_connection(missing)?;
        // Missing is said before anything is committed.
        let there: Option<mysql::Row> =
            c.exec_first("SELECT 1 FROM `chat` WHERE `id` = ?", (id_value(chat),)).map_err(sql("cannot read the conversation"))?;
        if there.is_none() {
            return Err(missing());
        }
        self.chat_commit(&mut c, "Conversations kept").map_err(|e| not_deleted(e.to_string()))?;
        let left: Vec<String> =
            changed_tables(&mut c, self.path()).map_err(|e| not_deleted(e.to_string()))?.into_iter().filter(|n| is_chat_table(n)).collect();
        if !left.is_empty() {
            return Err(not_deleted(format!("{} still holds changes not committed", left.join(", "))));
        }
        let before = head(&mut c).map_err(not_deleted)?;
        let mut tx = c.start_transaction(mysql::TxOpts::default()).map_err(sql("cannot start writing the conversations"))?;
        let origin = self.origin_of(&mut tx, chat)?;
        for statement in [
            "DELETE FROM `chat_change_element` WHERE `turn` IN (SELECT `id` FROM `chat_turn` WHERE `chat` = ?)",
            "DELETE FROM `chat_change` WHERE `turn` IN (SELECT `id` FROM `chat_turn` WHERE `chat` = ?)",
            "DELETE FROM `chat_anchor` WHERE `message` IN (SELECT `id` FROM `chat_message` WHERE `chat` = ?)",
            "DELETE FROM `chat_tool_call` WHERE `message` IN (SELECT `id` FROM `chat_message` WHERE `chat` = ?)",
            "DELETE FROM `chat_message` WHERE `chat` = ?",
            "DELETE FROM `chat_turn` WHERE `chat` = ?",
            "DELETE FROM `chat` WHERE `id` = ?",
        ] {
            tx.exec_drop(statement, (id_value(chat),)).map_err(sql("cannot delete the conversation"))?;
        }
        // A failed commit drops the transaction, which rolls the deletion back.
        if !self.chat_commit(&mut tx, &format!("{origin}: deleted")).map_err(|e| not_deleted(e.to_string()))? {
            return Err(not_deleted("the deletion changed no table".into()));
        }
        // Dolt's commit has ended the transaction with the rows in it: it is done, and what the
        // closing statement says only goes to the log.
        if let Err(e) = tx.commit() {
            log::warn!("the deletion of {origin} in {} was committed; closing its transaction said: {e}", self.path().display());
        }
        // Its turns stay in the session's ledger: Don't save passes a turn without a change link,
        // and puts a restored one back as any other.
        Ok(Deleted { origin, before })
    }

    /// Brings a deleted conversation back (AR-217, owner decision): every one of its rows in the
    /// seven tables as the commit `from` held them, written in one transaction that ends in one chat
    /// commit (`Chat N: restored`), all or nothing; a turn it held still running (a question that
    /// reached the project as it was deleted) comes back interrupted. `from` must be a commit the
    /// project's branch descends from that holds the conversation, as [`Project::delete_chat`]
    /// gave it. What of the conversations waits is committed first ("Conversations kept"), so the
    /// restoring commit holds this conversation alone. Refused, with nothing written, when the
    /// conversation is listed already ([`Error::ChatListed`]), `from` is not such a commit, another
    /// conversation has taken its label since, or the project holds as many conversations as it
    /// may; nothing of it is written then (the commit of what waited may have been made). A
    /// column added to a table after `from` comes back empty, and a table made before one takes it
    /// first, kept in that commit even when the restore is then refused (a nullable column, empty
    /// for the rows it holds). Gives its label.
    pub fn restore_chat(&self, chat: Uuid, from: &str) -> Result<String, Error> {
        let refuse = |why: String| Error::ChatNotRestored { id: chat.to_string(), why };
        if !is_commit(from) {
            return Err(refuse(format!("{:?} is not a commit of the project's saved history", from.chars().take(40).collect::<String>())));
        }
        let mut c = self.model_connection()?;
        let held = self.chat_tables(&mut c)?.ok_or_else(|| refuse("the project holds no conversations".into()))?;
        // One of the branch's own commits: the merge base of it and the branch is itself. A hash
        // Dolt does not know is an error here, so a refusal.
        let base: Option<mysql::Row> = c.exec_first("SELECT LEFT(CAST(DOLT_MERGE_BASE(?, 'HEAD') AS CHAR), 64)", (from,)).map_err(|e| {
            log::warn!("the commit {from} of {} could not be looked up: {e}", self.path().display());
            refuse(format!("{from} is not a commit of the project's saved history"))
        })?;
        let base = base.and_then(|r| r.get_opt::<String, usize>(0)).and_then(Result::ok);
        if base.as_deref() != Some(from) {
            return Err(refuse(format!("{from} is not a commit of the project's saved history")));
        }
        let listed: Option<mysql::Row> =
            c.exec_first("SELECT 1 FROM `chat` WHERE `id` = ?", (id_value(chat),)).map_err(sql("cannot read the conversations"))?;
        if listed.is_some() {
            return Err(Error::ChatListed { id: chat.to_string() });
        }
        // Every column is written back, so a table made before its later columns takes them first.
        self.widen(&mut c, &held)?;
        self.chat_commit(&mut c, "Conversations kept").map_err(|e| refuse(e.to_string()))?;
        let mut tx = c.start_transaction(mysql::TxOpts::default()).map_err(sql("cannot start writing the conversations"))?;
        let listed: Option<mysql::Row> =
            tx.exec_first("SELECT 1 FROM `chat` WHERE `id` = ?", (id_value(chat),)).map_err(sql("cannot read the conversations"))?;
        if listed.is_some() {
            return Err(Error::ChatListed { id: chat.to_string() });
        }
        // `from` is checked to be 32 letters and digits of a commit hash, so it is safe in the text.
        let then: Option<mysql::Row> = tx
            .exec_first(format!("SELECT `origin` FROM `chat` AS OF '{from}' WHERE `id` = ?"), (id_value(chat),))
            .map_err(|e| refuse(format!("the commit {from} cannot be read: {e}")))?;
        let Some(then) = then else {
            return Err(refuse(format!("the commit {from} does not hold it")));
        };
        let origin = text_at(&then, 0, ORIGIN_CHARS).map_err(|why| self.unreadable(why))?;
        let taken: Option<mysql::Row> =
            tx.exec_first("SELECT 1 FROM `chat` WHERE `origin` = ?", (origin.as_str(),)).map_err(sql("cannot read the conversations"))?;
        if taken.is_some() {
            return Err(refuse(format!("another conversation is labelled {origin:?} now")));
        }
        let held = first_int(tx.query_first("SELECT COUNT(*) FROM `chat`").map_err(sql("cannot count the conversations"))?)
            .map_err(|why| self.unreadable(why))?;
        if usize::try_from(held).unwrap_or(usize::MAX) >= CHATS_PER_PROJECT {
            return Err(Error::Full { what: "the project's list of conversations", max: CHATS_PER_PROJECT });
        }
        for def in &TABLES {
            let names: Vec<String> = def.columns.iter().map(|(name, _)| format!("`{name}`")).collect();
            let columns = names.join(", ");
            // A column added after the commit was made is not in it: its rows come back without it.
            // The commit's own columns are read from its table, so any other failure refuses.
            let had = if def.first < def.columns.len() { Some(columns_at(&mut tx, def.name, from).map_err(refuse)?) } else { None };
            let mut taken = Vec::new();
            for (n, ((plain, _), name)) in def.columns.iter().zip(names).enumerate() {
                let there = n < def.first || had.as_ref().is_some_and(|h| h.contains(*plain));
                taken.push(if there { name } else { "NULL".to_string() });
            }
            let taken = taken.join(", ");
            let rows = match def.name {
                "chat" => "`id` = ?".to_string(),
                "chat_turn" | "chat_message" => "`chat` = ?".to_string(),
                "chat_tool_call" | "chat_anchor" => format!("`message` IN (SELECT `id` FROM `chat_message` AS OF '{from}' WHERE `chat` = ?)"),
                "chat_change" | "chat_change_element" => format!("`turn` IN (SELECT `id` FROM `chat_turn` AS OF '{from}' WHERE `chat` = ?)"),
                other => return Err(refuse(format!("Archforge does not know how the table {other} holds a conversation"))),
            };
            tx.exec_drop(format!("INSERT INTO `{0}` ({columns}) SELECT {taken} FROM `{0}` AS OF '{from}' WHERE {rows}", def.name), (id_value(chat),))
                .map_err(|e| refuse(format!("its rows of {} cannot be written back: {e}", def.name)))?;
        }
        tx.exec_drop(
            "UPDATE `chat_turn` SET `outcome` = 'interrupted', `ended_at` = `started_at` WHERE `chat` = ? AND `outcome` = 'running'",
            (id_value(chat),),
        )
        .map_err(sql("cannot mark the turns left running"))?;
        // A failed commit drops the transaction, which rolls every row back.
        if !self.chat_commit(&mut tx, &format!("{origin}: restored")).map_err(|e| refuse(e.to_string()))? {
            return Err(refuse("no row was written back".into()));
        }
        // Dolt's commit has ended the transaction with the rows in it: it is done.
        if let Err(e) = tx.commit() {
            log::warn!("{origin} was brought back in {}; closing its transaction said: {e}", self.path().display());
        }
        Ok(origin)
    }

    /// Commits whatever of the conversations waits, before closing or when asked: false when
    /// nothing waited.
    pub fn commit_chats(&self) -> Result<bool, Error> {
        let mut c = self.model_connection()?;
        self.chat_commit(&mut c, "Conversations kept")
    }

    /// On opening: a turn left running (Archforge quit or crashed during it) becomes interrupted at
    /// `now`, and whatever of the conversations was not committed is, in a chat commit. Refused
    /// when the conversations cannot be read; the model is not affected.
    pub fn open_chats(&self, now: &str) -> Result<(), Error> {
        time(now)?;
        let mut c = self.model_connection()?;
        if !self.chats_held(&mut c)? {
            return Ok(());
        }
        c.exec_drop("UPDATE `chat_turn` SET `outcome` = 'interrupted', `ended_at` = ? WHERE `outcome` = 'running'", (now,))
            .map_err(sql("cannot mark the turns left running"))?;
        self.chat_commit(&mut c, "Conversations kept on opening").map(|_| ())
    }

    /// The conversations, newest activity first, each with its provider and its count of questions
    /// and replies; a row that does not read is listed with its reason. Refused when the chat
    /// tables cannot be read.
    pub fn list_chats(&self) -> Result<Vec<Listed>, Error> {
        let mut c = self.model_connection()?;
        let Some(held) = self.chat_tables(&mut c)? else {
            return Ok(Vec::new());
        };
        let columns = chat_columns(&held);
        let mut budget = READ_BYTES;
        let mut counts = BTreeMap::new();
        let rows = c
            .query_iter(format!(
                "SELECT m.`chat`, COUNT(*) FROM `chat_message` m JOIN `chat` c ON c.`id` = m.`chat` WHERE m.`role` IN ('user', 'assistant') GROUP BY m.`chat` LIMIT {}",
                CHATS_PER_PROJECT + 1
            ))
            .map_err(|e| self.unreadable(format!("the messages cannot be counted: {e}")))?;
        for row in rows {
            let row = row.map_err(|e| self.unreadable(format!("the messages cannot be counted: {e}")))?;
            spend(&mut budget, &row).map_err(|why| self.unreadable(why))?;
            let id = id_at(&row, 0).map_err(|why| self.unreadable(why))?;
            counts.insert(id, count_at(&row, 1));
        }
        let mut out = Vec::new();
        let mut seen = BTreeSet::new();
        let rows = c
            .query_iter(format!("SELECT {columns} FROM `chat` ORDER BY `updated_at` DESC, `id` DESC LIMIT {}", CHATS_PER_PROJECT + 1))
            .map_err(|e| self.unreadable(format!("the conversations cannot be listed: {e}")))?;
        for row in rows {
            let row = row.map_err(|e| self.unreadable(format!("the conversations cannot be listed: {e}")))?;
            if out.len() == CHATS_PER_PROJECT {
                return Err(self.unreadable(format!("it holds more than {CHATS_PER_PROJECT} conversations")));
            }
            spend(&mut budget, &row).map_err(|why| self.unreadable(why))?;
            let id = id_at(&row, 0).map_err(|why| self.unreadable(why))?;
            if !seen.insert(id) {
                out.push(Listed { id, entry: Err("two conversations have this id".into()) });
                continue;
            }
            let entry = chat_from(&row).and_then(|chat| Ok(ChatSummary { messages: counts.get(&chat.id).cloned().unwrap_or(Ok(0))?, chat }));
            out.push(Listed { id, entry });
        }
        Ok(out)
    }

    /// One conversation, whole: its row, turns and timeline in order, each message with its tool
    /// call and anchors, each turn with what it changed; read within [`limits::READ_BYTES`]. A row
    /// that does not read, an orphan or a broken order makes the conversation unreadable, with the
    /// reason; nothing is skipped.
    pub fn load_chat(&self, id: Uuid) -> Result<Conversation, Error> {
        let mut c = self.model_connection()?;
        let held = self.chat_tables(&mut c)?.ok_or_else(|| Error::ChatMissing { what: "conversation", id: id.to_string() })?;
        let bad = |why: String| Error::ChatUnreadable { id: id.to_string(), why };
        let mut budget = READ_BYTES;
        let rows: Vec<mysql::Row> = c
            .exec(format!("SELECT {} FROM `chat` WHERE `id` = ? LIMIT 2", chat_columns(&held)), (id_value(id),))
            .map_err(sql("cannot read the conversation"))?;
        let chat = match rows.as_slice() {
            [] => return Err(Error::ChatMissing { what: "conversation", id: id.to_string() }),
            [row] => chat_from(row).map_err(bad)?,
            _ => return Err(bad("two conversations have its id".into())),
        };
        let most = MESSAGES_PER_CHAT;
        let read = |c: &mut mysql::Conn, query: String, limit: usize, budget: &mut usize| -> Result<Vec<mysql::Row>, String> {
            let mut out = Vec::new();
            let rows = c.exec_iter(format!("{query} LIMIT {}", limit.saturating_add(1)), (id_value(id),)).map_err(|e| e.to_string())?;
            for row in rows {
                let row = row.map_err(|e| e.to_string())?;
                if out.len() == limit {
                    return Err(format!("it holds more than {limit} rows of one kind"));
                }
                spend(budget, &row)?;
                out.push(row);
            }
            Ok(out)
        };
        let mut turns = Vec::new();
        for row in
            read(&mut c, format!("SELECT {TURN_COLUMNS} FROM `chat_turn` WHERE `chat` = ? ORDER BY `number`"), most, &mut budget).map_err(bad)?
        {
            let turn = turn_from(&row).map_err(bad)?;
            if turns.last().is_some_and(|t: &TurnRecord| t.turn.number >= turn.number) {
                return Err(bad(format!("two turns are numbered {}", turn.number)));
            }
            turns.push(TurnRecord { turn, change: None });
        }
        let turn_at: BTreeMap<Uuid, usize> = turns.iter().enumerate().map(|(i, t)| (t.turn.id, i)).collect();
        let mut messages: Vec<MessageRecord> = Vec::new();
        for row in read(&mut c, format!("SELECT {MESSAGE_COLUMNS} FROM `chat_message` WHERE `chat` = ? ORDER BY `position`"), most, &mut budget)
            .map_err(bad)?
        {
            let message = message_from(&row).map_err(bad)?;
            if !turn_at.contains_key(&message.turn) {
                return Err(bad(format!("the message {} belongs to no turn of the conversation", message.id)));
            }
            if messages.last().is_some_and(|m| m.message.position >= message.position) {
                return Err(bad(format!("two messages are at position {}", message.position)));
            }
            messages.push(MessageRecord { message, tool: None, anchors: Vec::new() });
        }
        let message_at: BTreeMap<Uuid, usize> = messages.iter().enumerate().map(|(i, m)| (m.message.id, i)).collect();
        let tools = read(
            &mut c,
            format!("SELECT {TOOL_COLUMNS} FROM `chat_tool_call` t JOIN `chat_message` m ON m.`id` = t.`message` WHERE m.`chat` = ?"),
            most,
            &mut budget,
        )
        .map_err(bad)?;
        for row in tools {
            let tool = tool_from(&row).map_err(bad)?;
            let at = message_at.get(&tool.message).copied().ok_or_else(|| bad(format!("the tool call {} has no message", tool.message)))?;
            let record = messages.get_mut(at).ok_or_else(|| bad("a tool call has no message".into()))?;
            if record.message.role != Role::Tool || record.tool.is_some() {
                return Err(bad(format!("the message {} is not one tool row", tool.message)));
            }
            record.tool = Some(tool);
        }
        if let Some(m) = messages.iter().find(|m| (m.message.role == Role::Tool) != m.tool.is_some()) {
            return Err(bad(format!("the tool row {} has no details", m.message.id)));
        }
        let anchors = read(
            &mut c,
            format!(
                "SELECT {ANCHOR_COLUMNS} FROM `chat_anchor` a JOIN `chat_message` m ON m.`id` = a.`message` WHERE m.`chat` = ? ORDER BY a.`message`, a.`role`, a.`element`"
            ),
            most.saturating_mul(ANCHORS_PER_MESSAGE),
            &mut budget,
        )
        .map_err(bad)?;
        for row in anchors {
            let anchor = anchor_from(&row).map_err(bad)?;
            let at = message_at.get(&anchor.message).copied().ok_or_else(|| bad(format!("an anchor of {} has no message", anchor.message)))?;
            let record = messages.get_mut(at).ok_or_else(|| bad("an anchor has no message".into()))?;
            // Read in (message, role, element) order: one named twice comes next to itself.
            if record.anchors.last().is_some_and(|a| a.element == anchor.element && a.role == anchor.role) {
                return Err(bad(format!("the message {} names the element {} twice", anchor.message, anchor.element)));
            }
            if record.anchors.len() == ANCHORS_PER_MESSAGE {
                return Err(bad(format!("the message {} has more than {ANCHORS_PER_MESSAGE} anchors", anchor.message)));
            }
            record.anchors.push(anchor);
        }
        let changes = read(
            &mut c,
            format!("SELECT {CHANGE_COLUMNS} FROM `chat_change` x JOIN `chat_turn` t ON t.`id` = x.`turn` WHERE t.`chat` = ?"),
            most,
            &mut budget,
        )
        .map_err(bad)?;
        for row in changes {
            let change = change_from(&row).map_err(bad)?;
            let at = turn_at.get(&change.turn).copied().ok_or_else(|| bad(format!("the change link of {} has no turn", change.turn)))?;
            let t = turns.get_mut(at).ok_or_else(|| bad("a change link has no turn".into()))?;
            if t.change.is_some() {
                return Err(bad(format!("the turn {} has two change links", change.turn)));
            }
            t.change = Some(change);
        }
        let elements = read(
            &mut c,
            format!(
                "SELECT x.`turn`, {CHANGED_COLUMNS} FROM `chat_change_element` x JOIN `chat_turn` t ON t.`id` = x.`turn` WHERE t.`chat` = ? ORDER BY x.`turn`, x.`element`"
            ),
            most.saturating_mul(CHANGED_ELEMENTS_PER_TURN),
            &mut budget,
        )
        .map_err(bad)?;
        for row in elements {
            let turn = id_at(&row, 0).map_err(bad)?;
            let element = changed_from(&row, 1).map_err(bad)?;
            let change = turn_at
                .get(&turn)
                .and_then(|at| turns.get_mut(*at))
                .and_then(|t| t.change.as_mut())
                .ok_or_else(|| bad(format!("the changed element {} has no change link", element.element)))?;
            // Read in element order: one listed twice comes next to itself.
            if change.changed.last().is_some_and(|e| e.element >= element.element) {
                return Err(bad(format!("the turn {turn} lists the element {} twice", element.element)));
            }
            if change.changed.len() == CHANGED_ELEMENTS_PER_TURN {
                return Err(bad(format!("the turn {turn} lists more than {CHANGED_ELEMENTS_PER_TURN} changed elements")));
            }
            change.changed.push(element);
        }
        Ok(Conversation { chat, turns, messages })
    }
}

/// Writes one timeline row.
#[allow(clippy::too_many_arguments)]
fn insert_message(
    tx: &mut mysql::Transaction<'_>,
    id: Uuid,
    chat: Uuid,
    turn: Uuid,
    position: u32,
    role: Role,
    body: Option<&str>,
    complete: bool,
    shortened: bool,
    anchors_cut: bool,
    now: &str,
) -> Result<(), Error> {
    tx.exec_drop(
        "INSERT INTO `chat_message` (`id`, `chat`, `turn`, `position`, `role`, `body`, `complete`, `shortened`, `anchors_cut`, `created_at`) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        vec![
            id_value(id),
            id_value(chat),
            id_value(turn),
            int_value("the message's position", position)?,
            text_value(role.code()),
            opt_text_value(body),
            flag_value(complete),
            flag_value(shortened),
            flag_value(anchors_cut),
            text_value(now),
        ],
    )
    .map_err(sql("cannot write the message"))
}

/// Writes a message's anchors, already checked and capped.
fn insert_anchors(tx: &mut mysql::Transaction<'_>, message: Uuid, anchors: &[NewAnchor]) -> Result<(), Error> {
    for chunk in anchors.chunks(100) {
        let mut params = Vec::with_capacity(chunk.len() * 5);
        for a in chunk {
            params.extend([
                id_value(message),
                id_value(a.element),
                text_value(a.role.code()),
                text_value(&a.metaclass),
                opt_text_value(a.name.as_deref()),
            ]);
        }
        tx.exec_drop(
            format!(
                "INSERT INTO `chat_anchor` (`message`, `element`, `role`, `metaclass`, `name`) VALUES {}",
                vec!["(?, ?, ?, ?, ?)"; chunk.len()].join(", ")
            ),
            params,
        )
        .map_err(sql("cannot write the message's anchors"))?;
    }
    Ok(())
}

/// Moves a conversation's last activity to `now`.
fn touch(tx: &mut mysql::Transaction<'_>, chat: Uuid, now: &str) -> Result<(), Error> {
    tx.exec_drop("UPDATE `chat` SET `updated_at` = ? WHERE `id` = ?", (now, id_value(chat))).map_err(sql("cannot write the conversation"))
}

// The columns read back, in the order the readers below take them. The long texts are read only
// as far as their limits and one character more, so an oversized cell is refused, not loaded.
const CHAT_COLUMNS: &str = "`id`, `origin`, `title`, `provider`, `model`, `created_by`, `created_at`, `updated_at`";
const TURN_COLUMNS: &str = "`id`, `chat`, `number`, `model`, `allow_changes`, `outcome`, `error`, `started_at`, `ended_at`";
const MESSAGE_COLUMNS: &str = "`id`, `chat`, `turn`, `position`, `role`, LEFT(`body`, 262145), `complete`, `shortened`, `anchors_cut`, `created_at`";
const TOOL_COLUMNS: &str = "t.`message`, t.`tool`, t.`command`, t.`detail`, t.`status`, t.`changed_model`, t.`element_count`, \
     LEFT(t.`arguments`, 8193), LEFT(t.`result`, 2049), t.`result_bytes`, t.`error`, t.`shortened`";
const ANCHOR_COLUMNS: &str = "a.`message`, a.`element`, a.`role`, a.`metaclass`, a.`name`";
const CHANGE_COLUMNS: &str = "x.`turn`, x.`changes`, x.`elements`, x.`state`, x.`updated_at`";
const CHANGED_COLUMNS: &str = "x.`element`, x.`kind`, x.`metaclass`, x.`name`";

/// The columns of the table `table` as the commit `from` held it, by the names a select of none of
/// its rows gives. `from` must be checked to be a commit hash already.
fn columns_at(tx: &mut mysql::Transaction<'_>, table: &str, from: &str) -> Result<BTreeSet<String>, String> {
    let result = tx
        .query_iter(format!("SELECT * FROM `{table}` AS OF '{from}' LIMIT 0"))
        .map_err(|e| format!("the table {table} of the commit {from} cannot be read: {e}"))?;
    let names = result.columns().as_ref().iter().take(MAX_PAST_COLUMNS).map(|c| c.name_str().into_owned()).collect();
    drop(result);
    Ok(names)
}

/// The columns of a past commit's chat table looked at: more than any of the tables has.
const MAX_PAST_COLUMNS: usize = 64;

/// The `chat` columns read back: the chips removed as empty from a table made before that column.
fn chat_columns(held: &Held) -> String {
    format!("{CHAT_COLUMNS}, {}", if held.chips_column() { "`removed_chips`" } else { "NULL" })
}

/// The chips removed of a conversation as stored: none for an empty or missing cell, else a
/// comma-separated list of chip codes, each once. Anything else is refused, naming the value.
fn chips_at(row: &mysql::Row, i: usize) -> Result<BTreeSet<AnchorRole>, String> {
    let mut out = BTreeSet::new();
    let Some(text) = opt_text_at(row, i, REMOVED_CHIPS_CHARS).map_err(|why| format!("its chips removed: {why}"))? else { return Ok(out) };
    if text.chars().count() > REMOVED_CHIPS_CHARS {
        return Err(format!("its chips removed are longer than {REMOVED_CHIPS_CHARS} characters"));
    }
    if text.is_empty() {
        return Ok(out);
    }
    for code in text.split(',') {
        let role = AnchorRole::from_code(code).map_err(|why| format!("its chips removed: {why}"))?;
        if !role.is_chip() {
            return Err(format!("its chips removed name {code:?}, which is not a chip (project, diagram, selection)"));
        }
        if !out.insert(role) {
            return Err(format!("its chips removed name {code:?} twice"));
        }
    }
    Ok(out)
}

/// The chips removed as stored: null for none, else their codes in order, comma-separated.
fn chips_value(removed: &BTreeSet<AnchorRole>) -> mysql::Value {
    if removed.is_empty() {
        return mysql::Value::NULL;
    }
    text_value(&removed.iter().map(|r| r.code()).collect::<Vec<_>>().join(","))
}

/// Takes a row's size from what a reading may still take.
fn spend(budget: &mut usize, row: &mysql::Row) -> Result<(), String> {
    let mut cost = std::mem::size_of::<mysql::Row>();
    for i in 0..row.len() {
        cost = cost.saturating_add(std::mem::size_of::<mysql::Value>() + 16).saturating_add(match row.as_ref(i) {
            Some(mysql::Value::Bytes(b)) => b.len(),
            _ => 0,
        });
    }
    *budget = budget.checked_sub(cost).ok_or_else(|| format!("they are larger than Archforge reads ({} MB)", READ_BYTES >> 20))?;
    Ok(())
}

fn cell(row: &mysql::Row, i: usize) -> &mysql::Value {
    row.as_ref(i).unwrap_or(&mysql::Value::NULL)
}

fn id_at(row: &mysql::Row, i: usize) -> Result<Uuid, String> {
    match cell(row, i) {
        mysql::Value::Bytes(b) => <[u8; 16]>::try_from(b.as_slice()).map(Uuid).map_err(|_| "an id is not 16 bytes".to_string()),
        _ => Err("an id is missing".into()),
    }
}

fn opt_text_at(row: &mysql::Row, i: usize, max_bytes: usize) -> Result<Option<String>, String> {
    match cell(row, i) {
        mysql::Value::NULL => Ok(None),
        mysql::Value::Bytes(b) if b.len() <= max_bytes.saturating_mul(4) => {
            let s = std::str::from_utf8(b).map_err(|_| "a text is not UTF-8".to_string())?;
            Ok(Some(s.to_string()))
        }
        mysql::Value::Bytes(_) => Err(format!("a text is longer than {max_bytes}")),
        _ => Err("a text is not text".into()),
    }
}

fn text_at(row: &mysql::Row, i: usize, max_chars: usize) -> Result<String, String> {
    let text = opt_text_at(row, i, max_chars)?.ok_or("a text that may not be empty is")?;
    if text.chars().count() > max_chars {
        return Err(format!("a text is longer than {max_chars} characters"));
    }
    Ok(text)
}

/// A long text read back, within `max` bytes.
fn body_at(row: &mysql::Row, i: usize, max: usize) -> Result<Option<String>, String> {
    let text = opt_text_at(row, i, max)?;
    if text.as_ref().is_some_and(|t| t.len() > max) {
        return Err(format!("a text is longer than the {max} bytes stored"));
    }
    Ok(text)
}

fn int_at(row: &mysql::Row, i: usize) -> Result<i64, String> {
    match cell(row, i) {
        mysql::Value::Int(n) => Ok(*n),
        mysql::Value::UInt(n) => i64::try_from(*n).map_err(|_| "a number is out of range".to_string()),
        mysql::Value::Bytes(b) => std::str::from_utf8(b).ok().and_then(|s| s.parse().ok()).ok_or_else(|| "a number is not a number".to_string()),
        _ => Err("a number is missing".into()),
    }
}

/// A count or a position: 0 to what an `INT` holds.
fn count_at(row: &mysql::Row, i: usize) -> Result<u32, String> {
    let n = int_at(row, i)?;
    u32::try_from(n).ok().filter(|n| *n <= INT_MAX).ok_or_else(|| format!("{n} is not a count"))
}

fn opt_count_at(row: &mysql::Row, i: usize) -> Result<Option<u32>, String> {
    if *cell(row, i) == mysql::Value::NULL { Ok(None) } else { count_at(row, i).map(Some) }
}

fn flag_at(row: &mysql::Row, i: usize) -> Result<bool, String> {
    match int_at(row, i)? {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err("a flag is neither 0 nor 1".into()),
    }
}

fn code_at<T>(row: &mysql::Row, i: usize, what: &str, parse: fn(&str) -> Result<T, String>) -> Result<T, String> {
    parse(&text_at(row, i, 32)?).map_err(|why| format!("its {what} {why}"))
}

fn chat_from(row: &mysql::Row) -> Result<Chat, String> {
    Ok(Chat {
        id: id_at(row, 0)?,
        origin: text_at(row, 1, ORIGIN_CHARS)?,
        title: text_at(row, 2, TITLE_CHARS)?,
        provider: code_at(row, 3, "provider", Provider::from_code)?,
        model: text_at(row, 4, MODEL_CHARS)?,
        created_by: text_at(row, 5, CREATED_BY_CHARS)?,
        created_at: text_at(row, 6, 20)?,
        updated_at: text_at(row, 7, 20)?,
        removed_chips: chips_at(row, 8)?,
    })
}

fn turn_from(row: &mysql::Row) -> Result<Turn, String> {
    let error = match opt_text_at(row, 6, 32)? {
        None => None,
        Some(code) => Some(TurnError::from_code(&code).map_err(|why| format!("its error {why}"))?),
    };
    Ok(Turn {
        id: id_at(row, 0)?,
        chat: id_at(row, 1)?,
        number: count_at(row, 2)?,
        model: text_at(row, 3, MODEL_CHARS)?,
        allow_changes: flag_at(row, 4)?,
        outcome: code_at(row, 5, "outcome", TurnOutcome::from_code)?,
        error,
        started_at: text_at(row, 7, 20)?,
        ended_at: opt_text_at(row, 8, 20)?,
    })
}

fn message_from(row: &mysql::Row) -> Result<Message, String> {
    Ok(Message {
        id: id_at(row, 0)?,
        chat: id_at(row, 1)?,
        turn: id_at(row, 2)?,
        position: count_at(row, 3)?,
        role: code_at(row, 4, "role", Role::from_code)?,
        body: body_at(row, 5, REPLY_BYTES)?,
        complete: flag_at(row, 6)?,
        shortened: flag_at(row, 7)?,
        anchors_cut: flag_at(row, 8)?,
        created_at: text_at(row, 9, 20)?,
    })
}

fn tool_from(row: &mysql::Row) -> Result<ToolCall, String> {
    Ok(ToolCall {
        message: id_at(row, 0)?,
        tool: text_at(row, 1, TOOL_CHARS)?,
        command: opt_text_at(row, 2, COMMAND_CHARS)?,
        detail: opt_text_at(row, 3, DETAIL_CHARS)?,
        status: code_at(row, 4, "status", ToolStatus::from_code)?,
        changed_model: flag_at(row, 5)?,
        element_count: opt_count_at(row, 6)?,
        arguments: body_at(row, 7, ARGUMENTS_BYTES)?,
        result: body_at(row, 8, RESULT_BYTES)?,
        result_bytes: opt_count_at(row, 9)?,
        error: opt_text_at(row, 10, ERROR_BYTES)?,
        shortened: flag_at(row, 11)?,
    })
}

fn anchor_from(row: &mysql::Row) -> Result<Anchor, String> {
    Ok(Anchor {
        message: id_at(row, 0)?,
        element: id_at(row, 1)?,
        role: code_at(row, 2, "anchor role", AnchorRole::from_code)?,
        metaclass: text_at(row, 3, METACLASS_CHARS)?,
        name: opt_text_at(row, 4, NAME_CHARS)?,
    })
}

fn change_from(row: &mysql::Row) -> Result<ChangeRecord, String> {
    Ok(ChangeRecord {
        turn: id_at(row, 0)?,
        changes: count_at(row, 1)?,
        elements: count_at(row, 2)?,
        state: code_at(row, 3, "change state", ChangeState::from_code)?,
        updated_at: text_at(row, 4, 20)?,
        changed: Vec::new(),
    })
}

fn changed_from(row: &mysql::Row, at: usize) -> Result<ChangedElement, String> {
    Ok(ChangedElement {
        element: id_at(row, at)?,
        kind: code_at(row, at + 1, "change kind", ChangeKind::from_code)?,
        metaclass: text_at(row, at + 2, METACLASS_CHARS)?,
        name: opt_text_at(row, at + 3, NAME_CHARS)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_code_reads_back_and_an_unknown_one_is_refused_naming_the_choices() {
        for p in Provider::ALL {
            assert_eq!(Provider::from_code(p.code()), Ok(*p));
        }
        for s in ChangeState::ALL {
            assert_eq!(ChangeState::from_code(s.code()), Ok(*s));
        }
        assert_eq!(Provider::from_code("gemini"), Err("\"gemini\" is not one of claude, codex".to_string()));
        assert!(TurnError::from_code("").is_err());
    }

    #[test]
    fn texts_are_cut_at_character_boundaries_and_say_so() {
        assert_eq!(cut_bytes("Kunde", 10), ("Kunde", false));
        // "顧" takes three bytes: a cut inside it goes back to before it.
        assert_eq!(cut_bytes("a顧客", 2), ("a", true));
        assert_eq!(cut_bytes("a顧客", 4), ("a顧", true));
        assert_eq!(cut_chars("顧客様", 2), ("顧客".to_string(), true));
        assert_eq!(cut_chars("顧客", 2), ("顧客".to_string(), false));
        assert_eq!(auto_title("  Rename\nCustomer  ").ok(), Some("Rename Customer".to_string()));
        assert!(auto_title(" \n ").is_err());
    }

    #[test]
    fn times_and_metaclasses_of_another_form_are_refused() {
        assert!(time("2026-11-06T14:02:11Z").is_ok());
        for bad in ["2026-11-06 14:02:11Z", "2026-11-06T14:02:11", "", "2026-11-06T14:02:11+01:00"] {
            assert!(time(bad).is_err(), "{bad}");
        }
        assert!(metaclass("uml_class").is_ok());
        assert!(metaclass("UML class").is_err() && metaclass("").is_err());
    }

    #[test]
    fn a_turn_span_moves_its_state_only_at_its_own_ends() {
        let span = TurnSpan { before: 4, after: 9 };
        assert_eq!(span.moved(ChangeState::Applied, 4), Some(ChangeState::Undone));
        assert_eq!(span.moved(ChangeState::Applied, 6), None, "part way back is not undone");
        assert_eq!(span.moved(ChangeState::Undone, 9), Some(ChangeState::Applied));
        assert_eq!(span.moved(ChangeState::Undone, 4), None);
        assert_eq!(span.moved(ChangeState::Discarded, 4), None);
    }

    #[test]
    fn dont_save_puts_each_turn_back_to_its_state_at_the_save_or_discards_it() {
        let t = |n: u8| Uuid([n; 16]);
        let mut ledger = Ledger::default();
        ledger.now.insert(t(1), ChangeState::Applied);
        ledger.now.insert(t(2), ChangeState::Applied);
        ledger.saved = ledger.now.clone();
        ledger.now.insert(t(1), ChangeState::Undone);
        ledger.now.insert(t(3), ChangeState::Applied);
        assert_eq!(ledger.discarded(), vec![(t(1), ChangeState::Applied), (t(3), ChangeState::Discarded)]);
    }
}
