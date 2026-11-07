//! Where Chat's conversations are kept: in the open project (AR-206), through the host. The shell
//! never touches a project's database; it asks [`ChatStore`] and takes the answer on a channel, as
//! it does for the model's rows. The application runs every write on the project's writer thread,
//! in order with the model's writes, and checks a question (and a title made from it) against the
//! API keys in the system's secure store before anything is written; [`fake::Memory`] keeps
//! conversations in memory for the tests and the offscreen screens, by the same rules.
//!
//! What a conversation holds is the schema's (`docs/design/chat-storage.md`): a turn per question,
//! its timeline of the question, the agent's replies and tool rows, and what the turn changed in
//! the model. Times are seconds since 1970 here; the host stores them as the project stores times.

use std::path::Path;
use std::sync::mpsc::Receiver;

use archforge_mof::Uuid;

use crate::ai_agents::Provider;
use crate::chat::FailureKind;

/// How a turn ended, or that it runs; interrupted when Archforge found it running on opening.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Running,
    Completed,
    Stopped,
    Failed,
    Interrupted,
}

/// What Archforge last knew of a turn's changes to the model: a record, not a promise.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeState {
    Applied,
    Undone,
    /// Don't save put the model back past them.
    Discarded,
}

/// What a turn did to an element, net over the turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeKind {
    Added,
    Changed,
    Removed,
}

/// How a tool call went: done, refused by Archforge (a read-only conversation, a revoked token),
/// or failed (the command refused its parameters, say).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolStatus {
    Done,
    Refused,
    Failed,
}

/// Why a message names an element: a context chip sent with a question, an element a tool's answer
/// named, or the element a tool call was about (what it read, changed, added or removed).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AnchorRole {
    Project,
    Diagram,
    Selection,
    Read,
    Target,
}

/// An element a message names, with its metaclass (its generated table) and its name then.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Anchor {
    pub element: Uuid,
    pub role: AnchorRole,
    pub metaclass: String,
    pub name: Option<String>,
}

/// A tool activity row as kept: what the control channel answered for the conversation's origin.
/// Its words are made when it is drawn, so a kept row shows in the language of the day.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolFacts {
    /// The control channel's method: `engine.execute`, `engine.batch`, `document.inspect`.
    pub tool: String,
    /// The command it ran, for `engine.execute` and a batch's step.
    pub command: Option<String>,
    /// The property or aspect it was about (`description`).
    pub detail: Option<String>,
    pub status: ToolStatus,
    pub changed_model: bool,
    /// How many elements its answer named; the read anchors may be fewer.
    pub element_count: Option<u32>,
    pub anchors: Vec<Anchor>,
}

/// An element a turn changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangedElement {
    pub element: Uuid,
    pub kind: ChangeKind,
    pub metaclass: String,
    pub name: Option<String>,
}

/// What a turn changed in the model: the commands that changed it, the elements in all, and those
/// elements (perhaps fewer).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Changes {
    pub changes: u32,
    pub elements: u32,
    pub changed: Vec<ChangedElement>,
}

/// A conversation as the history lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub id: Uuid,
    /// The label of its changes' undo steps and of its agent access, such as `Chat 7`.
    pub origin: String,
    pub title: String,
    pub provider: Provider,
    /// The model its next turn asks for.
    pub model: String,
    /// Its last activity, in seconds since 1970.
    pub updated_at: u64,
    /// Its questions and replies.
    pub messages: u32,
}

/// One entry of the history: a conversation, or why its row cannot be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listed {
    pub id: Uuid,
    pub entry: Result<Row, String>,
}

/// What a kept turn changed, and the state of those changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredChange {
    pub changes: u32,
    pub state: ChangeState,
}

/// A kept turn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredTurn {
    pub id: Uuid,
    pub number: u32,
    /// The model that answered.
    pub model: String,
    pub allow_changes: bool,
    pub outcome: Outcome,
    pub error: Option<FailureKind>,
    pub change: Option<StoredChange>,
}

/// A kept message's content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoredBody {
    /// A question, with the context chips sent with it.
    Question {
        text: String,
        chips: Vec<Anchor>,
    },
    /// An agent's reply: cut off by Stop, a failure or a crash when not complete, cut to its limit
    /// when shortened.
    Reply {
        text: String,
        complete: bool,
        shortened: bool,
    },
    Tool(ToolFacts),
}

/// A kept message, in its turn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredMessage {
    pub id: Uuid,
    pub turn: Uuid,
    pub body: StoredBody,
}

/// A whole conversation: its row, its turns in order and its timeline in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stored {
    pub row: Row,
    pub turns: Vec<StoredTurn>,
    pub messages: Vec<StoredMessage>,
}

/// A write to a project's conversations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Write {
    /// Makes a conversation; its title comes from the first question, and the host gives it its
    /// origin, the next free `Chat N` of the project.
    Create { chat: Uuid, title: String, provider: Provider, model: String, now: u64 },
    /// Starts a turn with its question and the context chips sent with it, before the agent gets
    /// the text.
    Ask { chat: Uuid, turn: Uuid, question: Uuid, text: String, chips: Vec<Anchor>, model: String, allow_changes: bool, now: u64 },
    /// Runs a failed turn again in place (owner decision): an edited question, with its chips this
    /// time, replaces the kept one; none sends the kept one again.
    Rerun { turn: Uuid, model: String, allow_changes: bool, question: Option<(String, Vec<Anchor>)>, now: u64 },
    /// An agent's reply, when it completes or when the turn ends with it cut off.
    /// `shortened` when the panel cut it already.
    Reply { id: Uuid, turn: Uuid, text: String, complete: bool, shortened: bool, now: u64 },
    /// A tool row: its facts, its arguments and the start of its answer as JSON, and Archforge's
    /// refusal or failure text.
    Tool { id: Uuid, turn: Uuid, facts: ToolFacts, arguments: Option<String>, result: Option<String>, error: Option<String>, now: u64 },
    /// Ends a turn, with what it changed; the host commits the conversation then.
    End { turn: Uuid, outcome: Outcome, error: Option<FailureKind>, changes: Option<Changes>, now: u64 },
    /// Undo or redo moved turns' changes.
    States { moves: Vec<(Uuid, ChangeState)>, now: u64 },
    /// The model the conversation's next turn asks for.
    Model { chat: Uuid, model: String, now: u64 },
}

impl Write {
    /// The write's kind, for the log and the tests; never its text.
    pub fn kind(&self) -> &'static str {
        match self {
            Write::Create { .. } => "create",
            Write::Ask { .. } => "ask",
            Write::Rerun { .. } => "rerun",
            Write::Reply { .. } => "reply",
            Write::Tool { .. } => "tool",
            Write::End { .. } => "end",
            Write::States { .. } => "states",
            Write::Model { .. } => "model",
        }
    }
}

/// What a write answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answer {
    /// The conversation was made, labelled `origin`.
    Created {
        origin: String,
    },
    /// The turn was started, its number in the conversation.
    Asked {
        number: u32,
    },
    Done,
}

/// Why a write was refused; nothing was written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// The text holds one of the API keys Archforge keeps, the provider's when the host can tell.
    HeldKey { provider: Option<Provider> },
    /// Anything else, in the host's words.
    Failed(String),
}

/// The host's part in keeping conversations: every call returns at once, with a channel the host
/// answers on once.
pub trait ChatStore: Send + Sync {
    /// The conversations of the project at `project`, newest activity first.
    fn list(&self, project: &Path) -> Receiver<Result<Vec<Listed>, String>>;
    /// One conversation, whole.
    fn load(&self, project: &Path, chat: Uuid) -> Receiver<Result<Stored, String>>;
    /// Writes `write` to the project's conversations, in order with the project's other writes.
    fn write(&self, project: &Path, write: Write) -> Receiver<Result<Answer, Refusal>>;
}

/// Conversations kept in memory, by the rules the project keeps them by: for the tests and the
/// offscreen screens.
#[cfg(any(test, feature = "snapshot"))]
pub mod fake {
    use std::sync::Mutex;

    use super::*;

    /// One conversation as the fake keeps it.
    #[derive(Debug, Clone)]
    pub struct Kept {
        pub row: Row,
        pub turns: Vec<StoredTurn>,
        pub messages: Vec<StoredMessage>,
        /// Each tool row's arguments and answer as written, by message.
        pub calls: Vec<(Uuid, Option<String>, Option<String>)>,
        /// What each turn's end said it changed, by turn.
        pub ends: Vec<(Uuid, Option<Changes>)>,
    }

    /// The fake store: what it holds, the keys it refuses, and a refusal for the writes to come.
    #[derive(Default)]
    pub struct Memory {
        pub chats: Mutex<Vec<Kept>>,
        /// The keys held, as `(provider, key)`: a question or title holding one is refused.
        pub keys: Vec<(Provider, String)>,
        /// While set, every write is refused with it, as a project that cannot be written.
        pub refuse: Mutex<Option<String>>,
        /// The kinds of the writes asked for, in order, refused ones included.
        pub log: Mutex<Vec<&'static str>>,
        /// The project each write named.
        pub projects: Mutex<Vec<std::path::PathBuf>>,
    }

    /// A channel already holding `value`.
    fn answered<T: Send + 'static>(value: T) -> Receiver<T> {
        let (tx, rx) = std::sync::mpsc::channel();
        let _ = tx.send(value);
        rx
    }

    /// A lock a panic poisoned is taken over: what it guards stays usable.
    fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
        m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    impl Memory {
        /// The conversation `chat` as kept.
        pub fn kept(&self, chat: Uuid) -> Option<Kept> {
            lock(&self.chats).iter().find(|c| c.row.id == chat).cloned()
        }

        /// Keeps `kept` as a conversation of its own, as one written before.
        pub fn hold(&self, kept: Kept) {
            lock(&self.chats).push(kept);
        }

        /// The provider whose key `text` holds, if any.
        fn held_key(&self, text: &str) -> Option<Provider> {
            self.keys.iter().find(|(_, k)| !k.is_empty() && text.contains(k.as_str())).map(|(p, _)| *p)
        }

        /// Runs one write against `chats`.
        fn apply(&self, chats: &mut Vec<Kept>, write: Write) -> Result<Answer, Refusal> {
            let failed = |why: &str| Refusal::Failed(why.to_string());
            let turn_of = |chats: &mut Vec<Kept>, turn: Uuid| -> Option<(usize, usize)> {
                chats.iter().enumerate().find_map(|(c, k)| k.turns.iter().position(|t| t.id == turn).map(|t| (c, t)))
            };
            match write {
                Write::Create { chat, title, provider, model, now } => {
                    if let Some(p) = self.held_key(&title) {
                        return Err(Refusal::HeldKey { provider: Some(p) });
                    }
                    let n = chats.iter().filter_map(|c| c.row.origin.strip_prefix("Chat ").and_then(|n| n.parse::<u32>().ok())).max().unwrap_or(0);
                    let origin = format!("Chat {}", n.saturating_add(1));
                    let title: String =
                        title.chars().map(|c| if c.is_control() { ' ' } else { c }).collect::<String>().trim().chars().take(200).collect();
                    let row = Row { id: chat, origin: origin.clone(), title, provider, model, updated_at: now, messages: 0 };
                    chats.insert(0, Kept { row, turns: Vec::new(), messages: Vec::new(), calls: Vec::new(), ends: Vec::new() });
                    Ok(Answer::Created { origin })
                }
                Write::Ask { chat, turn, question, text, chips, model, allow_changes, now } => {
                    if text.len() > crate::chat_panel::QUESTION_BYTES {
                        return Err(failed("a question is longer than 64 KiB"));
                    }
                    if let Some(p) = self.held_key(&text) {
                        return Err(Refusal::HeldKey { provider: Some(p) });
                    }
                    let kept = chats.iter_mut().find(|c| c.row.id == chat).ok_or_else(|| failed("no such conversation"))?;
                    if kept.turns.iter().any(|t| t.outcome == Outcome::Running) {
                        return Err(failed("another turn of this conversation is still running"));
                    }
                    let number = u32::try_from(kept.turns.len()).unwrap_or(u32::MAX).saturating_add(1);
                    kept.turns.push(StoredTurn { id: turn, number, model, allow_changes, outcome: Outcome::Running, error: None, change: None });
                    kept.messages.push(StoredMessage { id: question, turn, body: StoredBody::Question { text, chips } });
                    kept.row.updated_at = now;
                    kept.row.messages = kept.row.messages.saturating_add(1);
                    Ok(Answer::Asked { number })
                }
                Write::Rerun { turn, model, allow_changes, question, now } => {
                    if let Some((text, _)) = &question {
                        if text.len() > crate::chat_panel::QUESTION_BYTES {
                            return Err(failed("a question is longer than 64 KiB"));
                        }
                        if let Some(p) = self.held_key(text) {
                            return Err(Refusal::HeldKey { provider: Some(p) });
                        }
                    }
                    let (c, t) = turn_of(chats, turn).ok_or_else(|| failed("no such turn"))?;
                    let kept = &mut chats[c];
                    let last = kept.turns.len() == t + 1;
                    let row = &mut kept.turns[t];
                    if row.outcome != Outcome::Failed || !last || row.change.is_some() {
                        return Err(failed("only the last failed turn that changed nothing runs again"));
                    }
                    row.outcome = Outcome::Running;
                    row.error = None;
                    row.model = model;
                    row.allow_changes = allow_changes;
                    if let Some((text, chips)) = question
                        && let Some(m) = kept.messages.iter_mut().find(|m| m.turn == turn && matches!(m.body, StoredBody::Question { .. }))
                    {
                        m.body = StoredBody::Question { text, chips };
                    }
                    kept.row.updated_at = now;
                    Ok(Answer::Done)
                }
                Write::Reply { id, turn, text, complete, shortened: cut_before, now } => {
                    let (c, t) = turn_of(chats, turn).ok_or_else(|| failed("no such turn"))?;
                    if chats[c].turns[t].outcome != Outcome::Running {
                        return Err(failed("the turn has ended"));
                    }
                    let shortened = cut_before || text.len() > crate::chat_panel::REPLY_BYTES;
                    let text = crate::chat_panel::cut(&text, crate::chat_panel::REPLY_BYTES).to_string();
                    chats[c].messages.push(StoredMessage { id, turn, body: StoredBody::Reply { text, complete, shortened } });
                    chats[c].row.messages = chats[c].row.messages.saturating_add(1);
                    chats[c].row.updated_at = now;
                    Ok(Answer::Done)
                }
                Write::Tool { id, turn, facts, arguments, result, error: _, now } => {
                    let (c, t) = turn_of(chats, turn).ok_or_else(|| failed("no such turn"))?;
                    if chats[c].turns[t].outcome != Outcome::Running {
                        return Err(failed("the turn has ended"));
                    }
                    chats[c].messages.push(StoredMessage { id, turn, body: StoredBody::Tool(facts) });
                    chats[c].calls.push((id, arguments, result));
                    chats[c].row.updated_at = now;
                    Ok(Answer::Done)
                }
                Write::End { turn, outcome, error, changes, now } => {
                    let (c, t) = turn_of(chats, turn).ok_or_else(|| failed("no such turn"))?;
                    let row = &mut chats[c].turns[t];
                    if row.outcome != Outcome::Running {
                        return Err(failed("the turn has ended"));
                    }
                    if (outcome == Outcome::Failed) != error.is_some() {
                        return Err(failed("a failed turn names why, and only a failed one"));
                    }
                    row.outcome = outcome;
                    row.error = error;
                    row.change = changes.as_ref().map(|c| StoredChange { changes: c.changes, state: ChangeState::Applied });
                    chats[c].ends.push((turn, changes));
                    chats[c].row.updated_at = now;
                    Ok(Answer::Done)
                }
                Write::States { moves, now: _ } => {
                    for (turn, state) in moves {
                        let (c, t) = turn_of(chats, turn).ok_or_else(|| failed("no such turn"))?;
                        let change = chats[c].turns[t].change.as_mut().ok_or_else(|| failed("the turn changed nothing"))?;
                        change.state = state;
                    }
                    Ok(Answer::Done)
                }
                Write::Model { chat, model, now } => {
                    let kept = chats.iter_mut().find(|c| c.row.id == chat).ok_or_else(|| failed("no such conversation"))?;
                    kept.row.model = model;
                    kept.row.updated_at = now;
                    Ok(Answer::Done)
                }
            }
        }
    }

    impl ChatStore for Memory {
        /// The conversations, newest activity first.
        fn list(&self, _project: &Path) -> Receiver<Result<Vec<Listed>, String>> {
            let mut rows: Vec<Row> = lock(&self.chats).iter().map(|c| c.row.clone()).collect();
            rows.sort_by_key(|r| std::cmp::Reverse(r.updated_at));
            answered(Ok(rows.into_iter().map(|r| Listed { id: r.id, entry: Ok(r) }).collect()))
        }

        /// One conversation, whole.
        fn load(&self, _project: &Path, chat: Uuid) -> Receiver<Result<Stored, String>> {
            answered(
                self.kept(chat)
                    .map(|k| Stored { row: k.row, turns: k.turns, messages: k.messages })
                    .ok_or_else(|| format!("no conversation {chat} is in the project")),
            )
        }

        /// Runs `write` at once, or refuses it as the project would.
        fn write(&self, project: &Path, write: Write) -> Receiver<Result<Answer, Refusal>> {
            lock(&self.log).push(write.kind());
            lock(&self.projects).push(project.to_path_buf());
            if let Some(why) = lock(&self.refuse).clone() {
                return answered(Err(Refusal::Failed(why)));
            }
            let mut chats = lock(&self.chats);
            answered(self.apply(&mut chats, write))
        }
    }
}
