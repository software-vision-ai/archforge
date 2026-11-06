//! The control channel an agent drives the running application through (agent-control-surfaces
//! §10.1): a loopback port the application opens only when started with `--control <port>`, a
//! token minted for each start and kept in a file only its owner can read, one JSON object per
//! line. The application answers through its command engine, so the window shows every change and
//! undo covers it. `archforge-cli` reaches the port through [`Client`].
//!
//! The wire: a request is `{id, method, params}`, the first on a connection also carrying `token`,
//! and any request may carry `origin`, the label of who sent it (a chat, say), which the
//! application puts on the undo steps the request makes; a reply is `{id, ok: true, result}` or
//! `{id, ok: false, error}`. The origin is a key of the request, never of its parameters, so the
//! one who chooses the parameters (an agent through `call_app`) cannot choose it. A first line that is not a
//! well-formed request with the right token closes the connection without a word, so a web page
//! cannot drive the port (CS-6). Lines, requests, replies, connections and waits are capped (CS-7).
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unreachable, clippy::todo, clippy::unimplemented)]

#[cfg(unix)]
use std::io::Read;
use std::io::{BufRead, BufReader, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

/// The longest line read, the largest request handled and the largest reply sent, in bytes.
pub const MAX_LINE: usize = 4 * 1024 * 1024;
pub const MAX_REQUEST: usize = 1024 * 1024;
pub const MAX_REPLY: usize = 16 * 1024 * 1024;
/// The most connections served at once.
pub const MAX_CONNECTIONS: usize = 16;
/// How long a read or a write may wait, and how long the application may take to answer.
pub const TIMEOUT: Duration = Duration::from_secs(30);
/// The token's length in hexadecimal characters: 32 bytes, 256 bits.
pub const TOKEN_LEN: usize = 64;
/// How long a connection's first line may take to arrive in full: a peer that has not shown the
/// token gets little time, so it cannot hold a slot (each later line has [`TIMEOUT`] in all).
const FIRST_LINE: Duration = if cfg!(test) { Duration::from_millis(800) } else { Duration::from_secs(5) };

/// The application's per-user settings folder, where the token file lives: the platform's
/// convention, or a folder in the temporary directory when the platform gives none. The
/// application and the command-line tool find the same one.
pub fn settings_folder() -> PathBuf {
    settings_dir("Archforge").unwrap_or_else(|| std::env::temp_dir().join("archforge"))
}

fn settings_dir(app: &str) -> Option<PathBuf> {
    let var = |k: &str| std::env::var_os(k).filter(|v| !v.is_empty()).map(PathBuf::from);
    if cfg!(target_os = "windows") {
        var("APPDATA").map(|d| d.join(app))
    } else if cfg!(target_os = "macos") {
        var("HOME").map(|h| h.join("Library").join("Application Support").join(app))
    } else {
        var("XDG_CONFIG_HOME").or_else(|| var("HOME").map(|h| h.join(".config"))).map(|d| d.join(app.to_lowercase().replace(' ', "-")))
    }
}

/// The token file of the application listening on `port`.
pub fn token_path(settings: &Path, port: u16) -> PathBuf {
    settings.join(format!("control-{port}.token"))
}

/// A token minted from the operating system's random source, as 64 lowercase hexadecimal
/// characters. Never logged.
pub fn mint_token() -> Result<String, String> {
    let mut bytes = [0u8; TOKEN_LEN / 2];
    getrandom::fill(&mut bytes).map_err(|e| format!("the system's random source failed: {e}"))?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

/// Whether two tokens are the same, in time that does not depend on where they differ.
pub fn same_token(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    // Every byte is looked at whatever the earlier ones held; `black_box` keeps the compiler from
    // stopping early.
    a.len() == b.len() && std::hint::black_box(a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y))) == 0
}

#[cfg_attr(not(unix), allow(dead_code))]
fn is_token(t: &str) -> bool {
    t.len() == TOKEN_LEN && t.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

/// Writes `token` to `path`, created exclusively and readable by its owner alone. A file left there
/// by an earlier start of ours (a regular file we own) is replaced; anything else refuses.
#[cfg(unix)]
pub fn write_token(path: &Path, token: &str) -> Result<(), String> {
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
    if let Ok(m) = std::fs::symlink_metadata(path) {
        if !m.file_type().is_file() || m.uid() != euid() {
            return Err(format!("{} is there and is not a token file of ours", path.display()));
        }
        std::fs::remove_file(path).map_err(|e| format!("the old token file {} could not be removed: {e}", path.display()))?;
    }
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .map_err(|e| format!("the token file {} could not be created: {e}", path.display()))?;
    f.write_all(token.as_bytes())
        .and_then(|()| f.write_all(b"\n"))
        .and_then(|()| f.sync_all())
        .map_err(|e| format!("the token file could not be written: {e}"))
}

#[cfg(not(unix))]
pub fn write_token(_path: &Path, _token: &str) -> Result<(), String> {
    Err(NOT_HERE.to_string())
}

/// Why agent access cannot start on this platform.
pub const NOT_HERE: &str = "agent access is not available on this platform yet";

/// Reads the token of `path`, trusting the file only when it is a regular file owned by the
/// effective user with no group or other permission bits, the same file once opened (CS-8).
#[cfg(unix)]
pub fn read_token(path: &Path) -> Result<String, String> {
    use std::os::unix::fs::MetadataExt;
    let named = std::fs::symlink_metadata(path).map_err(|e| format!("no token at {}: {e}", path.display()))?;
    let trusted = |m: &std::fs::Metadata| m.file_type().is_file() && m.uid() == euid() && m.mode() & 0o077 == 0;
    if !trusted(&named) {
        return Err(format!("{} is not a token file only you can read", path.display()));
    }
    // Never through a link, never waiting on a pipe put in its place.
    use std::os::unix::fs::OpenOptionsExt;
    let flags = rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::NONBLOCK;
    let f = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(i32::try_from(flags.bits()).map_err(|_| "the token file's open flags do not fit".to_string())?)
        .open(path)
        .map_err(|e| format!("the token file could not be opened: {e}"))?;
    let opened = f.metadata().map_err(|e| format!("the token file could not be read: {e}"))?;
    if !trusted(&opened) || opened.dev() != named.dev() || opened.ino() != named.ino() {
        return Err(format!("{} changed while it was opened", path.display()));
    }
    let mut text = String::new();
    f.take((TOKEN_LEN + 2) as u64).read_to_string(&mut text).map_err(|e| format!("the token file could not be read: {e}"))?;
    let token = text.trim_end_matches(['\n', '\r']);
    if !is_token(token) {
        return Err(format!("{} does not hold a token", path.display()));
    }
    Ok(token.to_string())
}

#[cfg(not(unix))]
pub fn read_token(_path: &Path) -> Result<String, String> {
    Err(NOT_HERE.to_string())
}

#[cfg(unix)]
fn euid() -> u32 {
    rustix::process::geteuid().as_raw()
}

/// Reads one line of at most `max` bytes, without its line break: none at the end of the stream,
/// an error for a longer line.
pub fn read_line(r: &mut impl BufRead, max: usize) -> std::io::Result<Option<Vec<u8>>> {
    read_line_with(r, max, || Ok(()))
}

/// [`read_line`] with `before` run ahead of every read, to keep a deadline for the whole line.
fn read_line_with(r: &mut impl BufRead, max: usize, mut before: impl FnMut() -> std::io::Result<()>) -> std::io::Result<Option<Vec<u8>>> {
    let mut out = Vec::new();
    loop {
        before()?;
        let buf = r.fill_buf()?;
        if buf.is_empty() {
            return Ok((!out.is_empty()).then_some(out));
        }
        if let Some(i) = buf.iter().position(|b| *b == b'\n') {
            if out.len() + i > max {
                return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "the line is too long"));
            }
            out.extend_from_slice(buf.get(..i).unwrap_or_default());
            r.consume(i + 1);
            if out.last() == Some(&b'\r') {
                out.pop();
            }
            return Ok(Some(out));
        }
        let n = buf.len();
        if out.len() + n > max {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "the line is too long"));
        }
        out.extend_from_slice(buf);
        r.consume(n);
    }
}

/// What answers a request: the method, its parameters and its origin (checked by [`check_origin`])
/// in, the result or why not out. It may block until the application has answered, at most
/// [`TIMEOUT`].
pub type Handler = Arc<dyn Fn(&str, Value, Option<&str>) -> Result<Value, String> + Send + Sync>;

/// The longest origin, in characters.
pub const MAX_ORIGIN: usize = 64;

/// Why an origin label was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OriginError {
    /// Empty, or nothing but spaces.
    Empty,
    /// Longer than [`MAX_ORIGIN`] characters.
    TooLong { chars: usize },
    /// Space at either end, which would hide in a label.
    Padded,
    /// A character that is not printable text: a control character, a line break, or one that
    /// changes how the text around it shows (direction marks, zero-width characters).
    NotPrintable { at: usize, code: u32 },
}

impl std::fmt::Display for OriginError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OriginError::Empty => write!(f, "the origin is empty; give a short label such as \"Chat 1\""),
            OriginError::TooLong { chars } => write!(f, "the origin has {chars} characters; it may have at most {MAX_ORIGIN}"),
            OriginError::Padded => f.write_str("the origin starts or ends with a space"),
            OriginError::NotPrintable { at, code } => {
                write!(f, "the origin's character {at} (U+{code:04X}) is not printable text; an origin is letters, digits, spaces and punctuation")
            }
        }
    }
}

impl std::error::Error for OriginError {}

/// Checks an origin label: 1 to [`MAX_ORIGIN`] characters of printable text, no space at either
/// end. It is refused, never cleaned up, so the label shown is the label given.
pub fn check_origin(origin: &str) -> Result<(), OriginError> {
    if origin.trim().is_empty() {
        return Err(OriginError::Empty);
    }
    let chars = origin.chars().count();
    if chars > MAX_ORIGIN {
        return Err(OriginError::TooLong { chars });
    }
    if let Some((at, c)) = origin.chars().enumerate().find(|(_, c)| !printable(*c)) {
        return Err(OriginError::NotPrintable { at: at.saturating_add(1), code: u32::from(c) });
    }
    if origin.trim() != origin {
        return Err(OriginError::Padded);
    }
    Ok(())
}

/// Whether `c` is printable text: not a control character, and not one of the format characters
/// that reorder or hide text (direction marks and overrides, zero-width characters, the byte order
/// mark, the line and paragraph separators). A plain space is printable; other spaces are not.
fn printable(c: char) -> bool {
    !(c.is_control()
        || (c.is_whitespace() && c != ' ')
        || matches!(c, '\u{00AD}' | '\u{034F}' | '\u{061C}' | '\u{115F}' | '\u{1160}' | '\u{180E}' | '\u{3164}' | '\u{FEFF}' | '\u{FFA0}')
        || matches!(c, '\u{180B}'..='\u{180F}' | '\u{200B}'..='\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2060}'..='\u{206F}' | '\u{FE00}'..='\u{FE0F}' | '\u{FFF0}'..='\u{FFFB}')
        || matches!(c, '\u{13430}'..='\u{1343F}' | '\u{1BCA0}'..='\u{1BCA3}' | '\u{1D173}'..='\u{1D17A}' | '\u{E0000}'..='\u{E0FFF}'))
}

/// The listening port, its token file and its threads; dropping it stops listening and removes
/// the token file.
pub struct Server {
    port: u16,
    token_file: PathBuf,
    stop: Arc<AtomicBool>,
}

impl Server {
    /// Listens on `port` of the loopback interface (0 for any free port), writes a fresh token to
    /// the settings folder and answers each connection's requests through `handler`.
    pub fn start(port: u16, settings: &Path, handler: Handler) -> Result<Server, String> {
        trusted_folder(settings)?;
        let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, port)))
            .map_err(|e| format!("port {port} on the loopback interface could not be opened: {e}"))?;
        let port = listener.local_addr().map_err(|e| format!("the port could not be read: {e}"))?.port();
        let token = mint_token()?;
        let token_file = token_path(settings, port);
        write_token(&token_file, &token)?;
        let stop = Arc::new(AtomicBool::new(false));
        let (flag, open) = (stop.clone(), Arc::new(AtomicUsize::new(0)));
        let token = Arc::new(token);
        std::thread::Builder::new()
            .name("control-accept".into())
            .spawn(move || {
                let mut warned = false;
                for stream in listener.incoming() {
                    if flag.load(Ordering::SeqCst) {
                        break;
                    }
                    let stream = match stream {
                        Ok(s) => s,
                        Err(e) => {
                            // Out of file handles and the like: said once, then a pause, never a spin.
                            if !warned {
                                log::warn!("the control channel could not take a connection: {e}");
                                warned = true;
                            }
                            std::thread::sleep(Duration::from_millis(100));
                            continue;
                        }
                    };
                    // Past the cap a connection is closed at once.
                    if open.fetch_add(1, Ordering::SeqCst) >= MAX_CONNECTIONS {
                        open.fetch_sub(1, Ordering::SeqCst);
                        continue;
                    }
                    let (slots, token, handler) = (open.clone(), token.clone(), handler.clone());
                    let spawned = std::thread::Builder::new().name("control-connection".into()).spawn(move || {
                        serve_connection(stream, &token, &handler);
                        slots.fetch_sub(1, Ordering::SeqCst);
                    });
                    if spawned.is_err() {
                        open.fetch_sub(1, Ordering::SeqCst);
                        log::warn!("a control connection could not be served");
                    }
                }
            })
            .map_err(|e| format!("the control channel could not start: {e}"))?;
        log::info!("agent access is on: the control channel listens on 127.0.0.1:{port}");
        Ok(Server { port, token_file, stop })
    }

    pub fn port(&self) -> u16 {
        self.port
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        // Wakes the accepting thread so it sees the stop.
        let _ = TcpStream::connect_timeout(&SocketAddr::from((Ipv4Addr::LOCALHOST, self.port)), Duration::from_millis(200));
        let _ = std::fs::remove_file(&self.token_file);
    }
}

/// Makes sure no one else can write to the settings folder, where the token file could otherwise
/// be swapped or removed: a folder of ours that others may write to (made under a umask of 002,
/// say) loses those bits; a folder of someone else's, a shared temporary one for instance, refuses.
#[cfg(unix)]
fn trusted_folder(settings: &Path) -> Result<(), String> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let m = std::fs::symlink_metadata(settings).map_err(|e| format!("the settings folder {} cannot be read: {e}", settings.display()))?;
    if !m.file_type().is_dir() || m.uid() != euid() {
        return Err(format!("the settings folder {} is not a folder of yours", settings.display()));
    }
    let mode = m.mode() & 0o7777;
    if mode & 0o022 != 0 {
        std::fs::set_permissions(settings, std::fs::Permissions::from_mode(mode & !0o022)).map_err(|e| {
            format!("the settings folder {} (mode {mode:o}) can be written by others and could not be closed to them: {e}", settings.display())
        })?;
    }
    Ok(())
}

#[cfg(not(unix))]
fn trusted_folder(_settings: &Path) -> Result<(), String> {
    Err(NOT_HERE.to_string())
}

/// The keys a request may carry.
const REQUEST_KEYS: [&str; 5] = ["id", "method", "params", "token", "origin"];

/// Serves one connection until it closes, an unauthorised or malformed first line, or a failure.
fn serve_connection(stream: TcpStream, token: &str, handler: &Handler) {
    if stream.set_read_timeout(Some(TIMEOUT)).is_err() || stream.set_write_timeout(Some(TIMEOUT)).is_err() {
        return;
    }
    let (Ok(write), Ok(timer)) = (stream.try_clone(), stream.try_clone()) else { return };
    let mut writer = std::io::BufWriter::new(write);
    let mut reader = BufReader::new(stream);
    let mut first = true;
    loop {
        // A whole line has a deadline, however slowly its bytes arrive: the first a short one and
        // at most a request's size, so an unauthenticated peer cannot hold its slot.
        let deadline = Instant::now() + if first { FIRST_LINE } else { TIMEOUT };
        let keep_time = || {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "the line took too long"));
            }
            timer.set_read_timeout(Some(left))
        };
        let line = match read_line_with(&mut reader, if first { MAX_REQUEST } else { MAX_LINE }, keep_time) {
            Ok(Some(line)) => line,
            _ => return,
        };
        let request = serde_json::from_slice::<Value>(&line).ok().filter(Value::is_object);
        let well_formed = request.as_ref().is_some_and(|r| r.get("method").is_some_and(Value::is_string));
        if first {
            // The first line must be a request with the right token, else the connection closes.
            let authorised = request.as_ref().and_then(|r| r.get("token")).and_then(Value::as_str).is_some_and(|t| same_token(t, token));
            if !(well_formed && authorised) || line.len() > MAX_REQUEST {
                return;
            }
            first = false;
        }
        let reply = match request {
            Some(r) if well_formed => answer(&r, line.len(), handler),
            Some(r) => fail(r.get("id").cloned().unwrap_or(Value::Null), "a request needs a method"),
            None => fail(Value::Null, "the line is not a JSON object"),
        };
        let mut text = serde_json::to_vec(&reply).unwrap_or_default();
        if text.len() > MAX_REPLY {
            let why = if reply.get("ok") == Some(&Value::Bool(true)) {
                "the request was carried out, but its reply is larger than 16 MiB"
            } else {
                "the reply is larger than 16 MiB"
            };
            text = serde_json::to_vec(&fail(reply.get("id").cloned().unwrap_or(Value::Null), why)).unwrap_or_default();
        }
        text.push(b'\n');
        if writer.write_all(&text).and_then(|()| writer.flush()).is_err() {
            return;
        }
    }
}

fn answer(r: &Value, size: usize, handler: &Handler) -> Value {
    let id = r.get("id").cloned().unwrap_or(Value::Null);
    if size > MAX_REQUEST {
        return fail(id, &format!("the request is larger than {MAX_REQUEST} bytes"));
    }
    if let Some(key) = r.as_object().and_then(|o| o.keys().find(|k| !REQUEST_KEYS.contains(&k.as_str())).cloned()) {
        return fail(id, &format!("unknown key {key:?}; a request has {}", REQUEST_KEYS.join(", ")));
    }
    let method = r.get("method").and_then(Value::as_str).unwrap_or_default();
    let params = r.get("params").cloned().unwrap_or_else(|| json!({}));
    if !params.is_object() {
        return fail(id, "params must be an object");
    }
    let origin = match r.get("origin") {
        None => None,
        Some(Value::String(o)) => match check_origin(o) {
            Ok(()) => Some(o.as_str()),
            Err(e) => return fail(id, &e.to_string()),
        },
        Some(_) => return fail(id, "origin must be a string"),
    };
    // A handler that panics answers with an error; the connection and the application go on.
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| handler(method, params, origin))) {
        Ok(Ok(result)) => json!({ "id": id, "ok": true, "result": result }),
        Ok(Err(e)) => fail(id, &e),
        Err(_) => fail(id, "the request failed inside the application"),
    }
}

fn fail(id: Value, error: &str) -> Value {
    json!({ "id": id, "ok": false, "error": error })
}

/// Why a call through the channel did not get an answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallError {
    /// No application is listening, its token cannot be read, or it closed the connection: the
    /// command-line tool exits 3.
    Unreachable(String),
    /// The application answered with an error: the command-line tool exits 1.
    Refused(String),
}

impl std::fmt::Display for CallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CallError::Unreachable(e) | CallError::Refused(e) => f.write_str(e),
        }
    }
}

/// A client of the application listening on one port: each call opens a connection, sends the
/// token with its one request and reads the reply, and the client's origin when it has one.
pub struct Client {
    port: u16,
    token: String,
    origin: Option<String>,
}

impl Client {
    /// The client for the application on `port`, its token read from the settings folder.
    pub fn new(settings: &Path, port: u16) -> Result<Client, CallError> {
        let path = token_path(settings, port);
        let token = read_token(&path)
            .map_err(|e| CallError::Unreachable(format!("no Archforge with agent access on port {port} (start it with --control {port}): {e}")))?;
        Ok(Client { port, token, origin: None })
    }

    /// The client with a token in hand, for tests.
    pub fn with_token(port: u16, token: String) -> Client {
        Client { port, token, origin: None }
    }

    /// The client sending `origin` with every request, so the application labels the undo steps
    /// its requests make with it; a label [`check_origin`] refuses is refused here.
    pub fn with_origin(mut self, origin: Option<String>) -> Result<Client, OriginError> {
        if let Some(o) = &origin {
            check_origin(o)?;
        }
        self.origin = origin;
        Ok(self)
    }

    /// Sends `method` with `params` and gives the result.
    pub fn call(&self, method: &str, params: Value) -> Result<Value, CallError> {
        let unreachable = |e: std::io::Error| CallError::Unreachable(format!("Archforge on port {} could not be reached: {e}", self.port));
        let mut request = json!({ "id": 1, "method": method, "params": params, "token": self.token });
        if let Some(o) = &self.origin {
            request["origin"] = Value::String(o.clone());
        }
        let mut line = serde_json::to_vec(&request).map_err(|e| CallError::Refused(format!("the request could not be written: {e}")))?;
        if line.len() > MAX_REQUEST {
            return Err(CallError::Refused(format!("the request is larger than {} KiB; split it", MAX_REQUEST / 1024)));
        }
        line.push(b'\n');
        let stream = TcpStream::connect_timeout(&SocketAddr::from((Ipv4Addr::LOCALHOST, self.port)), Duration::from_secs(5)).map_err(unreachable)?;
        stream.set_read_timeout(Some(TIMEOUT + Duration::from_secs(10))).map_err(unreachable)?;
        stream.set_write_timeout(Some(TIMEOUT)).map_err(unreachable)?;
        let mut writer = stream.try_clone().map_err(unreachable)?;
        writer.write_all(&line).and_then(|()| writer.flush()).map_err(unreachable)?;
        let mut reader = BufReader::new(stream);
        let reply = match read_line(&mut reader, MAX_REPLY) {
            Ok(Some(reply)) => reply,
            Ok(None) => {
                return Err(CallError::Unreachable(format!(
                    "Archforge on port {} closed the connection: the token is not the current one, or the channel is busy",
                    self.port
                )));
            }
            Err(e) => return Err(unreachable(e)),
        };
        let reply: Value = serde_json::from_slice(&reply).map_err(|e| CallError::Refused(format!("the reply is not JSON: {e}")))?;
        match reply.get("ok").and_then(Value::as_bool) {
            Some(true) => Ok(reply.get("result").cloned().unwrap_or(Value::Null)),
            _ => Err(CallError::Refused(reply.get("error").and_then(Value::as_str).unwrap_or("the request was refused").to_string())),
        }
    }
}

#[cfg(test)]
mod tests;
