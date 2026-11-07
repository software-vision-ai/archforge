//! The Chat panel's conversations (AR-205): what the panel holds per project, and how a question
//! goes out, an answer comes in, a turn ends, is stopped, sent again or undone.
//!
//! - **A question** is kept before the agent gets it: the conversation is made in the project with
//!   the first one ([`Write::Create`]), each question starts a turn there ([`Write::Ask`]), and only
//!   once the project took it does the composer empty and the agent receive the text, with the
//!   context chips as a line after it. A refusal (a kept key in the question, a project that could
//!   not be written) keeps the draft and says why.
//! - **A turn** groups the model's edits its tool calls make into one undo step of the project's
//!   history (`Chat N: Turn n`), so "Undo this turn's changes" undoes exactly them and keeps the
//!   person's own edits; it is offered while that step is on top of the history. Another change
//!   during the turn ends the step, and the turn's next edit starts another: a turn of several
//!   steps is not undone as one.
//! - **Tool rows** are worded from what the control channel answered for the conversation's
//!   origin, never from what the agent says it did.
//! - **Stop** keeps the reply so far, marked cut off; **Send again** runs a failed turn again in
//!   place (owner decision), its question kept once, an edited one replacing it; after a lost
//!   connection the question goes back to the composer when the composer is empty.
//! - **The provider and model** a conversation uses change only when the person changes them in its
//!   badge (no silent switch): Settings' default applies to a conversation that has not started.
//!   A changed model or switch ends the conversation's program once it is idle, and the next
//!   question starts it again where it was.
//! - **Project tabs**: switching away stops a running turn; closing a project ends its
//!   conversation's program.
//!
//! Everything held is bounded: the text kept in memory per conversation ([`MAX_HELD`]), a reply
//! ([`REPLY_BYTES`]), the tool rows of a turn ([`MAX_ROWS`]), the turns of a conversation (the
//! project's own cap).

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, TryRecvError};

use archforge_mof::{Obj, Store, Uuid};
use serde_json::Value;

use crate::Shell;
use crate::ai_agents::{Provider, Readiness};
use crate::chat::{ChatEvent, ChatFailure, ChatStart, FailureKind, TurnEnd};
use crate::chat_store::{
    Anchor, AnchorRole, Answer, ChangeKind, ChangeState, ChangedElement, Changes, ChatStore, Listed, Outcome, Refusal, Stored, StoredBody, ToolFacts,
    ToolStatus, Write,
};
use crate::chat_words::Clock;
use crate::i18n::{fmt, tr};

/// A question, in bytes: a longer one is refused, neither sent nor kept (the project's limit).
pub const QUESTION_BYTES: usize = 64 << 10;
/// A reply, in bytes: a longer one is cut at a character boundary and marked.
pub const REPLY_BYTES: usize = 256 << 10;
/// The text a conversation holds in memory, in bytes: what streams in past it is not held.
pub const MAX_HELD: usize = 64 << 20;
/// The tool rows one turn shows and keeps; later ones are counted only.
pub const MAX_ROWS: usize = 2_000;
/// How long a Stop waits for the program to say the turn stopped before the panel ends it itself,
/// in seconds.
pub const STOP_GRACE: u64 = 10;
/// The arguments and the answer of a tool call handed to the project, in bytes: it keeps less.
const CALL_BYTES: usize = 64 << 10;
/// The elements a tool row keeps of those its answer named, as the project keeps them.
const READ_ANCHORS: usize = 500;
/// The elements an answer is searched for, and the JSON values walked to find them.
const MAX_FOUND: usize = 2_000;
const MAX_WALK: usize = 100_000;

/// `text` cut to at most `max` bytes at a character boundary.
pub fn cut(text: &str, max: usize) -> &str {
    if text.len() <= max {
        return text;
    }
    let mut end = max;
    while !text.is_char_boundary(end) {
        end = end.saturating_sub(1);
    }
    text.get(..end).unwrap_or_default()
}

/// A context chip: what goes with a question unless the person removed it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Chip {
    Project,
    Diagram,
    Selection,
}

/// What the panel shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum View {
    #[default]
    Conversation,
    History,
}

/// Whether the conversation's agent program runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Session {
    #[default]
    Off,
    Running,
    /// Asked to end (its model or switch changed); it starts again with the next question.
    Ending,
}

/// A question on its way to the project, and what it carries.
#[derive(Debug, Clone)]
pub struct Pending {
    pub text: String,
    pub chips: Vec<Anchor>,
    /// The line of context after the question, for the agent.
    pub context: String,
    pub turn: Uuid,
    pub question: Uuid,
    /// The question came from the composer, which empties once it is kept.
    pub from_draft: bool,
}

/// What the conversation waits for from the project.
#[derive(Default)]
pub enum Step {
    #[default]
    Idle,
    /// The conversation being made, then the question.
    Creating(Receiver<Result<Answer, Refusal>>, Pending),
    /// The question being kept as a new turn.
    Asking(Receiver<Result<Answer, Refusal>>, Pending),
    /// The last turn being run again.
    Rerunning(Receiver<Result<Answer, Refusal>>, Pending),
}

impl Step {
    /// Whether a question is on its way.
    pub fn busy(&self) -> bool {
        !matches!(self, Step::Idle)
    }
}

/// How a turn stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TurnState {
    Running,
    Completed,
    Stopped,
    Failed(ChatFailure),
    /// Archforge closed while it ran.
    Interrupted,
}

/// A turn's changes to the model in this window's undo history: the state before its first change
/// and after its last, and the undo steps it took.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub before: u64,
    pub after: u64,
    pub steps: u32,
}

/// What a turn changed, and whether it can still be undone here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub changes: u32,
    pub state: ChangeState,
    /// None once the project was closed since (or the conversation was read back from it).
    pub span: Option<Span>,
}

/// A rendered reply, kept until its width or appearance changes: its layout to lay out again while
/// it shows (the fonts' atlas may change between frames, so no galley is kept), its height, and its
/// links.
pub struct Laid {
    pub width: f32,
    pub dark: bool,
    pub job: Arc<egui::text::LayoutJob>,
    pub height: f32,
    pub links: Vec<(std::ops::Range<usize>, String)>,
}

/// One entry of a turn's timeline.
pub enum Item {
    /// A reply, cut off when not complete; shortened when cut to its limit.
    Text {
        text: String,
        complete: bool,
        shortened: bool,
        laid: Option<Laid>,
    },
    Tool(ToolFacts),
}

/// A turn as the panel holds it.
pub struct Turn {
    pub id: Uuid,
    pub number: u32,
    pub question: String,
    pub question_id: Uuid,
    /// The context chips sent with the question, sent again with it.
    pub chips: Vec<Anchor>,
    /// The question went back to the composer after a lost connection: its bubble is hidden.
    pub returned: bool,
    pub items: Vec<Item>,
    /// The reply being written: where it goes in the timeline, and its text so far.
    pub streaming: Option<(usize, String)>,
    pub state: TurnState,
    /// The model it asked for, as the badge said it then.
    pub model: String,
    pub change: Option<Change>,
    /// Stop was asked and not yet answered, and since when (seconds since 1970).
    pub stopping: bool,
    pub stopping_since: Option<u64>,
    /// Text the agent wrote past what the conversation may hold was not kept.
    pub cut: bool,
    /// Tool rows past [`MAX_ROWS`], counted.
    pub rows_dropped: u64,
    /// The rendered streaming text.
    pub laid: Option<Laid>,
    /// While it runs, the agent's program tries the provider again (AR-224): its attempt and the
    /// most it makes, as it said them; gone once anything comes from the provider.
    pub retrying: Option<(u32, Option<u32>)>,
    /// Its question was kept but never sent, as Let Chat use the model went off first: its card
    /// says so and offers Send again. Not kept in the project, which keeps the turn as failed.
    pub unsent: bool,
}

impl Turn {
    /// Whether the agent answered anything in it, text or a tool row.
    pub fn answered(&self) -> bool {
        !self.items.is_empty() || self.streaming.as_ref().is_some_and(|(_, t)| !t.is_empty())
    }
}

/// One conversation as the panel holds it.
pub struct Chat {
    pub id: Uuid,
    /// The id as the host takes it: lowercase UUID text.
    pub key: String,
    /// Its label (`Chat 7`), once the project made it.
    pub origin: Option<String>,
    pub title: String,
    pub provider: Provider,
    /// The model it asks for, as the program takes it (`sonnet`, `default`).
    pub model: String,
    /// The model the program named, once it did.
    pub named: Option<String>,
    /// Whether its provider and model are its own: picked in the badge, used, or read back. A
    /// conversation that is not follows Settings' default.
    pub chosen: bool,
    pub allow_changes: bool,
    pub removed: BTreeSet<Chip>,
    pub turns: Vec<Turn>,
    pub session: Session,
    /// Its program ran before (here or when it was kept): the next start resumes it.
    pub resume: bool,
    pub step: Step,
    /// The text for the agent waiting for the program to end and start again.
    pub queued: Option<String>,
    /// Writes not yet answered.
    pub writes: Vec<Receiver<Result<Answer, Refusal>>>,
    /// Why the last write failed: "This conversation could not be saved: {why}".
    pub save_error: Option<String>,
    /// The text it holds in memory, in bytes.
    pub held: usize,
}

impl Chat {
    /// A new conversation with `provider` and `model`, not yet in the project.
    pub fn new(id: Uuid, provider: Provider, model: &str) -> Chat {
        Chat {
            id,
            key: id.to_string(),
            origin: None,
            title: String::new(),
            provider,
            model: model.to_string(),
            named: None,
            chosen: false,
            allow_changes: false,
            removed: BTreeSet::new(),
            turns: Vec::new(),
            session: Session::Off,
            resume: false,
            step: Step::Idle,
            queued: None,
            writes: Vec::new(),
            save_error: None,
            held: 0,
        }
    }

    /// Whether a turn runs, or a question is on its way: Enter, the badge, the switch, New
    /// conversation and the history wait for it.
    pub fn busy(&self) -> bool {
        self.step.busy() || self.turns.last().is_some_and(|t| t.state == TurnState::Running)
    }

    /// The running turn.
    fn running(&mut self) -> Option<&mut Turn> {
        self.turns.last_mut().filter(|t| t.state == TurnState::Running)
    }

    /// The model as the badge and the turn name it.
    pub fn model_label(&self) -> String {
        crate::chat_words::badge(self.provider, &self.model, self.named.as_deref())
    }
}

/// The Chat panel of one project.
#[derive(Default)]
pub struct Panel {
    pub view: View,
    pub chat: Option<Chat>,
    /// The composer's text: kept when a question could not go, and while a reply runs.
    pub draft: String,
    /// Why the last question did not go, under the composer.
    pub notice: Option<String>,
    /// The history as last read, and a reading under way.
    pub history: Option<Result<Vec<Listed>, String>>,
    pub listing: Option<Receiver<Result<Vec<Listed>, String>>>,
    /// A conversation being read back, and why the last one could not be.
    pub loading: Option<(Uuid, Receiver<Result<Stored, String>>)>,
    pub load_error: Option<String>,
    /// A link waiting for the confirmation.
    pub link: Option<String>,
    /// The badge's menu is open.
    pub menu: bool,
    /// Each turn's place in this window's undo history, by turn, kept while the project is open so a
    /// conversation opened again from the history still offers its undo.
    pub spans: BTreeMap<Uuid, Span>,
    /// The scroll position of the conversation, and whether it follows the end.
    pub scroll: f32,
    pub follow: bool,
    /// Whether the conversation was shown to its end the last time it was drawn: what comes in
    /// then keeps the end in view; scrolled up, the person's place stays.
    pub at_end: bool,
    /// The history's scroll position.
    pub history_scroll: f32,
    /// Why the history's conversations cannot be changed: a re-read of the list failed, and the
    /// rows read before stay shown (AR-217).
    pub writing_off: Option<String>,
    /// Why the last deletion or Undo failed, under the history.
    pub list_notice: Option<String>,
    /// The confirmation of a deletion, while it is open.
    pub ask: Option<crate::chat_delete::Ask>,
    /// A history row's menu, while it is open.
    pub row_menu: Option<crate::chat_delete::RowMenu>,
    /// A deletion confirmed while its conversation's question was on its way: it goes on once
    /// the project has answered the question.
    pub delete_after: Option<crate::chat_delete::Ask>,
    /// Where the keyboard goes when the history is next drawn, and how many more frames it is
    /// asked for before it is given up.
    pub focus: Option<crate::chat_delete::FocusTo>,
    pub focus_tries: u8,
    /// The conversation being read back opens without leaving the history: it replaces a deleted
    /// one.
    pub loading_stays: bool,
}

/// The time now and the local offset, from the host, or the system's time in UTC.
pub fn clock(shell: &Shell) -> Clock {
    match &shell.services.clock {
        Some(c) => c(),
        None => Clock { now: crate::home::now(), offset: None },
    }
}

/// A fresh id for a conversation, a turn or a message: the host's (version 7), else none.
fn fresh(shell: &Shell) -> Option<Uuid> {
    shell.services.ids.as_ref().map(|ids| ids())
}

// The model's elements, as rows and chips name them.

/// The generated table of each class, as the project stores elements.
fn tables() -> &'static Vec<(archforge_mof::ClassId, &'static str)> {
    static TABLES: std::sync::OnceLock<Vec<(archforge_mof::ClassId, &'static str)>> = std::sync::OnceLock::new();
    TABLES.get_or_init(|| archforge_notations::storage().0.iter().map(|t| (t.class, t.name)).collect())
}

/// The metaclass (generated table) of `obj`.
pub fn metaclass(store: &Store, obj: Obj) -> Option<&'static str> {
    let class = store.class_of(obj).ok()?;
    tables().iter().find(|(c, _)| *c == class).map(|(_, n)| *n)
}

/// An anchor to the live element `id` in `role`, with its name as a person reads it.
fn anchor(shell: &Shell, store: &Store, id: Uuid, role: AnchorRole) -> Option<Anchor> {
    let obj = store.by_uuid(id).filter(|o| store.is_alive(*o))?;
    let metaclass = metaclass(store, obj)?.to_string();
    let name = if store.class_of(obj).ok() == Some(archforge_diagram::DIAGRAM) {
        crate::diagrams::diagram_name(store, obj)
    } else {
        crate::notation_ui::label(shell.notation_uis, store, obj)
    };
    Some(Anchor { element: id, role, metaclass, name: Some(name).filter(|n| !n.is_empty()) })
}

/// A chip as the composer shows it: its kind, its label and its icon, and the elements it sends.
pub struct ChipView {
    pub chip: Chip,
    pub label: String,
    pub icon: &'static crate::ux::Icon,
    pub anchors: Vec<Anchor>,
}

/// The chips the composer offers now: the project, the diagram open, the selection.
pub fn chips(shell: &Shell) -> Vec<ChipView> {
    let mut out = Vec::new();
    let Some(session) = shell.session.as_ref() else { return out };
    let store = session.doc();
    let project = shell.project.as_ref().map(|p| p.name.clone()).unwrap_or_default();
    if let Some(root) = store.alive().find(|o| store.class_of(*o).ok() == Some(archforge_core::MODEL) && store.container(*o).ok().flatten().is_none())
        && let Ok(id) = store.uuid(root)
        && let Some(mut a) = anchor(shell, store, id, AnchorRole::Project)
    {
        a.name = Some(project.clone()).filter(|n| !n.is_empty()).or(a.name);
        out.push(ChipView { chip: Chip::Project, label: a.name.clone().unwrap_or_default(), icon: &crate::ux::icons::FOLDER, anchors: vec![a] });
    }
    if let Some(d) = shell.canvas.active
        && let Some(obj) = store.by_uuid(d).filter(|o| store.is_alive(*o) && store.class_of(*o).ok() == Some(archforge_diagram::DIAGRAM))
        && let Some(a) = anchor(shell, store, d, AnchorRole::Diagram)
    {
        let icon = crate::kinds::diagram_icon(shell.kinds, store, obj);
        out.push(ChipView { chip: Chip::Diagram, label: a.name.clone().unwrap_or_default(), icon, anchors: vec![a] });
    }
    let selected: Vec<Obj> = session.view().selection.iter().copied().filter(|o| store.is_alive(*o)).take(crate::chat_panel::MAX_FOUND).collect();
    let anchors: Vec<Anchor> =
        selected.iter().filter_map(|o| store.uuid(*o).ok()).filter_map(|id| anchor(shell, store, id, AnchorRole::Selection)).collect();
    match (selected.as_slice(), anchors.as_slice()) {
        ([obj], [a]) => {
            let icon = crate::notation_ui::of(shell.notation_uis, store, *obj)
                .and_then(|u| u.entry(store, *obj))
                .map_or(&crate::ux::icons::CUBE, |e| e.icon);
            out.push(ChipView { chip: Chip::Selection, label: a.name.clone().unwrap_or_default(), icon, anchors: anchors.clone() });
        }
        (_, []) => {}
        (_, many) => out.push(ChipView {
            chip: Chip::Selection,
            label: crate::i18n::trn("{n} element", "{n} elements", u64::try_from(many.len()).unwrap_or(u64::MAX)),
            icon: &crate::ux::icons::CUBE,
            anchors: anchors.clone(),
        }),
    }
    out
}

/// A name as the context line quotes it: on one line, cut to 200 characters.
fn quoted(name: &str) -> String {
    let flat: String = name.chars().map(|c| if c.is_control() || c == '"' { ' ' } else { c }).take(200).collect();
    format!("\"{}\"", flat.trim())
}

/// The context line after a question, for the agent: each chip by name and id.
pub fn context_line(chips: &[Anchor]) -> String {
    let parts: Vec<String> = chips
        .iter()
        .take(50)
        .map(|a| {
            let what = match a.role {
                AnchorRole::Project => "project",
                AnchorRole::Diagram => "open diagram",
                _ => "selected",
            };
            format!("{what} {} ({}, id {})", quoted(a.name.as_deref().unwrap_or_default()), a.metaclass, a.element)
        })
        .collect();
    if parts.is_empty() {
        return String::new();
    }
    let more = chips.len().saturating_sub(50);
    let tail = if more > 0 { format!("; and {more} more selected") } else { String::new() };
    format!("\n\n[Context the person chose: {}{tail}.]", parts.join("; "))
}

// Whether Chat can answer here.

/// Whether the agent `provider` is set up for Chat, as far as is known without starting anything:
/// its program found and its key saved, or Codex signed in (or its sign-in not read yet); and Let
/// Chat use the model on. None while the keys are still being read.
pub fn set_up(shell: &Shell, provider: Provider) -> Option<bool> {
    if shell.services.agents.as_ref().is_none_or(|h| h.system() != crate::ai_agents::System::Supported) {
        return Some(false);
    }
    if !shell.agents.keys_known() {
        return None;
    }
    let ready = match crate::ai_agents::readiness(shell, provider) {
        Readiness::Ready => true,
        Readiness::NotLooked => {
            provider == Provider::Codex
                && shell.state.agents.codex_sign_in == crate::ai_agents::CodexSignIn::ChatGpt
                && shell.agents.look.program(provider).found()
        }
        _ => false,
    };
    Some(ready && shell.state.agents.chat_uses_model)
}

/// Shows the panel: the keys are read the first time, a new conversation is ready when there is
/// none, and one that has not started follows Settings' default agent and model.
pub fn showing(shell: &mut Shell) {
    crate::ai_agents::need_keys(shell);
    let provider = shell.state.agents.default_agent;
    let model = shell.state.agents.model(provider).to_string();
    match shell.chat_panel.chat.as_mut() {
        Some(c) if !c.chosen && c.turns.is_empty() && !c.step.busy() => {
            c.provider = provider;
            c.model = model;
        }
        Some(_) => {}
        None => {
            if let Some(id) = fresh(shell) {
                shell.chat_panel.chat = Some(Chat::new(id, provider, &model));
                shell.chat_panel.follow = true;
            }
        }
    }
}

// The project's answers.

/// The store of conversations and the project the panel writes to.
fn store_of(shell: &Shell) -> Option<Arc<dyn ChatStore>> {
    shell.services.chats.clone()
}

/// Where a conversation's writes go and what they need: the store, the project, the time, ids.
pub(crate) struct Out {
    store: Option<Arc<dyn ChatStore>>,
    project: PathBuf,
    clock: Clock,
    ids: Option<Arc<dyn Fn() -> Uuid + Send + Sync>>,
    /// Whether the composer is empty, so a question can go back to it.
    draft_empty: bool,
}

impl Out {
    /// The writes of the project at `project`, as the shell's services give them.
    pub(crate) fn of(shell: &Shell, project: PathBuf, draft_empty: bool) -> Out {
        Out { store: store_of(shell), project, clock: clock(shell), ids: shell.services.ids.clone(), draft_empty }
    }

    /// The writes of the project shown.
    fn shown(shell: &Shell) -> Out {
        let project = shell.project.as_ref().map(|p| p.path.clone()).unwrap_or_default();
        Out::of(shell, project, shell.chat_panel.draft.trim().is_empty())
    }

    /// A fresh id for a message.
    fn id(&self) -> Option<Uuid> {
        self.ids.as_ref().map(|ids| ids())
    }

    /// Sends `w`, its answer watched by `chat`.
    fn write(&self, chat: &mut Chat, w: Write) {
        match &self.store {
            Some(s) => chat.writes.push(s.write(&self.project, w)),
            None => chat.save_error = Some(tr("This project keeps no conversations")),
        }
    }
}

/// The words for a refused write.
pub fn refusal_text(r: &Refusal) -> String {
    match r {
        Refusal::HeldKey { provider } => {
            let who = provider.map(|p| p.name()).unwrap_or("an agent's");
            fmt(&tr("This question holds your saved {agent} API key, so it was neither sent nor saved."), &[("agent", who)])
        }
        Refusal::Failed(why) => fmt(&tr("This conversation could not be saved: {why}"), &[("why", why)]),
        Refusal::Listed => fmt(&tr("This conversation could not be saved: {why}"), &[("why", &tr("it is in the conversation list already"))]),
    }
}

// Asking.

/// What the panel says while Let Chat use the model is off and something would be sent.
fn model_use_off() -> String {
    tr("Let Chat use the model is off, so nothing is sent. Turn it on in Settings ▸ AI Agents.")
}

/// Whether Let Chat use the model is off; then the panel says so, and nothing is sent (AR-221).
fn refused_while_off(shell: &mut Shell) -> bool {
    if shell.state.agents.chat_uses_model {
        return false;
    }
    shell.chat_panel.notice = Some(model_use_off());
    true
}

/// Let Chat use the model turned `on` or off. Off, every panel's conversation, the shown one and
/// those of tabs left, that runs (its program, or a question on its way) says why it stops, and one
/// whose program is in `ended` waits for that end, as [`restart`] does, so a question asked before
/// it comes waits for it and starts the program again. On, that line goes.
pub(crate) fn model_use_changed(shell: &mut Shell, on: bool, ended: &[String]) {
    let off_line = model_use_off();
    let mut panels: Vec<&mut Panel> = vec![&mut shell.chat_panel];
    panels.extend(shell.tabs.iter_mut().filter_map(|t| t.parked.as_mut()).map(|p| &mut p.chat_panel));
    for panel in panels {
        if on {
            if panel.notice.as_deref() == Some(off_line.as_str()) {
                panel.notice = None;
            }
            continue;
        }
        let Some(chat) = panel.chat.as_mut() else { continue };
        if chat.busy() || chat.session != Session::Off {
            panel.notice = Some(off_line.clone());
        }
        if chat.session == Session::Running && ended.contains(&chat.key) {
            chat.session = Session::Ending;
        }
    }
}

/// Sends the composer's text: a new turn, or the failed turn run again when its question went back
/// to the composer. Refused while a turn runs, for an empty or too long question, without a
/// project, or while Let Chat use the model is off (the question then stays in the box, not kept).
pub fn send(shell: &mut Shell) {
    let text = shell.chat_panel.draft.trim().to_string();
    let Some(busy) = shell.chat_panel.chat.as_ref().map(Chat::busy) else { return };
    if busy || text.is_empty() || refused_while_off(shell) {
        return;
    }
    let Some(chat) = shell.chat_panel.chat.as_ref() else { return };
    if text.len() > QUESTION_BYTES {
        let kib = (text.len() / 1024).to_string();
        shell.chat_panel.notice = Some(fmt(&tr("A question is at most 64 KiB; this one is {n} KiB."), &[("n", &kib)]));
        return;
    }
    // A question back in the composer is sent again in place when its turn can be; else it is a
    // new question.
    let now = clock(shell).now;
    let returned = chat.turns.last().is_some_and(|t| t.returned && retryable(t, now));
    shell.chat_panel.loading = None;
    if returned {
        retry(shell, Some(text));
        return;
    }
    ask(shell, text, true);
}

/// Starts a new turn with `text`: the conversation is made first when it is new.
fn ask(shell: &mut Shell, text: String, from_draft: bool) {
    let (Some(project), Some(turn), Some(question)) = (shell.project.as_ref().map(|p| p.path.clone()), fresh(shell), fresh(shell)) else {
        shell.chat_panel.notice = Some(tr("This project keeps no conversations"));
        return;
    };
    let removed = shell.chat_panel.chat.as_ref().map(|c| c.removed.clone()).unwrap_or_default();
    let chips: Vec<Anchor> = chips(shell).into_iter().filter(|c| !removed.contains(&c.chip)).flat_map(|c| c.anchors).collect();
    let context = context_line(&chips);
    let store = store_of(shell);
    let now = clock(shell).now;
    let skip = crate::chat_delete::held_labels(shell, &project);
    let Some(chat) = shell.chat_panel.chat.as_mut() else { return };
    shell.chat_panel.notice = None;
    chat.chosen = true;
    let pending = Pending { text: text.clone(), chips, context, turn, question, from_draft };
    let Some(s) = store else {
        shell.chat_panel.notice = Some(tr("This project keeps no conversations"));
        return;
    };
    if chat.origin.is_none() {
        let rx = s.write(&project, Write::Create { chat: chat.id, title: text, provider: chat.provider, model: chat.model.clone(), skip, now });
        chat.step = Step::Creating(rx, pending);
    } else {
        chat.step = Step::Asking(s.write(&project, ask_write(chat, &pending, now)), pending);
    }
}

/// The write that keeps `pending` as a new turn of `chat`.
fn ask_write(chat: &Chat, pending: &Pending, now: u64) -> Write {
    Write::Ask {
        chat: chat.id,
        turn: pending.turn,
        question: pending.question,
        text: pending.text.clone(),
        chips: pending.chips.clone(),
        model: chat.named.clone().unwrap_or_else(|| chat.model.clone()),
        allow_changes: chat.allow_changes,
        now,
    }
}

/// Sends the last turn again in place, with `edited` as its question when it was edited (owner
/// decision): only a failed turn that changed nothing, and not before a rate limit's reset. While
/// the question is back in the composer, what the composer holds is the question sent. The question
/// always goes with its chips (an edited one with the chips now), so the project checks it against
/// the kept keys again and the agent gets its context line again. Refused while Let Chat use the
/// model is off.
pub fn retry(shell: &mut Shell, edited: Option<String>) {
    if refused_while_off(shell) {
        return;
    }
    let removed = shell.chat_panel.chat.as_ref().map(|c| c.removed.clone()).unwrap_or_default();
    let chips_now: Vec<Anchor> = chips(shell).into_iter().filter(|c| !removed.contains(&c.chip)).flat_map(|c| c.anchors).collect();
    let store = store_of(shell);
    let now = clock(shell).now;
    let draft = shell.chat_panel.draft.trim().to_string();
    let (Some(project), Some(s)) = (shell.project.as_ref().map(|p| p.path.clone()), store) else {
        shell.chat_panel.notice = Some(tr("This project keeps no conversations"));
        return;
    };
    let Some(chat) = shell.chat_panel.chat.as_mut() else { return };
    if chat.step.busy() {
        return;
    }
    let model = chat.named.clone().unwrap_or_else(|| chat.model.clone());
    let allow = chat.allow_changes;
    let Some(last) = chat.turns.last() else { return };
    if !retryable(last, now) {
        return;
    }
    let edited = edited.or_else(|| (last.returned && !draft.is_empty()).then_some(draft));
    if edited.as_ref().is_some_and(|t| t.len() > QUESTION_BYTES) {
        let kib = edited.as_ref().map_or(0, |t| t.len() / 1024).to_string();
        shell.chat_panel.notice = Some(fmt(&tr("A question is at most 64 KiB; this one is {n} KiB."), &[("n", &kib)]));
        return;
    }
    let from_draft = edited.is_some();
    let text = edited.unwrap_or_else(|| last.question.clone());
    let chips = if from_draft { chips_now } else { last.chips.clone() };
    let context = context_line(&chips);
    let pending = Pending { text: text.clone(), chips: chips.clone(), context, turn: last.id, question: last.question_id, from_draft };
    shell.chat_panel.notice = None;
    let rx = s.write(&project, Write::Rerun { turn: last.id, model, allow_changes: allow, question: Some((text, chips)), now });
    chat.step = Step::Rerunning(rx, pending);
}

/// Whether `turn` can be sent again now: it failed, changed nothing, and its provider's wait is over.
pub fn retryable(turn: &Turn, now: u64) -> bool {
    match &turn.state {
        TurnState::Failed(f) => turn.change.is_none() && f.reset.is_none_or(|at| now >= at),
        _ => false,
    }
}

/// The project took the question, or refused it: the turn starts, or the draft stays and the panel
/// says why.
fn asked(shell: &mut Shell, answer: Result<Answer, Refusal>, pending: Pending, rerun: bool) {
    let label_turn = |n: u32| fmt(&tr("Turn {n}"), &[("n", &n.to_string())]);
    match answer {
        Err(r) => {
            shell.chat_panel.notice = Some(refusal_text(&r));
        }
        Ok(answer) => {
            let Some(chat) = shell.chat_panel.chat.as_mut() else { return };
            let number = if rerun {
                let Some(t) = chat.turns.last_mut() else { return };
                t.state = TurnState::Running;
                t.question = pending.text.clone();
                t.chips = pending.chips.clone();
                // What belonged to the failed attempt goes: its stop, its cut, its streaming
                // layout. Its rows and reply stay in the timeline as what it did.
                t.returned = false;
                t.unsent = false;
                t.stopping = false;
                t.stopping_since = None;
                t.retrying = None;
                t.cut = false;
                t.streaming = None;
                t.laid = None;
                t.model = chat.named.clone().unwrap_or_else(|| chat.model.clone());
                t.number
            } else {
                let Answer::Asked { number } = answer else { return };
                chat.held = chat.held.saturating_add(pending.text.len());
                chat.turns.push(Turn {
                    id: pending.turn,
                    number,
                    question: pending.text.clone(),
                    question_id: pending.question,
                    chips: pending.chips.clone(),
                    returned: false,
                    items: Vec::new(),
                    streaming: None,
                    state: TurnState::Running,
                    model: chat.named.clone().unwrap_or_else(|| chat.model.clone()),
                    change: None,
                    stopping: false,
                    stopping_since: None,
                    cut: false,
                    rows_dropped: 0,
                    laid: None,
                    retrying: None,
                    unsent: false,
                });
                number
            };
            if chat.title.is_empty() {
                chat.title = pending.text.chars().take(200).collect();
            }
            if pending.from_draft && shell.chat_panel.draft.trim() == pending.text {
                shell.chat_panel.draft.clear();
            }
            shell.chat_panel.follow = true;
            let origin = chat.origin.clone().unwrap_or_default();
            if let Some(s) = shell.session.as_mut()
                && let Err(e) = s.begin_group(&origin, &label_turn(number))
            {
                log::warn!("the turn's changes cannot be grouped into one undo step: {e}");
            }
            deliver(shell, format!("{}{}", pending.text, pending.context));
        }
    }
}

/// Gives the agent `text`: its program starts first when it does not run, and the text waits
/// while it ends to start again. Let Chat use the model turned off while the question was being
/// kept: nothing is sent, and the turn ends unsent ([`not_sent`]).
fn deliver(shell: &mut Shell, text: String) {
    if refused_while_off(shell) {
        not_sent(shell);
        return;
    }
    let Some(chat) = shell.chat_panel.chat.as_mut() else { return };
    match chat.session {
        Session::Ending => {
            chat.queued = Some(text);
            return;
        }
        Session::Running => {}
        Session::Off => {
            let request = ChatStart {
                chat: chat.key.clone(),
                provider: chat.provider,
                model: chat.model.clone(),
                origin: chat.origin.clone().unwrap_or_default(),
                allow_changes: chat.allow_changes,
                resume: chat.resume,
                codex_sign_in: shell.state.agents.codex_sign_in,
            };
            match crate::chat::start(shell, request) {
                Ok(()) => {
                    if let Some(c) = shell.chat_panel.chat.as_mut() {
                        c.session = Session::Running;
                        c.resume = true;
                    }
                }
                Err(e) => {
                    log::warn!("the conversation could not start: {e}");
                    end_here(shell, TurnState::Failed(ChatFailure { kind: FailureKind::Other, detail: e.to_string(), reset: None }));
                    return;
                }
            }
        }
    }
    let key = shell.chat_panel.chat.as_ref().map(|c| c.key.clone()).unwrap_or_default();
    if let Err(e) = crate::chat::send(shell, &key, text) {
        log::warn!("the question could not reach the conversation: {e}");
        // The program is gone: the next question starts it again.
        if let Some(c) = shell.chat_panel.chat.as_mut() {
            c.session = Session::Off;
        }
        end_here(shell, TurnState::Failed(ChatFailure { kind: FailureKind::Other, detail: e.to_string(), reset: None }));
    }
}

/// Ends the running turn of the panel shown unsent, as Let Chat use the model went off while its
/// question was being kept: failed, so Send again sends it once the switch is on again. Its
/// question goes back into an empty box, its bubble hidden, as after a lost connection; when the
/// box holds something else, that stays, and the question stays in its bubble with Send again.
fn not_sent(shell: &mut Shell) {
    let empty = shell.chat_panel.draft.trim().is_empty();
    let failure = ChatFailure { kind: FailureKind::Other, detail: "Let Chat use the model is off".into(), reset: None };
    end_here(shell, TurnState::Failed(failure));
    let Some(turn) = shell.chat_panel.chat.as_mut().and_then(|c| c.turns.last_mut()) else { return };
    turn.unsent = true;
    if empty && turn.change.is_none() {
        turn.returned = true;
        shell.chat_panel.draft = turn.question.clone();
    }
}

/// Ends the running turn of `chat` as `state`: the reply so far is kept (cut off unless it
/// completed), the turn's changes are taken from its undo group, a retry it showed is over, and the
/// turn is written.
pub(crate) fn finish(chat: &mut Chat, session: Option<&mut archforge_notations::Session>, out: &Out, state: TurnState) -> bool {
    let origin = chat.origin.clone().unwrap_or_default();
    let Some(turn) = chat.turns.last_mut().filter(|t| t.state == TurnState::Running) else { return false };
    let complete = state == TurnState::Completed;
    let answered = turn.answered();
    let mut reply: Option<(String, bool, bool)> = None;
    if let Some((at, text)) = turn.streaming.take()
        && !text.is_empty()
    {
        let shortened = text.len() >= REPLY_BYTES || turn.cut;
        let at = at.min(turn.items.len());
        turn.items.insert(at, Item::Text { text: text.clone(), complete, shortened, laid: None });
        reply = Some((text, complete, shortened));
    }
    let mut changes: Option<Changes> = None;
    if let Some(s) = session
        && let Some(group) = s.end_group(&origin)
    {
        let (elements, changed) = group.deltas.as_deref().map(|d| changed_elements(s.doc(), d)).unwrap_or((0, Vec::new()));
        changes = Some(Changes { changes: group.changes, elements, changed });
        turn.change = Some(Change {
            changes: group.changes,
            state: ChangeState::Applied,
            span: Some(Span { before: group.before, after: group.after, steps: group.steps }),
        });
    }
    let (outcome, error) = match &state {
        TurnState::Completed => (Outcome::Completed, None),
        TurnState::Stopped => (Outcome::Stopped, None),
        TurnState::Interrupted => (Outcome::Interrupted, None),
        TurnState::Failed(f) => (Outcome::Failed, Some(f.kind)),
        TurnState::Running => (Outcome::Running, None),
    };
    // After a lost connection before any answer, the question goes back to the composer when it is
    // empty, ready to send again; a turn that changed the model is not sent again, so its question
    // stays where it is.
    if let TurnState::Failed(f) = &state
        && f.kind == FailureKind::Network
        && out.draft_empty
        && !answered
        && turn.change.is_none()
    {
        turn.returned = true;
    }
    turn.state = state;
    turn.stopping = false;
    turn.stopping_since = None;
    turn.retrying = None;
    let turn_id = turn.id;
    if let Some((text, complete, shortened)) = reply
        && let Some(id) = out.id()
    {
        out.write(chat, Write::Reply { id, turn: turn_id, text, complete, shortened, now: out.clock.now });
    }
    out.write(chat, Write::End { turn: turn_id, outcome, error: error.map(stored_kind), changes, now: out.clock.now });
    true
}

/// The kind of failure as the project keeps it: one with no key saved is kept as a refused key,
/// both send the person to Settings.
fn stored_kind(kind: FailureKind) -> FailureKind {
    if kind == FailureKind::NoCredential { FailureKind::InvalidCredential } else { kind }
}

/// The elements a turn's changes touched over the turn's undo steps (`deltas`, each step's own, so
/// nothing of another's change between them), those a person reads about only: how many in all,
/// and each with what happened to it. An element added and removed within the turn is left out;
/// one changed and changed back is still changed, as values are not compared.
pub fn changed_elements(store: &Store, deltas: &[archforge_mof::Delta]) -> (u32, Vec<ChangedElement>) {
    use archforge_mof::Op;
    let mut seen: BTreeMap<Uuid, (bool, bool, Obj)> = BTreeMap::new();
    for op in deltas.iter().flat_map(|d| d.0.iter()) {
        let (obj, created, destroyed) = match op {
            Op::Create { obj, .. } => (*obj, true, false),
            Op::Destroy { obj, .. } => (*obj, false, true),
            Op::Set { obj, .. } | Op::Insert { obj, .. } | Op::Remove { obj, .. } | Op::Move { obj, .. } | Op::Reorder { obj, .. } => {
                (*obj, false, false)
            }
        };
        let Ok(id) = store.uuid(obj) else { continue };
        let entry = seen.entry(id).or_insert((false, false, obj));
        entry.0 |= created;
        entry.1 |= destroyed;
    }
    let mut out = Vec::new();
    for (id, (created, destroyed, obj)) in seen {
        let kind = match (created, destroyed) {
            (true, true) => continue,
            (true, false) => ChangeKind::Added,
            (false, true) => ChangeKind::Removed,
            (false, false) => ChangeKind::Changed,
        };
        let Some(metaclass) = metaclass(store, obj).filter(|m| crate::chat_words::counted(m)) else { continue };
        let name = Some(archforge_core::name_of(store, obj)).filter(|n| !n.is_empty());
        out.push(ChangedElement { element: id, kind, metaclass: metaclass.to_string(), name });
    }
    (u32::try_from(out.len()).unwrap_or(u32::MAX), out)
}

/// Stops the running turn: the program stops it and says so; when it cannot be reached, the turn
/// ends here as stopped.
pub fn stop(shell: &mut Shell) {
    let now = clock(shell).now;
    let Some(chat) = shell.chat_panel.chat.as_mut() else { return };
    let key = chat.key.clone();
    let Some(turn) = chat.running() else { return };
    if turn.stopping {
        return;
    }
    turn.stopping = true;
    turn.stopping_since = Some(now);
    if let Err(e) = crate::chat::stop(shell, &key) {
        log::warn!("the turn could not be stopped by its program ({e}); it ends here");
        end_here(shell, TurnState::Stopped);
    }
}

/// Ends the running turn of the panel shown as `state`, without the program.
pub(crate) fn end_here(shell: &mut Shell, state: TurnState) {
    let out = Out::shown(shell);
    let mut session = shell.session.take();
    if let Some(c) = shell.chat_panel.chat.as_mut() {
        finish(c, session.as_mut(), &out, state);
    }
    shell.session = session;
}

// The conversation's own controls.

/// Turns Allow changes to the model on or off, for the next question: an idle program ends, to
/// start again with the access it now gives. Refused while a turn runs.
pub fn set_allow_changes(shell: &mut Shell, on: bool) {
    let Some(chat) = shell.chat_panel.chat.as_mut() else { return };
    if chat.busy() || chat.allow_changes == on {
        return;
    }
    chat.allow_changes = on;
    restart(shell);
}

/// Changes the conversation's provider (only before its first question) and model, from its badge.
pub fn choose(shell: &mut Shell, provider: Provider, model: &str) {
    let out = Out::shown(shell);
    let Some(chat) = shell.chat_panel.chat.as_mut() else { return };
    if chat.busy() || (provider != chat.provider && !chat.turns.is_empty()) {
        return;
    }
    chat.chosen = true;
    if chat.provider == provider && chat.model == model {
        return;
    }
    chat.provider = provider;
    chat.model = model.to_string();
    chat.named = None;
    if chat.origin.is_some() {
        out.write(chat, Write::Model { chat: chat.id, model: model.to_string(), now: out.clock.now });
    }
    restart(shell);
}

/// Ends the conversation's program when it runs idle, so the next question starts it again with
/// the switch and model now set.
fn restart(shell: &mut Shell) {
    let Some(chat) = shell.chat_panel.chat.as_mut() else { return };
    if chat.session != Session::Running {
        return;
    }
    let key = chat.key.clone();
    chat.session = Session::Ending;
    if crate::chat::end(shell, &key).is_err()
        && let Some(c) = shell.chat_panel.chat.as_mut()
    {
        c.session = Session::Off;
    }
}

/// Starts a new conversation (B2): the one shown ends its program. Refused while a turn runs.
pub fn new_conversation(shell: &mut Shell) {
    if shell.chat_panel.chat.as_ref().is_some_and(Chat::busy) {
        return;
    }
    retire(shell);
    shell.chat_panel.view = View::Conversation;
    shell.chat_panel.notice = None;
    showing(shell);
}

/// Lets the conversation shown go: its program ends, and its last events are taken and dropped.
pub(crate) fn retire(shell: &mut Shell) {
    let Some(old) = shell.chat_panel.chat.take() else { return };
    let_go(shell, old.key, old.origin, old.session != Session::Off);
}

/// Ends the program of the conversation `key` when `running`, and keeps taking and dropping its
/// events until it has ended, whatever becomes of its panel; meanwhile requests on behalf of its
/// `origin` are refused.
fn let_go(shell: &mut Shell, key: String, origin: Option<String>, running: bool) {
    let ended = running && crate::chat::end(shell, &key).is_ok();
    if ended || shell.chats.running(&key) {
        shell.chats.letting_go.push((key, origin));
    }
}

/// Opens or closes the history: the project's conversations are read each time it opens.
pub fn toggle_history(shell: &mut Shell) {
    if shell.chat_panel.chat.as_ref().is_some_and(Chat::busy) {
        return;
    }
    if shell.chat_panel.view == View::History {
        shell.chat_panel.view = View::Conversation;
        return;
    }
    shell.chat_panel.view = View::History;
    shell.chat_panel.load_error = None;
    if let (Some(store), Some(project)) = (store_of(shell), shell.project.as_ref()) {
        shell.chat_panel.listing = Some(store.list(&project.path));
    } else {
        shell.chat_panel.history = Some(Err(tr("This project keeps no conversations")));
    }
}

/// Opens a conversation from the history, read back whole; it opens read-only.
pub fn open(shell: &mut Shell, id: Uuid) {
    if shell.chat_panel.chat.as_ref().is_some_and(Chat::busy) {
        return;
    }
    if shell.chat_panel.chat.as_ref().is_some_and(|c| c.id == id) {
        shell.chat_panel.view = View::Conversation;
        return;
    }
    if let (Some(store), Some(project)) = (store_of(shell), shell.project.as_ref()) {
        shell.chat_panel.loading = Some((id, store.load(&project.path, id)));
    } else {
        shell.chat_panel.load_error = Some(tr("This project keeps no conversations"));
    }
}

/// A conversation read back from the project, as the panel holds it.
fn from_stored(stored: Stored, spans: &BTreeMap<Uuid, Span>) -> Chat {
    let row = stored.row;
    let mut chat = Chat::new(row.id, row.provider, &row.model);
    chat.origin = Some(row.origin);
    chat.title = row.title;
    chat.chosen = true;
    chat.resume = true;
    let mut at: BTreeMap<Uuid, usize> = BTreeMap::new();
    for t in stored.turns {
        let state = match t.outcome {
            Outcome::Running | Outcome::Interrupted => TurnState::Interrupted,
            Outcome::Completed => TurnState::Completed,
            Outcome::Stopped => TurnState::Stopped,
            Outcome::Failed => TurnState::Failed(ChatFailure { kind: t.error.unwrap_or(FailureKind::Other), detail: String::new(), reset: None }),
        };
        let change = t.change.map(|c| Change { changes: c.changes, state: c.state, span: spans.get(&t.id).copied() });
        at.insert(t.id, chat.turns.len());
        chat.turns.push(Turn {
            id: t.id,
            number: t.number,
            question: String::new(),
            question_id: t.id,
            chips: Vec::new(),
            returned: false,
            items: Vec::new(),
            streaming: None,
            state,
            model: t.model,
            change,
            stopping: false,
            stopping_since: None,
            cut: false,
            rows_dropped: 0,
            laid: None,
            retrying: None,
            unsent: false,
        });
    }
    for m in stored.messages {
        let Some(turn) = at.get(&m.turn).and_then(|i| chat.turns.get_mut(*i)) else { continue };
        match m.body {
            StoredBody::Question { text, chips } => {
                chat.held = chat.held.saturating_add(text.len());
                turn.question = text;
                turn.question_id = m.id;
                turn.chips = chips;
            }
            StoredBody::Reply { text, complete, shortened } => {
                chat.held = chat.held.saturating_add(text.len());
                turn.items.push(Item::Text { text, complete, shortened, laid: None });
            }
            StoredBody::Tool(facts) => turn.items.push(Item::Tool(facts)),
        }
    }
    chat
}

// Once per frame.

/// Once per frame: takes in the agent's events for every project's conversation, the project's
/// answers, and the moves of undo and redo over the turns of the project shown.
pub(crate) fn frame(shell: &mut Shell) {
    // The conversations let go of: their events are dropped until they have ended.
    let letting_go = std::mem::take(&mut shell.chats.letting_go);
    for (key, origin) in letting_go {
        let ended = crate::chat::take_events(shell, &key).iter().any(|e| matches!(e, ChatEvent::Ended { .. }));
        if !ended && shell.chats.running(&key) {
            shell.chats.letting_go.push((key, origin));
        }
    }
    // A Stop the program has not answered within the grace ends the program, and the turn here.
    let now = clock(shell).now;
    let overdue = shell.chat_panel.chat.as_ref().and_then(|c| {
        c.turns
            .last()
            .filter(|t| t.state == TurnState::Running && t.stopping_since.is_some_and(|at| now >= at.saturating_add(STOP_GRACE)))
            .map(|_| c.key.clone())
    });
    if let Some(key) = overdue {
        log::warn!("the conversation {key:?} did not stop its turn within {STOP_GRACE} s; its program ends");
        if crate::chat::end(shell, &key).is_ok()
            && let Some(c) = shell.chat_panel.chat.as_mut()
        {
            c.session = Session::Ending;
        }
        end_here(shell, TurnState::Stopped);
    }
    // So for a parked project's turn, stopped when its tab was left: it ends against its own
    // project and model.
    for i in 0..shell.tabs.len() {
        let overdue = shell.tabs.get(i).and_then(|t| t.parked.as_ref()).and_then(|p| p.chat_panel.chat.as_ref()).and_then(|c| {
            c.turns
                .last()
                .filter(|t| t.state == TurnState::Running && t.stopping_since.is_some_and(|at| now >= at.saturating_add(STOP_GRACE)))
                .map(|_| c.key.clone())
        });
        let Some(key) = overdue else { continue };
        log::warn!("the parked conversation {key:?} did not stop its turn within {STOP_GRACE} s; its program ends");
        let ended = crate::chat::end(shell, &key).is_ok();
        let Some(tab) = shell.tabs.get(i) else { continue };
        let out = Out::of(shell, tab.path.clone(), false);
        let Some(parked) = shell.tabs.get_mut(i).and_then(|t| t.parked.as_mut()) else { continue };
        let crate::Parked { chat_panel: panel, session, .. } = &mut **parked;
        if let Some(chat) = panel.chat.as_mut() {
            if ended {
                chat.session = Session::Ending;
            }
            finish(chat, session.as_mut(), &out, TurnState::Stopped);
        }
    }
    let shown = shell.active;
    for i in 0..shell.tabs.len() {
        if Some(i) == shown {
            events_shown(shell);
        } else {
            events_parked(shell, i);
        }
    }
    answers(shell);
    track_undo(shell);
}

/// The events of the conversation shown.
fn events_shown(shell: &mut Shell) {
    let Some(key) = shell.chat_panel.chat.as_ref().map(|c| c.key.clone()) else { return };
    let events = crate::chat::take_events(shell, &key);
    if events.is_empty() {
        return;
    }
    let out = Out::shown(shell);
    let mut session = shell.session.take();
    let mut restart_with: Option<String> = None;
    if let Some(chat) = shell.chat_panel.chat.as_mut() {
        for e in events {
            if let Some(text) = event(chat, e, session.as_mut(), &out) {
                restart_with = Some(text);
            }
        }
    }
    shell.session = session;
    shell.chat_panel.follow |= shell.chat_panel.at_end;
    // A question that went back to the composer is in it, ready to send again.
    if shell.chat_panel.draft.trim().is_empty()
        && let Some(t) = shell.chat_panel.chat.as_ref().and_then(|c| c.turns.last()).filter(|t| t.returned)
    {
        shell.chat_panel.draft = t.question.clone();
    }
    if let Some(text) = restart_with {
        deliver(shell, text);
    }
}

/// The events of tab `i`'s parked conversation: its turn was stopped when the tab was left, and its
/// end is written to its own project.
fn events_parked(shell: &mut Shell, i: usize) {
    let Some(key) = shell.tabs.get(i).and_then(|t| t.parked.as_ref()).and_then(|p| p.chat_panel.chat.as_ref()).map(|c| c.key.clone()) else { return };
    let events = crate::chat::take_events(shell, &key);
    if events.is_empty() {
        return;
    }
    let Some(tab) = shell.tabs.get(i) else { return };
    let draft_empty = tab.parked.as_ref().is_some_and(|p| p.chat_panel.draft.trim().is_empty());
    let out = Out::of(shell, tab.path.clone(), draft_empty);
    let Some(parked) = shell.tabs.get_mut(i).and_then(|t| t.parked.as_mut()) else { return };
    let crate::Parked { chat_panel: panel, session, .. } = &mut **parked;
    if let Some(chat) = panel.chat.as_mut() {
        for e in events {
            // A parked conversation waits for its tab: nothing is sent while it is hidden.
            let _ = event(chat, e, session.as_mut(), &out);
        }
        chat.queued = None;
        if panel.draft.trim().is_empty()
            && let Some(t) = chat.turns.last().filter(|t| t.returned)
        {
            panel.draft = t.question.clone();
        }
    }
}

/// Takes one event of `chat`; gives the text to send when the program ended to start again. A
/// retry the program reports is kept on the running turn until anything else comes of it.
fn event(chat: &mut Chat, e: ChatEvent, session: Option<&mut archforge_notations::Session>, out: &Out) -> Option<String> {
    if let Some(turn) = chat.turns.last_mut().filter(|t| t.state == TurnState::Running) {
        match &e {
            ChatEvent::Retrying { attempt, max } => turn.retrying = Some((*attempt, *max)),
            // The program starting says nothing of the provider.
            ChatEvent::Started { .. } => {}
            _ => turn.retrying = None,
        }
    }
    match e {
        ChatEvent::Started { model, .. } => {
            if !model.trim().is_empty() && model != "default" {
                let named: String = model.chars().filter(|c| !c.is_control()).take(128).collect();
                // The running turn is answered by the model the program named.
                if let Some(turn) = chat.turns.last_mut().filter(|t| t.state == TurnState::Running) {
                    turn.model = named.clone();
                }
                chat.named = Some(named);
            }
        }
        ChatEvent::Text { delta } => {
            let room = MAX_HELD.saturating_sub(chat.held);
            let held = &mut chat.held;
            if let Some(turn) = chat.turns.last_mut().filter(|t| t.state == TurnState::Running) {
                let at = turn.items.len();
                let (_, text) = turn.streaming.get_or_insert_with(|| (at, String::new()));
                let take = cut(&delta, REPLY_BYTES.saturating_sub(text.len()).min(room));
                text.push_str(take);
                if take.len() < delta.len() && !turn.cut {
                    turn.cut = true;
                    log::warn!("a reply passed what the conversation may hold; the rest is not kept");
                }
                *held = held.saturating_add(take.len());
                turn.laid = None;
            }
        }
        ChatEvent::Message { text } => {
            let room = MAX_HELD.saturating_sub(chat.held);
            let mut reply = None;
            if let Some(turn) = chat.turns.last_mut().filter(|t| t.state == TurnState::Running) {
                let (at, before) = turn.streaming.take().unwrap_or((turn.items.len(), String::new()));
                let kept = cut(&text, REPLY_BYTES.min(room.saturating_add(before.len())));
                let shortened = kept.len() < text.len();
                let at = at.min(turn.items.len());
                turn.items.insert(at, Item::Text { text: kept.to_string(), complete: true, shortened, laid: None });
                chat.held = chat.held.saturating_sub(before.len()).saturating_add(kept.len());
                turn.laid = None;
                reply = Some((turn.id, kept.to_string(), shortened));
            }
            if let Some((turn, text, shortened)) = reply
                && let Some(id) = out.id()
            {
                out.write(chat, Write::Reply { id, turn, text, complete: true, shortened, now: out.clock.now });
            }
        }
        ChatEvent::ToolStarted { .. } | ChatEvent::ToolFinished { .. } | ChatEvent::Retrying { .. } => {}
        ChatEvent::TurnEnded(end) => {
            let state = match end {
                TurnEnd::Completed => TurnState::Completed,
                TurnEnd::Stopped => TurnState::Stopped,
                TurnEnd::Failed(f) => TurnState::Failed(f),
            };
            finish(chat, session, out, state);
        }
        ChatEvent::Ended { failure } => {
            let was = chat.session;
            chat.session = Session::Off;
            // A program that ended during a turn ends the turn: with its failure, or as stopped.
            let state = match failure {
                Some(f) => TurnState::Failed(f),
                None => TurnState::Stopped,
            };
            let running = chat.turns.last().is_some_and(|t| t.state == TurnState::Running);
            if running {
                if let Some(text) = chat.queued.take()
                    && was == Session::Ending
                {
                    return Some(text);
                }
                finish(chat, session, out, state);
            }
        }
    }
    None
}

/// Takes the project's answers: the step under way, the writes, the history and a conversation
/// read back.
fn answers(shell: &mut Shell) {
    // The step of the conversation shown.
    let step = shell.chat_panel.chat.as_mut().map(|c| std::mem::take(&mut c.step));
    match step {
        Some(Step::Creating(rx, pending)) => match rx.try_recv() {
            Ok(Ok(Answer::Created { origin })) => {
                let project = shell.project.as_ref().map(|p| p.path.clone()).unwrap_or_default();
                let store = store_of(shell);
                let now = clock(shell).now;
                if let (Some(chat), Some(s)) = (shell.chat_panel.chat.as_mut(), store) {
                    chat.origin = Some(origin);
                    chat.step = Step::Asking(s.write(&project, ask_write(chat, &pending, now)), pending);
                }
            }
            Ok(Ok(_)) => shell.chat_panel.notice = Some(refusal_text(&Refusal::Failed(tr("the project gave no label for the conversation")))),
            Ok(Err(r)) => shell.chat_panel.notice = Some(refusal_text(&r)),
            Err(TryRecvError::Empty) => {
                if let Some(c) = shell.chat_panel.chat.as_mut() {
                    c.step = Step::Creating(rx, pending);
                }
            }
            Err(TryRecvError::Disconnected) => shell.chat_panel.notice = Some(refusal_text(&Refusal::Failed(tr("the project did not answer")))),
        },
        Some(Step::Asking(rx, pending)) => match rx.try_recv() {
            Ok(answer) => asked(shell, answer, pending, false),
            Err(TryRecvError::Empty) => {
                if let Some(c) = shell.chat_panel.chat.as_mut() {
                    c.step = Step::Asking(rx, pending);
                }
            }
            Err(TryRecvError::Disconnected) => shell.chat_panel.notice = Some(refusal_text(&Refusal::Failed(tr("the project did not answer")))),
        },
        Some(Step::Rerunning(rx, pending)) => match rx.try_recv() {
            Ok(answer) => asked(shell, answer, pending, true),
            Err(TryRecvError::Empty) => {
                if let Some(c) = shell.chat_panel.chat.as_mut() {
                    c.step = Step::Rerunning(rx, pending);
                }
            }
            Err(TryRecvError::Disconnected) => shell.chat_panel.notice = Some(refusal_text(&Refusal::Failed(tr("the project did not answer")))),
        },
        Some(Step::Idle) | None => {}
    }
    // The writes of every project's conversation.
    let take = |chat: &mut Chat| {
        chat.writes.retain(|rx| match rx.try_recv() {
            Ok(Ok(_)) => false,
            Ok(Err(r)) => {
                log::warn!("a write to the conversation was refused: {}", refusal_text(&r));
                chat.save_error = Some(refusal_text(&r));
                false
            }
            Err(TryRecvError::Empty) => true,
            Err(TryRecvError::Disconnected) => {
                chat.save_error = Some(refusal_text(&Refusal::Failed(tr("the project did not answer"))));
                false
            }
        });
    };
    if let Some(c) = shell.chat_panel.chat.as_mut() {
        take(c);
    }
    for tab in shell.tabs.iter_mut() {
        if let Some(c) = tab.parked.as_mut().and_then(|p| p.chat_panel.chat.as_mut()) {
            take(c);
        }
    }
    // The history. A list that cannot be read again keeps the rows read before shown, and turns
    // their changes off with the reason (AR-217).
    if let Some(rx) = &shell.chat_panel.listing {
        match rx.try_recv() {
            Ok(Err(why)) if matches!(shell.chat_panel.history, Some(Ok(_))) => {
                log::warn!("the conversations could not be listed again: {why}");
                shell.chat_panel.writing_off = Some(why);
                shell.chat_panel.listing = None;
            }
            Ok(list) => {
                shell.chat_panel.writing_off = None;
                shell.chat_panel.history = Some(list);
                shell.chat_panel.listing = None;
            }
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => {
                shell.chat_panel.history = Some(Err(tr("the project did not answer")));
                shell.chat_panel.listing = None;
            }
        }
    }
    if let Some((id, rx)) = shell.chat_panel.loading.take() {
        match rx.try_recv() {
            // A conversation that got busy meanwhile is not let go of: the opening is dropped.
            Ok(Ok(_)) if shell.chat_panel.chat.as_ref().is_some_and(Chat::busy) => {}
            Ok(Ok(stored)) => {
                retire(shell);
                let mut chat = from_stored(stored, &shell.chat_panel.spans);
                // Its program let go of and still ending: taken back, so a question waits for the
                // end and starts it again, and its last events are its own again.
                if let Some(at) = shell.chats.letting_go.iter().position(|(k, _)| *k == chat.key) {
                    shell.chats.letting_go.remove(at);
                    chat.session = Session::Ending;
                }
                shell.chat_panel.chat = Some(chat);
                if !std::mem::take(&mut shell.chat_panel.loading_stays) {
                    shell.chat_panel.view = View::Conversation;
                }
                shell.chat_panel.follow = true;
            }
            Ok(Err(why)) => {
                shell.chat_panel.loading_stays = false;
                shell.chat_panel.load_error = Some(fmt(&tr("This conversation cannot be read: {why}"), &[("why", &why)]));
            }
            Err(TryRecvError::Empty) => shell.chat_panel.loading = Some((id, rx)),
            Err(TryRecvError::Disconnected) => {
                shell.chat_panel.loading_stays = false;
                shell.chat_panel.load_error = Some(tr("the project did not answer"));
            }
        }
    }
}

/// Moves turns' changes between applied and undone as undo and redo bring the model of the project
/// shown before a turn's first change or back after its last, and writes the moves.
fn track_undo(shell: &mut Shell) {
    let Some(revision) = shell.session.as_ref().map(|s| s.revision()) else { return };
    let out = Out::shown(shell);
    let Some(chat) = shell.chat_panel.chat.as_mut() else { return };
    let mut moves = Vec::new();
    for turn in chat.turns.iter_mut() {
        let Some(change) = turn.change.as_mut() else { continue };
        let Some(span) = change.span.filter(|s| s.before != s.after) else { continue };
        let to = match change.state {
            ChangeState::Applied if revision == span.before => ChangeState::Undone,
            ChangeState::Undone if revision == span.after => ChangeState::Applied,
            _ => continue,
        };
        change.state = to;
        moves.push((turn.id, to));
    }
    for turn in chat.turns.iter() {
        if let Some(span) = turn.change.as_ref().and_then(|c| c.span) {
            shell.chat_panel.spans.insert(turn.id, span);
        }
    }
    if !moves.is_empty() {
        out.write(chat, Write::States { moves, now: out.clock.now });
    }
}

/// Why "Undo this turn's changes" cannot run on `turn` of the conversation labelled `origin` now,
/// or none when it can: its changes must be the step on top of the project's undo history, made in
/// this window, in one step. Empty when the changes are not applied (the card says so itself). The
/// reason names what happened: the model changed during the turn (its changes are several steps),
/// after it (the model is no longer at the turn's end), or the history no longer holds its step.
pub fn undo_refusal(shell: &Shell, origin: Option<&str>, turn: &Turn) -> Option<String> {
    let change = turn.change.as_ref()?;
    if change.state != ChangeState::Applied {
        return Some(String::new());
    }
    let Some(span) = change.span else {
        return Some(tr("The undo history ends when the project closes, so these changes can no longer be undone here."));
    };
    let gone = || Some(tr("The undo history no longer holds this turn's changes, so they can no longer be undone here."));
    let Some(s) = shell.session.as_ref() else { return gone() };
    let at_end = s.revision() == span.after;
    if span.steps == 1 && at_end && s.undo_revision() == Some(span.before) && origin.is_some() && s.undo_origin() == origin {
        return None;
    }
    if span.steps > 1 {
        Some(tr("The model changed during this turn, so its changes can no longer be undone on their own."))
    } else if !at_end {
        Some(tr("The model changed after this turn, so its changes can no longer be undone on their own."))
    } else {
        gone()
    }
}

/// Undoes `turn`'s changes: the one step of the undo history they are, while it is on top.
pub fn undo_turn(shell: &mut Shell, turn: Uuid) {
    let Some(chat) = shell.chat_panel.chat.as_ref() else { return };
    let ok = chat.turns.iter().find(|t| t.id == turn).is_some_and(|t| undo_refusal(shell, chat.origin.as_deref(), t).is_none());
    if !ok {
        return;
    }
    if let Some(s) = shell.session.as_mut()
        && let Err(e) = s.undo()
    {
        log::warn!("the turn's changes could not be undone: {e}");
    }
    track_undo(shell);
}

/// Whether the panel waits for the project: the window looks again soon while it does.
pub(crate) fn waiting(shell: &Shell) -> bool {
    let stopping = |c: &Chat| c.turns.last().is_some_and(|t| t.stopping);
    let busy = |p: &Panel| {
        p.chat.as_ref().is_some_and(|c| c.step.busy() || !c.writes.is_empty() || stopping(c)) || p.listing.is_some() || p.loading.is_some()
    };
    busy(&shell.chat_panel) || shell.tabs.iter().any(|t| t.parked.as_ref().is_some_and(|p| busy(&p.chat_panel)))
}

// Project tabs.

/// The tab shown is left: a running turn is stopped, as its control would now reach another
/// project.
pub(crate) fn leaving(shell: &mut Shell) {
    if shell.chat_panel.chat.as_ref().is_some_and(|c| c.turns.last().is_some_and(|t| t.state == TurnState::Running)) {
        stop(shell);
    }
    if let Some(c) = shell.chat_panel.chat.as_mut() {
        c.queued = None;
    }
    shell.chat_panel.menu = false;
    shell.chat_panel.link = None;
    // A deletion's confirmation and a row's menu close; its toast waits, and shows its whole time
    // again when the tab is shown again.
    shell.chat_panel.ask = None;
    shell.chat_panel.row_menu = None;
    if let Some(path) = shell.project.as_ref().map(|p| p.path.clone()) {
        for d in shell.chats.deletions.iter_mut().filter(|d| d.project == path) {
            d.until = None;
        }
    }
}

/// A project closes: its conversation's program ends, and the panel's conversation is let go of.
pub(crate) fn closing(shell: &mut Shell, chat: Option<(String, Option<String>)>) {
    if let Some((key, origin)) = chat {
        let running = shell.chats.running(&key);
        let_go(shell, key, origin, running);
    }
}

/// Whether a request of `method` with `params` sent on behalf of `origin` may run. An origin that
/// is one of this window's conversations may act only while that conversation's turn runs in the
/// project shown, never while a conversation of the same label still runs or ends elsewhere (labels
/// are only unique within a project), and may undo or redo only a step of the running turn: never
/// the person's, another conversation's, or one of its own earlier turns.
pub(crate) fn gate(shell: &Shell, origin: Option<&str>, method: &str, params: &Value) -> Result<(), String> {
    let Some(origin) = origin else { return Ok(()) };
    let mine = |p: &Panel| p.chat.as_ref().is_some_and(|c| c.origin.as_deref() == Some(origin));
    let parked = shell.tabs.iter().filter_map(|t| t.parked.as_ref()).filter(|p| mine(&p.chat_panel)).collect::<Vec<_>>();
    let ending = shell.chats.letting_go.iter().any(|(_, o)| o.as_deref() == Some(origin));
    let known = mine(&shell.chat_panel) || !parked.is_empty() || ending;
    if !known {
        return Ok(());
    }
    let running = |p: &Panel| {
        p.chat.as_ref().is_some_and(|c| c.origin.as_deref() == Some(origin) && c.turns.last().is_some_and(|t| t.state == TurnState::Running))
    };
    let elsewhere = ending || parked.iter().any(|p| running(&p.chat_panel));
    if !running(&shell.chat_panel) || elsewhere {
        return Err(format!("the conversation {origin:?} has no turn running in the project shown in Archforge"));
    }
    let command = params.get("command").and_then(Value::as_str);
    let undo = method == "engine.undo" || (method == "engine.execute" && command == Some("edit.undo"));
    let redo = method == "engine.redo" || (method == "engine.execute" && command == Some("edit.redo"));
    let session = shell.session.as_ref();
    if undo && session.is_none_or(|s| !s.undo_in_group(origin)) {
        return Err(format!("the conversation {origin:?} may undo only its own changes in this turn, and the step on top is not one"));
    }
    if redo && session.is_none_or(|s| !s.redo_in_group(origin)) {
        return Err(format!("the conversation {origin:?} may redo only its own changes in this turn, and the step to redo is not one"));
    }
    Ok(())
}

/// What a request was about before it ran: each step's command, the elements its parameters name
/// and the aspect it is about, taken while those elements still have their names.
pub(crate) struct Before {
    steps: Vec<StepBefore>,
}

struct StepBefore {
    command: Option<String>,
    targets: Vec<Anchor>,
    detail: Option<String>,
    query: bool,
    params: Value,
}

/// The ids a JSON value holds as strings, at most [`MAX_FOUND`], walking at most [`MAX_WALK`]
/// values.
fn ids_in(v: &Value) -> Vec<Uuid> {
    let mut out = Vec::new();
    let mut stack = vec![v];
    let mut walked = 0usize;
    while let Some(v) = stack.pop() {
        walked += 1;
        if walked > MAX_WALK || out.len() >= MAX_FOUND {
            break;
        }
        match v {
            Value::String(s) if s.len() == 36 => {
                if let Some(id) = Uuid::parse(s)
                    && !out.contains(&id)
                {
                    out.push(id);
                }
            }
            Value::Array(a) => stack.extend(a.iter().rev()),
            Value::Object(m) => stack.extend(m.values().rev()),
            _ => {}
        }
    }
    out
}

/// The aspect a call's parameters are about: a `property` named, else the first key that is
/// neither an element's id nor a list of them.
fn aspect(params: &Value) -> Option<String> {
    let m = params.as_object()?;
    if let Some(p) = m.get("property").and_then(Value::as_str) {
        return Some(p.chars().take(64).collect());
    }
    m.iter()
        .find(|(k, v)| ids_in(v).is_empty() && !matches!(k.as_str(), "coalesce" | "index" | "at" | "x" | "y" | "position"))
        .map(|(k, _)| k.chars().take(64).collect())
}

/// The aspect a command is about, by its name, when its parameters do not say it: a description
/// for the commands that describe.
fn aspect_of(command: Option<&str>, params: &Value) -> Option<String> {
    if command.is_some_and(|c| c.ends_with(".describe")) {
        return Some("description".into());
    }
    aspect(params)
}

/// What `method` with `params` is about, when its origin is the running conversation's.
pub(crate) fn before(shell: &Shell, method: &str, params: &Value, origin: Option<&str>) -> Option<Before> {
    let origin = origin?;
    let chat = shell.chat_panel.chat.as_ref().filter(|c| c.origin.as_deref() == Some(origin))?;
    chat.turns.last().filter(|t| t.state == TurnState::Running)?;
    let session = shell.session.as_ref();
    let step = |command: Option<&str>, p: &Value| -> StepBefore {
        let targets = match session {
            Some(s) => ids_in(p).into_iter().take(50).filter_map(|id| anchor(shell, s.doc(), id, AnchorRole::Target)).collect(),
            None => Vec::new(),
        };
        let query = command.and_then(|c| session.and_then(|s| s.registry().is_query(c))).unwrap_or(true);
        StepBefore { command: command.map(|c| c.chars().take(128).collect()), targets, detail: aspect_of(command, p), query, params: p.clone() }
    };
    let steps = match method {
        "engine.execute" => {
            let command = params.get("command").and_then(Value::as_str);
            vec![step(command, params.get("params").unwrap_or(&Value::Null))]
        }
        "engine.batch" => params
            .get("steps")
            .and_then(Value::as_array)
            .map(|steps| {
                steps
                    .iter()
                    .take(crate::control::MAX_STEPS)
                    .map(|s| step(s.get("command").and_then(Value::as_str), s.get("params").unwrap_or(&Value::Null)))
                    .collect()
            })
            .unwrap_or_default(),
        // What reads nothing of the model makes no row: the commands, the window's controls.
        "engine.commands" | "engine.describe" | "ui.inspect" | "app.saved" => return None,
        _ => vec![StepBefore {
            command: None,
            targets: Vec::new(),
            detail: None,
            query: !matches!(method, "engine.undo" | "engine.redo" | "app.save"),
            params: Value::Null,
        }],
    };
    Some(Before { steps })
}

/// The JSON text of `v`, at most [`CALL_BYTES`].
fn json_text(v: &Value) -> Option<String> {
    serde_json::to_string(v).ok().map(|s| cut(&s, CALL_BYTES).to_string())
}

/// Adds the rows of a request that ran on behalf of the running conversation, as the channel
/// answered it: done, refused (`refused`) or failed; each is kept with the turn.
pub(crate) fn after(shell: &mut Shell, method: &str, before: Before, answer: &Result<Value, String>, refused: bool) {
    let out = Out::shown(shell);
    let status = match (answer, refused) {
        (Ok(_), _) => ToolStatus::Done,
        (Err(_), true) => ToolStatus::Refused,
        (Err(_), false) => ToolStatus::Failed,
    };
    let results: Vec<Value> = match (method, answer) {
        ("engine.execute", Ok(v)) => vec![v.get("result").cloned().unwrap_or(Value::Null)],
        ("engine.batch", Ok(v)) => v.get("results").and_then(Value::as_array).cloned().unwrap_or_default(),
        (_, Ok(v)) => vec![v.clone()],
        _ => Vec::new(),
    };
    let mut rows = Vec::new();
    for (n, step) in before.steps.into_iter().enumerate() {
        let result = results.get(n);
        let changed = status == ToolStatus::Done && !step.query;
        let mut anchors = step.targets;
        // An addition is about what it added: its id in the answer.
        if changed
            && crate::chat_words::verb(step.command.as_deref(), true, ToolStatus::Done) == crate::chat_words::Verb::Added
            && let (Some(r), Some(s)) = (result, shell.session.as_ref())
        {
            let made = r.get("id").and_then(Value::as_str).and_then(Uuid::parse).and_then(|id| anchor(shell, s.doc(), id, AnchorRole::Target));
            if let Some(made) = made {
                anchors = vec![made];
            }
        }
        let mut count = None;
        if status == ToolStatus::Done
            && step.query
            && let (Some(r), Some(s)) = (result, shell.session.as_ref())
        {
            let about: BTreeSet<Uuid> = anchors.iter().map(|a| a.element).collect();
            let named: Vec<Anchor> = ids_in(r)
                .into_iter()
                .filter(|id| !about.contains(id))
                .filter_map(|id| anchor(shell, s.doc(), id, AnchorRole::Read))
                .filter(|a| crate::chat_words::counted(&a.metaclass))
                .collect();
            count = Some(u32::try_from(named.len()).unwrap_or(u32::MAX));
            // As many as the project keeps for a row; the count stays exact.
            anchors.extend(named.into_iter().take(READ_ANCHORS));
        }
        // What the rows hold counts against the conversation's memory; past it a row keeps its
        // count and what it was about, not every element its answer named.
        let size: usize = anchors.iter().map(|a| a.metaclass.len() + a.name.as_ref().map_or(0, String::len) + 48).sum();
        let held = shell.chat_panel.chat.as_ref().map_or(0, |c| c.held);
        if held.saturating_add(size) > MAX_HELD {
            anchors.retain(|a| a.role == AnchorRole::Target);
        }
        if let Some(c) = shell.chat_panel.chat.as_mut() {
            c.held = c.held.saturating_add(anchors.iter().map(|a| a.metaclass.len() + a.name.as_ref().map_or(0, String::len) + 48).sum());
        }
        let facts = ToolFacts {
            tool: method.to_string(),
            command: step.command,
            detail: step.detail,
            status,
            changed_model: changed,
            element_count: count,
            anchors,
        };
        let error = answer.as_ref().err().map(|e| cut(e, 1024).to_string());
        rows.push((facts, json_text(&step.params), result.and_then(json_text), error));
    }
    let Some(chat) = shell.chat_panel.chat.as_mut() else { return };
    let mut writes = Vec::new();
    if let Some(turn) = chat.turns.last_mut().filter(|t| t.state == TurnState::Running) {
        for (facts, arguments, result, error) in rows {
            if turn.items.len() >= MAX_ROWS {
                turn.rows_dropped = turn.rows_dropped.saturating_add(1);
                continue;
            }
            let Some(id) = out.id() else { continue };
            turn.items.push(Item::Tool(facts.clone()));
            writes.push(Write::Tool { id, turn: turn.id, facts, arguments, result, error, now: out.clock.now });
        }
    }
    for w in writes {
        out.write(chat, w);
    }
    shell.chat_panel.follow |= shell.chat_panel.at_end;
}
