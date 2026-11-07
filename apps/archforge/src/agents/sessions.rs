//! Chat's conversations in the host: each runs its agent program on a thread of its own.
//!
//! [`start`] answers at once: it refuses an id that is not a conversation's UUID (checked as the
//! agents crate's `ConversationId` and `SessionId` before anything uses it), one already running,
//! one whose private folder is being removed ([`forget`]), and more than [`MAX_RUNNING`] at once;
//! the thread does the rest. The thread finds the program,
//! takes the key from the secure store (or Codex's ChatGPT sign-in), takes a token for agent access
//! when Let Chat use the model is on (read-only unless the conversation allows changes), starts the
//! adapter with the bridge (`archforge-cli mcp --bridge <port> --token-file <file> [--read-only]
//! --origin <label>`) or with no tool server at all, and then passes the person's messages in and
//! the program's events out, each with its place among the waiting ones ([`Ticket`]), until the
//! conversation ends.
//!
//! A guard on every thread ([`Guard`]) gives everything back however the thread ends, a panic
//! included: the conversation leaves the running list, its token is revoked, agent access closes
//! with the last conversation that held it, and the window is told the conversation ended. The
//! token goes first: asking a conversation to end ([`command`] with [`Command::End`],
//! [`Handle::end`]) revokes its token on the asking thread, before the call returns, so no later
//! request can use it whatever origin it names; its thread then closes the opening, when nothing
//! else holds it, before the program is shut down, which can take seconds; so does a program that
//! ends by itself (AR-228). The window also refuses requests in the name of a conversation whose
//! program is being ended.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use archforge_agents::claude::{ClaudeConfig, ClaudeSession, Model, SessionId, Start};
use archforge_agents::codex::{Auth, Bridge, CodexConfig, CodexHome, CodexSession};
use archforge_agents::{AgentError, AgentSession, ConversationId, ErrorKind, Limits, SessionEvent, TurnOutcome, Workspace};
use archforge_ui_egui::ai_agents::{Awaited, CodexSignIn, Provider};
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

/// A running conversation as the host keeps it: the way to its thread, and its token.
pub struct Handle {
    commands: Sender<Command>,
    token: Token,
}

impl Handle {
    /// Asks the conversation to end, its token revoked first (AR-228); a thread already gone
    /// needs nothing.
    pub fn end(&self) {
        revoke_now(&self.token);
        let _ = self.commands.send(Command::End);
    }
}

/// A conversation's token, shared by its thread and its handle, so that asking it to end revokes
/// the token on the asking thread, before the call returns, and no later call can use it whatever
/// origin it names (AR-228). `ended` keeps a grant that comes after that from being kept.
type Token = Arc<Mutex<Held>>;

/// What [`Token`] holds.
#[derive(Default)]
struct Held {
    grant: Option<archforge_control::Grant>,
    ended: bool,
}

/// Revokes the token now and marks it ended: its grant drops, which revokes it (its file removed,
/// its connections refused); closing the opening is left to the conversation's thread.
fn revoke_now(token: &Token) {
    let grant = {
        let mut held = locked(token);
        held.ended = true;
        held.grant.take()
    };
    drop(grant);
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
    let token = Token::default();
    {
        // Held while the conversation joins the running list, so a removal of its folder either
        // began before (refused here, and the folder kept for the panel that asked) or finds it
        // running and ends it first.
        let mut forgetting = locked(&inner.forgetting);
        if let Some(removing) = forgetting.get_mut(&request.chat) {
            removing.wanted = true;
            return Err(ChatRefusal::Forgetting);
        }
        let mut running = locked(&inner.sessions);
        if running.contains_key(&request.chat) {
            return Err(ChatRefusal::AlreadyRunning);
        }
        if running.len() >= MAX_RUNNING {
            return Err(ChatRefusal::TooMany { most: MAX_RUNNING });
        }
        running.insert(request.chat.clone(), Handle { commands, token: token.clone() });
    }
    log::info!("the conversation {:?} starts with {}", request.chat, request.provider.name());
    let chat = request.chat.clone();
    let thread = inner.clone();
    if !inner.spawn("agents-chat", move || run(&thread, request, id, session, &rx, token)) {
        locked(&inner.sessions).remove(&chat);
        return Err(if inner.quitting() { ChatRefusal::Closing } else { ChatRefusal::Refused { why: "a thread could not start".into() } });
    }
    Ok(())
}

/// Passes `command` to the running conversation `chat`; [`Command::End`] revokes its token first,
/// before this returns (AR-228).
pub fn command(inner: &Inner, chat: &str, command: Command) -> Result<(), ChatRefusal> {
    if ConversationId::new(chat).is_err() || SessionId::new(chat).is_err() {
        return Err(ChatRefusal::BadId { bytes: chat.len() });
    }
    let running = locked(&inner.sessions);
    let handle = running.get(chat).ok_or(ChatRefusal::NotRunning)?;
    if matches!(command, Command::End) {
        revoke_now(&handle.token);
    }
    handle.commands.send(command).map_err(|_| ChatRefusal::NotRunning)
}

/// How long removing a deleted conversation's folder waits for its program to end: the program's
/// own shutdown takes up to two of [`GRACE`].
const FORGET_WAIT: Duration = Duration::from_secs(5);

/// The removals of one conversation's private folder under way (AR-218).
#[derive(Debug, Default)]
pub struct Removing {
    /// How many: the conversation may start again once none is left.
    under_way: usize,
    /// A start of it was refused meanwhile: a panel shows it (a copy of the project, say), so its
    /// folder is kept.
    wanted: bool,
}

/// A conversation whose private folder is being removed, counted in [`Inner::forgetting`] from
/// the moment the removal is asked until its folder is moved aside or the removal ends, however
/// it ends: [`start`] refuses the conversation meanwhile (AR-218).
struct Forgetting {
    inner: Arc<Inner>,
    chat: String,
    /// Counted done already ([`Forgetting::done`]), when its folder was moved aside.
    done: bool,
}

impl Forgetting {
    /// Counts one more removal of `chat` under way.
    fn new(inner: &Arc<Inner>, chat: &str) -> Forgetting {
        let mut forgetting = locked(&inner.forgetting);
        let removing = forgetting.entry(chat.to_string()).or_default();
        removing.under_way = removing.under_way.saturating_add(1);
        Forgetting { inner: inner.clone(), chat: chat.to_string(), done: false }
    }

    /// Counts the removal done in `forgetting`, the map behind [`Inner::forgetting`] its caller
    /// holds locked; with none left the conversation may start again. Once only.
    fn done(&mut self, forgetting: &mut HashMap<String, Removing>) {
        if std::mem::replace(&mut self.done, true) {
            return;
        }
        if let Some(removing) = forgetting.get_mut(&self.chat) {
            removing.under_way = removing.under_way.saturating_sub(1);
            if removing.under_way == 0 {
                forgetting.remove(&self.chat);
            }
        }
    }
}

impl Drop for Forgetting {
    /// Counts the removal done, unless it was already.
    fn drop(&mut self) {
        let inner = self.inner.clone();
        self.done(&mut locked(&inner.forgetting));
    }
}

/// Names a folder moved aside for removal takes, unique in this process.
static ASIDE: AtomicU64 = AtomicU64::new(0);

/// Removes the private folder of the deleted conversation `chat` (AR-217), on a thread the host
/// tracks: first the project's answers in `after` are waited for, in order, at most
/// [`FORGET_WAIT`] in all, and only when every one lets the folder go does it go on (an Undo
/// pressed before its deletion answered passes the deletion's and the restoration's, so a deletion
/// that failed keeps the folder whatever the restoration says, AR-218); then its program, when it
/// runs, is asked to end and waited for, at most [`FORGET_WAIT`] again; a folder whose program has
/// not ended by then is kept, as a running program may write into it again; the folder is then
/// moved aside in one rename and removed under its new name. From this call until the folder is
/// moved aside or kept, [`start`] refuses `chat`, so no new program of it is ended, and a start
/// refused meanwhile keeps the folder for the panel that asked, which starts it with Send again
/// (AR-218). Answers whether there was a folder (false too when an answer kept it), or why
/// it is kept, once `chat` may start again. Started at quit before the host stops, it is waited for
/// with the host's other threads, within [`super::QUIT_BUDGET`].
pub fn forget(inner: &Arc<Inner>, chat: &str, after: Awaited) -> Receiver<Result<bool, String>> {
    let (tx, rx) = std::sync::mpsc::channel();
    let id = match (ConversationId::new(chat), SessionId::new(chat)) {
        (Ok(id), Ok(_)) => id,
        _ => {
            let _ = tx.send(Err(ChatRefusal::BadId { bytes: chat.len() }.to_string()));
            return rx;
        }
    };
    // Counted before the thread starts, so a start asked after this call is refused; a thread that
    // cannot start drops it again.
    let registered = Forgetting::new(inner, chat);
    let thread = inner.clone();
    let chat = chat.to_string();
    let started = inner.spawn("agents-forget", move || {
        let mut registered = registered;
        let answer = remove(&thread, &chat, &id, after, &mut registered);
        // Done before the answer goes, so whoever reads it may start the conversation again.
        drop(registered);
        let _ = tx.send(answer);
    });
    if !started {
        log::warn!("the private folder of a deleted conversation is kept: Archforge is closing or a thread could not start");
    }
    rx
}

/// The work of [`forget`] on its thread: the answers waited for, the program ended, the folder
/// moved aside and counted done (`registered`), then removed. Gives whether there was a folder, or
/// why it is kept.
fn remove(thread: &Arc<Inner>, chat: &str, id: &ConversationId, after: Awaited, registered: &mut Forgetting) -> Result<bool, String> {
    let deadline = std::time::Instant::now() + FORGET_WAIT;
    for (answer, goes) in after {
        match answer.recv_timeout(deadline.saturating_duration_since(std::time::Instant::now())) {
            Ok(a) if goes.decides(&a) => {}
            Ok(_) => {
                log::info!("the private folder of the conversation {chat:?} is kept: the project's answer keeps it");
                return Ok(false);
            }
            Err(_) => {
                log::warn!("the private folder of the conversation {chat:?} is kept: the project did not answer within {} s", FORGET_WAIT.as_secs());
                return Ok(false);
            }
        }
    }
    let running = || locked(&thread.sessions).contains_key(chat);
    if let Some(handle) = locked(&thread.sessions).get(chat) {
        handle.end();
    }
    let deadline = std::time::Instant::now() + FORGET_WAIT;
    while running() && std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    // No start of it is taken while it is counted in `forgetting`, so once it is not running it
    // stays so until the folder is moved aside.
    let answer = if running() {
        Err(format!("its program did not end within {} s", FORGET_WAIT.as_secs()))
    } else {
        match Workspace::open(&thread.workspace) {
            Ok(workspace) => {
                // Under the lock `start` checks and marks it under, in one step: kept when a start
                // was refused before; otherwise moved aside, in one rename, and counted done, so a
                // start after it is taken and makes a new folder. No refused start loses the folder,
                // and the folder moved aside is removed outside the lock, so no start waits for it.
                let aside = ConversationId::new(&format!("{chat}-rm-{}-{}", std::process::id(), ASIDE.fetch_add(1, Ordering::Relaxed)))
                    .map_err(|e| e.to_string())?;
                {
                    let mut forgetting = locked(&thread.forgetting);
                    if forgetting.get(chat).is_some_and(|r| r.wanted) {
                        log::info!("the private folder of the deleted conversation {chat:?} is kept: a project asked to start it meanwhile");
                        return Ok(false);
                    }
                    let moved = workspace.move_conversation(id, &aside).map_err(|e| e.to_string())?;
                    registered.done(&mut forgetting);
                    if !moved {
                        return Ok(false);
                    }
                }
                #[cfg(test)]
                if let Some(then) = locked(&thread.as_folder_goes).take() {
                    then();
                }
                workspace.remove_conversation(&aside).map_err(|e| format!("{e} (moved aside as {aside})"))
            }
            Err(e) => Err(e.to_string()),
        }
    };
    match &answer {
        Ok(true) => log::info!("the private folder of the deleted conversation {chat:?} was removed"),
        Ok(false) => log::info!("the deleted conversation {chat:?} had no private folder"),
        Err(why) => log::warn!("the private folder of the deleted conversation {chat:?} is kept: {why}"),
    }
    answer
}

/// What a conversation's thread holds that must be given back however it ends.
struct Guard {
    inner: Arc<Inner>,
    chat: String,
    /// The conversation's token, shared with its handle.
    token: Token,
    /// Whether a grant was taken for it, so the opening is released once, however it was revoked.
    granted: bool,
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

    /// Revokes the conversation's token, when the handle has not already, and closes agent access
    /// when no conversation holds it any more (AR-228): a conversation asked to end, or whose
    /// program ended, gives its access back before its program is shut down, so nothing it asks for
    /// after that is run (a request the window already took finishes). Closing the opening is
    /// guarded as the guard's drop guards it: a panic there is said in the log, never carried on.
    fn revoke(&mut self) {
        // The token is revoked as the grant drops, whatever follows (the handle may have revoked it
        // already, when the end was asked).
        revoke_now(&self.token);
        if !std::mem::take(&mut self.granted) {
            return;
        }
        let inner = self.inner.clone();
        let closed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            let sink = inner.sink();
            locked(&inner.access).released(sink.as_ref());
        }));
        if closed.is_err() {
            log::error!("agent access could not be closed after the conversation {:?} gave its token back", self.chat);
        }
    }
}

impl Drop for Guard {
    /// Gives everything back: the conversation leaves the running list, its token is revoked, agent
    /// access closes when no conversation holds it, and the window is told the conversation ended.
    fn drop(&mut self) {
        locked(&self.inner.sessions).remove(&self.chat);
        let sink = self.inner.sink();
        // The token is revoked as the grant drops, whatever follows.
        revoke_now(&self.token);
        let held = std::mem::take(&mut self.granted);
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

/// A conversation's thread: everything but the checks [`start`] made, with `token`, shared with its
/// handle. Asking it to end revokes the token at once, on the asking thread; this thread then
/// closes the opening, when nothing else holds it, before the program is shut down; so does its
/// program ending (AR-228).
fn run(inner: &Arc<Inner>, request: ChatStart, id: ConversationId, session: SessionId, commands: &Receiver<Command>, token: Token) {
    let mut guard = Guard {
        inner: inner.clone(),
        chat: request.chat.clone(),
        token,
        granted: false,
        failure: None,
        waiting: Arc::new(Waiting::default()),
        dropped: false,
    };
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
        let access = locked(&guard.token).grant.as_ref().is_some_and(archforge_control::Grant::live);
        if let Some(event) = chat_event(event, access) {
            guard.emit(event);
        }
    }
    // Its access goes first (AR-228): the program takes seconds to shut down, and nothing it asks
    // for after this is run.
    guard.revoke();
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
    // Held by the guard from the moment it exists, so any end after it gives it back; one asked to
    // end meanwhile has it revoked here, never kept.
    let grant = locked(&inner.access).grant(request.allow_changes, sink.as_ref()).map_err(other)?;
    guard.granted = grant.is_some();
    let place = grant.as_ref().map(|g| (g.port(), g.path().to_path_buf()));
    {
        let mut held = locked(&guard.token);
        if held.ended {
            drop(grant);
        } else {
            held.grant = grant;
        }
    }
    let bridge = match place {
        Some((port, token_file)) => Some(Bridge {
            cli: (inner.cli)().map_err(other)?,
            port,
            token_file,
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
        SessionEvent::Retrying { attempt, max_attempts, .. } => ChatEvent::Retrying { attempt, max: max_attempts },
        SessionEvent::TurnFinished { outcome, .. } => ChatEvent::TurnEnded(match outcome {
            TurnOutcome::Completed => TurnEnd::Completed,
            TurnOutcome::Stopped => TurnEnd::Stopped,
            TurnOutcome::Failed(e) => TurnEnd::Failed(failure(&e)),
        }),
        SessionEvent::Ignored(_) | SessionEvent::Ended { .. } => return None,
    })
}
