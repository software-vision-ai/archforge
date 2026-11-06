//! Codex's wire: JSON-RPC 2.0 over standard input and output, one object a line, without the
//! `jsonrpc` field (AR-196). Each line read becomes an [`Incoming`]; the requests sent are matched to
//! their answers by id in [`Requests`], each with its own deadline, because Codex never answers a
//! line it cannot read and answers every error, an unknown method among them, with code -32600.

use std::time::{Duration, Instant};

use serde_json::{Map, Value, json};

use crate::AgentError;
use crate::session::Skipped;

/// The most requests waiting for an answer at once; one more is refused.
pub const MAX_PENDING: usize = 64;
/// The longest error message kept from an answer, in bytes (Codex lists every method it knows in
/// the message for an unknown one).
pub const MAX_ERROR_MESSAGE_BYTES: usize = 1024;
/// The longest method name read, in bytes; a longer one is skipped as malformed.
pub const MAX_METHOD_BYTES: usize = 128;

/// A JSON-RPC error answer: its code and its message, cut to [`MAX_ERROR_MESSAGE_BYTES`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
}

impl RpcError {
    /// Whether the answer says Codex does not know the method, which is how a Codex too old for a
    /// method Archforge needs shows at run time (`-32600 … unknown variant`).
    pub fn unknown_method(&self) -> bool {
        self.message.contains("unknown variant")
    }
}

/// One line from Codex, read but not yet acted on.
#[derive(Debug, Clone, PartialEq)]
pub enum Incoming {
    /// The answer to a request of ours, by its id.
    Response { id: u64, result: Result<Value, RpcError> },
    /// A notification: no answer is expected.
    Notification { method: String, params: Value },
    /// A request from Codex (an approval, an elicitation): it waits for an answer under `id`.
    Request { id: Value, method: String, params: Value },
    /// A line that is none of these; it is counted and dropped.
    Skipped(Skipped),
}

/// Reads one line. Nothing here panics or allocates by a size the line declares; a line longer than
/// `max_line_bytes` is never parsed.
pub fn read_line(line: &str, max_line_bytes: usize) -> Incoming {
    if line.len() > max_line_bytes {
        return Incoming::Skipped(Skipped::TooLong { bytes: u64::try_from(line.len()).unwrap_or(u64::MAX) });
    }
    if line.trim().is_empty() {
        return Incoming::Skipped(Skipped::Empty);
    }
    // serde_json refuses nesting past 128 levels with an error, never a stack overflow.
    let Ok(value) = serde_json::from_str::<Value>(line) else { return Incoming::Skipped(Skipped::NotJson { bytes: line.len() }) };
    let Value::Object(mut object) = value else { return Incoming::Skipped(Skipped::NotAnObject) };
    match object.remove("method") {
        Some(Value::String(method)) => {
            if method.is_empty() || method.len() > MAX_METHOD_BYTES {
                return Incoming::Skipped(Skipped::Malformed { kind: "message", reason: "its method name is empty or too long" });
            }
            let params = object.remove("params").unwrap_or(Value::Null);
            match object.remove("id") {
                None => Incoming::Notification { method, params },
                Some(id @ (Value::Number(_) | Value::String(_))) => Incoming::Request { id, method, params },
                Some(_) => Incoming::Skipped(Skipped::Malformed { kind: "request", reason: "its id is neither a number nor text" }),
            }
        }
        Some(_) => Incoming::Skipped(Skipped::Malformed { kind: "message", reason: "its method is not text" }),
        None => match object.remove("id") {
            None => Incoming::Skipped(Skipped::Untyped),
            Some(id) => response(id, object),
        },
    }
}

/// The answer whose id is `id`: its error or its result.
fn response(id: Value, mut object: Map<String, Value>) -> Incoming {
    let Some(id) = id.as_u64() else {
        return Incoming::Skipped(Skipped::Malformed { kind: "response", reason: "its id is not one Archforge gives" });
    };
    if let Some(error) = object.remove("error") {
        let code = error.get("code").and_then(Value::as_i64).unwrap_or(0);
        let message = capped(error.get("message").and_then(Value::as_str).unwrap_or_default(), MAX_ERROR_MESSAGE_BYTES);
        return Incoming::Response { id, result: Err(RpcError { code, message }) };
    }
    match object.remove("result") {
        Some(result) => Incoming::Response { id, result: Ok(result) },
        None => Incoming::Skipped(Skipped::Malformed { kind: "response", reason: "it has neither a result nor an error" }),
    }
}

/// A request Archforge sent and still waits for.
#[derive(Debug)]
struct Pending<P> {
    id: u64,
    method: &'static str,
    deadline: Instant,
    purpose: P,
}

/// The requests waiting for their answers, each with the deadline it was given: ids count up from
/// 1, an answer closes its request, and a request past its deadline is handed back as expired.
#[derive(Debug)]
pub struct Requests<P> {
    next: u64,
    pending: Vec<Pending<P>>,
}

impl<P> Default for Requests<P> {
    fn default() -> Self {
        Requests::new()
    }
}

impl<P> Requests<P> {
    /// No request yet; the first id is 1.
    pub fn new() -> Requests<P> {
        Requests { next: 1, pending: Vec::new() }
    }

    /// Opens a request for `method`, due `timeout` after `now`, and answers its id. Refused: more
    /// than [`MAX_PENDING`] requests waiting (Codex is not answering), and a deadline past what the
    /// clock can hold.
    pub fn open(&mut self, method: &'static str, purpose: P, timeout: Duration, now: Instant) -> Result<u64, AgentError> {
        if self.pending.len() >= MAX_PENDING {
            return Err(AgentError::reported(
                crate::ErrorKind::Other,
                &format!("codex has not answered {MAX_PENDING} requests; no more are sent until it does"),
            ));
        }
        let deadline = now.checked_add(timeout).ok_or(AgentError::InvalidLimit {
            limit: "request timeout (ms)",
            value: timeout.as_millis(),
            accepted: "a duration the clock can hold".to_owned(),
        })?;
        let id = self.next;
        self.next = self.next.checked_add(1).ok_or_else(|| AgentError::reported(crate::ErrorKind::Other, "the request ids are used up"))?;
        self.pending.push(Pending { id, method, deadline, purpose });
        Ok(id)
    }

    /// Closes the request `id` on its answer, handing back its method and purpose; `None` when no
    /// such request waits (answered already, expired, or never sent).
    pub fn close(&mut self, id: u64) -> Option<(&'static str, P)> {
        let at = self.pending.iter().position(|p| p.id == id)?;
        let pending = self.pending.remove(at);
        Some((pending.method, pending.purpose))
    }

    /// Takes out every request whose deadline is at or before `now`, oldest first.
    pub fn expired(&mut self, now: Instant) -> Vec<(u64, &'static str, P)> {
        let mut out = Vec::new();
        let mut kept = Vec::with_capacity(self.pending.len());
        for pending in self.pending.drain(..) {
            if pending.deadline <= now {
                out.push((pending.id, pending.method, pending.purpose));
            } else {
                kept.push(pending);
            }
        }
        self.pending = kept;
        out
    }

    /// The earliest deadline of the requests waiting.
    pub fn next_deadline(&self) -> Option<Instant> {
        self.pending.iter().map(|p| p.deadline).min()
    }

    /// Takes out every request still waiting (the runtime ended), oldest first.
    pub fn drain(&mut self) -> Vec<(u64, &'static str, P)> {
        self.pending.drain(..).map(|p| (p.id, p.method, p.purpose)).collect()
    }

    /// How many requests wait.
    pub fn len(&self) -> usize {
        self.pending.len()
    }

    /// Whether no request waits.
    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }
}

/// A request line: `{"id":…,"method":…,"params":…}` (no `params` when `None`).
pub fn request_line(id: u64, method: &str, params: Option<Value>) -> String {
    match params {
        Some(params) => json!({ "id": id, "method": method, "params": params }).to_string(),
        None => json!({ "id": id, "method": method }).to_string(),
    }
}

/// A notification line without parameters.
pub fn notification_line(method: &str) -> String {
    json!({ "method": method }).to_string()
}

/// The answer to a request from Codex.
pub fn result_line(id: &Value, result: Value) -> String {
    json!({ "id": id, "result": result }).to_string()
}

/// An error answer to a request from Codex.
pub fn error_line(id: &Value, code: i64, message: &str) -> String {
    json!({ "id": id, "error": { "code": code, "message": message } }).to_string()
}

/// `text` cut to at most `max` bytes at a character boundary, with an ellipsis when cut.
pub(crate) fn capped(text: &str, max: usize) -> String {
    if text.len() <= max {
        return text.to_owned();
    }
    log::debug!("codex wrote a text of {} bytes; the first {max} are kept", text.len());
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

    /// Each shape of line lands in its kind; nothing panics on odd shapes.
    #[test]
    fn lines_are_classified() {
        let max = 1 << 20;
        assert!(matches!(read_line(r#"{"id":3,"result":{"a":1}}"#, max), Incoming::Response { id: 3, result: Ok(_) }));
        let Incoming::Response { id: 0, result: Err(error) } = read_line(r#"{"error":{"code":-32600,"message":"Not initialized"},"id":0}"#, max)
        else {
            panic!()
        };
        assert_eq!((error.code, error.message.as_str(), error.unknown_method()), (-32600, "Not initialized", false));
        let unknown = read_line(r#"{"error":{"code":-32600,"message":"Invalid request: unknown variant `x/y`, expected one of"},"id":6}"#, max);
        assert!(matches!(unknown, Incoming::Response { result: Err(e), .. } if e.unknown_method()));
        assert!(
            matches!(read_line(r#"{"method":"turn/started","params":{}}"#, max), Incoming::Notification { method, .. } if method == "turn/started")
        );
        assert!(matches!(read_line(r#"{"id":"s-1","method":"item/fileChange/requestApproval"}"#, max), Incoming::Request { .. }));
        assert!(matches!(read_line(r#"{"id":0,"method":"mcpServer/elicitation/request","params":{}}"#, max), Incoming::Request { .. }));
        for (line, skipped) in [
            ("", Skipped::Empty),
            ("  ", Skipped::Empty),
            ("nope", Skipped::NotJson { bytes: 4 }),
            ("[1]", Skipped::NotAnObject),
            ("{}", Skipped::Untyped),
            (r#"{"id":-1,"result":{}}"#, Skipped::Malformed { kind: "response", reason: "its id is not one Archforge gives" }),
            (r#"{"id":null,"error":{}}"#, Skipped::Malformed { kind: "response", reason: "its id is not one Archforge gives" }),
            (r#"{"id":4}"#, Skipped::Malformed { kind: "response", reason: "it has neither a result nor an error" }),
            (r#"{"method":7}"#, Skipped::Malformed { kind: "message", reason: "its method is not text" }),
            (r#"{"method":""}"#, Skipped::Malformed { kind: "message", reason: "its method name is empty or too long" }),
            (r#"{"method":"x","id":[1]}"#, Skipped::Malformed { kind: "request", reason: "its id is neither a number nor text" }),
        ] {
            assert_eq!(read_line(line, max), Incoming::Skipped(skipped), "{line}");
        }
        assert_eq!(read_line(&"x".repeat(20), 10), Incoming::Skipped(Skipped::TooLong { bytes: 20 }));
        // An error without a code or message still answers its request.
        assert!(matches!(read_line(r#"{"id":1,"error":7}"#, max), Incoming::Response { id: 1, result: Err(RpcError { code: 0, .. }) }));
        let long = format!(r#"{{"id":1,"error":{{"code":1,"message":"{}"}}}}"#, "é".repeat(MAX_ERROR_MESSAGE_BYTES));
        assert!(matches!(read_line(&long, max), Incoming::Response { result: Err(e), .. } if e.message.len() <= MAX_ERROR_MESSAGE_BYTES + 3));
    }

    /// Answers close their own request, in any order; an answer to nothing closes nothing.
    #[test]
    fn answers_are_matched_by_id() {
        let now = Instant::now();
        let mut requests = Requests::new();
        let a = requests.open("initialize", 'a', Duration::from_secs(5), now).unwrap();
        let b = requests.open("account/read", 'b', Duration::from_secs(5), now).unwrap();
        assert_eq!((a, b), (1, 2));
        assert_eq!(requests.close(b), Some(("account/read", 'b')));
        assert_eq!(requests.close(b), None, "an answer counts once");
        assert_eq!(requests.close(99), None);
        assert_eq!(requests.close(a), Some(("initialize", 'a')));
        assert!(requests.is_empty());
    }

    /// Every request has its own deadline: an early one expires alone, and an answer after the
    /// deadline finds nothing to close.
    #[test]
    fn each_request_times_out_on_its_own() {
        let now = Instant::now();
        let mut requests = Requests::new();
        let short = requests.open("thread/start", 1, Duration::from_millis(100), now).unwrap();
        let long = requests.open("turn/interrupt", 2, Duration::from_secs(30), now).unwrap();
        assert_eq!(requests.next_deadline(), now.checked_add(Duration::from_millis(100)));
        assert!(requests.expired(now).is_empty());
        let gone = requests.expired(now + Duration::from_millis(100));
        assert_eq!(gone, vec![(short, "thread/start", 1)]);
        assert_eq!(requests.close(short), None, "a late answer to an expired request closes nothing");
        assert_eq!(requests.len(), 1);
        assert_eq!(requests.drain(), vec![(long, "turn/interrupt", 2)]);
    }

    /// Past the cap of waiting requests, and with a deadline the clock cannot hold, a request is
    /// refused rather than queued.
    #[test]
    fn requests_past_the_cap_are_refused() {
        let now = Instant::now();
        let mut requests = Requests::new();
        for _ in 0..MAX_PENDING {
            requests.open("model/list", (), Duration::from_secs(1), now).unwrap();
        }
        assert!(matches!(requests.open("model/list", (), Duration::from_secs(1), now), Err(AgentError::Reported { .. })));
        let mut requests = Requests::new();
        assert!(matches!(requests.open("x", (), Duration::MAX, now), Err(AgentError::InvalidLimit { .. })));
    }

    /// Lines are one compact JSON object each, without the `jsonrpc` field.
    #[test]
    fn lines_are_written_compact() {
        assert_eq!(request_line(1, "account/read", Some(json!({}))), r#"{"id":1,"method":"account/read","params":{}}"#);
        assert_eq!(request_line(2, "account/logout", None), r#"{"id":2,"method":"account/logout"}"#);
        assert_eq!(notification_line("initialized"), r#"{"method":"initialized"}"#);
        assert_eq!(result_line(&json!(0), json!({"decision":"decline"})), r#"{"id":0,"result":{"decision":"decline"}}"#);
        assert_eq!(error_line(&json!("a"), -32601, "no"), r#"{"error":{"code":-32601,"message":"no"},"id":"a"}"#);
    }
}
