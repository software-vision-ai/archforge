//! Chat's conversations in the host: each runs its agent program on a thread of its own.
//!
//! [`start`] answers at once: it refuses an id that is not a conversation's UUID (checked as the
//! agents crate's `ConversationId` and `SessionId` before anything uses it), one already running,
//! and more than [`MAX_RUNNING`] at once; the thread does the rest. The thread finds the program,
//! takes the key from the secure store (or Codex's ChatGPT sign-in), takes a token for agent access
//! when Let Chat use the model is on (read-only unless the conversation allows changes), starts the
//! adapter with the bridge (`archforge-cli mcp --bridge <port> --token-file <file> [--read-only]
//! --origin <label>`) or with no tool server at all, and then passes the person's messages in and
//! the program's events out, each with its place among the waiting ones ([`Ticket`]), until the
//! conversation ends.
//!
//! A guard on every thread ([`Guard`]) gives everything back however the thread ends, a panic
//! included: the conversation leaves the running list, its token is revoked, agent access closes
//! with the last conversation that held it, and the window is told the conversation ended.

use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, TryRecvError};
use std::time::Duration;

use archforge_agents::claude::{ClaudeConfig, ClaudeSession, Model, SessionId, Start};
use archforge_agents::codex::{Auth, Bridge, CodexConfig, CodexHome, CodexSession};
use archforge_agents::{AgentError, AgentSession, ConversationId, ErrorKind, Limits, SessionEvent, TurnOutcome, Workspace};
use archforge_ui_egui::ai_agents::{CodexSignIn, Provider};
use archforge_ui_egui::chat::{ChatEvent, ChatFailure, ChatRefusal, ChatStart, FailureKind, HostEvent, MAX_RUNNING, Ticket, TurnEnd, Waiting};

use super::{Inner, locked};

/// How long one wait for the program's next event lasts, between looks at the commands.
const TICK: Duration = Duration::from_millis(100);
/// How long a conversation's program gets to end at each step of its shutdown.
const GRACE: Duration = Duration::from_secs(2);

/// What the window asks of a running conversation.
pub enum Command {
    Send(String),
    Stop,
    End,
}

impl std::fmt::Debug for Command {
    /// The kind of command; the person's message by its length only.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Command::Send(text) => f.debug_struct("Send").field("bytes", &text.len()).finish(),
            Command::Stop => f.write_str("Stop"),
            Command::End => f.write_str("End"),
        }
    }
}

/// A running conversation as the host keeps it: the way to its thread.
pub struct Handle {
    commands: Sender<Command>,
}

impl Handle {
    /// Asks the conversation to end; a thread already gone needs nothing.
    pub fn end(&self) {
        let _ = self.commands.send(Command::End);
    }
}

/// Starts the conversation `request` names, answering at once.
pub fn start(inner: &Arc<Inner>, request: ChatStart) -> Result<(), ChatRefusal> {
    if inner.quitting() {
        return Err(ChatRefusal::Closing);
    }
    let bad = || ChatRefusal::BadId { bytes: request.chat.len() };
    let id = ConversationId::new(&request.chat).map_err(|_| bad())?;
    let session = SessionId::new(&request.chat).map_err(|_| bad())?;
    archforge_control::check_origin(&request.origin).map_err(|e| ChatRefusal::Refused { why: format!("the origin label: {e}") })?;
    let (commands, rx) = std::sync::mpsc::channel();
    {
        let mut running = locked(&inner.sessions);
        if running.contains_key(&request.chat) {
            return Err(ChatRefusal::AlreadyRunning);
        }
        if running.len() >= MAX_RUNNING {
            return Err(ChatRefusal::TooMany { most: MAX_RUNNING });
        }
        running.insert(request.chat.clone(), Handle { commands });
    }
    log::info!("the conversation {:?} starts with {}", request.chat, request.provider.name());
    let chat = request.chat.clone();
    let thread = inner.clone();
    if !inner.spawn("agents-chat", move || run(&thread, request, id, session, &rx)) {
        locked(&inner.sessions).remove(&chat);
        return Err(if inner.quitting() { ChatRefusal::Closing } else { ChatRefusal::Refused { why: "a thread could not start".into() } });
    }
    Ok(())
}

/// Passes `command` to the running conversation `chat`.
pub fn command(inner: &Inner, chat: &str, command: Command) -> Result<(), ChatRefusal> {
    if ConversationId::new(chat).is_err() || SessionId::new(chat).is_err() {
        return Err(ChatRefusal::BadId { bytes: chat.len() });
    }
    let running = locked(&inner.sessions);
    let handle = running.get(chat).ok_or(ChatRefusal::NotRunning)?;
    handle.commands.send(command).map_err(|_| ChatRefusal::NotRunning)
}

/// What a conversation's thread holds that must be given back however it ends.
struct Guard {
    inner: Arc<Inner>,
    chat: String,
    grant: Option<archforge_control::Grant>,
    /// Why the conversation ended, when nobody asked; sent with its end.
    failure: Option<ChatFailure>,
    /// The events waiting in the window, counted against the cap.
    waiting: Arc<Waiting>,
    /// Whether a drop past the cap was said in the log yet.
    dropped: bool,
}

impl Guard {
    /// Sends `event` to the window with its place among the waiting ones; past the cap it is
    /// dropped, the first drop said in the log.
    fn emit(&mut self, event: ChatEvent) {
        let Some(sink) = self.inner.sink() else { return };
        let Some(ticket) = Ticket::take(&self.waiting, event.bytes()) else {
            if !self.dropped {
                log::warn!("the conversation {:?} has too many events waiting in the window; later ones are dropped until they are taken", self.chat);
                self.dropped = true;
            }
            return;
        };
        sink.send(HostEvent::Chat { chat: self.chat.clone(), event, ticket });
    }
}

impl Drop for Guard {
    /// Gives everything back: the conversation leaves the running list, its token is revoked, agent
    /// access closes when no conversation holds it, and the window is told the conversation ended.
    fn drop(&mut self) {
        locked(&self.inner.sessions).remove(&self.chat);
        let sink = self.inner.sink();
        // The token is revoked as the grant drops, whatever follows.
        let held = self.grant.take().is_some();
        let failure = self.failure.take();
        let chat = self.chat.clone();
        let inner = self.inner.clone();
        // A drop during a panic must not panic again: closing the opening and telling the window
        // (whose wake runs here) are guarded.
        let told = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            if held {
                locked(&inner.access).released(sink.as_ref());
            }
            if let Some(sink) = sink {
                sink.send(HostEvent::Chat { chat, event: ChatEvent::Ended { failure }, ticket: Ticket::uncounted() });
            }
        }));
        if told.is_err() {
            log::error!("the end of the conversation {:?} could not be told to the window", self.chat);
        }
        if std::thread::panicking() {
            log::error!("the conversation {:?} stopped on a failure inside Archforge; its access was given back", self.chat);
        } else {
            log::info!("the conversation {:?} ended", self.chat);
        }
    }
}

/// The failure an adapter error is to the window.
pub(super) fn failure(e: &AgentError) -> ChatFailure {
    // The runtimes give no reset time with a rate limit yet, so the panel names none.
    ChatFailure { kind: failure_kind(e.kind()), detail: e.to_string(), reset: None }
}

/// The window's category of a failure's kind.
fn failure_kind(kind: ErrorKind) -> FailureKind {
    match kind {
        ErrorKind::MissingRuntime => FailureKind::MissingRuntime,
        ErrorKind::InvalidCredential => FailureKind::InvalidCredential,
        ErrorKind::SignInExpired => FailureKind::SignInExpired,
        ErrorKind::RateLimited => FailureKind::RateLimited,
        ErrorKind::Network => FailureKind::Network,
        ErrorKind::ModelUnavailable => FailureKind::ModelUnavailable,
        ErrorKind::Other => FailureKind::Other,
    }
}

/// A failure of Archforge's own, with its words.
fn other(detail: String) -> ChatFailure {
    ChatFailure { kind: FailureKind::Other, detail, reset: None }
}

/// The model to name: `default` leaves it to the program, which Codex takes as none.
fn model(name: &str) -> Result<Model, ChatFailure> {
    Model::new(name).map_err(|e| failure(&e))
}

/// A conversation's thread: everything but the checks [`start`] made.
fn run(inner: &Arc<Inner>, request: ChatStart, id: ConversationId, session: SessionId, commands: &Receiver<Command>) {
    let mut guard =
        Guard { inner: inner.clone(), chat: request.chat.clone(), grant: None, failure: None, waiting: Arc::new(Waiting::default()), dropped: false };
    let mut program = match begin(inner, &request, &id, session, &mut guard) {
        Ok(p) => p,
        Err(f) => {
            log::warn!("the conversation {:?} could not start: {}", request.chat, f.detail);
            guard.failure = Some(f);
            return;
        }
    };
    loop {
        if inner.quitting() {
            break;
        }
        let mut end = false;
        loop {
            match commands.try_recv() {
                Ok(Command::Send(text)) => match program.send(&text) {
                    Ok(()) => {}
                    // The turn under way goes on; the window sends the next message after its end.
                    Err(e @ AgentError::TurnRunning { .. }) => log::warn!("the conversation {:?}: {e}", request.chat),
                    Err(e) => guard.emit(ChatEvent::TurnEnded(TurnEnd::Failed(failure(&e)))),
                },
                Ok(Command::Stop) => {
                    if let Err(e) = program.stop() {
                        log::warn!("the conversation {:?}'s turn could not be stopped: {e}", request.chat);
                    }
                }
                Ok(Command::End) | Err(TryRecvError::Disconnected) => {
                    end = true;
                    break;
                }
                Err(TryRecvError::Empty) => break,
            }
        }
        if end {
            break;
        }
        let Some(event) = program.next_event(TICK) else { continue };
        if let SessionEvent::Ended { error, .. } = &event {
            guard.failure = error.as_ref().map(failure);
            break;
        }
        // Whether the conversation holds access now: the opt-in may have closed it since the start.
        let access = guard.grant.as_ref().is_some_and(archforge_control::Grant::live);
        if let Some(event) = chat_event(event, access) {
            guard.emit(event);
        }
    }
    // The program is shut down while the guard still holds the conversation, so a new start of the
    // same id waits for it.
    if let Err(e) = program.shutdown(GRACE) {
        log::warn!("the conversation {:?}'s program did not shut down cleanly: {e}", request.chat);
    }
}

/// The program of the conversation, started: found, its key or sign-in, its token and bridge when
/// agent access is on, its folder. The token goes into the guard first, so a failure after it
/// still revokes it.
fn begin(
    inner: &Arc<Inner>,
    request: &ChatStart,
    id: &ConversationId,
    session: SessionId,
    guard: &mut Guard,
) -> Result<Box<dyn AgentSession + Send>, ChatFailure> {
    let runtime = inner.runtime(super::kind(request.provider)).map_err(|e| failure(&e))?;
    let workspace = Workspace::open(&inner.workspace).map_err(|e| failure(&e))?;
    let folder = workspace.conversation(id).map_err(|e| failure(&e))?;
    let auth = match (request.provider, request.codex_sign_in) {
        (Provider::Claude, _) | (Provider::Codex, CodexSignIn::ApiKey) => Some(key(inner, request.provider)?),
        (Provider::Codex, CodexSignIn::ChatGpt) => None,
    };
    let sink = inner.sink();
    // Held by the guard from the moment it exists, so any end after it gives it back.
    guard.grant = locked(&inner.access).grant(request.allow_changes, sink.as_ref()).map_err(other)?;
    let bridge = match &guard.grant {
        Some(grant) => Some(Bridge {
            cli: (inner.cli)().map_err(other)?,
            port: grant.port(),
            token_file: grant.path().to_path_buf(),
            origin: request.origin.clone(),
            allow_changes: request.allow_changes,
        }),
        None => None,
    };
    if inner.quitting() {
        return Err(other("Archforge is closing".into()));
    }
    let start = if request.resume { Start::Resume } else { Start::New };
    let program: Box<dyn AgentSession + Send> = match request.provider {
        Provider::Claude => {
            let key = auth.ok_or_else(|| other("no key".into()))?;
            let config = ClaudeConfig { session, start, model: model(&request.model)?, bridge, limits: Limits::default() };
            Box::new(ClaudeSession::start(&runtime, &folder, config, key).map_err(|e| failure(&e))?)
        }
        Provider::Codex => {
            let home = CodexHome::open(&workspace).map_err(|e| failure(&e))?;
            let config = CodexConfig {
                start,
                model: if request.model == "default" { None } else { Some(model(&request.model)?) },
                bridge,
                instructions: None,
                limits: Limits::default(),
                request_timeout: archforge_agents::codex::DEFAULT_REQUEST_TIMEOUT,
            };
            let auth = match auth {
                Some(key) => Auth::ApiKey(key),
                None => Auth::ChatGpt(super::chatgpt_store(inner.store())),
            };
            Box::new(CodexSession::start(&runtime, &folder, &home, config, auth).map_err(|e| failure(&e))?)
        }
    };
    Ok(program)
}

/// The API key of `provider`, or the failure that says there is none.
fn key(inner: &Inner, provider: Provider) -> Result<archforge_secrets::Secret, ChatFailure> {
    inner.key(provider).map_err(|e| match e {
        archforge_secrets::SecretError::NotFound { .. } | archforge_secrets::SecretError::NoSecureStore { .. } => {
            ChatFailure { kind: FailureKind::NoCredential, detail: format!("no {} API key is saved: {e}", provider.name()), reset: None }
        }
        e => other(format!("the {} API key could not be read: {e}", provider.name())),
    })
}

/// An adapter's event as the window takes it; what the window has no use for is none.
fn chat_event(event: SessionEvent, access: bool) -> Option<ChatEvent> {
    Some(match event {
        SessionEvent::Started { model, .. } => ChatEvent::Started { model, access },
        SessionEvent::TextDelta { text } => ChatEvent::Text { delta: text },
        SessionEvent::MessageDone { text } => ChatEvent::Message { text },
        SessionEvent::ToolStarted { id, tool, input, .. } => ChatEvent::ToolStarted { id, tool, input },
        SessionEvent::ToolFinished { id, ok, summary, .. } => ChatEvent::ToolFinished { id, ok, summary },
        SessionEvent::Retrying { attempt, .. } => ChatEvent::Retrying { attempt },
        SessionEvent::TurnFinished { outcome, .. } => ChatEvent::TurnEnded(match outcome {
            TurnOutcome::Completed => TurnEnd::Completed,
            TurnOutcome::Stopped => TurnEnd::Stopped,
            TurnOutcome::Failed(e) => TurnEnd::Failed(failure(&e)),
        }),
        SessionEvent::Ignored(_) | SessionEvent::Ended { .. } => return None,
    })
}
