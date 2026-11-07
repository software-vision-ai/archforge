//! The control channel an agent drives the running application through (agent-control-surfaces
//! §10.1): a loopback port the application opens when started with `--control <port>` (the
//! person's own access, [`Server::start`], one full-access token in `control-<port>.token`), or on
//! a free port while a Chat conversation needs agent access ([`Server::open`], one token per
//! conversation from [`Server::grant`], each in a new file of its own and with its own [`Role`]).
//! Tokens are minted from the system's random source and kept in files only their owner can read;
//! one JSON object per line. The application answers through its command engine, so the window shows every change and
//! undo covers it. `archforge-cli` reaches the port through [`Client`].
//!
//! The wire: a request is `{id, method, params}`, the first on a connection also carrying `token`,
//! and any request may carry `origin`, the label of who sent it (a chat, say), which the
//! application puts on the undo steps the request makes; a reply is `{id, ok: true, result}` or
//! `{id, ok: false, error}`. The origin is a key of the request, never of its parameters, so the
//! one who chooses the parameters (an agent through `call_app`) cannot choose it. A first line that is not a
//! well-formed request with the right token closes the connection without a word, so a web page
//! cannot drive the port (CS-6). Lines, requests, replies, connections and waits are capped (CS-7).
//!
//! Every request reaches the handler with the role and the liveness of the token its connection
//! showed ([`Call`]). Revoking a token (dropping its [`Grant`]) answers its held connections
//! [`REVOKED`] and closes them, and its queued requests see their liveness go false; closing a
//! server (dropping it) revokes every token it granted, so a connection authenticated before the
//! close is never served after it.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unreachable, clippy::todo, clippy::unimplemented)]

#[cfg(unix)]
use std::io::Read;
use std::io::{BufRead, BufReader, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
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

/// The command-line tool's file name on this system.
pub const CLI_FILE: &str = if cfg!(windows) { "archforge-cli.exe" } else { "archforge-cli" };

/// Why the command-line tool the application starts for an agent was not found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CliError {
    /// The running executable's own path is unknown, so nothing lies beside it.
    NoExecutable(String),
    /// No `archforge-cli` at any of these places, in the order they were looked at.
    NotFound { looked: Vec<PathBuf> },
}

impl std::fmt::Display for CliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CliError::NoExecutable(e) => write!(f, "the application's own path is unknown, so {CLI_FILE} cannot be found beside it: {e}"),
            CliError::NotFound { looked } => {
                let places: Vec<String> = looked.iter().map(|p| p.display().to_string()).collect();
                write!(f, "{CLI_FILE} is not installed with this application; looked at {}", places.join(", "))
            }
        }
    }
}

/// The command-line tool agents run (`archforge-cli mcp --bridge`), which the application is to
/// start for an agent from its in-app chat on: the one the macOS and Linux packages place beside
/// the application's executable (`Contents/MacOS/` in the macOS bundle, `bin/` on Linux). A debug build also looks in the workspace's cargo target folder, where
/// `cargo build -p archforge-cli` puts it while developing; a release build never does.
pub fn cli_path() -> Result<PathBuf, CliError> {
    let exe = std::env::current_exe().map_err(|e| CliError::NoExecutable(e.to_string()))?;
    find_cli(&exe, &dev_cli_dirs(cfg!(debug_assertions)))
}

/// The development folders a debug build also looks in: `debug` under `CARGO_TARGET_DIR` when it
/// is set, then under the workspace's `target`. None for a release build.
fn dev_cli_dirs(debug: bool) -> Vec<PathBuf> {
    if !debug {
        return Vec::new();
    }
    let workspace_target = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..").join("target");
    let env_target = std::env::var_os("CARGO_TARGET_DIR").filter(|v| !v.is_empty()).map(PathBuf::from);
    env_target.into_iter().chain([workspace_target]).map(|t| t.join("debug")).collect()
}

/// The first `archforge-cli` that is a file: beside the file `exe` is, following links, so a link
/// to the application elsewhere never picks up another install's tool lying beside the link; then
/// beside `exe` as given; then in each of `dev`.
fn find_cli(exe: &Path, dev: &[PathBuf]) -> Result<PathBuf, CliError> {
    let mut folders: Vec<PathBuf> = std::fs::canonicalize(exe).ok().and_then(|r| r.parent().map(Path::to_path_buf)).into_iter().collect();
    if let Some(given) = exe.parent().map(Path::to_path_buf)
        && !folders.contains(&given)
    {
        folders.push(given);
    }
    folders.extend(dev.iter().cloned());
    let looked: Vec<PathBuf> = folders.iter().map(|d| d.join(CLI_FILE)).collect();
    match looked.iter().find(|p| p.is_file()) {
        Some(found) => Ok(found.clone()),
        None => Err(CliError::NotFound { looked }),
    }
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

/// Writes `token` to `path`, created exclusively and readable by its owner alone, and gives the
/// file's identity, by which it is removed later. A file left there by an earlier start of ours (a
/// regular file we own) is replaced; anything else refuses.
#[cfg(unix)]
pub fn write_token(path: &Path, token: &str) -> Result<FileId, String> {
    use std::os::unix::fs::MetadataExt;
    if let Ok(m) = std::fs::symlink_metadata(path) {
        if !m.file_type().is_file() || m.uid() != euid() {
            return Err(format!("{} is there and is not a token file of ours", path.display()));
        }
        std::fs::remove_file(path).map_err(|e| format!("the old token file {} could not be removed: {e}", path.display()))?;
    }
    match create_token_file(path, token) {
        Ok(file) => Ok(file),
        Err(Created::Taken) => Err(format!("{} appeared while the token file was written", path.display())),
        Err(Created::Failed(e)) => Err(e),
    }
}

/// Refused on this platform: agent access is Unix only for now.
#[cfg(not(unix))]
pub fn write_token(_path: &Path, _token: &str) -> Result<FileId, String> {
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

/// What a token may do on the channel. The person's own access (`--control`) is [`Role::Full`];
/// a Chat conversation's token is [`Role::ReadOnly`] unless that conversation allows changes. The
/// server hands the role to the handler with every request, and the handler refuses what the role
/// does not allow (the application classifies each method and command once).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// Reads only: no change to the model or the project, no undo or redo, no save.
    ReadOnly,
    /// Everything the channel offers.
    Full,
}

/// One request as the handler gets it: the method, its parameters, its origin (checked by
/// [`check_origin`]), the role of the token the connection showed, and that token's liveness,
/// true until the token is revoked or its server closes. A request queued for later must check
/// `live` before it runs: a revoked token's queued request never runs.
pub struct Call<'a> {
    pub method: &'a str,
    pub params: Value,
    pub origin: Option<&'a str>,
    pub role: Role,
    pub live: &'a Arc<AtomicBool>,
}

/// What answers a request: a [`Call`] in, the result or why not out. It may block until the
/// application has answered, at most [`TIMEOUT`].
pub type Handler = Arc<dyn Fn(Call<'_>) -> Result<Value, String> + Send + Sync>;

/// The error a revoked token's requests get, and the last line a connection holding it receives.
pub const REVOKED: &str = "revoked: the access this token gave has ended";

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

/// A token file's identity, its device and inode as created: the file is removed only while the
/// name still holds this very file, so a file someone else put there is never removed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileId {
    pub dev: u64,
    pub ino: u64,
}

/// A token granted on a server: its id, role, liveness and the file that holds it.
struct Granted {
    id: u64,
    token: String,
    role: Role,
    live: Arc<AtomicBool>,
    file: Option<(PathBuf, FileId)>,
}

/// A connection the server serves: the grant it authenticated with (0 before it has), a handle to
/// shut its socket, and its thread.
struct Held {
    bound: Arc<AtomicU64>,
    /// `None` once the connection has ended.
    stream: Option<TcpStream>,
    thread: Option<std::thread::JoinHandle<()>>,
}

/// What a server's threads and its grants share. Each opening has its own: a connection
/// authenticated on one is never served by another.
struct Shared {
    /// True while the server is open; its gate for every request.
    open: AtomicBool,
    grants: Mutex<Vec<Granted>>,
    held: Mutex<Vec<Held>>,
    next_grant: AtomicU64,
}

/// A lock that a panicking thread poisoned is taken over: the lists stay usable for the close.
fn locked<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl Shared {
    /// Revokes the grant `id`: its liveness goes false, its connections are shut for reading (each
    /// answers [`REVOKED`] and closes), and its file is removed by its identity.
    fn revoke(&self, id: u64) {
        let gone = {
            let mut grants = locked(&self.grants);
            let at = grants.iter().position(|g| g.id == id);
            at.map(|i| grants.remove(i))
        };
        let Some(gone) = gone else { return };
        gone.live.store(false, Ordering::SeqCst);
        for h in locked(&self.held).iter().filter(|h| h.bound.load(Ordering::SeqCst) == id) {
            if let Some(s) = &h.stream {
                let _ = s.shutdown(std::net::Shutdown::Read);
            }
        }
        if let Some((path, file)) = &gone.file {
            remove_token_file(path, *file);
        }
    }
}

/// A listening control channel and its threads. Dropping it closes it: no request runs after,
/// every token it granted is revoked and its file removed, held connections are answered
/// [`REVOKED`] and closed, and its threads are joined.
pub struct Server {
    port: u16,
    shared: Arc<Shared>,
    accept: Option<std::thread::JoinHandle<()>>,
    settings: PathBuf,
}

impl std::fmt::Debug for Server {
    /// The port and how many tokens are granted; never a token.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Server").field("port", &self.port).field("grants", &self.grants()).finish()
    }
}

impl Server {
    /// The person's own access, for `--control <port>`: listens on `port` of the loopback
    /// interface (0 for any free port), writes a fresh full-access token to `control-<port>.token`
    /// in the settings folder and answers each connection's requests through `handler`.
    pub fn start(port: u16, settings: &Path, handler: Handler) -> Result<Server, String> {
        let server = Server::listen(port, settings, handler)?;
        let token = mint_token()?;
        let path = token_path(settings, server.port);
        let file = write_token(&path, &token)?;
        server.add(token, Role::Full, Some((path, file)));
        log::info!("agent access is on: the control channel listens on 127.0.0.1:{}", server.port);
        Ok(server)
    }

    /// An opening with no token yet, on a free port of the loopback interface: Chat's agent access,
    /// opened while a conversation needs it. Each conversation gets its own token with
    /// [`Server::grant`].
    pub fn open(settings: &Path, handler: Handler) -> Result<Server, String> {
        let server = Server::listen(0, settings, handler)?;
        log::info!("agent access opened for Chat on 127.0.0.1:{}", server.port);
        Ok(server)
    }

    /// Grants a new token with `role`, written to a new file of its own,
    /// `control-<port>-<16 hex>.token` in the settings folder, created exclusively and readable by
    /// its owner alone; a name already taken is never replaced, another is tried. Dropping the
    /// [`Grant`] revokes it.
    pub fn grant(&self, role: Role) -> Result<Grant, String> {
        if !self.shared.open.load(Ordering::SeqCst) {
            return Err("the control channel is closed".into());
        }
        let token = mint_token()?;
        let mut last = String::new();
        for _ in 0..16 {
            let path = self.settings.join(format!("control-{}-{}.token", self.port, name_suffix()?));
            match create_token_file(&path, &token) {
                Ok(file) => {
                    let (id, live) = self.add(token, role, Some((path.clone(), file)));
                    return Ok(Grant { id, path, port: self.port, live, shared: self.shared.clone() });
                }
                Err(Created::Taken) => last = format!("{} is taken", path.display()),
                Err(Created::Failed(e)) => return Err(e),
            }
        }
        Err(format!("no free token file name was found: {last}"))
    }

    /// How many tokens are granted and not revoked.
    pub fn grants(&self) -> usize {
        locked(&self.shared.grants).len()
    }

    /// The port it listens on.
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Records a token and gives its grant's id and liveness.
    fn add(&self, token: String, role: Role, file: Option<(PathBuf, FileId)>) -> (u64, Arc<AtomicBool>) {
        let id = self.shared.next_grant.fetch_add(1, Ordering::SeqCst).saturating_add(1);
        let live = Arc::new(AtomicBool::new(true));
        locked(&self.shared.grants).push(Granted { id, token, role, live: live.clone(), file });
        (id, live)
    }

    /// Binds the loopback port and starts the accepting thread.
    fn listen(port: u16, settings: &Path, handler: Handler) -> Result<Server, String> {
        trusted_folder(settings)?;
        let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, port)))
            .map_err(|e| format!("port {port} on the loopback interface could not be opened: {e}"))?;
        let port = listener.local_addr().map_err(|e| format!("the port could not be read: {e}"))?.port();
        let shared = Arc::new(Shared {
            open: AtomicBool::new(true),
            grants: Mutex::new(Vec::new()),
            held: Mutex::new(Vec::new()),
            next_grant: AtomicU64::new(0),
        });
        let accepting = shared.clone();
        let accept = std::thread::Builder::new()
            .name("control-accept".into())
            .spawn(move || accept_loop(&listener, &accepting, &handler))
            .map_err(|e| format!("the control channel could not start: {e}"))?;
        Ok(Server { port, shared, accept: Some(accept), settings: settings.to_path_buf() })
    }
}

impl Drop for Server {
    /// Closes the opening: the gate shuts, every grant is revoked (files removed by identity), the
    /// accepting thread is woken and joined, held connections are shut for reading so each answers
    /// [`REVOKED`] and ends, and their threads are joined; one that has not ended within a second
    /// has its socket shut both ways first, so every wait is bounded.
    fn drop(&mut self) {
        self.shared.open.store(false, Ordering::SeqCst);
        let ids: Vec<u64> = locked(&self.shared.grants).iter().map(|g| g.id).collect();
        for id in ids {
            self.shared.revoke(id);
        }
        if let Some(accept) = self.accept.take() {
            // Wakes the accepting thread so it sees the gate shut; without a wake it is left, never waited on.
            let woken =
                (0..3).any(|_| TcpStream::connect_timeout(&SocketAddr::from((Ipv4Addr::LOCALHOST, self.port)), Duration::from_millis(200)).is_ok());
            if woken {
                let _ = accept.join();
            } else {
                log::warn!("the control channel's accepting thread on port {} could not be woken; it ends with the application", self.port);
            }
        }
        let mut held = std::mem::take(&mut *locked(&self.shared.held));
        for s in held.iter().filter_map(|h| h.stream.as_ref()) {
            let _ = s.shutdown(std::net::Shutdown::Read);
        }
        let deadline = Instant::now() + Duration::from_secs(1);
        while held.iter().any(|h| h.thread.as_ref().is_some_and(|t| !t.is_finished())) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        for h in &mut held {
            if h.thread.as_ref().is_some_and(|t| !t.is_finished())
                && let Some(s) = &h.stream
            {
                let _ = s.shutdown(std::net::Shutdown::Both);
            }
            if let Some(t) = h.thread.take() {
                let _ = t.join();
            }
        }
        log::info!("agent access closed on 127.0.0.1:{}", self.port);
    }
}

/// A token a server granted. Dropping it revokes it: its connections are answered [`REVOKED`] and
/// closed, its queued requests see their liveness go false, and its file is removed.
pub struct Grant {
    id: u64,
    path: PathBuf,
    port: u16,
    live: Arc<AtomicBool>,
    shared: Arc<Shared>,
}

impl std::fmt::Debug for Grant {
    /// The port and the file; never the token.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Grant").field("port", &self.port).field("path", &self.path).finish()
    }
}

impl Grant {
    /// The absolute path of the file that holds the token (`archforge-cli --token-file`).
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The port of the server that granted it.
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Whether the token still opens anything: false once its server closed.
    pub fn live(&self) -> bool {
        self.live.load(Ordering::SeqCst) && self.shared.open.load(Ordering::SeqCst)
    }
}

impl Drop for Grant {
    /// Revokes the token.
    fn drop(&mut self) {
        self.shared.revoke(self.id);
    }
}

/// Accepts connections until the gate shuts: each within the cap gets a thread of its own,
/// registered so a revocation or the close can reach it; finished ones are joined as it goes.
fn accept_loop(listener: &TcpListener, shared: &Arc<Shared>, handler: &Handler) {
    let mut warned = false;
    for stream in listener.incoming() {
        if !shared.open.load(Ordering::SeqCst) {
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
        let mut held = locked(&shared.held);
        // Finished connections are joined and forgotten, so the list holds only live ones.
        held.retain_mut(|h| {
            let done = h.thread.as_ref().is_none_or(std::thread::JoinHandle::is_finished);
            if done && let Some(t) = h.thread.take() {
                let _ = t.join();
            }
            !done
        });
        // Past the cap a connection is closed at once.
        if held.len() >= MAX_CONNECTIONS {
            continue;
        }
        let Ok(handle) = stream.try_clone() else { continue };
        let bound = Arc::new(AtomicU64::new(0));
        let (serving, by, answer) = (shared.clone(), bound.clone(), handler.clone());
        match std::thread::Builder::new().name("control-connection".into()).spawn(move || serve_connection(stream, &serving, &by, &answer)) {
            Ok(thread) => held.push(Held { bound, stream: Some(handle), thread: Some(thread) }),
            Err(_) => log::warn!("a control connection could not be served"),
        }
    }
}

/// The 16 hexadecimal characters that make a granted token's file name unpredictable.
fn name_suffix() -> Result<String, String> {
    let mut bytes = [0u8; 8];
    getrandom::fill(&mut bytes).map_err(|e| format!("the system's random source failed: {e}"))?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

/// Why a new token file was not made.
enum Created {
    /// The name is taken by something else, which is left alone.
    Taken,
    Failed(String),
}

/// Creates `path` exclusively (never through a link, never over anything), readable by its owner
/// alone, writes `token` and gives the file's identity from the open handle.
#[cfg(unix)]
fn create_token_file(path: &Path, token: &str) -> Result<FileId, Created> {
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
    let mut f = match std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => return Err(Created::Taken),
        Err(e) => return Err(Created::Failed(format!("the token file {} could not be created: {e}", path.display()))),
    };
    let written = f.write_all(token.as_bytes()).and_then(|()| f.write_all(b"\n")).and_then(|()| f.sync_all()).and_then(|()| f.metadata());
    match written {
        Ok(m) => Ok(FileId { dev: m.dev(), ino: m.ino() }),
        Err(e) => {
            // Made by this call, so it is ours to remove.
            if let Ok(m) = f.metadata() {
                remove_token_file(path, FileId { dev: m.dev(), ino: m.ino() });
            }
            Err(Created::Failed(format!("the token file {} could not be written: {e}", path.display())))
        }
    }
}

/// Refused on this platform: agent access is Unix only for now.
#[cfg(not(unix))]
fn create_token_file(_path: &Path, _token: &str) -> Result<FileId, Created> {
    Err(Created::Failed(NOT_HERE.to_string()))
}

/// Removes the token file at `path` only while that name still holds the file `id` names; anything
/// else there (another start's file, a link, a folder) is left and the fact logged.
#[cfg(unix)]
pub fn remove_token_file(path: &Path, id: FileId) {
    use std::os::unix::fs::MetadataExt;
    match std::fs::symlink_metadata(path) {
        Ok(m) if m.file_type().is_file() && m.dev() == id.dev && m.ino() == id.ino => {
            if let Err(e) = std::fs::remove_file(path) {
                log::warn!("the token file {} could not be removed: {e}", path.display());
            }
        }
        Ok(_) => log::warn!("{} is no longer the token file written there, so it is left alone", path.display()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => log::warn!("the token file {} could not be checked: {e}", path.display()),
    }
}

/// Nothing to remove on this platform: no token file is made.
#[cfg(not(unix))]
pub fn remove_token_file(_path: &Path, _id: FileId) {}

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

/// The grant whose token `shown` is, compared with every grant in constant time each, and bound to
/// the connection under the grants' lock, so a revocation either sees the binding or the token is
/// already gone.
fn authorise(shared: &Shared, shown: &str, bound: &AtomicU64) -> Option<(Role, Arc<AtomicBool>)> {
    let grants = locked(&shared.grants);
    if !shared.open.load(Ordering::SeqCst) {
        return None;
    }
    let mut found: Option<&Granted> = None;
    for g in grants.iter() {
        if same_token(shown, &g.token) {
            found = Some(g);
        }
    }
    let g = found?;
    bound.store(g.id, Ordering::SeqCst);
    Some((g.role, g.live.clone()))
}

/// Serves one connection until it closes, an unauthorised or malformed first line, a failure, or
/// its token's revocation, which is answered [`REVOKED`] before the connection closes. At the end
/// the socket is shut both ways and the server's own handle to it (kept for revocations) is
/// dropped, so the connection is closed at once.
fn serve_connection(stream: TcpStream, shared: &Shared, bound: &Arc<AtomicU64>, handler: &Handler) {
    let closer = stream.try_clone();
    serve(stream, shared, bound, handler);
    if let Ok(c) = closer {
        let _ = c.shutdown(std::net::Shutdown::Both);
    }
    for h in locked(&shared.held).iter_mut().filter(|h| Arc::ptr_eq(&h.bound, bound)) {
        h.stream = None;
    }
}

/// [`serve_connection`]'s loop.
fn serve(stream: TcpStream, shared: &Shared, bound: &AtomicU64, handler: &Handler) {
    if stream.set_read_timeout(Some(TIMEOUT)).is_err() || stream.set_write_timeout(Some(TIMEOUT)).is_err() {
        return;
    }
    let (Ok(write), Ok(timer)) = (stream.try_clone(), stream.try_clone()) else { return };
    let mut writer = std::io::BufWriter::new(write);
    let mut reader = BufReader::new(stream);
    let mut access: Option<(Role, Arc<AtomicBool>)> = None;
    loop {
        let first = access.is_none();
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
            _ => {
                // Shut by a revocation or the close: the peer is told before the connection ends.
                if let Some((_, live)) = &access
                    && !(live.load(Ordering::SeqCst) && shared.open.load(Ordering::SeqCst))
                {
                    let _ = timer.set_write_timeout(Some(Duration::from_secs(1)));
                    let _ = send(&mut writer, &fail(Value::Null, REVOKED));
                }
                return;
            }
        };
        let request = serde_json::from_slice::<Value>(&line).ok().filter(Value::is_object);
        let well_formed = request.as_ref().is_some_and(|r| r.get("method").is_some_and(Value::is_string));
        if first {
            // The first line must be a request with a granted token, else the connection closes.
            let shown = request.as_ref().and_then(|r| r.get("token")).and_then(Value::as_str);
            if !well_formed || line.len() > MAX_REQUEST {
                return;
            }
            match shown.and_then(|t| authorise(shared, t, bound)) {
                Some(granted) => access = Some(granted),
                None => return,
            }
        }
        let Some((role, live)) = access.clone() else { return };
        let id = request.as_ref().and_then(|r| r.get("id")).cloned().unwrap_or(Value::Null);
        if !(live.load(Ordering::SeqCst) && shared.open.load(Ordering::SeqCst)) {
            let _ = send(&mut writer, &fail(id, REVOKED));
            return;
        }
        let reply = match request {
            Some(r) if well_formed => answer(&r, line.len(), role, &live, handler),
            Some(_) => fail(id, "a request needs a method"),
            None => fail(Value::Null, "the line is not a JSON object"),
        };
        if send(&mut writer, &reply).is_err() {
            return;
        }
    }
}

/// Writes one reply line, bounded: a reply past [`MAX_REPLY`] is replaced by an error that says so.
fn send(writer: &mut impl Write, reply: &Value) -> std::io::Result<()> {
    let mut text = serde_json::to_vec(reply).unwrap_or_default();
    if text.len() > MAX_REPLY {
        let why = if reply.get("ok") == Some(&Value::Bool(true)) {
            "the request was carried out, but its reply is larger than 16 MiB"
        } else {
            "the reply is larger than 16 MiB"
        };
        text = serde_json::to_vec(&fail(reply.get("id").cloned().unwrap_or(Value::Null), why)).unwrap_or_default();
    }
    text.push(b'\n');
    writer.write_all(&text).and_then(|()| writer.flush())
}

/// Answers one well-formed request through `handler`, with the role and liveness of the token the
/// connection showed; a request past the size cap, with an unknown key, params that are not an
/// object or a bad origin is refused first.
fn answer(r: &Value, size: usize, role: Role, live: &Arc<AtomicBool>, handler: &Handler) -> Value {
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
    let call = Call { method, params, origin, role, live };
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| handler(call))) {
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

    /// The client for the application on `port`, its token read from `path`, a file one Chat
    /// conversation was granted (`archforge-cli --token-file`), trusted only as [`read_token`] says.
    pub fn from_token_file(path: &Path, port: u16) -> Result<Client, CallError> {
        let token =
            read_token(path).map_err(|e| CallError::Unreachable(format!("no Archforge agent access on port {port} with that token file: {e}")))?;
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
