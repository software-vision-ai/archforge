//! Codex's notifications turned into neutral [`SessionEvent`]s.
//!
//! Every line is hostile until read: one that is empty, not JSON, too deep, not an object, neither
//! a request, an answer nor a notification, or a notification missing the fields it needs becomes
//! [`SessionEvent::Ignored`] and is counted; nothing here panics or allocates by a size the input
//! declares. The outcome of a turn is read from `turn/completed`'s status and the error category
//! (`codexErrorInfo`) of the turn's error or of the last `error` notification before it, never from
//! the error's message, which is for the log.

use std::collections::HashMap;

use serde_json::Value;

use super::SignInKind;
use super::rpc::{Incoming, capped, read_line};
use crate::session::{SessionEvent, Skipped, ToolServer, TurnOutcome, Usage};
use crate::{AgentError, Dropped, ErrorKind};

/// The longest id, model, tool or server name an event keeps, in bytes.
pub const MAX_NAME_BYTES: usize = 256;
/// The longest tool result summary an event keeps, in bytes.
pub const MAX_SUMMARY_BYTES: usize = 2 * 1024;
/// The longest tool input an event keeps, as JSON text, in bytes.
pub const MAX_INPUT_BYTES: usize = 64 * 1024;
/// The longest error detail kept from the runtime's own words, in bytes.
const MAX_ERROR_TEXT_BYTES: usize = 1024;
/// The most tool servers a `Started` event lists.
const MAX_SERVERS: usize = 64;
/// The most tool calls remembered by id within one turn, to name their results.
const MAX_OPEN_CALLS: usize = 1024;
/// The longest unknown method name kept for an `Ignored` event, in bytes.
const MAX_KIND_BYTES: usize = 48;

/// The notifications this adapter reads into events or state.
pub const HANDLED_NOTIFICATIONS: [&str; 10] = [
    "turn/started",
    "turn/completed",
    "item/started",
    "item/completed",
    "item/agentMessage/delta",
    "error",
    "thread/tokenUsage/updated",
    "mcpServer/startupStatus/updated",
    "account/updated",
    "account/login/completed",
];

/// The notifications that mean nothing to a conversation and are dropped without a word: progress
/// and state the Chat panel does not show, and the switched-off features' news.
pub const QUIET_NOTIFICATIONS: [&str; 39] = [
    "thread/started",
    "thread/status/changed",
    "thread/readState/changed",
    "thread/closed",
    "thread/name/updated",
    "thread/settings/updated",
    "thread/compacted",
    "thread/environment/connected",
    "thread/environment/disconnected",
    "thread/queue/changed",
    "thread/goal/updated",
    "thread/goal/cleared",
    "skills/changed",
    "serverRequest/resolved",
    "remoteControl/status/changed",
    "account/rateLimits/updated",
    "account/gatewayOAuth/changed",
    "app/list/updated",
    "turn/diff/updated",
    "turn/plan/updated",
    "turn/moderationMetadata",
    "item/plan/delta",
    "item/reasoning/summaryTextDelta",
    "item/reasoning/summaryPartAdded",
    "item/reasoning/textDelta",
    "item/mcpToolCall/progress",
    "item/autoApprovalReview/started",
    "item/autoApprovalReview/completed",
    "hook/started",
    "hook/completed",
    "mcpServer/event/stream/notification",
    "model/rerouted",
    "model/verification",
    "model/safetyBuffering/updated",
    "modelProvider/authRecoveryStarted",
    "modelProvider/authRecoveryCompleted",
    "warning",
    "configWarning",
    "deprecationNotice",
];

/// Reads Codex's notifications one by one and keeps what a turn needs across them: the thread, the
/// tool servers' states, the error category seen before `turn/completed`, the tokens used, whether
/// a stop was asked for, and the names of tool calls.
pub struct CodexParser {
    max_line_bytes: usize,
    sign_in: SignInKind,
    skipped: u64,
    thread: Option<(String, String)>,
    servers: Vec<ToolServer>,
    turn: TurnState,
    tool_names: HashMap<String, String>,
}

impl std::fmt::Debug for CodexParser {
    /// Writes counts only: the turn's state holds the runtime's own words.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CodexParser")
            .field("skipped", &self.skipped)
            .field("thread", &self.thread.is_some())
            .field("open_calls", &self.tool_names.len())
            .field("error_seen", &self.turn.error.is_some())
            .field("stop_requested", &self.turn.stop_requested)
            .finish()
    }
}

/// What the parser has learned about the running turn.
#[derive(Debug, Default)]
struct TurnState {
    /// The turn's id, once Codex named it.
    id: Option<String>,
    /// The category and words of the last error that was not retried.
    error: Option<(ErrorKind, String)>,
    /// How many retries were announced.
    retries: u32,
    /// The tokens of every model response in the turn, summed.
    usage: Usage,
    /// A stop was asked for in this turn.
    stop_requested: bool,
}

impl CodexParser {
    /// A parser that skips lines longer than `max_line_bytes`, for a runtime signed in as
    /// `sign_in` (a refused sign-in reads as an invalid key or an expired ChatGPT sign-in).
    pub fn new(max_line_bytes: usize, sign_in: SignInKind) -> CodexParser {
        CodexParser { max_line_bytes, sign_in, skipped: 0, thread: None, servers: Vec::new(), turn: TurnState::default(), tool_names: HashMap::new() }
    }

    /// How many lines were skipped so far.
    pub fn skipped(&self) -> u64 {
        self.skipped
    }

    /// Notes that the running turn was asked to stop, so a failure without a category reads as
    /// stopped.
    pub fn stop_requested(&mut self) {
        self.turn.stop_requested = true;
    }

    /// Notes the running turn's id from the answer to `turn/start`.
    pub fn turn_started(&mut self, id: &str) {
        self.turn.id = Some(capped(id, MAX_NAME_BYTES));
    }

    /// The running turn's id, once known.
    pub fn turn_id(&self) -> Option<&str> {
        self.turn.id.as_deref()
    }

    /// Reads the answer to `thread/start` (`thread.id`, and `model`): the `Started` event, or why
    /// it could not be read.
    pub fn thread_started(&mut self, result: &Value) -> Result<SessionEvent, Skipped> {
        let id = result.get("thread").and_then(|t| t.get("id")).and_then(Value::as_str).filter(|id| !id.is_empty());
        let Some(id) = id else { return Err(Skipped::Malformed { kind: "thread/start", reason: "its answer names no thread id" }) };
        let model = result.get("model").or_else(|| result.get("thread").and_then(|t| t.get("model"))).and_then(Value::as_str).unwrap_or_default();
        self.thread = Some((capped(id, MAX_NAME_BYTES), capped(model, MAX_NAME_BYTES)));
        Ok(self.started())
    }

    /// The thread's id, once started.
    pub fn thread_id(&self) -> Option<&str> {
        self.thread.as_ref().map(|(id, _)| id.as_str())
    }

    /// The event for a line the process reader dropped.
    pub fn dropped(&mut self, why: Dropped) -> SessionEvent {
        self.skip(match why {
            Dropped::TooLong { bytes } => Skipped::TooLong { bytes },
            Dropped::NotText { bytes } => Skipped::NotText { bytes },
        })
    }

    /// The events one line means, for a reader without a session: a notification's events, or one
    /// `Ignored` for a line that is not a message. Answers and Codex's own requests mean no event
    /// here; [`CodexSession`](super::CodexSession) acts on them.
    pub fn line(&mut self, line: &str) -> Vec<SessionEvent> {
        match read_line(line, self.max_line_bytes) {
            Incoming::Notification { method, params } => self.notification(&method, &params),
            Incoming::Skipped(why) => vec![self.skip(why)],
            Incoming::Response { .. } | Incoming::Request { .. } => Vec::new(),
        }
    }

    /// A skipped line, counted.
    pub(crate) fn skip(&mut self, why: Skipped) -> SessionEvent {
        self.skipped = self.skipped.saturating_add(1);
        SessionEvent::Ignored(why)
    }

    /// The events of one notification, in order; most mean none or one.
    pub fn notification(&mut self, method: &str, params: &Value) -> Vec<SessionEvent> {
        match method {
            "turn/started" => {
                if let Some(id) = params.get("turn").and_then(|t| t.get("id")).and_then(Value::as_str) {
                    self.turn.id = Some(capped(id, MAX_NAME_BYTES));
                }
                Vec::new()
            }
            "turn/completed" => self.turn_completed(params),
            "item/started" => self.item_started(params),
            "item/completed" => self.item_completed(params),
            "item/agentMessage/delta" => match params.get("delta").and_then(Value::as_str) {
                Some("") => Vec::new(),
                Some(text) => vec![SessionEvent::TextDelta { text: text.to_owned() }],
                None => vec![self.skip(Skipped::Malformed { kind: "item/agentMessage/delta", reason: "it has no delta text" })],
            },
            "error" => self.error(params),
            "thread/tokenUsage/updated" => {
                let last = params.get("tokenUsage").and_then(|u| u.get("last"));
                let count = |key: &str| last.and_then(|l| l.get(key)).and_then(Value::as_u64).unwrap_or(0);
                let usage = &mut self.turn.usage;
                usage.input_tokens = usage.input_tokens.saturating_add(count("inputTokens"));
                usage.output_tokens = usage.output_tokens.saturating_add(count("outputTokens"));
                usage.cache_read_input_tokens = usage.cache_read_input_tokens.saturating_add(count("cachedInputTokens"));
                usage.cache_creation_input_tokens = usage.cache_creation_input_tokens.saturating_add(count("cacheWriteInputTokens"));
                Vec::new()
            }
            "mcpServer/startupStatus/updated" => self.server_status(params),
            "account/updated" | "account/login/completed" => Vec::new(),
            quiet if QUIET_NOTIFICATIONS.contains(&quiet) => Vec::new(),
            other => {
                let kind = kind_name(other);
                log::debug!("codex sent a notification of unknown method {kind}; it was skipped");
                vec![self.skip(Skipped::Unknown { kind })]
            }
        }
    }

    /// The `Started` event for the thread and the tool servers as last reported.
    fn started(&self) -> SessionEvent {
        let (session_id, model) = self.thread.clone().unwrap_or_default();
        SessionEvent::Started { session_id, model, tools: Vec::new(), servers: self.servers.clone() }
    }

    /// A tool server's start: its state is kept, and `Started` says it again.
    fn server_status(&mut self, params: &Value) -> Vec<SessionEvent> {
        let name = params.get("name").and_then(Value::as_str);
        let status = params.get("status").and_then(Value::as_str);
        let (Some(name), Some(status)) = (name, status) else {
            return vec![self.skip(Skipped::Malformed { kind: "mcpServer/startupStatus/updated", reason: "it has no name or status" })];
        };
        let (name, status) = (capped(name, MAX_NAME_BYTES), capped(status, MAX_NAME_BYTES));
        if status == "failed" {
            log::warn!("codex could not start the tool server {}", kind_name(&name));
        }
        if name != super::SERVER_NAME {
            log::warn!("codex started a tool server that is not Archforge's: {}", kind_name(&name));
        }
        let known = self.servers.len();
        match self.servers.iter_mut().find(|s| s.name == name) {
            Some(server) => server.status = status,
            None if known < MAX_SERVERS => self.servers.push(ToolServer { name, status }),
            None => log::debug!("codex reported more than {MAX_SERVERS} tool servers; the rest are not listed"),
        }
        if self.thread.is_some() { vec![self.started()] } else { Vec::new() }
    }

    /// `item/started`: a tool call starts; the answer's text arrives as deltas.
    fn item_started(&mut self, params: &Value) -> Vec<SessionEvent> {
        let Some(item) = params.get("item") else {
            return vec![self.skip(Skipped::Malformed { kind: "item/started", reason: "it has no item" })];
        };
        match item.get("type").and_then(Value::as_str) {
            Some("mcpToolCall") => {
                let id = capped(item.get("id").and_then(Value::as_str).unwrap_or_default(), MAX_NAME_BYTES);
                let server = item.get("server").and_then(Value::as_str).map(|s| capped(s, MAX_NAME_BYTES));
                let tool = capped(item.get("tool").and_then(Value::as_str).unwrap_or_default(), MAX_NAME_BYTES);
                let input = item.get("arguments").map(Value::to_string).unwrap_or_else(|| "{}".to_owned());
                if self.tool_names.len() < MAX_OPEN_CALLS {
                    self.tool_names.insert(id.clone(), tool.clone());
                } else {
                    log::debug!("codex made more than {MAX_OPEN_CALLS} tool calls in one turn; the result of this one is not named");
                }
                vec![SessionEvent::ToolStarted { id, server, tool, input: capped(&input, MAX_INPUT_BYTES) }]
            }
            Some(kind @ ("commandExecution" | "fileChange" | "dynamicToolCall" | "webSearch" | "imageGeneration" | "collabAgentToolCall")) => {
                log::warn!("codex started a {kind} item, which Archforge switched off; it is not shown");
                Vec::new()
            }
            _ => Vec::new(),
        }
    }

    /// `item/completed`: a message of the answer is whole, or a tool call ended.
    fn item_completed(&mut self, params: &Value) -> Vec<SessionEvent> {
        let Some(item) = params.get("item") else {
            return vec![self.skip(Skipped::Malformed { kind: "item/completed", reason: "it has no item" })];
        };
        match item.get("type").and_then(Value::as_str) {
            Some("agentMessage") => match item.get("text").and_then(Value::as_str) {
                Some("") => Vec::new(),
                Some(text) => vec![SessionEvent::MessageDone { text: text.to_owned() }],
                None => vec![self.skip(Skipped::Malformed { kind: "item/completed", reason: "its agent message has no text" })],
            },
            Some("mcpToolCall") => {
                let id = capped(item.get("id").and_then(Value::as_str).unwrap_or_default(), MAX_NAME_BYTES);
                let error = item.get("error").and_then(|e| e.get("message")).and_then(Value::as_str);
                let ok = item.get("status").and_then(Value::as_str) == Some("completed") && error.is_none();
                let summary = match error {
                    Some(message) => capped(message, MAX_SUMMARY_BYTES),
                    None => joined_text(item.get("result").and_then(|r| r.get("content")), MAX_SUMMARY_BYTES),
                };
                let tool = self.tool_names.remove(&id).or_else(|| item.get("tool").and_then(Value::as_str).map(|t| capped(t, MAX_NAME_BYTES)));
                vec![SessionEvent::ToolFinished { id, tool, ok, summary }]
            }
            _ => Vec::new(),
        }
    }

    /// `error`: a retried failure is progress; one that is not retried is the turn's failure.
    fn error(&mut self, params: &Value) -> Vec<SessionEvent> {
        let Some(error) = params.get("error") else {
            return vec![self.skip(Skipped::Malformed { kind: "error", reason: "it has no error object" })];
        };
        let (kind, detail) = self.category(error);
        if params.get("willRetry").and_then(Value::as_bool).unwrap_or(false) {
            self.turn.retries = self.turn.retries.saturating_add(1);
            vec![SessionEvent::Retrying { attempt: self.turn.retries, max_attempts: None, delay_ms: None, kind }]
        } else {
            self.turn.error = Some((kind, detail));
            Vec::new()
        }
    }

    /// The category and the detail of one of Codex's error objects (`TurnError`).
    fn category(&self, error: &Value) -> (ErrorKind, String) {
        let info = error.get("codexErrorInfo");
        let kind = kind_of(info, self.sign_in);
        let message = capped(error.get("message").and_then(Value::as_str).unwrap_or_default(), MAX_ERROR_TEXT_BYTES);
        (kind, describe(info, &message))
    }

    /// `turn/completed`, which ends every turn.
    fn turn_completed(&mut self, params: &Value) -> Vec<SessionEvent> {
        let Some(turn) = params.get("turn") else {
            return vec![self.skip(Skipped::Malformed { kind: "turn/completed", reason: "it has no turn" })];
        };
        let id = turn.get("id").and_then(Value::as_str);
        if let (Some(ours), Some(id)) = (self.turn.id.as_deref(), id)
            && ours != capped(id, MAX_NAME_BYTES)
        {
            log::debug!("codex completed a turn that is not the running one; it was skipped");
            return vec![self.skip(Skipped::Malformed { kind: "turn/completed", reason: "it completes a turn that is not running" })];
        }
        let state = std::mem::take(&mut self.turn);
        self.tool_names.clear();
        let outcome = match turn.get("status").and_then(Value::as_str) {
            Some("completed") => TurnOutcome::Completed,
            Some("interrupted") => TurnOutcome::Stopped,
            Some("failed") => {
                let own = turn.get("error").filter(|e| e.is_object()).map(|e| self.category(e));
                match own.or(state.error) {
                    Some((kind, detail)) => TurnOutcome::Failed(AgentError::reported(kind, &detail)),
                    None if state.stop_requested => TurnOutcome::Stopped,
                    None => TurnOutcome::Failed(AgentError::reported(ErrorKind::Other, "codex failed the turn without saying why")),
                }
            }
            other => {
                let named = other.map_or_else(|| "no status".to_owned(), |s| format!("the status {}", kind_name(s)));
                TurnOutcome::Failed(AgentError::reported(ErrorKind::Other, &format!("codex ended the turn with {named}")))
            }
        };
        vec![SessionEvent::TurnFinished { outcome, usage: state.usage }]
    }
}

/// The neutral category of a `codexErrorInfo` (a name, or an object whose one key is the name and
/// may hold an HTTP status), for a runtime signed in as `sign_in` (the spike's error mapping,
/// AR-196). A refused sign-in is an invalid key for an API key and an expired sign-in for ChatGPT.
pub fn kind_of(info: Option<&Value>, sign_in: SignInKind) -> ErrorKind {
    let refused = match sign_in {
        SignInKind::ApiKey => ErrorKind::InvalidCredential,
        SignInKind::ChatGpt => ErrorKind::SignInExpired,
    };
    let (name, status) = info_name(info);
    match name.as_deref() {
        Some("unauthorized") => refused,
        Some("usageLimitExceeded" | "rateLimitExceeded" | "sessionBudgetExceeded") => ErrorKind::RateLimited,
        Some("modelNotFound" | "serverOverloaded" | "flexUnavailable" | "policyCapacityUnavailable") => ErrorKind::ModelUnavailable,
        Some("httpConnectionFailed" | "responseStreamConnectionFailed" | "responseStreamDisconnected" | "responseTooManyFailedAttempts") => {
            match status {
                Some(401 | 403) => refused,
                Some(429) => ErrorKind::RateLimited,
                Some(404) => ErrorKind::ModelUnavailable,
                _ => ErrorKind::Network,
            }
        }
        _ => ErrorKind::Other,
    }
}

/// The name of a `codexErrorInfo` and the HTTP status it carries.
fn info_name(info: Option<&Value>) -> (Option<String>, Option<u64>) {
    match info {
        Some(Value::String(name)) => (Some(name.clone()), None),
        Some(Value::Object(map)) => match map.iter().next() {
            Some((name, inner)) if map.len() == 1 => (Some(name.clone()), inner.get("httpStatusCode").and_then(Value::as_u64)),
            _ => (None, None),
        },
        _ => (None, None),
    }
}

/// An error's detail: the category, the HTTP status, then the runtime's words for the log.
fn describe(info: Option<&Value>, message: &str) -> String {
    let (name, status) = info_name(info);
    let mut text = format!("codex reported {}", name.as_deref().map_or_else(|| "an error".to_owned(), kind_name));
    if let Some(status) = status {
        text.push_str(&format!(", HTTP status {status}"));
    }
    if !message.trim().is_empty() {
        text.push_str(": ");
        text.push_str(message.trim());
    }
    text
}

/// The text blocks of an MCP tool result's `content`, one per line; other blocks are named by
/// their type. Capped at `max` bytes.
fn joined_text(content: Option<&Value>, max: usize) -> String {
    let Some(blocks) = content.and_then(Value::as_array) else { return String::new() };
    let mut text = String::new();
    for block in blocks.iter().filter_map(Value::as_object) {
        if text.len() > max {
            break;
        }
        if !text.is_empty() {
            text.push('\n');
        }
        match block.get("type").and_then(Value::as_str) {
            Some("text") => text.push_str(block.get("text").and_then(Value::as_str).unwrap_or_default()),
            Some(other) => text.push_str(&format!("[{}]", kind_name(other))),
            None => {}
        }
    }
    capped(&text, max)
}

/// A method or category name from the input, safe to log: cut to a few bytes and kept to letters,
/// digits, `_`, `/` and `-`.
pub(crate) fn kind_name(name: &str) -> String {
    let kept: String = name.chars().filter(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '/' | '-')).collect();
    let kept = capped(&kept, MAX_KIND_BYTES);
    if kept.is_empty() { "<unnamed>".to_owned() } else { kept }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    /// Every category Codex names lands in its neutral category; a refused sign-in depends on how
    /// the runtime signed in, and an HTTP status refines a connection failure.
    #[test]
    fn categories_map_to_neutral_kinds() {
        let api = SignInKind::ApiKey;
        let chat = SignInKind::ChatGpt;
        let cases = [
            (json!("unauthorized"), api, ErrorKind::InvalidCredential),
            (json!("unauthorized"), chat, ErrorKind::SignInExpired),
            (json!("usageLimitExceeded"), api, ErrorKind::RateLimited),
            (json!("rateLimitExceeded"), api, ErrorKind::RateLimited),
            (json!("sessionBudgetExceeded"), chat, ErrorKind::RateLimited),
            (json!("modelNotFound"), api, ErrorKind::ModelUnavailable),
            (json!("serverOverloaded"), api, ErrorKind::ModelUnavailable),
            (json!("flexUnavailable"), api, ErrorKind::ModelUnavailable),
            (json!("policyCapacityUnavailable"), api, ErrorKind::ModelUnavailable),
            (json!({"httpConnectionFailed": {"httpStatusCode": null}}), api, ErrorKind::Network),
            (json!({"responseStreamConnectionFailed": {"httpStatusCode": 502}}), api, ErrorKind::Network),
            (json!({"responseStreamDisconnected": {}}), api, ErrorKind::Network),
            (json!({"responseTooManyFailedAttempts": {"httpStatusCode": 429}}), api, ErrorKind::RateLimited),
            (json!({"responseStreamConnectionFailed": {"httpStatusCode": 401}}), chat, ErrorKind::SignInExpired),
            (json!({"httpConnectionFailed": {"httpStatusCode": 403}}), api, ErrorKind::InvalidCredential),
            (json!({"httpConnectionFailed": {"httpStatusCode": 404}}), api, ErrorKind::ModelUnavailable),
            (json!("contextWindowExceeded"), api, ErrorKind::Other),
            (json!("badRequest"), api, ErrorKind::Other),
            (json!("internalServerError"), api, ErrorKind::Other),
            (json!("sandboxError"), api, ErrorKind::Other),
            (json!("cyberPolicy"), api, ErrorKind::Other),
            (json!("tooManyDenials"), api, ErrorKind::Other),
            (json!("other"), api, ErrorKind::Other),
            (json!({"activeTurnNotSteerable": {"turnKind": "review"}}), api, ErrorKind::Other),
            (json!({"a": 1, "b": 2}), api, ErrorKind::Other),
            (json!(7), api, ErrorKind::Other),
            (Value::Null, api, ErrorKind::Other),
        ];
        for (info, sign_in, kind) in cases {
            assert_eq!(kind_of(Some(&info), sign_in), kind, "{info} {sign_in:?}");
        }
        assert_eq!(kind_of(None, SignInKind::ApiKey), ErrorKind::Other);
    }

    /// Names are cut and kept safe for the log.
    #[test]
    fn names_are_cleaned() {
        assert_eq!(kind_name("brand/new\u{202E}\n"), "brand/new");
        assert_eq!(kind_name("\u{1F600}"), "<unnamed>");
        assert_eq!(
            describe(Some(&json!({"httpConnectionFailed": {"httpStatusCode": 503}})), " down "),
            "codex reported httpConnectionFailed, HTTP status 503: down"
        );
        assert_eq!(describe(None, ""), "codex reported an error");
    }
}
