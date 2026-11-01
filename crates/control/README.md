# archforge-control

**Purpose.** The control channel an agent drives the running Archforge through
(agent-control-surfaces §10.1): the per-start token and its owner-only file, the loopback server
the application opens with `--control <port>`, and the client `archforge-cli` reaches it with.
One JSON object per line: `{id, method, params}`, the first on a connection also carrying `token`;
replies `{id, ok, result | error}`. A first line that is not a well-formed request with the right
token closes the connection without a word.

**Layer.** 9, a host service beside `dolt`: network and files, never built for the portable target.

**Public API.**

| Item | Signature |
|---|---|
| `settings_folder` | `pub fn settings_folder() -> PathBuf` (the folder the application and the CLI share) |
| `token_path` | `pub fn token_path(settings: &Path, port: u16) -> PathBuf` (`control-<port>.token`) |
| `mint_token`, `same_token` | 256 bits from the OS as 64 hex characters; a comparison in constant time |
| `write_token`, `read_token` | exclusive creation with mode 0600; trusted only as a regular file of the effective user with no group or other bits, re-checked on the open handle (CS-8); Unix only, elsewhere `NOT_HERE` |
| `read_line` | `pub fn read_line(r: &mut impl BufRead, max: usize) -> io::Result<Option<Vec<u8>>>` |
| `Server::start` | `pub fn start(port: u16, settings: &Path, handler: Handler) -> Result<Server, String>`; the settings folder must be yours (others' write bits are removed); a first line has 5 s and 1 MiB, later lines 30 s each; dropping it stops listening and removes the token file |
| `Handler` | `Arc<dyn Fn(&str, Value) -> Result<Value, String> + Send + Sync>` |
| `Client::new`, `Client::call` | the client of the application on a port; `CallError::{Unreachable, Refused}` |
| `MAX_LINE`, `MAX_REQUEST`, `MAX_REPLY`, `MAX_CONNECTIONS`, `TIMEOUT`, `TOKEN_LEN` | 4 MiB, 1 MiB, 16 MiB, 16, 30 s, 64 |

**Non-goals.** Answering requests: the shell does (`archforge_ui_egui::control`). Windows: its
protected token file is archforge-vge.1.2.
