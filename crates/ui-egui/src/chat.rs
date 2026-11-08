//! Chat's conversations as the shell knows them: what it asks the host to start, send, stop and end,
//! and what the host answers on one channel, with a repaint, while the conversations run.
//!
//! The shell runs no program and opens no port: the host does ([`crate::ai_agents::AgentHost`]),
//! starting each conversation's agent program on a thread of its own and giving it agent access
//! through a loopback control channel that is open only while a conversation holds it.
//! Conversations run only while Let Chat use the model is on, which [`start`] and [`send`] check.
//! The host says when access opens and closes ([`HostEvent::Access`]), and the status bar shows
//! it.
//!
//! Everything here is bounded: at most [`MAX_SESSIONS`] conversations are kept, each with at most
//! [`MAX_WAITING`] events and [`MAX_WAITING_BYTES`] of their text waiting to be taken; past either
//! an event is dropped and the drop said once in the log for that conversation. A conversation's end is never dropped. A conversation id is the
//! conversation's UUID in its lowercase text form, checked before anything is asked of the host, and
//! quoted in the log.

use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, Sender, TryRecvError};

use crate::ai_agents::Provider;

/// The most conversations the shell keeps, running or ended with events not yet taken.
pub const MAX_SESSIONS: usize = 64;
/// The most events one conversation keeps waiting to be taken.
pub const MAX_WAITING: usize = 4096;
/// The most bytes of text one conversation's waiting events hold.
pub const MAX_WAITING_BYTES: usize = 16 * 1024 * 1024;
/// The most conversations the host runs at once.
pub const MAX_RUNNING: usize = 8;
/// The most events taken from the host's channel in one frame; the rest wait for the next.
const PER_FRAME: usize = 8192;

/// What a conversation is started with. The id is the conversation's UUID (`8-4-4-4-12` lowercase
/// hexadecimal digits), which also names its private folder and, for Claude, its session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatStart {
    pub chat: String,
    pub provider: Provider,
    /// The model as the program takes it (`default` leaves the choice to the program).
    pub model: String,
    /// The label on every change the conversation makes (`Chat 1`).
    pub origin: String,
    /// Allow changes: without it the conversation's agent access only reads.
    pub allow_changes: bool,
    /// Whether it continues a stored conversation rather than starting a new one.
    pub resume: bool,
    /// How a Codex conversation signs in (Claude always uses its API key).
    pub codex_sign_in: crate::ai_agents::CodexSignIn,
}

/// Why the shell or the host refused a call about a conversation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChatRefusal {
    /// The id is not a conversation's UUID; `bytes` is its length, the id itself is not repeated.
    BadId { bytes: usize },
    /// A conversation with that id is running already.
    AlreadyRunning,
    /// No conversation with that id is running.
    NotRunning,
    /// As many conversations as allowed are kept or running: `most` of them.
    TooMany { most: usize },
    /// The host runs no conversations (none given, or a system agents do not run on yet).
    NotAvailable,
    /// Archforge is closing.
    Closing,
    /// Let Chat use the model is off: no conversation starts or is sent anything.
    ModelUseOff,
    /// The conversation's private folder is being removed, after a deletion of it (AR-218): it
    /// starts once that is done, so asking again shortly after succeeds.
    Forgetting,
    /// Anything else, in the host's words.
    Refused { why: String },
}

impl std::fmt::Display for ChatRefusal {
    /// Says what was refused, for the log; the conversation's text never appears.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChatRefusal::BadId { bytes } => write!(f, "the conversation id ({bytes} bytes) is not a lowercase UUID"),
            ChatRefusal::AlreadyRunning => f.write_str("the conversation is running already"),
            ChatRefusal::NotRunning => f.write_str("the conversation is not running"),
            ChatRefusal::TooMany { most } => write!(f, "at most {most} conversations run at once"),
            ChatRefusal::NotAvailable => f.write_str("conversations are not available here"),
            ChatRefusal::Closing => f.write_str("Archforge is closing"),
            ChatRefusal::ModelUseOff => f.write_str("Let Chat use the model is off"),
            ChatRefusal::Forgetting => f.write_str("the conversation's private folder is being removed; it starts once that is done"),
            ChatRefusal::Refused { why } => f.write_str(why),
        }
    }
}

/// Whether `id` is a conversation's UUID in its lowercase text form.
pub fn valid_id(id: &str) -> bool {
    let groups: Vec<&str> = id.split('-').collect();
    id.len() == 36
        && groups.len() == 5
        && groups.iter().zip([8, 4, 4, 4, 12]).all(|(g, n)| g.len() == n && g.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)))
}

/// Refuses an id that is not a conversation's UUID.
pub fn check_id(id: &str) -> Result<(), ChatRefusal> {
    if valid_id(id) { Ok(()) } else { Err(ChatRefusal::BadId { bytes: id.len() }) }
}

/// The neutral category of a failure, as the Chat panel shows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureKind {
    /// The program is not installed, too old, or could not be started.
    MissingRuntime,
    /// No key or sign-in to use.
    NoCredential,
    /// The key was refused.
    InvalidCredential,
    /// A sign-in that was valid has expired.
    SignInExpired,
    /// The provider's rate or usage limit.
    RateLimited,
    /// The provider could not be reached.
    Network,
    /// The model is unknown or not available now.
    ModelUnavailable,
    Other,
}

/// A failure: its category and the host's words (never a key), and for a rate limit the time the
/// provider said it may be asked again (seconds since 1970), when it said one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatFailure {
    pub kind: FailureKind,
    pub detail: String,
    pub reset: Option<u64>,
}

/// How a turn ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TurnEnd {
    Completed,
    Stopped,
    Failed(ChatFailure),
}

/// What a running conversation says, in the order it happens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChatEvent {
    /// The program is ready, with the model it resolved and whether it holds agent access.
    Started { model: String, access: bool },
    /// A piece of the answer being written.
    Text { delta: String },
    /// One whole message of the answer, replacing the pieces since the last one.
    Message { text: String },
    /// The model called a tool: its id, name and arguments (capped).
    ToolStarted { id: String, tool: String, input: String },
    /// A tool call ended.
    ToolFinished { id: String, ok: bool, summary: String },
    /// The provider did not answer for now (unreachable, overloaded or limiting the rate), and the
    /// program tries again: its attempt (0 when the program named none) and, when it said, how many
    /// it makes at most.
    Retrying { attempt: u32, max: Option<u32> },
    /// The turn ended.
    TurnEnded(TurnEnd),
    /// The conversation ended: its program stopped and its agent access was given back; `failure`
    /// says why when nobody asked for the end.
    Ended { failure: Option<ChatFailure> },
}

impl ChatEvent {
    /// The bytes of text the event holds, which count against [`MAX_WAITING_BYTES`].
    pub fn bytes(&self) -> usize {
        let failure = |f: &ChatFailure| f.detail.len();
        match self {
            ChatEvent::Started { model, .. } => model.len(),
            ChatEvent::Text { delta } => delta.len(),
            ChatEvent::Message { text } => text.len(),
            ChatEvent::ToolStarted { id, tool, input } => id.len().saturating_add(tool.len()).saturating_add(input.len()),
            ChatEvent::ToolFinished { id, summary, .. } => id.len().saturating_add(summary.len()),
            ChatEvent::Retrying { .. } | ChatEvent::TurnEnded(TurnEnd::Completed | TurnEnd::Stopped) => 0,
            ChatEvent::TurnEnded(TurnEnd::Failed(f)) => failure(f),
            ChatEvent::Ended { failure: f } => f.as_ref().map_or(0, failure),
        }
    }
}

/// What a conversation has waiting in the window, as the host counts it: events and their bytes.
#[derive(Debug, Default)]
pub struct Waiting {
    events: AtomicUsize,
    bytes: AtomicUsize,
}

/// A place among a conversation's waiting events, held from the host until the event is taken
/// (or dropped), so the host can count what waits and keep each conversation within
/// [`MAX_WAITING`] and [`MAX_WAITING_BYTES`] too.
pub struct Ticket(Option<(Arc<Waiting>, usize)>);

impl Ticket {
    /// A place for an event of `bytes` counted in `waiting`, or none when [`MAX_WAITING`] events or
    /// [`MAX_WAITING_BYTES`] already wait.
    pub fn take(waiting: &Arc<Waiting>, bytes: usize) -> Option<Ticket> {
        let counted = waiting.events.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| (n < MAX_WAITING).then(|| n.saturating_add(1)));
        counted.ok()?;
        let room =
            waiting.bytes.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_add(bytes).filter(|total| *total <= MAX_WAITING_BYTES));
        if room.is_err() {
            let _ = waiting.events.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| Some(n.saturating_sub(1)));
            return None;
        }
        Some(Ticket(Some((waiting.clone(), bytes))))
    }

    /// A ticket that counts nothing: for an event the host must never drop (a conversation's end).
    pub fn uncounted() -> Ticket {
        Ticket(None)
    }
}

impl Drop for Ticket {
    /// Gives the place and its bytes back.
    fn drop(&mut self) {
        if let Some((w, bytes)) = &self.0 {
            let _ = w.events.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| Some(n.saturating_sub(1)));
            let _ = w.bytes.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| Some(n.saturating_sub(*bytes)));
        }
    }
}

impl std::fmt::Debug for Ticket {
    /// Whether it is counted.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(if self.0.is_some() { "Ticket(counted)" } else { "Ticket(uncounted)" })
    }
}

/// What the host sends the shell.
#[derive(Debug)]
pub enum HostEvent {
    /// An event of the conversation `chat`, with its place among the waiting ones.
    Chat { chat: String, event: ChatEvent, ticket: Ticket },
    /// Agent access opened (`held`: a conversation holds it) or closed.
    Access { held: bool },
}

/// Where the host sends its events: the channel, and the wake that makes the window take them.
#[derive(Clone)]
pub struct EventSink {
    tx: Sender<HostEvent>,
    wake: Arc<dyn Fn() + Send + Sync>,
}

impl EventSink {
    /// A sink sending on `tx` and calling `wake` after each event.
    pub fn new(tx: Sender<HostEvent>, wake: Arc<dyn Fn() + Send + Sync>) -> EventSink {
        EventSink { tx, wake }
    }

    /// Sends `event` and wakes the window; false once the shell is gone.
    pub fn send(&self, event: HostEvent) -> bool {
        let sent = self.tx.send(event).is_ok();
        (self.wake)();
        sent
    }
}

impl std::fmt::Debug for EventSink {
    /// Names the sink; it holds nothing to show.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("EventSink")
    }
}

/// One conversation the shell keeps: its events waiting to be taken, whether it runs, and how many
/// events were dropped past the cap.
struct Kept {
    chat: String,
    events: VecDeque<(ChatEvent, Ticket)>,
    /// The bytes of text the waiting events hold.
    bytes: usize,
    running: bool,
    dropped: u64,
}

/// The conversations as this window knows them.
#[derive(Default)]
pub struct State {
    rx: Option<Receiver<HostEvent>>,
    kept: Vec<Kept>,
    /// Whether a conversation holds agent access now, as the host last said.
    access: bool,
    /// Let Chat use the model as last seen.
    seen: Option<bool>,
    /// The Chat conversations let go of (a new one started, another opened, their project closed),
    /// with their labels: their events are taken and dropped until each has ended, and requests on
    /// behalf of their labels are refused meanwhile.
    pub(crate) letting_go: Vec<(String, Option<String>)>,
    /// The conversations deleted whose deletion, toast or Undo is not settled yet, of every open
    /// project (AR-217).
    pub(crate) deletions: Vec<crate::chat_delete::Deletion>,
    /// The deleted conversations' folders asked of the host to remove, with its answers.
    pub(crate) forgetting: Vec<(String, Receiver<Result<bool, String>>)>,
}

impl State {
    /// Whether a conversation holds agent access now.
    pub fn holds_access(&self) -> bool {
        self.access
    }

    /// Whether the conversation `chat` runs.
    pub fn running(&self, chat: &str) -> bool {
        self.kept.iter().any(|k| k.chat == chat && k.running)
    }

    /// How many conversations are kept.
    pub fn kept(&self) -> usize {
        self.kept.len()
    }

    /// How many events of `chat` were dropped past the cap.
    pub fn dropped(&self, chat: &str) -> u64 {
        self.kept.iter().find(|k| k.chat == chat).map_or(0, |k| k.dropped)
    }
}

/// The host, when it runs conversations here.
fn host(shell: &crate::Shell) -> Option<Arc<dyn crate::ai_agents::AgentHost>> {
    shell.services.agents.clone().filter(|h| h.system() == crate::ai_agents::System::Supported)
}

/// Once per frame: connects the host's events to this window the first time (their wake repaints
/// it), follows Let Chat use the model ([`follow_switch`]), and takes in what the host sent.
pub(crate) fn frame(shell: &mut crate::Shell, ctx: &egui::Context) {
    let Some(host) = host(shell) else { return };
    if shell.chats.rx.is_none() {
        let (tx, rx) = std::sync::mpsc::channel();
        let window = ctx.clone();
        host.connect(EventSink::new(tx, Arc::new(move || window.request_repaint())));
        shell.chats.rx = Some(rx);
    }
    follow_switch(shell);
    poll(shell);
}

/// Follows Let Chat use the model when it differs from what was seen last: turning it off ends
/// every conversation that runs, each end revoking its token at once and agent access closing with
/// the last, so none sends anything more (AR-221); the panel says so.
pub(crate) fn follow_switch(shell: &mut crate::Shell) {
    let on = shell.state.agents.chat_uses_model;
    if shell.chats.seen == Some(on) {
        return;
    }
    if let Some(host) = host(shell) {
        shell.chats.seen = Some(on);
        log::info!("Let Chat use the model is {}", if on { "on" } else { "off" });
        let mut ended = Vec::new();
        if !on {
            for kept in shell.chats.kept.iter().filter(|k| k.running) {
                match host.end(&kept.chat) {
                    Ok(()) => ended.push(kept.chat.clone()),
                    // Already ending: its end is on its way.
                    Err(ChatRefusal::NotRunning) => {}
                    Err(e) => log::warn!("the conversation {:?} could not be ended as Let Chat use the model went off: {e}", kept.chat),
                }
            }
        }
        crate::chat_panel::model_use_changed(shell, on, &ended);
    }
}

/// Takes in the host's events, at most [`PER_FRAME`] a frame.
fn poll(shell: &mut crate::Shell) {
    for _ in 0..PER_FRAME {
        let Some(rx) = &shell.chats.rx else { return };
        let event = match rx.try_recv() {
            Ok(e) => e,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => {
                shell.chats.rx = None;
                shell.chats.access = false;
                return;
            }
        };
        match event {
            HostEvent::Access { held } => shell.chats.access = held,
            HostEvent::Chat { chat, event, ticket } => keep(&mut shell.chats, chat, event, ticket),
        }
    }
}

/// Keeps `event` for `chat`. Past [`MAX_WAITING`] events or [`MAX_WAITING_BYTES`] it is dropped,
/// said once in the log; an end makes room by dropping the oldest events, so it is never lost.
fn keep(state: &mut State, chat: String, event: ChatEvent, ticket: Ticket) {
    let Some(kept) = state.kept.iter_mut().find(|k| k.chat == chat) else {
        log::warn!("an event came for the conversation {chat:?}, which this window does not know; it is dropped");
        return;
    };
    let ended = matches!(event, ChatEvent::Ended { .. });
    let size = event.bytes().min(MAX_WAITING_BYTES);
    let full = |k: &Kept| k.events.len() >= MAX_WAITING || k.bytes.saturating_add(size) > MAX_WAITING_BYTES;
    if full(kept) && !ended {
        drop_one(kept);
        return;
    }
    while full(kept) {
        let Some((oldest, _)) = kept.events.pop_front() else { break };
        kept.bytes = kept.bytes.saturating_sub(oldest.bytes().min(MAX_WAITING_BYTES));
        drop_one(kept);
    }
    if ended {
        kept.running = false;
    }
    kept.bytes = kept.bytes.saturating_add(size);
    kept.events.push_back((event, ticket));
}

/// Counts a dropped event; the first drop of a conversation is said in the log.
fn drop_one(kept: &mut Kept) {
    if kept.dropped == 0 {
        log::warn!("the conversation {:?} has {MAX_WAITING} events waiting; later ones are dropped until they are taken", kept.chat);
    }
    kept.dropped = kept.dropped.saturating_add(1);
}

/// Starts a conversation: checked here (its id, Let Chat use the model on, a duplicate, the cap),
/// then asked of the host, which answers at once and reports the rest as events. A Codex
/// conversation reads Codex's account the first time, as opening AI Agents does.
pub fn start(shell: &mut crate::Shell, request: ChatStart) -> Result<(), ChatRefusal> {
    check_id(&request.chat)?;
    model_use_on(shell)?;
    let host = host(shell).ok_or(ChatRefusal::NotAvailable)?;
    if shell.chats.running(&request.chat) {
        return Err(ChatRefusal::AlreadyRunning);
    }
    // An ended conversation whose events were all taken is forgotten to make room.
    shell.chats.kept.retain(|k| k.running || !k.events.is_empty() || k.chat == request.chat);
    let known = shell.chats.kept.iter().any(|k| k.chat == request.chat);
    if !known && shell.chats.kept.len() >= MAX_SESSIONS {
        return Err(ChatRefusal::TooMany { most: MAX_SESSIONS });
    }
    let (chat, provider) = (request.chat.clone(), request.provider);
    host.start(request)?;
    match shell.chats.kept.iter_mut().find(|k| k.chat == chat) {
        Some(k) => k.running = true,
        None => shell.chats.kept.push(Kept { chat, events: VecDeque::new(), bytes: 0, running: true, dropped: 0 }),
    }
    if provider == Provider::Codex {
        crate::ai_agents::need_account(shell);
    }
    Ok(())
}

/// Sends the person's message to the running conversation `chat`; refused while Let Chat use the
/// model is off.
pub fn send(shell: &crate::Shell, chat: &str, text: String) -> Result<(), ChatRefusal> {
    check_id(chat)?;
    model_use_on(shell)?;
    host(shell).ok_or(ChatRefusal::NotAvailable)?.send(chat, text)
}

/// Refuses while Let Chat use the model is off: then nothing of any conversation leaves Archforge.
fn model_use_on(shell: &crate::Shell) -> Result<(), ChatRefusal> {
    if shell.state.agents.chat_uses_model { Ok(()) } else { Err(ChatRefusal::ModelUseOff) }
}

/// Stops the turn the conversation `chat` runs; the conversation stays up.
pub fn stop(shell: &crate::Shell, chat: &str) -> Result<(), ChatRefusal> {
    check_id(chat)?;
    host(shell).ok_or(ChatRefusal::NotAvailable)?.stop(chat)
}

/// Ends the conversation `chat`: its program stops and its agent access is given back.
pub fn end(shell: &crate::Shell, chat: &str) -> Result<(), ChatRefusal> {
    check_id(chat)?;
    host(shell).ok_or(ChatRefusal::NotAvailable)?.end(chat)
}

/// Takes the events of `chat` waiting so far, oldest first; an ended conversation whose events are
/// all taken is forgotten.
pub fn take_events(shell: &mut crate::Shell, chat: &str) -> Vec<ChatEvent> {
    let Some(kept) = shell.chats.kept.iter_mut().find(|k| k.chat == chat) else { return Vec::new() };
    let events = kept.events.drain(..).map(|(e, _)| e).collect();
    kept.bytes = 0;
    shell.chats.kept.retain(|k| k.running || !k.events.is_empty());
    events
}
