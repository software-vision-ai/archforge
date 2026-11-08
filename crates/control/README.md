# archforge-control

**Purpose.** The control channel an agent drives the running Archforge through
(agent-control-surfaces §10.1): the per-start token and its owner-only file, the loopback server
the application opens with `--control <port>`, and the client `archforge-cli` reaches it with.
One JSON object per line: `{id, method, params}`, the first on a connection also carrying `token`,
any one optionally `origin` (who sent it, checked by `check_origin`; the application labels the undo
steps the request makes with it); replies `{id, ok, result | error}`. The origin is a key of the
request, not of its parameters, so whoever chooses the parameters cannot choose it. A first line that is not a well-formed request with the right
token closes the connection without a word.

**Layer.** 9, a host service beside `dolt`: network and files, never built for the portable target.

**Public API.**

| Item | Signature |
|---|---|
| `settings_folder` | `pub fn settings_folder() -> PathBuf` (the folder the application and the CLI share) |
| `token_path` | `pub fn token_path(settings: &Path, port: u16) -> PathBuf` (`control-<port>.token`, the person's own access) |
| `mint_token`, `same_token` | 256 bits from the OS as 64 hex characters; a comparison in constant time |
| `write_token`, `read_token` | exclusive creation with mode 0600, giving the file's `FileId`; trusted only as a regular file of the effective user with no group or other bits, re-checked on the open handle (CS-8); Unix only, elsewhere `NOT_HERE` |
| `FileId`, `remove_token_file` | a token file's device and inode; the file is removed only while its name still holds that very file, anything else there is left and logged |
| `read_line` | `pub fn read_line(r: &mut impl BufRead, max: usize) -> io::Result<Option<Vec<u8>>>` |
| `Server::start` | `pub fn start(port: u16, settings: &Path, handler: Handler) -> Result<Server, String>`: the person's own access (`--control`), one `Role::Full` token in `control-<port>.token` (an earlier start's file of ours under that fixed name is replaced, as before; a granted file never is); the settings folder must be yours (others' write bits are removed); a first line has 5 s and 1 MiB, later lines 30 s each |
| `Server::open`, `Server::grant`, `Server::grants`, `Grant`, `Bound` | Chat's agent access: an opening on a free loopback port with no token; `grant(Role, Option<Bound>)` mints a token into a new file `control-<port>-<16 hex>.token`, created exclusively (a taken name is never replaced; another is tried, up to 16), and gives a `Grant` (`path()`, `port()`, `live()`) whose drop revokes it; a `Bound { key, origin }` grant speaks only for that conversation: a request on it naming no origin takes the bound one, one naming another is refused (AR-230) |
| `Role`, `Call`, `Handler`, `REVOKED` | `ReadOnly` or `Full`; a request as the handler gets it: method, params, origin (a bound token's own), `conversation` (a bound token's key, never the request's), the role of the token the connection showed and its liveness (`live`, false once revoked or closed); `Handler` is `Arc<dyn Fn(Call<'_>) -> Result<Value, String> + Send + Sync>`; the server only carries the role, the application refuses what it does not allow |
| `Client::new`, `Client::from_token_file`, `Client::with_origin`, `Client::call` | the client of the application on a port, its token from `control-<port>.token` or from a granted file (`archforge-cli --token-file`), sending an origin with every request when given one; `CallError::{Unreachable, Refused}` |
| `check_origin`, `OriginError`, `MAX_ORIGIN` | an origin is 1 to 64 characters of printable text with no space at either end; refused as `Empty`, `TooLong`, `Padded` or `NotPrintable` |
| `MAX_LINE`, `MAX_REQUEST`, `MAX_REPLY`, `MAX_CONNECTIONS`, `TIMEOUT`, `TOKEN_LEN` | 4 MiB, 1 MiB, 16 MiB, 16, 30 s, 64 |

**Revocation and the close.** Dropping a `Grant` revokes its token: its liveness goes false, so a
request of it still queued in the window never runs; its held connections are shut for reading, so
each answers `{"id": null, "ok": false, "error": REVOKED}` (or the request it was reading, with its
id) and closes; and its file is removed by identity. Dropping a `Server` closes its opening: the
gate shuts, every grant is revoked, the accepting thread is woken and joined, and every connection's
thread is joined, a connection that has not ended within a second being shut both ways first. Each
opening has its own gate and grants, so a connection authenticated on one is never served by
another or after its close.

**Limits.** A token file is removed by name after its device and inode are checked, so a file
swapped in between the check and the removal would go; only this user can do that, as the
settings folder is closed to others' writes. The close wakes the accepting thread with a loopback
connection; if three tries fail (no file handle left), the thread is left, logged, and the port
stays bound until the next connection or the end of the process.

**Non-goals.** Answering requests: the shell does (`archforge_ui_egui::control`). Windows: its
protected token file comes later.
