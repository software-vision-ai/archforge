//! What Codex's notifications mean in the neutral session events, and how its failures map to the
//! neutral categories (the error mapping of `docs/agent-runtimes/README.md`). Every function here
//! reads one message and keeps nothing; every text taken from Codex is capped.

use serde_json::Value;

use super::rpc::checked_id;
use crate::claude::{MAX_INPUT_BYTES, MAX_NAME_BYTES, MAX_SUMMARY_BYTES};
use crate::session::{SessionEvent, ToolServer, TurnOutcome};
use crate::{AgentError, ErrorKind};

/// The most of Codex's own words an error keeps, in bytes.
const MAX_ERROR_TEXT_BYTES: usize = 1024;

/// How a session signed in, which decides what a refusal of the credentials means: a key that is
/// refused is invalid, a ChatGPT sign-in that is refused has expired.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignedInWith {
    /// An API key.
    ApiKey,
    /// A ChatGPT sign-in.
    ChatGpt,
}

impl SignedInWith {
    /// The category of a refused credential.
    fn refused(self) -> ErrorKind {
        match self {
            SignedInWith::ApiKey => ErrorKind::InvalidCredential,
            SignedInWith::ChatGpt => ErrorKind::SignInExpired,
        }
    }
}

/// The neutral category of Codex's `codexErrorInfo`: `unauthorized` is a refused credential (see
/// [`SignedInWith`]); `usageLimitExceeded`, `rateLimitExceeded` and `sessionBudgetExceeded` are
/// [`ErrorKind::RateLimited`]; `modelNotFound`, `serverOverloaded`, `flexUnavailable` and
/// `policyCapacityUnavailable` [`ErrorKind::ModelUnavailable`]; the connection failures
/// (`httpConnectionFailed`, `responseStreamConnectionFailed`, `responseStreamDisconnected`,
/// `responseTooManyFailedAttempts`) are [`ErrorKind::Network`] unless their HTTP status says a
/// refused credential (401, 403), a limit (429) or a missing model (404); anything else, a missing
/// value among them, is [`ErrorKind::Other`].
pub fn kind_of(info: Option<&Value>, signed_in_with: SignedInWith) -> ErrorKind {
    match info {
        Some(Value::String(name)) => match name.as_str() {
            "unauthorized" => signed_in_with.refused(),
            "usageLimitExceeded" | "rateLimitExceeded" | "sessionBudgetExceeded" => ErrorKind::RateLimited,
            "modelNotFound" | "serverOverloaded" | "flexUnavailable" | "policyCapacityUnavailable" => ErrorKind::ModelUnavailable,
            _ => ErrorKind::Other,
        },
        Some(Value::Object(object)) => {
            const CONNECTION: [&str; 4] =
                ["httpConnectionFailed", "responseStreamConnectionFailed", "responseStreamDisconnected", "responseTooManyFailedAttempts"];
            let Some(failure) = CONNECTION.iter().find_map(|name| object.get(*name)) else { return ErrorKind::Other };
            match failure.get("httpStatusCode").and_then(Value::as_u64) {
                Some(401 | 403) => signed_in_with.refused(),
                Some(429) => ErrorKind::RateLimited,
                Some(404) => ErrorKind::ModelUnavailable,
                _ => ErrorKind::Network,
            }
        }
        _ => ErrorKind::Other,
    }
}

/// The name of a `codexErrorInfo` for an error's detail: its text, or its one key; cleaned.
fn info_name(info: Option<&Value>) -> String {
    let name = match info {
        Some(Value::String(name)) => name.as_str(),
        Some(Value::Object(object)) => object.keys().next().map_or("", String::as_str),
        _ => "",
    };
    let name = super::rpc::kind_name(name);
    if name == "<unnamed>" { "no category".to_owned() } else { name }
}

/// A `TurnError` (`{message, codexErrorInfo}`) as a failure in its category, with Codex's words cut
/// short; `None` when it is not an object.
pub(crate) fn failure(error: &Value, signed_in_with: SignedInWith) -> Option<AgentError> {
    let object = error.as_object()?;
    let info = object.get("codexErrorInfo");
    let message = object.get("message").and_then(Value::as_str).unwrap_or_default();
    let words = capped(message, MAX_ERROR_TEXT_BYTES);
    Some(AgentError::reported(kind_of(info, signed_in_with), &format!("codex: {} ({})", words, info_name(info))))
}

/// How a turn ended, from `turn/completed`'s turn: `completed` is completed, `interrupted`
/// stopped; `failed` fails with the turn's own error, else `last_error` (the last `error`
/// notification of the turn that Codex did not retry), else it is stopped when a stop was asked
/// for, else Other; any other status fails as Other.
pub(crate) fn turn_completed(turn: &Value, last_error: Option<AgentError>, stop_requested: bool, signed_in_with: SignedInWith) -> TurnOutcome {
    match turn.get("status").and_then(Value::as_str) {
        Some("completed") => TurnOutcome::Completed,
        Some("interrupted") => TurnOutcome::Stopped,
        Some("failed") => {
            if let Some(error) = turn.get("error").and_then(|e| failure(e, signed_in_with)) {
                TurnOutcome::Failed(error)
            } else if let Some(error) = last_error {
                TurnOutcome::Failed(error)
            } else if stop_requested {
                TurnOutcome::Stopped
            } else {
                TurnOutcome::Failed(AgentError::reported(ErrorKind::Other, "codex says the turn failed, without saying why"))
            }
        }
        other => {
            let status = other.map_or_else(|| "none".to_owned(), super::rpc::kind_name);
            TurnOutcome::Failed(AgentError::reported(ErrorKind::Other, &format!("codex ended the turn with the unknown status {status}")))
        }
    }
}

/// The thread id a notification names in its `threadId`.
pub(crate) fn thread_of(params: &Value) -> Option<String> {
    checked_id(params.get("threadId"))
}

/// The turn id a notification names, in its `turnId` or its `turn.id`.
pub(crate) fn turn_of(params: &Value) -> Option<String> {
    checked_id(params.get("turnId")).or_else(|| checked_id(params.get("turn").and_then(|t| t.get("id"))))
}

/// The event `item/started` means: an MCP tool call started; other items mean none.
pub(crate) fn item_started(params: &Value) -> Option<SessionEvent> {
    let item = params.get("item")?;
    if item.get("type").and_then(Value::as_str) != Some("mcpToolCall") {
        return None;
    }
    let id = checked_id(item.get("id"))?;
    let server = item.get("server").and_then(Value::as_str).map(|s| capped(s, MAX_NAME_BYTES));
    let tool = capped(item.get("tool").and_then(Value::as_str).unwrap_or_default(), MAX_NAME_BYTES);
    let input = capped(&item.get("arguments").map(Value::to_string).unwrap_or_default(), MAX_INPUT_BYTES);
    Some(SessionEvent::ToolStarted { id, server, tool, input })
}

/// What `item/completed` means: a finished agent message (its event and its whole text, for the
/// conversation's copy) or a finished MCP tool call; other items mean nothing.
pub(crate) fn item_completed(params: &Value) -> Option<(SessionEvent, Option<String>)> {
    let item = params.get("item")?;
    match item.get("type").and_then(Value::as_str)? {
        "agentMessage" => {
            let text = item.get("text").and_then(Value::as_str)?.to_owned();
            Some((SessionEvent::MessageDone { text: text.clone() }, Some(text)))
        }
        "mcpToolCall" => {
            let id = checked_id(item.get("id"))?;
            let tool = item.get("tool").and_then(Value::as_str).map(|t| capped(t, MAX_NAME_BYTES));
            let error = item.get("error").and_then(|e| e.get("message")).and_then(Value::as_str);
            let ok = item.get("status").and_then(Value::as_str) == Some("completed") && error.is_none();
            let summary = match error {
                Some(message) => capped(message, MAX_SUMMARY_BYTES),
                None => capped(&first_text(item.get("result")), MAX_SUMMARY_BYTES),
            };
            Some((SessionEvent::ToolFinished { id, tool, ok, summary }, None))
        }
        _ => None,
    }
}

/// The first text block of an MCP tool result (`{content: [{type: "text", text}]}`), or nothing.
fn first_text(result: Option<&Value>) -> String {
    result
        .and_then(|r| r.get("content"))
        .and_then(Value::as_array)
        .and_then(|blocks| blocks.iter().find_map(|b| (b.get("type").and_then(Value::as_str) == Some("text")).then(|| b.get("text")).flatten()))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

/// The event `item/agentMessage/delta` means: a piece of the answer.
pub(crate) fn delta(params: &Value) -> Option<SessionEvent> {
    params.get("delta").and_then(Value::as_str).map(|text| SessionEvent::TextDelta { text: text.to_owned() })
}

/// A tool server's state from `mcpServer/startupStatus/updated`, with the thread it names (none
/// when Codex names none); `None` when it has no name or state.
pub(crate) fn server_status(params: &Value) -> Option<(Option<String>, ToolServer)> {
    let name = params.get("name").and_then(Value::as_str)?;
    let status = params.get("status").and_then(Value::as_str)?;
    Some((thread_of(params), ToolServer { name: capped(name, MAX_NAME_BYTES), status: super::rpc::kind_name(status) }))
}

/// `text` cut to at most `max` bytes at a character boundary, with an ellipsis after the cut; a cut
/// is logged by its length alone.
pub(crate) fn capped(text: &str, max: usize) -> String {
    if text.len() > max {
        log::debug!("codex wrote a text of {} bytes; the first {max} are kept", text.len());
    }
    crate::error::capped_to(text.to_owned(), max)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    /// Each documented `codexErrorInfo` lands in its neutral category, the HTTP status deciding only
    /// inside the connection failures, and `unauthorized` by how the session signed in.
    #[test]
    fn error_infos_map_to_neutral_kinds() {
        let key = SignedInWith::ApiKey;
        let chatgpt = SignedInWith::ChatGpt;
        assert_eq!(kind_of(Some(&json!("unauthorized")), key), ErrorKind::InvalidCredential);
        assert_eq!(kind_of(Some(&json!("unauthorized")), chatgpt), ErrorKind::SignInExpired);
        for limited in ["usageLimitExceeded", "rateLimitExceeded", "sessionBudgetExceeded"] {
            assert_eq!(kind_of(Some(&json!(limited)), key), ErrorKind::RateLimited, "{limited}");
        }
        for missing in ["modelNotFound", "serverOverloaded", "flexUnavailable", "policyCapacityUnavailable"] {
            assert_eq!(kind_of(Some(&json!(missing)), key), ErrorKind::ModelUnavailable, "{missing}");
        }
        for other in ["badRequest", "internalServerError", "contextWindowExceeded", "sandboxError", "other", "somethingNew"] {
            assert_eq!(kind_of(Some(&json!(other)), key), ErrorKind::Other, "{other}");
        }
        let connection = |name: &str, status: Value| json!({ name: { "httpStatusCode": status } });
        assert_eq!(kind_of(Some(&connection("httpConnectionFailed", Value::Null)), key), ErrorKind::Network);
        assert_eq!(kind_of(Some(&connection("responseStreamDisconnected", json!(500))), key), ErrorKind::Network);
        assert_eq!(kind_of(Some(&connection("responseStreamConnectionFailed", json!(401))), chatgpt), ErrorKind::SignInExpired);
        assert_eq!(kind_of(Some(&connection("responseTooManyFailedAttempts", json!(403))), key), ErrorKind::InvalidCredential);
        assert_eq!(kind_of(Some(&connection("httpConnectionFailed", json!(429))), key), ErrorKind::RateLimited);
        assert_eq!(kind_of(Some(&connection("httpConnectionFailed", json!(404))), key), ErrorKind::ModelUnavailable);
        assert_eq!(kind_of(Some(&json!({ "activeTurnNotSteerable": {} })), key), ErrorKind::Other);
        assert_eq!(kind_of(None, key), ErrorKind::Other);
        assert_eq!(kind_of(Some(&json!(7)), key), ErrorKind::Other);
    }

    /// A failed turn takes its own error, else the last unretried error, else a stop, else Other.
    #[test]
    fn a_turn_outcome_reads_its_status_and_errors() {
        let key = SignedInWith::ApiKey;
        assert_eq!(turn_completed(&json!({"status": "completed"}), None, true, key), TurnOutcome::Completed);
        assert_eq!(turn_completed(&json!({"status": "interrupted"}), None, false, key), TurnOutcome::Stopped);
        let own = json!({"status": "failed", "error": {"message": "slow down", "codexErrorInfo": "rateLimitExceeded"}});
        let last = AgentError::reported(ErrorKind::Network, "earlier");
        let TurnOutcome::Failed(e) = turn_completed(&own, Some(last.clone()), true, key) else { panic!("not failed") };
        assert_eq!(e.kind(), ErrorKind::RateLimited);
        assert!(e.to_string().contains("slow down") && e.to_string().contains("rateLimitExceeded"), "{e}");
        assert_eq!(turn_completed(&json!({"status": "failed", "error": null}), Some(last.clone()), false, key), TurnOutcome::Failed(last));
        assert_eq!(turn_completed(&json!({"status": "failed"}), None, true, key), TurnOutcome::Stopped);
        assert!(matches!(turn_completed(&json!({"status": "failed"}), None, false, key), TurnOutcome::Failed(e) if e.kind() == ErrorKind::Other));
        assert!(matches!(turn_completed(&json!({"status": "weird"}), None, true, key), TurnOutcome::Failed(e) if e.kind() == ErrorKind::Other));
    }

    /// Ids are taken whole or not at all.
    #[test]
    fn ids_are_taken_whole_or_refused() {
        assert_eq!(checked_id(Some(&json!("019a0000-0000-7000-8000-000000000001"))).as_deref(), Some("019a0000-0000-7000-8000-000000000001"));
        assert_eq!(checked_id(Some(&json!("item-3"))).as_deref(), Some("item-3"));
        for bad in [json!(""), json!("a b"), json!("a\"b"), json!("x".repeat(super::super::rpc::MAX_ID_BYTES + 1)), json!(7), Value::Null] {
            assert_eq!(checked_id(Some(&bad)), None, "{bad}");
        }
        assert_eq!(checked_id(None), None);
        assert_eq!(turn_of(&json!({"turn": {"id": "t1"}})).as_deref(), Some("t1"));
        assert_eq!(turn_of(&json!({"turnId": "t2", "turn": {"id": "t1"}})).as_deref(), Some("t2"));
    }

    /// Tool calls and messages become events with capped texts.
    #[test]
    fn items_become_tool_and_message_events() {
        let started =
            json!({"item": {"type": "mcpToolCall", "id": "item-2", "server": "archforge", "tool": "er_entity_list", "arguments": {"schema": "s"}}});
        assert_eq!(
            item_started(&started),
            Some(SessionEvent::ToolStarted {
                id: "item-2".into(),
                server: Some("archforge".into()),
                tool: "er_entity_list".into(),
                input: r#"{"schema":"s"}"#.into()
            })
        );
        assert_eq!(item_started(&json!({"item": {"type": "agentMessage", "id": "i"}})), None);
        let done = json!({"item": {"type": "mcpToolCall", "id": "item-2", "tool": "t", "status": "failed", "error": {"message": "refused"}, "result": null}});
        assert_eq!(
            item_completed(&done),
            Some((SessionEvent::ToolFinished { id: "item-2".into(), tool: Some("t".into()), ok: false, summary: "refused".into() }, None))
        );
        let big = "x".repeat(MAX_SUMMARY_BYTES * 2);
        let ok = json!({"item": {"type": "mcpToolCall", "id": "i", "status": "completed", "error": null, "result": {"content": [{"type": "text", "text": big}]}}});
        let Some((SessionEvent::ToolFinished { ok: true, summary, .. }, None)) = item_completed(&ok) else { panic!("not finished") };
        assert!(summary.len() <= MAX_SUMMARY_BYTES + '…'.len_utf8());
        let message = json!({"item": {"type": "agentMessage", "id": "i", "text": "Hello"}});
        assert_eq!(item_completed(&message), Some((SessionEvent::MessageDone { text: "Hello".into() }, Some("Hello".into()))));
        assert_eq!(item_completed(&json!({"item": {"type": "userMessage", "id": "i"}})), None);
        assert_eq!(
            server_status(&json!({"name": "archforge", "status": "ready", "threadId": "t"})),
            Some((Some("t".into()), ToolServer { name: "archforge".into(), status: "ready".into() }))
        );
        assert_eq!(server_status(&json!({"status": "ready"})), None);
    }
}
