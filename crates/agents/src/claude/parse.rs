//! Claude Code's stream-json output turned into neutral [`SessionEvent`]s.
//!
//! Every line is hostile until read: a line that is empty, not JSON, too deep, not an object,
//! untyped, of an unknown type or missing the fields its type needs becomes
//! [`SessionEvent::Ignored`] and is counted; nothing here panics or allocates by a size the input
//! declares. The outcome of a turn is read from `result.is_error` and the error category fields
//! (`assistant.error`, `api_error`, `api_error_status`, `api_retry.error`), never from
//! `result.subtype`, which says `success` for failed API calls.

use std::collections::HashMap;

use serde_json::{Map, Value};

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
/// The most tools a `Started` event lists.
const MAX_TOOLS: usize = 4096;
/// The most tool servers a `Started` event lists.
const MAX_SERVERS: usize = 64;
/// The most tool calls remembered by id within one turn, to name their results.
const MAX_OPEN_CALLS: usize = 1024;
/// The longest unknown type name kept for an `Ignored` event, in bytes.
const MAX_KIND_BYTES: usize = 48;
/// The prefix of the request ids this adapter gives its stop requests.
pub(crate) const STOP_REQUEST_PREFIX: &str = "archforge-stop-";

/// Reads Claude Code's output lines one by one and keeps what a turn needs across lines: the error
/// category seen before the `result`, whether a stop was asked for, and the names of tool calls.
pub struct ClaudeParser {
    max_line_bytes: usize,
    skipped: u64,
    turn: TurnState,
    tool_names: HashMap<String, String>,
    stop_refused: bool,
}

impl std::fmt::Debug for ClaudeParser {
    /// Writes counts only: the turn's state holds the runtime's own words.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClaudeParser")
            .field("skipped", &self.skipped)
            .field("open_calls", &self.tool_names.len())
            .field("error_seen", &self.turn.error.is_some())
            .field("stop_requested", &self.turn.stop_requested)
            .finish()
    }
}

/// What the parser has learned about the running turn.
#[derive(Debug, Default)]
struct TurnState {
    /// The category of a synthetic error message or a rejected rate limit, with its words.
    error: Option<(ErrorKind, String)>,
    /// Why the last retry happened.
    last_retry: Option<ErrorKind>,
    /// A stop was asked for in this turn.
    stop_requested: bool,
}

impl ClaudeParser {
    /// A parser that skips lines longer than `max_line_bytes`.
    pub fn new(max_line_bytes: usize) -> ClaudeParser {
        ClaudeParser { max_line_bytes, skipped: 0, turn: TurnState::default(), tool_names: HashMap::new(), stop_refused: false }
    }

    /// How many lines were skipped so far.
    pub fn skipped(&self) -> u64 {
        self.skipped
    }

    /// Notes that the running turn was asked to stop, so its failed `result` reads as stopped.
    pub fn stop_requested(&mut self) {
        self.turn.stop_requested = true;
    }

    /// Whether the runtime refused a stop request since the last call (the session then falls back
    /// to an interrupt signal).
    pub(crate) fn take_stop_refused(&mut self) -> bool {
        std::mem::take(&mut self.stop_refused)
    }

    /// The event for a line the process reader dropped.
    pub fn dropped(&mut self, why: Dropped) -> SessionEvent {
        self.skip(match why {
            Dropped::TooLong { bytes } => Skipped::TooLong { bytes },
            Dropped::NotText { bytes } => Skipped::NotText { bytes },
        })
    }

    /// The events one line of output means, in order; most lines mean none or one.
    pub fn line(&mut self, line: &str) -> Vec<SessionEvent> {
        if line.len() > self.max_line_bytes {
            return vec![self.skip(Skipped::TooLong { bytes: u64::try_from(line.len()).unwrap_or(u64::MAX) })];
        }
        if line.trim().is_empty() {
            return vec![self.skip(Skipped::Empty)];
        }
        // serde_json refuses nesting past 128 levels with an error, never a stack overflow.
        let value: Value = match serde_json::from_str(line) {
            Ok(value) => value,
            Err(_) => return vec![self.skip(Skipped::NotJson { bytes: line.len() })],
        };
        let Some(object) = value.as_object() else { return vec![self.skip(Skipped::NotAnObject)] };
        let Some(kind) = object.get("type").and_then(Value::as_str) else { return vec![self.skip(Skipped::Untyped)] };
        match kind {
            "system" => self.system(object),
            "stream_event" => self.stream_event(object),
            "assistant" => self.assistant(object),
            "user" => self.user(object),
            "result" => vec![self.result(object)],
            "rate_limit_event" => {
                self.rate_limit(object);
                Vec::new()
            }
            "control_response" => {
                self.control_response(object);
                Vec::new()
            }
            other => {
                let kind = kind_name(other);
                log::debug!("claude wrote an event of unknown type {kind}; it was skipped");
                vec![self.skip(Skipped::Unknown { kind })]
            }
        }
    }

    /// A skipped line, counted.
    fn skip(&mut self, why: Skipped) -> SessionEvent {
        self.skipped = self.skipped.saturating_add(1);
        SessionEvent::Ignored(why)
    }

    /// `system` lines: `init` starts the session, `api_retry` is a retry, `status` and
    /// `ui_invalidate` are progress nobody shows.
    fn system(&mut self, object: &Map<String, Value>) -> Vec<SessionEvent> {
        let subtype = object.get("subtype").and_then(Value::as_str).unwrap_or_default();
        match subtype {
            "init" => vec![started(object)],
            "api_retry" => {
                let status = object.get("error_status").and_then(Value::as_u64);
                let category = object.get("error").and_then(Value::as_str).unwrap_or("unknown");
                // A retry with no HTTP status never reached the provider (observed, no network).
                let kind = if status.is_none() && category == "unknown" { ErrorKind::Network } else { kind_of(category, None, status) };
                self.turn.last_retry = Some(kind);
                let attempt = object.get("attempt").and_then(Value::as_u64).and_then(|n| u32::try_from(n).ok());
                if attempt.is_none() {
                    log::debug!("claude announced a retry without a readable attempt number; it is shown as 0");
                }
                vec![SessionEvent::Retrying {
                    attempt: attempt.unwrap_or(0),
                    max_attempts: object.get("max_retries").and_then(Value::as_u64).and_then(|n| u32::try_from(n).ok()),
                    delay_ms: object.get("retry_delay_ms").and_then(Value::as_u64),
                    kind,
                }]
            }
            "status" | "ui_invalidate" => Vec::new(),
            other => vec![self.skip(Skipped::Unknown { kind: kind_name(&format!("system/{other}")) })],
        }
    }

    /// `stream_event` lines: only the text deltas of the answer matter; tool input arrives whole in
    /// the `assistant` message.
    fn stream_event(&mut self, object: &Map<String, Value>) -> Vec<SessionEvent> {
        let Some(event) = object.get("event").and_then(Value::as_object) else {
            return vec![self.skip(Skipped::Malformed { kind: "stream_event", reason: "it has no event object" })];
        };
        if event.get("type").and_then(Value::as_str) != Some("content_block_delta") {
            return Vec::new();
        }
        let delta = event.get("delta").and_then(Value::as_object);
        match delta.filter(|d| d.get("type").and_then(Value::as_str) == Some("text_delta")).and_then(|d| d.get("text")).and_then(Value::as_str) {
            Some(text) if !text.is_empty() => vec![SessionEvent::TextDelta { text: text.to_owned() }],
            _ => Vec::new(),
        }
    }

    /// `assistant` lines: a complete message (its text, then its tool calls), or a synthetic message
    /// carrying the error category the coming `result` fails with.
    fn assistant(&mut self, object: &Map<String, Value>) -> Vec<SessionEvent> {
        let Some(message) = object.get("message").and_then(Value::as_object) else {
            return vec![self.skip(Skipped::Malformed { kind: "assistant", reason: "it has no message object" })];
        };
        if let Some(category) = object.get("error").and_then(Value::as_str) {
            let api_error = object.get("api_error").and_then(Value::as_str);
            let kind = kind_of(category, api_error, None);
            let words = joined_text(message.get("content"), MAX_ERROR_TEXT_BYTES);
            self.turn.error = Some((kind, describe(category, api_error, None, &words)));
            return Vec::new();
        }
        let Some(blocks) = message.get("content").and_then(Value::as_array) else {
            return vec![self.skip(Skipped::Malformed { kind: "assistant", reason: "its message has no content list" })];
        };
        let mut text = String::new();
        let mut calls = Vec::new();
        for block in blocks.iter().filter_map(Value::as_object) {
            match block.get("type").and_then(Value::as_str) {
                Some("text") => text.push_str(block.get("text").and_then(Value::as_str).unwrap_or_default()),
                Some("tool_use") => {
                    let id = capped(block.get("id").and_then(Value::as_str).unwrap_or_default(), MAX_NAME_BYTES);
                    let name = block.get("name").and_then(Value::as_str).unwrap_or_default();
                    let (server, tool) = split_tool(name);
                    let input = block.get("input").map(Value::to_string).unwrap_or_else(|| "{}".to_owned());
                    if self.tool_names.len() < MAX_OPEN_CALLS {
                        self.tool_names.insert(id.clone(), tool.clone());
                    } else {
                        log::debug!("claude made more than {MAX_OPEN_CALLS} tool calls in one turn; the result of this one is not named");
                    }
                    calls.push(SessionEvent::ToolStarted { id, server, tool, input: capped(&input, MAX_INPUT_BYTES) });
                }
                _ => {}
            }
        }
        let mut events = Vec::with_capacity(calls.len().saturating_add(1));
        if !text.is_empty() {
            events.push(SessionEvent::MessageDone { text });
        }
        events.extend(calls);
        events
    }

    /// `user` lines the runtime writes: the results of tool calls. The interrupt notice and replayed
    /// text mean nothing here.
    fn user(&mut self, object: &Map<String, Value>) -> Vec<SessionEvent> {
        let Some(message) = object.get("message").and_then(Value::as_object) else {
            return vec![self.skip(Skipped::Malformed { kind: "user", reason: "it has no message object" })];
        };
        let Some(blocks) = message.get("content").and_then(Value::as_array) else { return Vec::new() };
        let mut events = Vec::new();
        for block in blocks.iter().filter_map(Value::as_object) {
            if block.get("type").and_then(Value::as_str) != Some("tool_result") {
                continue;
            }
            let id = capped(block.get("tool_use_id").and_then(Value::as_str).unwrap_or_default(), MAX_NAME_BYTES);
            let ok = !block.get("is_error").and_then(Value::as_bool).unwrap_or(false);
            let summary = joined_text(block.get("content"), MAX_SUMMARY_BYTES);
            let tool = self.tool_names.remove(&id);
            events.push(SessionEvent::ToolFinished { id, tool, ok, summary });
        }
        events
    }

    /// The `result` line that ends every turn.
    fn result(&mut self, object: &Map<String, Value>) -> SessionEvent {
        let turn = std::mem::take(&mut self.turn);
        self.tool_names.clear();
        let usage = usage(object.get("usage"));
        let outcome = match object.get("is_error").and_then(Value::as_bool) {
            Some(false) => TurnOutcome::Completed,
            is_error => {
                let aborted = object.get("terminal_reason").and_then(Value::as_str) == Some("aborted_streaming");
                // A real category that came during a stop is still shown as the failure it was.
                if aborted || (turn.stop_requested && turn.error.is_none()) {
                    TurnOutcome::Stopped
                } else if is_error.is_none() {
                    TurnOutcome::Failed(AgentError::reported(ErrorKind::Other, "claude ended the turn with a result that could not be read"))
                } else {
                    TurnOutcome::Failed(failure(object, turn))
                }
            }
        };
        SessionEvent::TurnFinished { outcome, usage }
    }

    /// `rate_limit_event`: a rejected limit is the category the turn fails with.
    fn rate_limit(&mut self, object: &Map<String, Value>) {
        let status = object.get("rate_limit_info").and_then(Value::as_object).and_then(|i| i.get("status")).and_then(Value::as_str);
        if status == Some("rejected") {
            self.turn.error = Some((ErrorKind::RateLimited, "claude reported that the rate limit was reached".to_owned()));
        }
    }

    /// `control_response`: a refused stop request of ours falls back to the interrupt signal.
    fn control_response(&mut self, object: &Map<String, Value>) {
        let response = object.get("response").and_then(Value::as_object);
        let ours = response.and_then(|r| r.get("request_id")).and_then(Value::as_str).is_some_and(|id| id.starts_with(STOP_REQUEST_PREFIX));
        let refused = response.and_then(|r| r.get("subtype")).and_then(Value::as_str) == Some("error");
        if ours && refused {
            self.stop_refused = true;
        }
    }
}

/// The `Started` event of an `init` line.
fn started(object: &Map<String, Value>) -> SessionEvent {
    let text = |key: &str| capped(object.get(key).and_then(Value::as_str).unwrap_or_default(), MAX_NAME_BYTES);
    let tools: Vec<String> = object
        .get("tools")
        .and_then(Value::as_array)
        .map(|list| list.iter().filter_map(Value::as_str).take(MAX_TOOLS).map(|t| capped(t, MAX_NAME_BYTES)).collect())
        .unwrap_or_default();
    let servers = object
        .get("mcp_servers")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(Value::as_object)
                .take(MAX_SERVERS)
                .map(|s| ToolServer {
                    name: capped(s.get("name").and_then(Value::as_str).unwrap_or_default(), MAX_NAME_BYTES),
                    status: capped(s.get("status").and_then(Value::as_str).unwrap_or_default(), MAX_NAME_BYTES),
                })
                .collect()
        })
        .unwrap_or_default();
    let listed = |key: &str| object.get(key).and_then(Value::as_array).map_or(0, Vec::len);
    if listed("tools") > MAX_TOOLS || listed("mcp_servers") > MAX_SERVERS {
        log::debug!(
            "claude listed {} tools and {} servers; the first {MAX_TOOLS} and {MAX_SERVERS} are kept",
            listed("tools"),
            listed("mcp_servers")
        );
    }
    let foreign = tools.iter().filter(|t| !t.starts_with("mcp__archforge__")).count();
    if foreign > 0 {
        log::warn!("claude offers the model {foreign} tools that are not Archforge's");
    }
    SessionEvent::Started { session_id: text("session_id"), model: text("model"), tools, servers }
}

/// The error a failed `result` carries: the category seen before it in the turn, else the HTTP
/// status the result names, else why the last retry happened, else Other.
fn failure(object: &Map<String, Value>, turn: TurnState) -> AgentError {
    let status = object.get("api_error_status").and_then(Value::as_u64);
    let api_error = object.get("api_error").and_then(Value::as_str);
    if let Some((kind, detail)) = turn.error {
        return AgentError::reported(kind, &detail);
    }
    let words = match object.get("errors").and_then(Value::as_array) {
        Some(errors) if !errors.is_empty() => {
            capped(&errors.iter().filter_map(Value::as_str).take(16).collect::<Vec<_>>().join("; "), MAX_ERROR_TEXT_BYTES)
        }
        _ => capped(object.get("result").and_then(Value::as_str).unwrap_or_default(), MAX_ERROR_TEXT_BYTES),
    };
    let kind = match (status.and_then(status_kind), api_error) {
        (_, Some("connection_failed")) => ErrorKind::Network,
        (Some(kind), _) => kind,
        (None, _) => turn.last_retry.unwrap_or(ErrorKind::Other),
    };
    AgentError::reported(kind, &describe("error", api_error, status, &words))
}

/// The neutral category of one of Claude's error categories, with its `api_error` and HTTP status
/// where known (the spike's error mapping, AR-196).
pub fn kind_of(category: &str, api_error: Option<&str>, status: Option<u64>) -> ErrorKind {
    match category {
        "authentication_failed" | "cloud_credential_error" => ErrorKind::InvalidCredential,
        "rate_limit" | "billing_error" => ErrorKind::RateLimited,
        "model_not_found" | "overloaded" => ErrorKind::ModelUnavailable,
        "oauth_org_not_allowed" | "account_on_hold" | "invalid_request" | "max_output_tokens" => ErrorKind::Other,
        _ if api_error == Some("connection_failed") => ErrorKind::Network,
        _ => status.and_then(status_kind).unwrap_or(ErrorKind::Other),
    }
}

/// The category an HTTP status from the provider means, where it means one.
fn status_kind(status: u64) -> Option<ErrorKind> {
    match status {
        401 | 403 => Some(ErrorKind::InvalidCredential),
        429 => Some(ErrorKind::RateLimited),
        404 | 529 => Some(ErrorKind::ModelUnavailable),
        _ => None,
    }
}

/// An error's detail: the category fields, then the runtime's words for the log.
fn describe(category: &str, api_error: Option<&str>, status: Option<u64>, words: &str) -> String {
    let mut text = format!("claude reported {}", kind_name(category));
    if let Some(api_error) = api_error {
        text.push_str(&format!(" ({})", kind_name(api_error)));
    }
    if let Some(status) = status {
        text.push_str(&format!(", HTTP status {status}"));
    }
    if !words.trim().is_empty() {
        text.push_str(": ");
        text.push_str(words.trim());
    }
    text
}

/// The token counts of a `usage` object; missing or odd counts are zero.
fn usage(value: Option<&Value>) -> Usage {
    let count = |key: &str| value.and_then(|u| u.get(key)).and_then(Value::as_u64).unwrap_or(0);
    Usage {
        input_tokens: count("input_tokens"),
        output_tokens: count("output_tokens"),
        cache_read_input_tokens: count("cache_read_input_tokens"),
        cache_creation_input_tokens: count("cache_creation_input_tokens"),
    }
}

/// The text of `content`: a string, or the `text` of each text block of a list, one per line;
/// other blocks are named by their type. Capped at `max` bytes.
fn joined_text(content: Option<&Value>, max: usize) -> String {
    match content {
        Some(Value::String(text)) => capped(text, max),
        Some(Value::Array(blocks)) => {
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
        _ => String::new(),
    }
}

/// `mcp__<server>__<tool>` split into its server and tool; any other name is a tool alone.
fn split_tool(name: &str) -> (Option<String>, String) {
    match name.strip_prefix("mcp__").and_then(|rest| rest.split_once("__")) {
        Some((server, tool)) if !server.is_empty() && !tool.is_empty() => (Some(capped(server, MAX_NAME_BYTES)), capped(tool, MAX_NAME_BYTES)),
        _ => (None, capped(name, MAX_NAME_BYTES)),
    }
}

/// A type or category name from the input, safe to log: cut to a few bytes and kept to letters,
/// digits, `_`, `/` and `-`.
fn kind_name(name: &str) -> String {
    let kept: String = name.chars().filter(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '/' | '-')).collect();
    let kept = capped(&kept, MAX_KIND_BYTES);
    if kept.is_empty() { "<unnamed>".to_owned() } else { kept }
}

/// `text` cut to at most `max` bytes at a character boundary, with an ellipsis when cut.
pub(crate) fn capped(text: &str, max: usize) -> String {
    if text.len() <= max {
        return text.to_owned();
    }
    log::debug!("claude wrote a text of {} bytes; the first {max} are kept", text.len());
    let mut end = max;
    while !text.is_char_boundary(end) {
        end = end.saturating_sub(1);
    }
    let mut cut = text.get(..end).unwrap_or_default().to_owned();
    cut.push('…');
    cut
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each documented category lands in its neutral category; the HTTP status decides only when
    /// the category does not.
    #[test]
    fn categories_map_to_neutral_kinds() {
        let cases = [
            ("authentication_failed", None, None, ErrorKind::InvalidCredential),
            ("cloud_credential_error", None, None, ErrorKind::InvalidCredential),
            ("rate_limit", None, Some(429), ErrorKind::RateLimited),
            ("billing_error", None, None, ErrorKind::RateLimited),
            ("model_not_found", None, None, ErrorKind::ModelUnavailable),
            ("overloaded", None, Some(529), ErrorKind::ModelUnavailable),
            ("server_error", Some("connection_failed"), None, ErrorKind::Network),
            ("api_error", Some("connection_failed"), None, ErrorKind::Network),
            ("server_error", None, Some(500), ErrorKind::Other),
            ("unknown", None, None, ErrorKind::Other),
            ("oauth_org_not_allowed", None, None, ErrorKind::Other),
            ("account_on_hold", None, None, ErrorKind::Other),
            ("invalid_request", None, Some(400), ErrorKind::Other),
            ("max_output_tokens", None, None, ErrorKind::Other),
            ("something_new", None, Some(401), ErrorKind::InvalidCredential),
        ];
        for (category, api_error, status, kind) in cases {
            assert_eq!(kind_of(category, api_error, status), kind, "{category} {api_error:?} {status:?}");
        }
    }

    /// Names are cut at character boundaries and kept safe for the log.
    #[test]
    fn names_are_capped_and_cleaned() {
        assert_eq!(capped("ééé", 3), "é…");
        assert_eq!(capped("abc", 3), "abc");
        assert_eq!(kind_name("brand_new\u{202E}event\n"), "brand_newevent");
        assert_eq!(kind_name("\u{1F600}"), "<unnamed>");
        assert_eq!(kind_name(&"x".repeat(500)).len(), MAX_KIND_BYTES + '…'.len_utf8());
        assert_eq!(split_tool("mcp__archforge__er_entity_list"), (Some("archforge".into()), "er_entity_list".into()));
        assert_eq!(split_tool("mcp____x"), (None, "mcp____x".into()));
        assert_eq!(split_tool("Bash"), (None, "Bash".into()));
    }
}
