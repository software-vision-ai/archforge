//! JSON-RPC with `codex app-server` over its standard input and output, as the AR-196 spike
//! observed it in 0.166.0: one JSON object a line, without the `"jsonrpc"` field. [`Conn`] owns the
//! runtime process, numbers the requests, sorts each line into an answer, a notification or a
//! request of Codex's own, and carries the end of the process: the exit status and the error
//! stream's tail when Codex ends, and a bounded end when Archforge ends it.

use std::time::{Duration, Instant};

use serde_json::{Map, Value, json};
use zeroize::Zeroizing;

use crate::error::capped;
use crate::session::Skipped;
use crate::{AgentError, ErrorKind, Exit, OutputEvent, RuntimeKind, RuntimeProcess};

/// The most output lines one poll of a session or an account process reads in one call, so a Codex
/// writing only lines that mean nothing cannot hold its caller.
pub(crate) const LINES_PER_CALL: u32 = 256;
/// How long an ending Codex is given at each step: to report its exit once its output closed, to
/// end on its own once its input is closed, and to be collected once it was terminated.
pub(crate) const END_GRACE: Duration = Duration::from_millis(500);
/// The pause between two looks at a Codex that is ending, within the caller's time.
const END_POLL: Duration = Duration::from_millis(10);
/// The longest id Archforge takes from Codex (a thread, a turn, an item, a model), in bytes. A
/// longer one is refused, never cut, as ids are sent back.
pub const MAX_ID_BYTES: usize = 256;
/// The longest kind name kept from a line Codex wrote, for the log and [`Skipped::Unknown`].
const MAX_KIND_BYTES: usize = 64;

/// The JSON-RPC code Codex 0.166.0 answers an unknown method with (an invalid request, not -32601).
pub const INVALID_REQUEST: i64 = -32600;
/// The JSON-RPC code for a method the server does not have, which other versions may use.
pub const METHOD_NOT_FOUND: i64 = -32601;
/// The JSON-RPC code Codex answers internal failures with, an expired sign-in among them.
pub const INTERNAL_ERROR: i64 = -32603;

/// The sentences Codex 0.166.0 answers with (code -32603) when the ChatGPT sign-in's tokens can no
/// longer be refreshed, quoted from `codex-rs/login/src/auth/manager.rs` at tag `rust-v0.166.0`
/// (`REFRESH_TOKEN_EXPIRED_MESSAGE`, `REFRESH_TOKEN_REUSED_MESSAGE`,
/// `REFRESH_TOKEN_INVALIDATED_MESSAGE`, `REFRESH_TOKEN_UNKNOWN_MESSAGE`,
/// `REFRESH_TOKEN_ACCOUNT_MISMATCH_MESSAGE`). Only these, word for word, mean an expired sign-in.
pub const EXPIRED_SIGN_IN_MESSAGES: [&str; 5] = [
    "Your access token could not be refreshed because your refresh token has expired. Please log out and sign in again.",
    "Your access token could not be refreshed because your refresh token was already used. Please log out and sign in again.",
    "Your access token could not be refreshed because your refresh token was revoked. Please log out and sign in again.",
    "Your access token could not be refreshed. Please log out and sign in again.",
    "Your access token could not be refreshed because you have since logged out or signed in to another account. Please sign in again.",
];

/// An error answer from Codex: its code and its message, cut to [`crate::MAX_DETAIL_BYTES`] with
/// every secret Codex was given replaced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
}

impl RpcError {
    /// Whether this error says Codex does not know `method` at all: code -32600 (as 0.166.0
    /// answers) or -32601, and the first unknown variant the message names is `method` itself. An
    /// unknown value inside the parameters (`unknown variant `foo`` for a sign-in type) names
    /// another variant first and is not an unknown method.
    pub fn unknown_method(&self, method: &str) -> bool {
        if self.code != INVALID_REQUEST && self.code != METHOD_NOT_FOUND {
            return false;
        }
        const MARK: &str = "unknown variant `";
        let Some(at) = self.message.find(MARK) else { return false };
        let rest = self.message.get(at.saturating_add(MARK.len())..).unwrap_or_default();
        rest.split_once('`').is_some_and(|(variant, _)| variant == method)
    }

    /// Whether this error says the ChatGPT sign-in has expired: code -32603 with exactly one of
    /// [`EXPIRED_SIGN_IN_MESSAGES`]. Loose words ("revoked", "expired") elsewhere never count.
    pub fn expired_sign_in(&self) -> bool {
        self.code == INTERNAL_ERROR && EXPIRED_SIGN_IN_MESSAGES.contains(&self.message.as_str())
    }

    /// This answer as the error of the request `request`: an expired sign-in is
    /// [`ErrorKind::SignInExpired`], `refused_kind` (where given) the category of any other refusal,
    /// and otherwise [`AgentError::Refused`], which knows an unknown method.
    pub fn into_error(self, request: &'static str, refused_kind: Option<ErrorKind>) -> AgentError {
        if self.expired_sign_in() {
            return AgentError::reported(ErrorKind::SignInExpired, &format!("the ChatGPT sign-in has expired: {}", self.message));
        }
        let unknown_method = self.unknown_method(request);
        match refused_kind {
            Some(kind) if !unknown_method => AgentError::reported(kind, &format!("codex refused {request} (error {}): {}", self.code, self.message)),
            _ => AgentError::Refused { runtime: RuntimeKind::Codex, request, code: self.code, message: self.message, unknown_method },
        }
    }
}

/// One thing Codex said.
#[derive(Debug)]
pub(crate) enum Incoming {
    /// The answer to the request numbered `id`.
    Response { id: u64, result: Result<Value, RpcError> },
    /// A notification.
    Notification { method: String, params: Value },
    /// A request of Codex's own, which must be answered; `id` is echoed back as it came.
    Request { id: Value, method: String },
    /// A line that was not understood: counted by the owner.
    Skipped(Skipped),
    /// Codex has ended (or was given up on): the last thing a connection says. `error` is why when
    /// the end was not asked for with a shutdown.
    Ended { exit: Option<Exit>, error: Option<AgentError> },
}

/// Where the connection is in its life.
#[derive(Debug)]
enum Phase {
    /// Codex runs and its output is open.
    Open,
    /// Codex closed its output at `since`; its exit is awaited for up to [`END_GRACE`].
    Closed { since: Instant },
    /// Archforge is ending Codex for `reason`: its input was closed at `since`; it is terminated
    /// [`END_GRACE`] later, and given up on [`END_GRACE`] after that.
    Ending { reason: AgentError, since: Instant, terminated: Option<Instant> },
    /// [`Incoming::Ended`] was given; nothing more comes.
    Done,
}

/// A running `codex app-server` and the JSON-RPC spoken with it.
pub(crate) struct Conn {
    process: RuntimeProcess,
    next_id: u64,
    phase: Phase,
    shutting_down: bool,
}

impl std::fmt::Debug for Conn {
    /// Writes the process and the state, never a line.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Conn").field("process", &self.process).field("next_id", &self.next_id).field("phase", &self.phase).finish()
    }
}

impl Conn {
    /// A connection over `process`, which must be a freshly started `codex app-server`.
    pub(crate) fn new(process: RuntimeProcess) -> Conn {
        Conn { process, next_id: 1, phase: Phase::Open, shutting_down: false }
    }

    /// The runtime process.
    pub(crate) fn process(&self) -> &RuntimeProcess {
        &self.process
    }

    /// Whether requests can still be sent: Codex runs, its output is open and Archforge is not
    /// ending it.
    pub(crate) fn is_open(&self) -> bool {
        matches!(self.phase, Phase::Open)
    }

    /// `text` with every secret Codex was given replaced.
    pub(crate) fn redact(&self, text: String) -> String {
        self.process.redact(text)
    }

    /// Sends the request `method` with `params` (none when `None`) and answers its number. One line
    /// is queued; nothing waits. Refused once the connection is not open, and when the process
    /// refuses the line (a full input queue among them).
    pub(crate) fn request(&mut self, method: &'static str, params: Option<Value>) -> Result<u64, AgentError> {
        if !self.is_open() {
            return Err(AgentError::InputClosed { runtime: RuntimeKind::Codex });
        }
        let id = self.next_id;
        let mut message = Map::new();
        message.insert("id".to_owned(), json!(id));
        message.insert("method".to_owned(), json!(method));
        if let Some(params) = params {
            message.insert("params".to_owned(), params);
        }
        self.process.send_line(&Value::Object(message).to_string())?;
        self.next_id = self.next_id.saturating_add(1);
        Ok(id)
    }

    /// Sends a request whose line holds a secret: `line` makes the whole line from the request's
    /// number, in a buffer overwritten when dropped, as the process overwrites its own copy once
    /// written. Answers the number; refused as [`Conn::request`] is.
    pub(crate) fn request_secret(&mut self, line: impl FnOnce(u64) -> Zeroizing<String>) -> Result<u64, AgentError> {
        if !self.is_open() {
            return Err(AgentError::InputClosed { runtime: RuntimeKind::Codex });
        }
        let id = self.next_id;
        let text = line(id);
        self.process.send_line(&text)?;
        self.next_id = self.next_id.saturating_add(1);
        Ok(id)
    }

    /// Sends the notification `method`, without parameters.
    pub(crate) fn notify(&mut self, method: &'static str) -> Result<(), AgentError> {
        if !self.is_open() {
            return Err(AgentError::InputClosed { runtime: RuntimeKind::Codex });
        }
        self.process.send_line(&json!({ "method": method }).to_string())
    }

    /// Answers Codex's request `id` with `result`.
    pub(crate) fn respond(&mut self, id: &Value, result: Value) -> Result<(), AgentError> {
        if !self.is_open() {
            return Err(AgentError::InputClosed { runtime: RuntimeKind::Codex });
        }
        self.process.send_line(&json!({ "id": id, "result": result }).to_string())
    }

    /// Answers Codex's request `id` with the error `code` and `message`.
    pub(crate) fn respond_error(&mut self, id: &Value, code: i64, message: &str) -> Result<(), AgentError> {
        if !self.is_open() {
            return Err(AgentError::InputClosed { runtime: RuntimeKind::Codex });
        }
        self.process.send_line(&json!({ "id": id, "error": { "code": code, "message": message } }).to_string())
    }

    /// Starts ending Codex because of `reason`, which the end will carry: its input is closed now,
    /// it is terminated if it has not ended [`END_GRACE`] later, and given up on [`END_GRACE`] after
    /// that, so [`Incoming::Ended`] always comes within twice [`END_GRACE`]. Nothing here waits; the
    /// steps happen in [`Conn::poll`]. Once Codex closed its output on its own, its end is already
    /// coming with its exit status and error tail, which say more, so `reason` is only logged; so is
    /// a later reason once ending.
    pub(crate) fn fail(&mut self, reason: AgentError) {
        match self.phase {
            Phase::Open => {
                log::warn!("codex is being ended: {reason}");
                self.process.close_input();
                self.phase = Phase::Ending { reason, since: Instant::now(), terminated: None };
            }
            Phase::Closed { .. } | Phase::Ending { .. } | Phase::Done => {
                log::debug!("codex is already ending; a further reason is not kept: {reason}")
            }
        }
    }

    /// Ends Codex as [`RuntimeProcess::shutdown`] does (waiting up to twice `grace`); the end it
    /// then reports carries no error, unless Archforge was already ending it for one.
    pub(crate) fn shutdown(&mut self, grace: Duration) -> Result<Exit, AgentError> {
        self.shutting_down = true;
        self.process.shutdown(grace)
    }

    /// Collects an ended Codex without waiting, for the owner's drop and polls.
    pub(crate) fn reap(&mut self) {
        let _ = self.process.try_reap();
    }

    /// The next thing Codex said, never waiting past `deadline`: with a deadline already passed
    /// (a zero timeout) whatever is ready is read and nothing waited for. Each line read takes one
    /// from `budget`; at zero no further line is read. While Codex ends, its exit is awaited in steps
    /// of at most [`END_POLL`], never past `deadline`, and lines it still writes are drained without
    /// meaning anything. `None` when nothing came in time or the budget is spent.
    pub(crate) fn poll(&mut self, deadline: Instant, budget: &mut u32) -> Option<Incoming> {
        loop {
            match &self.phase {
                Phase::Done => return None,
                Phase::Open => {
                    if *budget == 0 {
                        return None;
                    }
                    let left = deadline.saturating_duration_since(Instant::now());
                    let event = if left.is_zero() { self.process.try_next_event() } else { self.process.next_event(left) };
                    let event = event?;
                    match event {
                        OutputEvent::Line(line) => {
                            *budget = budget.saturating_sub(1);
                            let line = self.process.redact(line);
                            return Some(classify(&line));
                        }
                        OutputEvent::Dropped(why) => {
                            *budget = budget.saturating_sub(1);
                            return Some(Incoming::Skipped(match why {
                                crate::Dropped::TooLong { bytes } => Skipped::TooLong { bytes },
                                crate::Dropped::NotText { bytes } => Skipped::NotText { bytes },
                            }));
                        }
                        OutputEvent::ReadFailed(detail) => log::warn!("codex's output could not be read: {detail}"),
                        OutputEvent::End => {
                            self.process.close_input();
                            self.phase = Phase::Closed { since: Instant::now() };
                        }
                    }
                }
                Phase::Closed { since } => {
                    let since = *since;
                    let now = Instant::now();
                    let exit = self.process.try_reap();
                    let grace_over = now.saturating_duration_since(since) >= END_GRACE;
                    if let Some(exit) = exit {
                        // The whole error tail, unless it stays open past the grace.
                        if self.process.stderr_closed() || grace_over {
                            let error = if self.shutting_down { None } else { Some(self.process.exit_error(exit)) };
                            self.phase = Phase::Done;
                            return Some(Incoming::Ended { exit: Some(exit), error });
                        }
                    } else if grace_over {
                        // Its output is closed but it has not ended: it is ended now (its input is
                        // already closed and the grace is spent).
                        let reason = AgentError::reported(ErrorKind::Other, "codex closed its output but did not end, so it was terminated");
                        log::warn!("{reason}");
                        self.process.terminate();
                        self.phase = Phase::Ending { reason, since, terminated: Some(now) };
                        continue;
                    }
                    if !self.pause(deadline, since.checked_add(END_GRACE)) {
                        return None;
                    }
                }
                Phase::Ending { since, terminated, .. } => {
                    let (since, terminated) = (*since, *terminated);
                    // Lines still written mean nothing now; drained so Codex is not held on a full pipe.
                    while *budget > 0 {
                        match self.process.try_next_event() {
                            Some(OutputEvent::Line(_) | OutputEvent::Dropped(_) | OutputEvent::ReadFailed(_)) => *budget = budget.saturating_sub(1),
                            Some(OutputEvent::End) | None => break,
                        }
                    }
                    let now = Instant::now();
                    if let Some(exit) = self.process.try_reap() {
                        return Some(self.end(Some(exit)));
                    }
                    match terminated {
                        None if now.saturating_duration_since(since) >= END_GRACE => {
                            self.process.terminate();
                            if let Phase::Ending { terminated, .. } = &mut self.phase {
                                *terminated = Some(now);
                            }
                            continue;
                        }
                        Some(at) if now.saturating_duration_since(at) >= END_GRACE => {
                            log::warn!("codex was terminated but its end was not seen within {} ms; it is collected later", END_GRACE.as_millis());
                            return Some(self.end(None));
                        }
                        _ => {}
                    }
                    let next = match terminated {
                        Some(at) => at.checked_add(END_GRACE),
                        None => since.checked_add(END_GRACE),
                    };
                    if !self.pause(deadline, next) {
                        return None;
                    }
                }
            }
        }
    }

    /// The end of an ending connection, carrying its reason.
    fn end(&mut self, exit: Option<Exit>) -> Incoming {
        let phase = std::mem::replace(&mut self.phase, Phase::Done);
        let error = match phase {
            Phase::Ending { reason, .. } => Some(reason),
            _ => None,
        };
        Incoming::Ended { exit, error }
    }

    /// Sleeps until the next look at an ending Codex: at most [`END_POLL`], never past `deadline`
    /// or `until`. Says whether the caller may look again (false once `deadline` has passed).
    fn pause(&self, deadline: Instant, until: Option<Instant>) -> bool {
        let now = Instant::now();
        let mut step = deadline.saturating_duration_since(now).min(END_POLL);
        if let Some(until) = until {
            step = step.min(until.saturating_duration_since(now).max(Duration::from_millis(1)));
        }
        if step.is_zero() {
            return false;
        }
        std::thread::sleep(step);
        true
    }
}

impl Drop for Conn {
    /// Ends Codex if it still runs: the process's own drop kills its group and collects it.
    fn drop(&mut self) {
        self.process.close_input();
    }
}

/// One line of Codex's output, sorted: an answer (an integer `id` with `result` or `error`), a
/// request of its own (`id` and `method`), a notification (`method` alone), or a skipped line.
pub(crate) fn classify(line: &str) -> Incoming {
    if line.trim().is_empty() {
        return Incoming::Skipped(Skipped::Empty);
    }
    // serde_json refuses nesting past 128 levels with an error, never a stack overflow.
    let value: Value = match serde_json::from_str(line) {
        Ok(value) => value,
        Err(_) => return Incoming::Skipped(Skipped::NotJson { bytes: line.len() }),
    };
    let Value::Object(mut object) = value else { return Incoming::Skipped(Skipped::NotAnObject) };
    let method = object.remove("method");
    let id = object.remove("id");
    match (method, id) {
        (Some(Value::String(method)), Some(id)) if matches!(id, Value::Number(_) | Value::String(_)) => Incoming::Request { id, method },
        (Some(Value::String(method)), None) => {
            let params = object.remove("params").unwrap_or(Value::Null);
            Incoming::Notification { method, params }
        }
        (Some(_), _) => Incoming::Skipped(Skipped::Malformed { kind: "message", reason: "its method or id is not readable" }),
        (None, Some(id)) => {
            let Some(id) = id.as_u64() else {
                return Incoming::Skipped(Skipped::Malformed { kind: "answer", reason: "its id is not a number Archforge sent" });
            };
            if let Some(error) = object.remove("error") {
                Incoming::Response { id, result: Err(rpc_error(&error)) }
            } else if let Some(result) = object.remove("result") {
                Incoming::Response { id, result: Ok(result) }
            } else {
                Incoming::Skipped(Skipped::Malformed { kind: "answer", reason: "it has neither a result nor an error" })
            }
        }
        (None, None) => Incoming::Skipped(Skipped::Untyped),
    }
}

/// An error answer's code and message; a missing or unreadable code is 0 and a missing message
/// empty, as the answer still settles its request.
fn rpc_error(error: &Value) -> RpcError {
    let code = error.get("code").and_then(Value::as_i64).unwrap_or(0);
    let message = error.get("message").and_then(Value::as_str).map(|m| capped(m.to_owned())).unwrap_or_default();
    RpcError { code, message }
}

/// A method name from Codex, safe to log and to keep in [`Skipped::Unknown`]: cut short and kept to
/// letters, digits, `_`, `/` and `-`.
pub(crate) fn kind_name(name: &str) -> String {
    let kept: String = name.chars().filter(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '/' | '-')).take(MAX_KIND_BYTES).collect();
    if kept.is_empty() { "<unnamed>".to_owned() } else { kept }
}

/// An id Codex gave (a thread's, a turn's, a login's), taken only whole: 1 to [`MAX_ID_BYTES`] of
/// ASCII letters, digits and `-_.:`. Anything else is `None`, never cut, as ids are sent back.
pub(crate) fn checked_id(value: Option<&Value>) -> Option<String> {
    let text = value?.as_str()?;
    let fits = !text.is_empty() && text.len() <= MAX_ID_BYTES && text.bytes().all(|b| b.is_ascii_alphanumeric() || b"-_.:".contains(&b));
    fits.then(|| text.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An error with `code` and `message`.
    fn error(code: i64, message: &str) -> RpcError {
        RpcError { code, message: message.to_owned() }
    }

    /// Only an error whose first unknown variant is the requested method is an unknown method; an
    /// unknown parameter value is not, nor another code.
    #[test]
    fn an_unknown_parameter_value_is_not_an_unknown_method() {
        let observed = "Invalid request: unknown variant `no/such/method`, expected one of `initialize`, `thread/start`";
        assert!(error(INVALID_REQUEST, observed).unknown_method("no/such/method"));
        assert!(error(METHOD_NOT_FOUND, observed).unknown_method("no/such/method"));
        assert!(!error(INVALID_REQUEST, observed).unknown_method("initialize"), "a method in the expected list is known");
        assert!(!error(INTERNAL_ERROR, observed).unknown_method("no/such/method"), "another code");
        let parameter = "Invalid request: unknown variant `chatgptMagic`, expected one of `apiKey`, `chatgpt`, `chatgptDeviceCode`";
        assert!(!error(INVALID_REQUEST, parameter).unknown_method("account/login/start"));
        // The method named later in the message (after a parameter's variant) does not count.
        let both = "Invalid request: unknown variant `chatgptMagic` in account/login/start, unknown variant `account/login/start`";
        assert!(!error(INVALID_REQUEST, both).unknown_method("account/login/start"));
        assert!(!error(INVALID_REQUEST, "Invalid request: unknown variant `account/login/start").unknown_method("account/login/start"), "unclosed");
        assert!(!error(INVALID_REQUEST, "Not initialized").unknown_method("account/read"));
    }

    /// Only -32603 with one of Codex's exact sentences is an expired sign-in; loose words, another
    /// code or a changed sentence are not.
    #[test]
    fn only_codex_exact_refresh_sentences_mean_an_expired_sign_in() {
        for sentence in EXPIRED_SIGN_IN_MESSAGES {
            assert!(error(INTERNAL_ERROR, sentence).expired_sign_in(), "{sentence}");
            assert!(!error(INVALID_REQUEST, sentence).expired_sign_in(), "{sentence} with another code");
            assert!(!error(INTERNAL_ERROR, &format!("{sentence} ")).expired_sign_in(), "a changed sentence");
            assert_eq!(error(INTERNAL_ERROR, sentence).into_error("account/read", None).kind(), ErrorKind::SignInExpired);
        }
        for loose in ["token revoked", "Your refresh token was revoked.", "expired", "401 Unauthorized", ""] {
            assert!(!error(INTERNAL_ERROR, loose).expired_sign_in(), "{loose}");
        }
        assert_eq!(error(INTERNAL_ERROR, "token revoked").into_error("account/read", None).kind(), ErrorKind::Other);
        let unknown = error(INVALID_REQUEST, "Invalid request: unknown variant `account/read`, expected one of `initialize`");
        assert_eq!(unknown.clone().into_error("account/read", Some(ErrorKind::InvalidCredential)).kind(), ErrorKind::MissingRuntime);
        assert_eq!(
            error(INVALID_REQUEST, "bad key").into_error("account/login/start", Some(ErrorKind::InvalidCredential)).kind(),
            ErrorKind::InvalidCredential
        );
    }

    /// Lines sort into answers, requests, notifications and skipped lines.
    #[test]
    fn lines_sort_into_answers_requests_and_notifications() {
        assert!(matches!(classify(r#"{"id":3,"result":{}}"#), Incoming::Response { id: 3, result: Ok(_) }));
        let Incoming::Response { id: 0, result: Err(e) } = classify(r#"{"error":{"code":-32600,"message":"Not initialized"},"id":0}"#) else {
            panic!("not an error answer")
        };
        assert_eq!((e.code, e.message.as_str()), (-32600, "Not initialized"));
        assert!(
            matches!(classify(r#"{"id":0,"method":"item/tool/requestUserInput","params":{}}"#), Incoming::Request { method, .. } if method == "item/tool/requestUserInput")
        );
        assert!(matches!(classify(r#"{"id":"x","method":"m"}"#), Incoming::Request { .. }));
        assert!(
            matches!(classify(r#"{"method":"turn/started","params":{"x":1}}"#), Incoming::Notification { method, .. } if method == "turn/started")
        );
        assert!(matches!(classify(""), Incoming::Skipped(Skipped::Empty)));
        assert!(matches!(classify("this is not json"), Incoming::Skipped(Skipped::NotJson { bytes: 16 })));
        assert!(matches!(classify("[1]"), Incoming::Skipped(Skipped::NotAnObject)));
        assert!(matches!(classify("{}"), Incoming::Skipped(Skipped::Untyped)));
        assert!(matches!(classify(r#"{"id":-1,"result":{}}"#), Incoming::Skipped(Skipped::Malformed { .. })));
        assert!(matches!(classify(r#"{"id":1}"#), Incoming::Skipped(Skipped::Malformed { .. })));
        assert!(matches!(classify(r#"{"method":7}"#), Incoming::Skipped(Skipped::Malformed { .. })));
        let deep = format!("{}{}", "[".repeat(200), "]".repeat(200));
        assert!(matches!(classify(&deep), Incoming::Skipped(Skipped::NotJson { .. })));
        assert_eq!(kind_name("a b/c\u{7}d"), "ab/cd");
        assert_eq!(kind_name("\u{1}"), "<unnamed>");
    }
}
