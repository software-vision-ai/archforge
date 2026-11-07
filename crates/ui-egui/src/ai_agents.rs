//! AI Agents: the agent programs Chat runs, Claude (`claude`) and Codex (`codex`), with their keys
//! and sign-ins, as Settings → AI Agents shows them.
//!
//! The shell never touches a program, a key or the system's secure store: the host does, through
//! one service, [`AgentHost`]. It looks for the programs and keeps the API keys (local work that
//! sends nothing); it sends the test message and runs Codex's ChatGPT sign-in with its device
//! code; and it starts, drives, stops and ends Chat's conversations ([`crate::chat`]), giving them
//! agent access while Let Chat use the model is on. Every call returns at once, with a channel the
//! host answers on from its own thread, or with the conversation's events on the one channel
//! [`AgentHost::connect`] gave it, so the window never waits.
//!
//! The programs are looked for once when Archforge starts (no key is read then: the secure store
//! is not touched at launch, so it never asks for access unless agents are used). The keys are
//! read from the store the first time the AI Agents page or Chat needs them, on the host's
//! thread, and kept until Look again, Check connection, a save or a removal, or a restart. Codex's
//! account is read the first time the AI Agents page shows or a Codex conversation starts, never
//! at launch: reading it starts the codex program.
//! Nothing here sends anything over the network unless the user asks for a test message or a
//! sign-in. A key goes from the field to the host as a
//! [`KeyText`], whose bytes are overwritten when it drops; the shell only ever keeps a saved key's
//! last four characters, never in the preferences.
//! A failure is always a typed state the page shows, never a panic.

use std::sync::mpsc::{Receiver, TryRecvError};

use serde::{Deserialize, Serialize};
use zeroize::Zeroize;

use crate::i18n::tr;

/// An agent Chat can talk to, by its provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Default)]
pub enum Provider {
    /// Anthropic's Claude, through the program `claude`, with an API key only.
    #[default]
    Claude,
    /// OpenAI's Codex, through the program `codex`, signed in with ChatGPT or with an API key.
    Codex,
}

impl Provider {
    /// Every agent, in the order the page lists them.
    pub const ALL: [Provider; 2] = [Provider::Claude, Provider::Codex];

    /// The agent's name: a proper name, the same in every language.
    pub fn name(self) -> &'static str {
        match self {
            Provider::Claude => "Claude",
            Provider::Codex => "Codex",
        }
    }

    /// The program's file name.
    pub fn program(self) -> &'static str {
        match self {
            Provider::Claude => "claude",
            Provider::Codex => "codex",
        }
    }

    /// The agent's part of an automation id.
    pub fn key(self) -> &'static str {
        self.program()
    }

    fn index(self) -> usize {
        match self {
            Provider::Claude => 0,
            Provider::Codex => 1,
        }
    }

    /// The command that installs the program, as its maker's documentation gives it.
    pub fn install_command(self) -> &'static str {
        match self {
            Provider::Claude => "npm install -g @anthropic-ai/claude-code",
            Provider::Codex => "npm install -g @openai/codex",
        }
    }

    /// The maker's installation guide, opened in the browser.
    pub fn install_guide(self) -> &'static str {
        match self {
            Provider::Claude => "https://code.claude.com/docs/en/setup",
            Provider::Codex => "https://github.com/openai/codex#readme",
        }
    }

    /// The models the page offers, as the program takes them (`default` leaves the choice to the
    /// program), with their names for people.
    pub fn models(self) -> &'static [(&'static str, &'static str)] {
        match self {
            // Claude Code's model aliases: each stands for the newest model of its family.
            Provider::Claude => &[("default", "The program's default"), ("sonnet", "Sonnet"), ("opus", "Opus"), ("haiku", "Haiku")],
            // Codex's list is read from the program once its adapter is built (AR-202).
            Provider::Codex => &[("default", "The program's default")],
        }
    }
}

/// The models' names for people, through the catalogues.
pub fn model_name(id: &str, provider: Provider) -> String {
    match provider.models().iter().find(|(m, _)| *m == id).map(|(_, n)| *n) {
        Some("The program's default") => tr("The program's default"),
        Some(name) => name.to_string(),
        None => id.to_string(),
    }
}

/// Whether this system runs agent programs at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum System {
    #[default]
    Supported,
    /// Agents work on macOS and Linux for now.
    NotYet,
}

/// What the look found of an agent's program.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Program {
    /// Not looked for yet in this run.
    #[default]
    NotLooked,
    /// Found, of a version Archforge accepts; `tested` is false for a version newer than the one
    /// Archforge was tested with, which is accepted with a warning.
    Found { version: String, tested: bool },
    /// Found, older than the oldest version Archforge accepts.
    TooOld { version: String, floor: String },
    /// Found, but it cannot be used: why, in the host's words.
    Unusable { why: String },
    /// Not on this computer.
    Missing,
}

impl Program {
    pub fn found(&self) -> bool {
        matches!(self, Program::Found { .. })
    }
}

/// An agent's API key as the secure store holds it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Key {
    /// Not read yet in this run.
    #[default]
    NotRead,
    Absent,
    /// Saved: only its last four characters are known to the window.
    Saved {
        last4: String,
    },
    /// The store could not be read: why, in the host's words.
    Unreadable {
        why: String,
    },
}

impl Key {
    pub fn saved(&self) -> bool {
        matches!(self, Key::Saved { .. })
    }
}

/// The system's secure store, where API keys are kept and nowhere else.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Store {
    /// Not looked at yet in this run.
    #[default]
    NotLooked,
    MacKeychain,
    SecretService,
    /// A store of another kind the host names.
    Other,
    /// None: keys cannot be saved, and nothing falls back. Why, in the host's words.
    Missing {
        why: String,
    },
}

impl Store {
    pub fn missing(&self) -> bool {
        matches!(self, Store::Missing { .. })
    }
}

/// What this run knows of the store, both programs and both keys. The programs come from one look,
/// the store and the keys from one read of the store ([`Keys`]). Neither sends anything.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Look {
    pub store: Store,
    pub claude: Program,
    pub codex: Program,
    pub claude_key: Key,
    pub codex_key: Key,
    /// Whether the codex program keeps a ChatGPT sign-in in its own files (no secure store opened)
    /// rather than the system keyring, as the host chose with the store's read.
    pub sign_in_files: bool,
}

impl Look {
    pub fn program(&self, p: Provider) -> &Program {
        match p {
            Provider::Claude => &self.claude,
            Provider::Codex => &self.codex,
        }
    }

    pub fn key(&self, p: Provider) -> &Key {
        match p {
            Provider::Claude => &self.claude_key,
            Provider::Codex => &self.codex_key,
        }
    }

    fn key_mut(&mut self, p: Provider) -> &mut Key {
        match p {
            Provider::Claude => &mut self.claude_key,
            Provider::Codex => &mut self.codex_key,
        }
    }
}

/// What one read of the secure store found: the store itself and both keys, by their last four.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Keys {
    pub store: Store,
    pub claude_key: Key,
    pub codex_key: Key,
    /// Whether a ChatGPT sign-in is kept in the codex program's own files rather than the keyring.
    pub sign_in_files: bool,
}

/// The most bytes a key may have, as the secure store takes it.
pub const MAX_KEY_BYTES: usize = 1024;

/// A key as typed or pasted, on its way to the host: its `Debug` prints a redaction, it has no
/// `Display` and no `Clone`, and its bytes are overwritten when it drops.
#[derive(Default)]
pub struct KeyText(String);

impl KeyText {
    /// The key as it is kept: without its outer spaces and line breaks. Refused when empty, holding
    /// a space or a control character, or longer than [`MAX_KEY_BYTES`]; the text is overwritten
    /// either way.
    pub fn new(mut typed: String) -> Result<KeyText, KeyRefusal> {
        let trimmed = typed.trim();
        let verdict = if trimmed.is_empty() {
            Err(KeyRefusal::Empty)
        } else if trimmed.len() > MAX_KEY_BYTES {
            Err(KeyRefusal::TooLong)
        } else if trimmed.chars().any(|c| c.is_whitespace() || c.is_control()) {
            Err(KeyRefusal::NotOneWord)
        } else {
            Ok(KeyText(trimmed.to_string()))
        };
        typed.zeroize();
        verdict
    }

    /// The key, for the one place that hands it to the secure store.
    pub fn expose(&self) -> &str {
        &self.0
    }

    /// The key itself, for a store that takes it by value; nothing is left behind here.
    pub fn take(mut self) -> String {
        std::mem::take(&mut self.0)
    }
}

impl Drop for KeyText {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

impl std::fmt::Debug for KeyText {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("KeyText(<redacted>)")
    }
}

/// Why a typed key was not taken.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyRefusal {
    Empty,
    NotOneWord,
    TooLong,
}

/// The last four characters a saved key is shown by; a key of fewer than twelve characters shows
/// none, so the mask never gives away much of a short one.
pub fn last_four(key: &str) -> String {
    let n = key.chars().count();
    if n < 12 { String::new() } else { key.chars().skip(n - 4).collect() }
}

/// Who Codex is signed in as, as the codex program keeps it. An expired sign-in whose email Codex
/// did not say has an empty one.
#[derive(Clone, PartialEq, Eq)]
pub enum CodexAccount {
    NotSignedIn,
    SignedIn { email: String, plan: Option<String> },
    Expired { email: String },
}

impl std::fmt::Debug for CodexAccount {
    /// The state and the plan; an email by its length only.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CodexAccount::NotSignedIn => f.write_str("NotSignedIn"),
            CodexAccount::SignedIn { email, plan } => f.debug_struct("SignedIn").field("email_bytes", &email.len()).field("plan", plan).finish(),
            CodexAccount::Expired { email } => f.debug_struct("Expired").field("email_bytes", &email.len()).finish(),
        }
    }
}

/// Why an account call failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccountError {
    /// This version cannot do it yet.
    NotAvailableYet,
    /// The provider refused the key: its own words (status and reason).
    KeyRefused { detail: String },
    /// The agent has no key or sign-in to use.
    NoCredential,
    /// A sign-in was not finished in the time it had.
    SignInTimedOut,
    /// The codex program cancelled a sign-in that nobody here cancelled.
    SignInCancelled,
    /// Anything else, in the host's words.
    Failed { detail: String },
}

/// How a sign-in with ChatGPT goes on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoginWay {
    /// The codex program opens the browser on this computer.
    Browser,
    /// A code to enter on any device with a browser.
    DeviceCode,
}

/// What a sign-in says while it runs.
#[derive(Clone, PartialEq, Eq)]
pub enum LoginEvent {
    /// The page the codex program's sign-in waits on, for the window to open in the browser.
    OpenBrowser {
        url: String,
    },
    /// The code to enter, where, and how many minutes it lasts.
    DeviceCode {
        code: String,
        url: String,
        minutes: u32,
    },
    Done(CodexAccount),
    Failed(AccountError),
}

impl std::fmt::Debug for LoginEvent {
    /// The kind of event; an address and a code by their lengths only, as they hold the sign-in.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoginEvent::OpenBrowser { url } => f.debug_struct("OpenBrowser").field("url_bytes", &url.len()).finish(),
            LoginEvent::DeviceCode { code, url, minutes } => {
                f.debug_struct("DeviceCode").field("code_bytes", &code.len()).field("url_bytes", &url.len()).field("minutes", minutes).finish()
            }
            LoginEvent::Done(account) => f.debug_tuple("Done").field(account).finish(),
            LoginEvent::Failed(e) => f.debug_tuple("Failed").field(e).finish(),
        }
    }
}

/// The host's side of AI Agents and Chat, the one service the shell asks.
///
/// Local work that sends nothing: finding the programs and keeping the keys in the system's secure
/// store. Work that talks to a provider or runs the codex program: the test message and Codex's
/// account. Chat's conversations: each runs its program on a thread of its own, its events reach
/// the window through the sink [`AgentHost::connect`] gave, and while Let Chat use the model is on
/// ([`AgentHost::set_model_access`]) each gets agent access, its own token on a loopback control
/// channel that is open only while a conversation holds it.
pub trait AgentHost: Send + Sync {
    /// Whether this system runs agents; answered at once, without looking at anything.
    fn system(&self) -> System {
        System::Supported
    }
    /// Looks for both programs; never touches the secure store, so the answer's store and keys
    /// are left unread.
    fn look(&self) -> Receiver<Look>;
    /// Opens the secure store and reads which keys are saved, by their last four: the only call
    /// that reads a key, made the first time the AI Agents page or Chat needs one.
    fn read_keys(&self) -> Receiver<Keys>;
    /// Saves `key` as `provider`'s, replacing the one saved before; the answer is the key as now
    /// kept, or why not in the host's words.
    fn save_key(&self, provider: Provider, key: KeyText) -> Receiver<Result<Key, String>>;
    /// Removes `provider`'s key; the answer is the key as now kept, or why not.
    fn remove_key(&self, provider: Provider) -> Receiver<Result<Key, String>>;
    /// Sends one short message to `provider` with the model given (Codex signed in as `sign_in`
    /// says), which costs a little of the plan or the API credit. It runs without agent access, in
    /// a scratch folder removed after.
    fn test_message(&self, provider: Provider, model: &str, sign_in: CodexSignIn) -> Receiver<Result<(), AccountError>>;
    /// Who Codex is signed in as, read from the codex program (which it starts); sends nothing.
    fn codex_account_read(&self) -> Receiver<Result<CodexAccount, AccountError>>;
    /// Starts signing Codex in with ChatGPT.
    fn codex_login_start(&self, way: LoginWay) -> Receiver<LoginEvent>;
    /// Stops a sign-in under way.
    fn codex_login_cancel(&self);
    /// Signs Codex out.
    fn codex_logout(&self) -> Receiver<Result<(), AccountError>>;
    /// Where the conversations' events and agent access's changes go from now on.
    fn connect(&self, sink: crate::chat::EventSink);
    /// Let Chat use the model, on or off. Off closes agent access at once: every conversation's
    /// token is revoked and the control channel closes; the conversations go on without it.
    fn set_model_access(&self, on: bool);
    /// Starts a conversation, answering at once: refused for a bad id, one already running, or past
    /// [`crate::chat::MAX_RUNNING`]; the rest (the program found, the key read, the start) comes
    /// as events, a failure as an end.
    fn start(&self, request: crate::chat::ChatStart) -> Result<(), crate::chat::ChatRefusal>;
    /// Sends the person's message to the running conversation `chat`.
    fn send(&self, chat: &str, text: String) -> Result<(), crate::chat::ChatRefusal>;
    /// Stops the turn `chat` runs; the conversation stays up.
    fn stop(&self, chat: &str) -> Result<(), crate::chat::ChatRefusal>;
    /// Ends the conversation `chat`: its program stops, its token is revoked, and the control
    /// channel closes with the last conversation that held it.
    fn end(&self, chat: &str) -> Result<(), crate::chat::ChatRefusal>;
    /// Removes the private folder of the conversation `chat` for good, once it was deleted
    /// (AR-217): a running program is ended first and waited for, a bounded time; a folder whose
    /// program has not ended by then is kept. The answer says whether there was a folder, or why it
    /// was not removed.
    fn forget(&self, chat: &str) -> Receiver<Result<bool, String>>;
}

/// How Codex signs in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum CodexSignIn {
    #[default]
    ChatGpt,
    ApiKey,
}

/// The choices of Settings → AI Agents that are kept in the preferences. None of them is a secret:
/// keys live in the system's secure store alone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AgentPrefs {
    pub default_agent: Provider,
    pub claude_model: String,
    pub codex_model: String,
    pub codex_sign_in: CodexSignIn,
    /// Let Chat use the model: off until the user turns it on.
    pub chat_uses_model: bool,
}

impl Default for AgentPrefs {
    fn default() -> Self {
        Self {
            default_agent: Provider::Claude,
            claude_model: "default".into(),
            codex_model: "default".into(),
            codex_sign_in: CodexSignIn::ChatGpt,
            chat_uses_model: false,
        }
    }
}

impl AgentPrefs {
    pub fn model(&self, p: Provider) -> &str {
        match p {
            Provider::Claude => &self.claude_model,
            Provider::Codex => &self.codex_model,
        }
    }

    pub fn set_model(&mut self, p: Provider, id: &str) {
        match p {
            Provider::Claude => self.claude_model = id.to_string(),
            Provider::Codex => self.codex_model = id.to_string(),
        }
    }

    /// A model the page does not offer goes back to the program's default, said in the log.
    pub fn sanitize(&mut self) {
        for p in Provider::ALL {
            if !p.models().iter().any(|(m, _)| *m == self.model(p)) {
                log::warn!("the stored {} model {:?} is not one Archforge offers; the program's default is used", p.name(), self.model(p));
                self.set_model(p, "default");
            }
        }
    }
}

/// How ready an agent is, as Check connection sums it up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Readiness {
    Ready,
    NotFound,
    NoKey,
    KeyRefused,
    NotSignedIn,
    SignInExpired,
    SignInNotAvailable,
    NotLooked,
    TooOld,
    Unusable,
    SignInUnreadable,
}

impl Readiness {
    /// The badge's tone: 0 ready, 1 missing something, 2 refused or expired.
    pub fn tone(self) -> u8 {
        match self {
            Readiness::Ready => 0,
            Readiness::KeyRefused | Readiness::SignInExpired => 2,
            _ => 1,
        }
    }
}

/// What the status badge under Check connection and Send test message says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Check connection: when (seconds since 1970), and each agent's readiness.
    Checked { at: u64, agents: Vec<(Provider, Readiness)> },
    /// A test message went and was answered.
    TestAnswered { at: u64, provider: Provider },
    /// A test message failed.
    TestFailed { at: u64, provider: Provider, error: AccountError },
}

/// A sign-in under way: how, the host's events, and the device code once it is given.
pub struct Login {
    pub way: LoginWay,
    rx: Receiver<LoginEvent>,
    pub code: Option<(String, String, u32)>,
}

/// AI Agents as the window knows it in this run.
#[derive(Default)]
pub struct State {
    /// What the last local look found.
    pub look: Look,
    looking: Option<Receiver<Look>>,
    asked: bool,
    /// The read of the secure store under way, and whether its answer is in.
    keys_reading: Option<Receiver<Keys>>,
    keys_known: bool,
    /// The keys saved or removed while the read runs: its answer, older than they are, leaves
    /// them as they are now.
    changed_while_reading: [bool; 2],
    /// Codex's account, once read: `None` before.
    pub codex: Option<Result<CodexAccount, AccountError>>,
    reading: Option<Receiver<Result<CodexAccount, AccountError>>>,
    /// Codex's account is wanted (the AI Agents page shows, or a Codex conversation started): it is
    /// read once the look has found the program.
    account_wanted: bool,
    /// Check connection waits for the look, the keys and the account read it started.
    checking: bool,
    pub outcome: Option<Outcome>,
    saving: [Option<Receiver<Result<Key, String>>>; 2],
    /// Why the last save or removal failed, per agent.
    pub key_error: [Option<String>; 2],
    /// The provider's refusal of the saved key, from the last test message, until the key changes.
    pub refused: [Option<String>; 2],
    testing: Option<(Provider, Receiver<Result<(), AccountError>>)>,
    pub login: Option<Login>,
    logout: Option<Receiver<Result<(), AccountError>>>,
    /// Why the last sign-in or sign-out failed.
    pub login_error: Option<AccountError>,
}

impl State {
    /// Whether anything is under way: the window looks again soon while it is.
    pub fn busy(&self) -> bool {
        self.looking.is_some()
            || self.keys_reading.is_some()
            || self.reading.is_some()
            || self.saving.iter().any(Option::is_some)
            || self.testing.is_some()
            || self.login.is_some()
            || self.logout.is_some()
    }

    pub fn looking(&self) -> bool {
        self.looking.is_some() || self.keys_reading.is_some()
    }

    /// Whether Codex's account is being read, or waits for the look to read it.
    pub fn reading_account(&self) -> bool {
        self.reading.is_some() || (self.account_wanted && self.codex.is_none() && self.looking.is_some())
    }

    /// Whether the keys have been read from the store in this run.
    pub fn keys_known(&self) -> bool {
        self.keys_known
    }

    pub fn checking(&self) -> bool {
        self.checking
    }

    pub fn saving(&self, p: Provider) -> bool {
        self.saving[p.index()].is_some()
    }

    pub fn testing(&self) -> bool {
        self.testing.is_some()
    }

    pub fn signing_out(&self) -> bool {
        self.logout.is_some()
    }

    /// Sets what a look found, as a host's answer does (offscreen screens and tests).
    pub fn set_look(&mut self, look: Look) {
        self.keys_known = look.claude_key != Key::NotRead || look.codex_key != Key::NotRead;
        self.look = look;
        self.asked = true;
    }
}

/// The look for the programs at start, once, when the host has the service and the system runs
/// agents. It reads no key: the secure store is left alone until agents are used.
pub(crate) fn start(shell: &mut crate::Shell) {
    if shell.agents.asked {
        return;
    }
    shell.agents.asked = true;
    look_programs(shell);
}

fn host(shell: &crate::Shell) -> Option<std::sync::Arc<dyn AgentHost>> {
    shell.services.agents.clone().filter(|h| h.system() == System::Supported)
}

fn look_programs(shell: &mut crate::Shell) {
    let Some(host) = host(shell) else { return };
    if shell.agents.looking.is_none() {
        shell.agents.looking = Some(host.look());
    }
}

/// Starts a read of the keys from the secure store, unless one runs already.
fn read_keys(shell: &mut crate::Shell) {
    let Some(host) = host(shell) else { return };
    if shell.agents.keys_reading.is_none() {
        shell.agents.changed_while_reading = [false; 2];
        shell.agents.keys_reading = Some(host.read_keys());
    }
}

/// Reads the keys from the secure store the first time they are needed (the AI Agents page, or
/// Chat); afterwards the answer is kept. A local read that sends nothing.
pub fn need_keys(shell: &mut crate::Shell) {
    if !shell.agents.keys_known {
        read_keys(shell);
    }
}

/// Looks again for the programs and reads the keys again (Look again); sends nothing.
pub fn look(shell: &mut crate::Shell) {
    look_programs(shell);
    read_keys(shell);
}

/// Check connection: the local look and Codex's account read, then a badge that sums up each
/// agent. It sends nothing.
pub fn check_connection(shell: &mut crate::Shell) {
    look(shell);
    shell.agents.checking = shell.agents.looking.is_some() || shell.agents.keys_reading.is_some();
    if !shell.agents.checking {
        // Nothing to look with: the badge says what is known.
        finish_check(shell);
    }
}

/// Saves `provider`'s key from what was typed.
pub fn save_key(shell: &mut crate::Shell, provider: Provider, key: KeyText) {
    let Some(host) = shell.services.agents.clone() else { return };
    let i = provider.index();
    // One change to a key at a time: a second waits for the first's answer (the key given is
    // overwritten as it drops).
    if shell.agents.saving[i].is_some() {
        return;
    }
    shell.agents.key_error[i] = None;
    shell.agents.saving[i] = Some(host.save_key(provider, key));
}

/// Removes `provider`'s key.
pub fn remove_key(shell: &mut crate::Shell, provider: Provider) {
    let Some(host) = shell.services.agents.clone() else { return };
    let i = provider.index();
    if shell.agents.saving[i].is_some() {
        return;
    }
    shell.agents.key_error[i] = None;
    shell.agents.saving[i] = Some(host.remove_key(provider));
}

/// Reads Codex's account the first time it is needed: the AI Agents page shows, or a Codex
/// conversation starts. Never at launch, as reading it starts the codex program. Once the look has
/// found the program it is read at once; before, it is read when the look answers.
pub fn need_account(shell: &mut crate::Shell) {
    if shell.agents.account_wanted {
        return;
    }
    shell.agents.account_wanted = true;
    read_account(shell);
}

/// Send test message: one short message to the default agent's provider.
pub fn send_test(shell: &mut crate::Shell) {
    let Some(accounts) = host(shell) else { return };
    if shell.agents.testing.is_some() {
        return;
    }
    let p = shell.state.agents.default_agent;
    let model = shell.state.agents.model(p).to_string();
    shell.agents.testing = Some((p, accounts.test_message(p, &model, shell.state.agents.codex_sign_in)));
}

/// Starts signing Codex in with ChatGPT.
pub fn sign_in(shell: &mut crate::Shell, way: LoginWay) {
    let Some(accounts) = host(shell) else { return };
    if shell.agents.login.is_some() {
        return;
    }
    shell.agents.login_error = None;
    shell.agents.login = Some(Login { way, rx: accounts.codex_login_start(way), code: None });
}

/// Stops the sign-in under way.
pub fn cancel_sign_in(shell: &mut crate::Shell) {
    if shell.agents.login.take().is_some()
        && let Some(accounts) = host(shell)
    {
        accounts.codex_login_cancel();
    }
}

/// Signs Codex out.
pub fn sign_out(shell: &mut crate::Shell) {
    let Some(accounts) = host(shell) else { return };
    shell.agents.login_error = None;
    shell.agents.logout = Some(accounts.codex_logout());
}

/// Starts reading Codex's account once the look found the program, unless a read runs already.
fn read_account(shell: &mut crate::Shell) {
    if shell.agents.reading.is_some() || !shell.agents.look.codex.found() {
        return;
    }
    if let Some(accounts) = host(shell) {
        shell.agents.reading = Some(accounts.codex_account_read());
    }
}

/// How ready `p` is now.
pub fn readiness(shell: &crate::Shell, p: Provider) -> Readiness {
    let a = &shell.agents;
    match a.look.program(p) {
        Program::NotLooked => return Readiness::NotLooked,
        Program::Found { .. } => {}
        Program::TooOld { .. } => return Readiness::TooOld,
        Program::Unusable { .. } => return Readiness::Unusable,
        Program::Missing => return Readiness::NotFound,
    }
    let by_key = |p: Provider| {
        if a.refused[p.index()].is_some() {
            Readiness::KeyRefused
        } else if a.look.key(p).saved() {
            Readiness::Ready
        } else {
            Readiness::NoKey
        }
    };
    match p {
        Provider::Claude => by_key(p),
        Provider::Codex if shell.state.agents.codex_sign_in == CodexSignIn::ApiKey => by_key(p),
        Provider::Codex => match &a.codex {
            Some(Ok(CodexAccount::SignedIn { .. })) => Readiness::Ready,
            Some(Ok(CodexAccount::Expired { .. })) => Readiness::SignInExpired,
            Some(Err(AccountError::NotAvailableYet)) => Readiness::SignInNotAvailable,
            Some(Err(_)) => Readiness::SignInUnreadable,
            Some(Ok(CodexAccount::NotSignedIn)) => Readiness::NotSignedIn,
            None => Readiness::NotLooked,
        },
    }
}

fn finish_check(shell: &mut crate::Shell) {
    shell.agents.checking = false;
    let agents = Provider::ALL.iter().map(|p| (*p, readiness(shell, *p))).collect();
    shell.agents.outcome = Some(Outcome::Checked { at: crate::home::now(), agents });
}

/// Takes in what the host has answered; once per frame. A sign-in's page opens in the browser
/// through `ctx` when it is the codex program's sign-in host.
pub(crate) fn poll(shell: &mut crate::Shell, ctx: &egui::Context) {
    if let Some(rx) = &shell.agents.looking {
        match rx.try_recv() {
            Ok(look) => {
                shell.agents.looking = None;
                // The programs only: the store and the keys come from their own read.
                shell.agents.look.claude = look.claude;
                shell.agents.look.codex = look.codex;
                // Codex's account only once it is wanted, never at launch.
                if shell.agents.account_wanted && shell.agents.codex.is_none() {
                    read_account(shell);
                }
            }
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => {
                shell.agents.looking = None;
                log::warn!("the look for the agent programs stopped without an answer");
            }
        }
    }
    if let Some(rx) = &shell.agents.keys_reading {
        let answer = match rx.try_recv() {
            Ok(keys) => Some(keys),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                let why = tr("The secure store stopped without an answer");
                Some(Keys {
                    store: Store::NotLooked,
                    claude_key: Key::Unreadable { why: why.clone() },
                    codex_key: Key::Unreadable { why },
                    sign_in_files: false,
                })
            }
        };
        if let Some(keys) = answer {
            shell.agents.keys_reading = None;
            shell.agents.keys_known = true;
            let look = &mut shell.agents.look;
            look.store = keys.store;
            look.sign_in_files = keys.sign_in_files;
            for (p, key) in [(Provider::Claude, keys.claude_key), (Provider::Codex, keys.codex_key)] {
                // A key saved or removed while the read ran is newer than what the read found.
                if std::mem::take(&mut shell.agents.changed_while_reading[p.index()]) {
                    continue;
                }
                // A key that changed since a refusal is no longer the one refused.
                if &key != look.key(p) && look.key(p) != &Key::NotRead {
                    shell.agents.refused[p.index()] = None;
                }
                *look.key_mut(p) = key;
            }
        }
    }
    if let Some(rx) = &shell.agents.reading {
        match rx.try_recv() {
            Ok(answer) => {
                shell.agents.reading = None;
                shell.agents.codex = Some(answer);
            }
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => {
                shell.agents.reading = None;
                shell.agents.codex = Some(Err(AccountError::Failed { detail: tr("The look stopped without an answer") }));
            }
        }
    }
    if shell.agents.checking && shell.agents.looking.is_none() && shell.agents.keys_reading.is_none() && shell.agents.reading.is_none() {
        finish_check(shell);
    }
    for p in Provider::ALL {
        let i = p.index();
        let Some(rx) = &shell.agents.saving[i] else { continue };
        match rx.try_recv() {
            Ok(Ok(key)) => {
                shell.agents.saving[i] = None;
                shell.agents.refused[i] = None;
                if shell.agents.keys_reading.is_some() {
                    shell.agents.changed_while_reading[i] = true;
                }
                *shell.agents.look.key_mut(p) = key;
            }
            Ok(Err(why)) => {
                shell.agents.saving[i] = None;
                shell.agents.key_error[i] = Some(why);
            }
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => {
                shell.agents.saving[i] = None;
                shell.agents.key_error[i] = Some(tr("The secure store stopped without an answer"));
            }
        }
    }
    if let Some((p, rx)) = &shell.agents.testing {
        let p = *p;
        let answer = match rx.try_recv() {
            Ok(a) => Some(a),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(Err(AccountError::Failed { detail: tr("The test stopped without an answer") })),
        };
        if let Some(answer) = answer {
            shell.agents.testing = None;
            let at = crate::home::now();
            shell.agents.outcome = Some(match answer {
                Ok(()) => {
                    shell.agents.refused[p.index()] = None;
                    Outcome::TestAnswered { at, provider: p }
                }
                Err(error) => {
                    if let AccountError::KeyRefused { detail } = &error {
                        shell.agents.refused[p.index()] = Some(detail.clone());
                    }
                    Outcome::TestFailed { at, provider: p, error }
                }
            });
        }
    }
    let mut login_over = false;
    if let Some(login) = &mut shell.agents.login {
        loop {
            match login.rx.try_recv() {
                // Only a code of letters, digits and hyphens is shown: nothing that hides or
                // reorders text reaches the page.
                Ok(LoginEvent::DeviceCode { code, url, minutes }) if device_code_ok(&code) => login.code = Some((code, url, minutes)),
                // The browser opens only on the codex program's sign-in host.
                Ok(LoginEvent::OpenBrowser { url }) => match approved_browser_page(&url) {
                    Some(page) => ctx.open_url(egui::OpenUrl::new_tab(page)),
                    None => {
                        shell.agents.login_error = Some(AccountError::Failed { detail: tr("Not an address Archforge opens") });
                        login_over = true;
                        break;
                    }
                },
                Ok(LoginEvent::DeviceCode { .. }) => {
                    shell.agents.login_error =
                        Some(AccountError::Failed { detail: tr("The sign-in gave a code that is not letters, digits and hyphens.") });
                    login_over = true;
                    break;
                }
                Ok(LoginEvent::Done(account)) => {
                    shell.agents.codex = Some(Ok(account));
                    login_over = true;
                    break;
                }
                Ok(LoginEvent::Failed(e)) => {
                    shell.agents.login_error = Some(e);
                    login_over = true;
                    break;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    shell.agents.login_error = Some(AccountError::Failed { detail: tr("The sign-in stopped without an answer") });
                    login_over = true;
                    break;
                }
            }
        }
    }
    if login_over {
        shell.agents.login = None;
    }
    if let Some(rx) = &shell.agents.logout {
        let answer = match rx.try_recv() {
            Ok(a) => Some(a),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(Err(AccountError::Failed { detail: tr("The sign-out stopped without an answer") })),
        };
        if let Some(answer) = answer {
            shell.agents.logout = None;
            match answer {
                Ok(()) => shell.agents.codex = Some(Ok(CodexAccount::NotSignedIn)),
                Err(e) => shell.agents.login_error = Some(e),
            }
        }
    }
}

/// Whether a device code is one the page shows: 1 to 32 ASCII letters, digits and hyphens.
fn device_code_ok(code: &str) -> bool {
    (1..=32).contains(&code.len()) && code.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

/// Only the codex program's sign-in page opens from the device code: an HTTPS address on
/// OpenAI's sign-in host.
pub fn approved_sign_in_page(url: &str) -> Option<&str> {
    let rest = url.strip_prefix("https://auth.openai.com/")?;
    let ok = rest.len() <= 128 && rest.chars().all(|c| c.is_ascii_alphanumeric() || "/-_".contains(c));
    ok.then_some(url)
}

/// The page a browser sign-in opens: an HTTPS address on OpenAI's sign-in host, at most 8 KiB of
/// printable ASCII without spaces (its query names the sign-in), or none.
pub fn approved_browser_page(url: &str) -> Option<&str> {
    let rest = url.strip_prefix("https://auth.openai.com/")?;
    let ok = url.len() <= 8 * 1024 && rest.bytes().all(|b| b.is_ascii_graphic() && b != b'\\' && b != b'"');
    ok.then_some(url)
}

/// A host for tests and the offscreen screens: answers each call with what it was set up with,
/// at once, and counts the calls.
#[cfg(any(test, feature = "snapshot"))]
pub mod fake {
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    /// The calls made, by kind.
    #[derive(Debug, Default)]
    pub struct Calls {
        pub look: AtomicUsize,
        pub keys: AtomicUsize,
        pub save: AtomicUsize,
        pub remove: AtomicUsize,
        pub test: AtomicUsize,
        pub account: AtomicUsize,
        pub login: AtomicUsize,
        pub cancel: AtomicUsize,
        pub logout: AtomicUsize,
        pub connect: AtomicUsize,
        pub start: AtomicUsize,
        pub send: AtomicUsize,
        pub stop: AtomicUsize,
        pub end: AtomicUsize,
    }

    impl Calls {
        /// Every call that sends something or runs a program for an account: tests, sign-ins.
        pub fn outward(&self) -> usize {
            self.test.load(Ordering::Relaxed) + self.login.load(Ordering::Relaxed) + self.logout.load(Ordering::Relaxed)
        }
        pub fn total(&self) -> usize {
            self.outward()
                + self.look.load(Ordering::Relaxed)
                + self.keys.load(Ordering::Relaxed)
                + self.save.load(Ordering::Relaxed)
                + self.remove.load(Ordering::Relaxed)
                + self.account.load(Ordering::Relaxed)
                + self.cancel.load(Ordering::Relaxed)
        }
    }

    /// The fake's answers, and the keys it keeps through the store it was given.
    pub struct Fake {
        pub system: System,
        pub look: Mutex<Look>,
        pub account: Mutex<Result<CodexAccount, AccountError>>,
        pub test: Mutex<Result<(), AccountError>>,
        pub login: Mutex<Vec<LoginEvent>>,
        /// Keys as the store keeps them: the last four of a key saved; the whole key is dropped.
        pub saved: Mutex<Vec<(Provider, String)>>,
        pub calls: Calls,
        /// The open sign-ins' senders: a sign-in set up without an ending waits, as a real one
        /// does until the browser finishes.
        pub(crate) open: Mutex<Vec<std::sync::mpsc::Sender<LoginEvent>>>,
        /// Holds each read of the keys until [`Fake::release_keys`], as a slow store would.
        pub hold_keys: bool,
        pub(crate) held: Mutex<Vec<(std::sync::mpsc::Sender<Keys>, Keys)>>,
        /// Where the shell takes the events, once it connected.
        pub sink: Mutex<Option<crate::chat::EventSink>>,
        /// Let Chat use the model, as each call told it.
        pub model_access: Mutex<Vec<bool>>,
        /// The conversations started, in order, and the ones running.
        pub started: Mutex<Vec<crate::chat::ChatStart>>,
        pub running: Mutex<Vec<String>>,
        /// The messages sent, by conversation, in order.
        pub sent: Mutex<Vec<(String, String)>>,
        /// While set, an end is taken but not said, as a program slow to end would leave it.
        pub hold_ends: std::sync::atomic::AtomicBool,
        /// The conversations whose private folders were removed, in order.
        pub forgotten: Mutex<Vec<String>>,
    }

    impl Default for Fake {
        fn default() -> Self {
            Fake {
                system: System::Supported,
                look: Mutex::new(Look::default()),
                account: Mutex::new(Ok(CodexAccount::NotSignedIn)),
                test: Mutex::new(Ok(())),
                login: Mutex::new(Vec::new()),
                saved: Mutex::new(Vec::new()),
                calls: Calls::default(),
                open: Mutex::new(Vec::new()),
                hold_keys: false,
                held: Mutex::new(Vec::new()),
                sink: Mutex::new(None),
                model_access: Mutex::new(Vec::new()),
                started: Mutex::new(Vec::new()),
                running: Mutex::new(Vec::new()),
                sent: Mutex::new(Vec::new()),
                hold_ends: std::sync::atomic::AtomicBool::new(false),
                forgotten: Mutex::new(Vec::new()),
            }
        }
    }

    fn answered<T: Send + 'static>(value: T) -> Receiver<T> {
        let (tx, rx) = std::sync::mpsc::channel();
        let _ = tx.send(value);
        rx
    }

    fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
        m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    impl AgentHost for Fake {
        fn system(&self) -> System {
            self.system
        }
        fn look(&self) -> Receiver<Look> {
            self.calls.look.fetch_add(1, Ordering::Relaxed);
            let look = lock(&self.look);
            answered(Look { claude: look.claude.clone(), codex: look.codex.clone(), ..Look::default() })
        }
        /// The keys as the look holds them, answered at once or held until released.
        fn read_keys(&self) -> Receiver<Keys> {
            self.calls.keys.fetch_add(1, Ordering::Relaxed);
            let look = lock(&self.look);
            let keys = Keys {
                store: look.store.clone(),
                claude_key: look.claude_key.clone(),
                codex_key: look.codex_key.clone(),
                sign_in_files: look.sign_in_files,
            };
            if self.hold_keys {
                let (tx, rx) = std::sync::mpsc::channel();
                lock(&self.held).push((tx, keys));
                return rx;
            }
            answered(keys)
        }
        fn save_key(&self, provider: Provider, key: KeyText) -> Receiver<Result<Key, String>> {
            self.calls.save.fetch_add(1, Ordering::Relaxed);
            let mut look = lock(&self.look);
            if look.store.missing() {
                return answered(Err("there is no secure store on this system".into()));
            }
            let kept = Key::Saved { last4: last_four(key.expose()) };
            *look.key_mut(provider) = kept.clone();
            lock(&self.saved).push((provider, last_four(key.expose())));
            answered(Ok(kept))
        }
        fn remove_key(&self, provider: Provider) -> Receiver<Result<Key, String>> {
            self.calls.remove.fetch_add(1, Ordering::Relaxed);
            *lock(&self.look).key_mut(provider) = Key::Absent;
            answered(Ok(Key::Absent))
        }
        /// Keeps the shell's sink, so the fake can emit events.
        fn connect(&self, sink: crate::chat::EventSink) {
            self.calls.connect.fetch_add(1, Ordering::Relaxed);
            *lock(&self.sink) = Some(sink);
        }
        /// Records the opt-in as told.
        fn set_model_access(&self, on: bool) {
            lock(&self.model_access).push(on);
        }
        /// Starts a conversation as the host does: a bad id, one running, or past the cap refused.
        fn start(&self, request: crate::chat::ChatStart) -> Result<(), crate::chat::ChatRefusal> {
            self.calls.start.fetch_add(1, Ordering::Relaxed);
            crate::chat::check_id(&request.chat)?;
            let mut running = lock(&self.running);
            if running.contains(&request.chat) {
                return Err(crate::chat::ChatRefusal::AlreadyRunning);
            }
            if running.len() >= crate::chat::MAX_RUNNING {
                return Err(crate::chat::ChatRefusal::TooMany { most: crate::chat::MAX_RUNNING });
            }
            running.push(request.chat.clone());
            lock(&self.started).push(request);
            Ok(())
        }
        /// Takes a message for a running conversation, and keeps it.
        fn send(&self, chat: &str, text: String) -> Result<(), crate::chat::ChatRefusal> {
            self.calls.send.fetch_add(1, Ordering::Relaxed);
            self.known(chat)?;
            lock(&self.sent).push((chat.to_string(), text));
            Ok(())
        }
        /// Takes a stop for a running conversation.
        fn stop(&self, chat: &str) -> Result<(), crate::chat::ChatRefusal> {
            self.calls.stop.fetch_add(1, Ordering::Relaxed);
            self.known(chat)
        }
        /// Ends a running conversation and says so on the sink.
        fn end(&self, chat: &str) -> Result<(), crate::chat::ChatRefusal> {
            self.calls.end.fetch_add(1, Ordering::Relaxed);
            self.known(chat)?;
            if self.hold_ends.load(Ordering::Relaxed) {
                return Ok(());
            }
            lock(&self.running).retain(|c| c != chat);
            self.emit(chat, crate::chat::ChatEvent::Ended { failure: None });
            Ok(())
        }
        /// Ends the conversation when it runs, then records its folder as removed.
        fn forget(&self, chat: &str) -> Receiver<Result<bool, String>> {
            if let Err(e) = crate::chat::check_id(chat) {
                return answered(Err(e.to_string()));
            }
            if lock(&self.running).iter().any(|c| c == chat) {
                let _ = self.end(chat);
            }
            lock(&self.forgotten).push(chat.to_string());
            answered(Ok(true))
        }
        fn test_message(&self, _provider: Provider, _model: &str, _sign_in: CodexSignIn) -> Receiver<Result<(), AccountError>> {
            self.calls.test.fetch_add(1, Ordering::Relaxed);
            answered(lock(&self.test).clone())
        }
        fn codex_account_read(&self) -> Receiver<Result<CodexAccount, AccountError>> {
            self.calls.account.fetch_add(1, Ordering::Relaxed);
            answered(lock(&self.account).clone())
        }
        fn codex_login_start(&self, _way: LoginWay) -> Receiver<LoginEvent> {
            self.calls.login.fetch_add(1, Ordering::Relaxed);
            let (tx, rx) = std::sync::mpsc::channel();
            for e in lock(&self.login).iter().cloned() {
                let _ = tx.send(e);
            }
            lock(&self.open).push(tx);
            rx
        }
        fn codex_login_cancel(&self) {
            self.calls.cancel.fetch_add(1, Ordering::Relaxed);
        }
        fn codex_logout(&self) -> Receiver<Result<(), AccountError>> {
            self.calls.logout.fetch_add(1, Ordering::Relaxed);
            *lock(&self.account) = Ok(CodexAccount::NotSignedIn);
            answered(Ok(()))
        }
    }

    impl Fake {
        /// Refuses a conversation that is not running.
        fn known(&self, chat: &str) -> Result<(), crate::chat::ChatRefusal> {
            if lock(&self.running).iter().any(|c| c == chat) { Ok(()) } else { Err(crate::chat::ChatRefusal::NotRunning) }
        }

        /// Sends `event` of `chat` to the shell, as a running conversation would; false before the
        /// shell connected or once it is gone.
        pub fn emit(&self, chat: &str, event: crate::chat::ChatEvent) -> bool {
            let sink = lock(&self.sink).clone();
            sink.is_some_and(|s| s.send(crate::chat::HostEvent::Chat { chat: chat.to_string(), event, ticket: crate::chat::Ticket::uncounted() }))
        }

        /// Says agent access opened or closed, as the host does.
        pub fn access(&self, held: bool) -> bool {
            let sink = lock(&self.sink).clone();
            sink.is_some_and(|s| s.send(crate::chat::HostEvent::Access { held }))
        }

        /// Answers the reads of the keys held so far, with what the store held when each began.
        pub fn release_keys(&self) {
            for (tx, keys) in lock(&self.held).drain(..) {
                let _ = tx.send(keys);
            }
        }
    }

    /// A found program of the version Archforge was tested with.
    pub fn found(p: Provider) -> Program {
        Program::Found { version: if p == Provider::Claude { "2.1.317".into() } else { "0.166.0".into() }, tested: true }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_key_is_kept_as_one_word_and_shown_by_its_last_four() {
        assert_eq!(
            KeyText::new("  sk-ant-api03-abcdefgh4F2a \n".into()).map(|k| k.expose().to_string()).ok().as_deref(),
            Some("sk-ant-api03-abcdefgh4F2a")
        );
        assert_eq!(KeyText::new("   ".into()).err(), Some(KeyRefusal::Empty));
        assert_eq!(KeyText::new("sk ant".into()).err(), Some(KeyRefusal::NotOneWord));
        assert_eq!(KeyText::new("x".repeat(MAX_KEY_BYTES + 1)).err(), Some(KeyRefusal::TooLong));
        assert_eq!(last_four("sk-ant-api03-abcdefgh4F2a"), "4F2a");
        assert_eq!(last_four("short-key"), "", "a short key shows nothing of itself");
        assert_eq!(last_four("ключ-ключ-ключ-ёжик"), "ёжик", "characters, not bytes");
        assert_eq!(format!("{:?}", KeyText::new("sk-secret-canary-0000".into()).ok()), "Some(KeyText(<redacted>))");
    }

    #[test]
    fn only_openais_sign_in_host_opens_from_a_device_code() {
        assert!(approved_sign_in_page("https://auth.openai.com/codex/device").is_some());
        for refused in [
            "http://auth.openai.com/codex/device",
            "https://auth.openai.com.evil.example/",
            "https://evil.example/?https://auth.openai.com/",
            "https://auth.openai.com/a?b=c",
        ] {
            assert!(approved_sign_in_page(refused).is_none(), "{refused}");
        }
    }

    #[test]
    fn a_stored_model_the_page_does_not_offer_goes_back_to_the_default() {
        let mut prefs = AgentPrefs { claude_model: "gpt-9".into(), codex_model: "default".into(), ..AgentPrefs::default() };
        prefs.sanitize();
        assert_eq!(prefs.claude_model, "default");
        let mut kept = AgentPrefs { claude_model: "opus".into(), ..AgentPrefs::default() };
        kept.sanitize();
        assert_eq!(kept.claude_model, "opus");
    }
}
