# archforge-agents

**Purpose.** The agent runtimes a person installs beside Archforge, Claude Code (`claude`) and OpenAI
Codex (`codex`): found on this computer, checked against the versions Archforge accepts, and run as
supervised child processes in private folders. Archforge never bundles them. The adapters that
speak each runtime's protocol (`claude`, AR-201; `codex`, AR-202) are part of it; the AR-196 spike's
note records how the runtimes behave.

**Layer.** 10, a host service above `secrets`, whose `Secret` carries the keys, and `control`, whose
origin label check the Claude adapter shares with `archforge-cli`: it starts processes and makes
folders, so the shell never depends on it and it is never built for the portable target.

**Public API.**

| Item | Signature |
|---|---|
| `RuntimeKind` | `Claude`, `Codex`; `program()`, `override_var()` (`ARCHFORGE_CLAUDE`, `ARCHFORGE_CODEX`), `config_var()` (`CLAUDE_CONFIG_DIR`, `CODEX_HOME`), `range()` |
| `HostEnv` | `from_process()`: the PATH, the two overrides, the installers' folders, `LANG`/`LC_*`; or given whole (the tests do) |
| `find` | `pub fn find(RuntimeKind, &HostEnv, &Workspace) -> Result<Discovery, AgentError>`; `Err` only when the workspace cannot hold a probe folder; blocks up to 5 s a candidate, so it runs on a worker |
| `Discovery` | `Found(Runtime)`, `TooOld { kind, path, version, floor }`, `Unsupported { kind, path, reason: Unsupported }`, `Missing { kind, looked, override_set }`, `NotAvailable { kind }`; `runtime()`, `into_result() -> Result<Runtime, AgentError>` |
| `Runtime` | `kind()`, `path()` (links resolved), `found_at()`, `version()`, `tested()` (false: newer than tested, "not yet tested with this version"), `child_path()`, `locale()` |
| `Version`, `VersionRange`, `Fit` | strict `X.Y.Z`; `floor`, `below`, `tested`; `Tested`, `NewerThanTested`, `TooOld`, `TooNew` |
| `Workspace` | `open(&Path) -> Result<Workspace, _>` (absolute; made with 0700, an existing one must be private and is never changed), `conversation(&ConversationId) -> Result<ConversationFolder, _>` (makes or checks), `remove_conversation(&ConversationId) -> Result<bool, _>`, `account(RuntimeKind) -> Result<ConversationFolder, _>` (`account-<program>`, shaped like a conversation's folder; its `config()` is the settings folder every conversation of that runtime shares) |
| `ConversationId`, `ConversationFolder` | 1 to 64 of `[a-z0-9-]`; `path()`, `home()`, `config()`, `work()`, `tmp()` |
| `spawn` | `pub fn spawn(&Runtime, &ConversationFolder, SpawnRequest) -> Result<RuntimeProcess, AgentError>` |
| `SpawnRequest`, `Limits` | `args`, `vars`, `secret_vars: Vec<(String, Secret)>`, `scrub: Vec<Secret>` (secrets sent another way, over the input: scrubbed and refused on the command line, never in the environment), `config_home: Option<PathBuf>` (a private folder the settings variable names instead of the conversation's), `limits`; `max_line_bytes` (8 MiB), `stderr_tail_bytes` (16 KiB), `queued_lines` (16), each refused at zero or past its maximum, and `max_line_bytes × (queued_lines + 1)` refused past `MAX_BUFFERED_BYTES` (256 MiB) |
| `RuntimeProcess` | `send_line` (never waits), `close_input`, `next_event(timeout)`, `try_next_event`, `interrupt`, `status`, `shutdown(grace)` (waits up to twice `grace`, so on a worker), `stderr_tail`, `exit_error(Exit)`, `pid`; kills its group on drop |
| `OutputEvent`, `Dropped`, `Exit` | `Line`, `Dropped(TooLong \| NotText { bytes })`, `ReadFailed`, `End`; `code`, `signal` |
| `AgentError`, `ErrorKind` | every failure; `kind()` gives `MissingRuntime`, `InvalidCredential`, `SignInExpired`, `RateLimited`, `Network`, `ModelUnavailable` or `Other`; `AgentError::reported(kind, detail)` for the adapters |
| `RuntimeProcess::redact` | `fn redact(&self, String) -> String`: the runtime's secrets replaced, for what an adapter shows from standard output |
| `AgentSession` | the neutral trait every adapter implements: `send(&mut self, &str)`, `stop(&mut self)`, `next_event(&mut self, Duration) -> Option<SessionEvent>`, `shutdown(&mut self, Duration) -> Result<Exit, _>` |
| `SessionEvent` | `Started { session_id, model, tools, servers }`, `TextDelta`, `MessageDone`, `ToolStarted { id, server, tool, input }`, `ToolFinished { id, tool, ok, summary }`, `Retrying { attempt, max_attempts, delay_ms, kind }`, `TurnFinished { outcome, usage }`, `Ignored(Skipped)`, `Ended { exit, error }` |
| `TurnOutcome`, `Usage`, `ToolServer`, `Skipped` | `Completed`, `Stopped`, `Failed(AgentError)`; token counts without a cost; a tool server's name and state; why a line was skipped |
| `claude::ClaudeSession` | `start(&Runtime, &ConversationFolder, ClaudeConfig, Secret) -> Result<ClaudeSession, _>`, `AgentSession`, `session_id`, `skipped`, `turn_running`, `process` |
| `claude::ClaudeConfig` | `session: SessionId` (the conversation's UUID), `start: Start` (`New`, `Resume`), `model: Model`, `bridge: Bridge { cli, port, origin, allow_changes, home: BridgeHome }`, `limits` |
| `claude::command_line`, `mcp_config`, `write_mcp_config` | the arguments; the tool server configuration as JSON; that configuration written to `claude-mcp.json` in the conversation's folder (atomic, 0600) |
| `claude::ClaudeParser`, `kind_of` | stream-json lines to events, usable alone; an error category to its `ErrorKind` |
| `codex::CodexSession` | `start(&Runtime, account: &ConversationFolder, folder: &ConversationFolder, CodexConfig, SignIn) -> Result<CodexSession, _>`, `AgentSession`, `thread_id`, `skipped`, `turn_running`, `history`, `process` |
| `codex::CodexConfig`, `SignIn`, `Start` | `start: Start` (`New`, `Resume`, `ResumeFrom(History)`), `model: Option<Model>`, `instructions: Option<String>` (64 KiB), `bridge: Bridge`, `limits`, `request_timeout` (1 ms to 600 s; `DEFAULT_REQUEST_TIMEOUT` 30 s); `SignIn::ApiKey(Secret)`, `SignIn::ChatGpt(KeyringAccess)`; `SignInKind` |
| `codex::CodexAccounts` | `open(&Runtime, account: &ConversationFolder, SignIn, AccountOptions) -> Result<CodexAccounts, _>`; `read(refresh) -> AccountState`, `check() -> AccountState`, `login_chatgpt() -> LoginStarted`, `login_device_code() -> LoginStarted`, `cancel_login(&str) -> CancelStatus`, `logout()`, `rate_limits() -> RateLimits`, `models() -> Vec<ModelInfo>`, `next_event(Duration) -> Option<AccountEvent>`, `shutdown(Duration)` |
| `codex::AccountState`, `AccountStatus`, `LoginStarted`, `AccountEvent` | `status` + `requires_openai_auth`; `SignedOut`, `ApiKey`, `ChatGpt { email, plan, workspace }`, `Expired { email, plan }`, `Other { kind }`; `Browser { login_id, auth_url }`, `DeviceCode { login_id, verification_url, user_code }`; `LoginStarted`, `LoginCompleted { login_id, success, error }`, `AccountChanged { auth_mode, plan }`, `RateLimits`, `Ended { exit, error }` |
| `codex::command_line`, `toml_string`, `StateFolders`, `History`, `history::path` | the arguments for a sign-in kind, state folders and bridge; a TOML basic string; the conversation's state and log folders; Archforge's copy of the conversation and its file |
| `codex::CodexParser`, `kind_of`, `rpc` | notifications to events, usable alone; a `codexErrorInfo` to its `ErrorKind`; the wire: `read_line`, `Incoming`, `Requests` (ids and per-request deadlines), `RpcError` |

**Discovery.** The override variable, when set, is the only place looked at: a relative path is
refused, a missing file is `Missing`, and nothing falls back to the search. Otherwise the PATH's
absolute entries are searched in order (relative and empty entries, which mean the current folder,
are skipped), then the installers' folders: `NPM_CONFIG_PREFIX/bin`, `~/.local/bin`,
`~/.claude/local`, `~/.npm-global/bin`, `~/.npm/bin`, `~/.volta/bin`, `~/.bun/bin`, the Node version
manager's version folders (newest first), `/opt/homebrew/bin` on macOS, Linuxbrew on Linux, and
`/usr/local/bin`. An application started from the macOS Finder gets only the system folders on its
PATH, so these folders are where a runtime installed from a terminal is found. Every candidate's
links are resolved and it must be a regular file with an execute bit. A candidate found only in an
installers' folder, which the person never put on the PATH, must also sit where no other account
can write: its folder, the program and the program's folder each owned by this user or the system,
none writable by others (a group write bit only on what this user owns, as Homebrew's folders are);
otherwise it is `Unsupported::Untrusted` and never run. Each candidate is asked `--version` in a
fresh private probe folder (its home and settings tree, removed afterwards), in the cleared
environment and its own process group, within 5 s and 64 KiB of output; a probe past the budget is
killed with its group. The first candidate of an accepted version wins, so an old copy earlier on the
PATH does not hide a current one; with none accepted, the first candidate's problem is answered. At
most 16 candidates, 256 PATH entries and 128 Node version entries are looked at; each cut is logged.

**Versions** (owner decision). Claude Code from 2.1.286 up to 3.0.0, tested with 2.1.317; Codex from
0.166.0 up to 1.0.0, tested with 0.166.0. A newer version in the range is `Found` with
`tested() == false` and a log warning; an older one is `TooOld` and the next major `Unsupported`, both
`MissingRuntime` to the interface. `2.1.317 (Claude Code)` and `codex-cli 0.166.0` are read strictly;
any other answer is `Unsupported`.

**The environment.** A runtime starts with a cleared environment holding only: `PATH` (the folder it
was found in, where an npm, Volta, nvm or Homebrew install's `node` lives, then the host PATH's
absolute entries), the host's `LANG` and `LC_*` (`find` keeps only those names from
`HostEnv::locale`, whoever filled it), `HOME` and `TMPDIR` in the conversation's folder,
`CLAUDE_CONFIG_DIR` or `CODEX_HOME` at its private configuration folder, and the request's
variables. Those names, and malformed or repeated names, are refused, as is a NUL in a value. Its
working folder is the conversation's `work` folder, empty when the conversation starts.

**Keys.** A key reaches a runtime as a `secret_vars` entry, through its environment, or, for Codex,
which reads no key from its environment, once over its standard input (a `scrub` entry, owner
decision 2026-11-06): a request whose command line or plain variables hold a secret's value is
refused, naming the variable and never the value. `SpawnRequest`'s and `RuntimeProcess`'s `Debug` never show a value (`Secret` redacts
itself). The `Command` that held copies of the environment is dropped right after the start; std
frees those copies without overwriting them, which this crate cannot change. The process keeps its
secrets (both kinds) only to scrub the error stream's tail and `redact`, where every occurrence becomes `<redacted>`, and a
tail cut at the front starts after a line break, so no part of a key is left at the cut. The tail's
buffer is allocated once and overwritten when dropped, as are the copies `render` makes, the input
lines once written, and the secrets themselves. Standard output reaches the adapter unscrubbed, as it
carries the protocol. Every process the runtime starts inherits its environment, the key among it
(its tool server among them): the adapters (AR-201, AR-202) decide what to blank there. A test with a
canary key holds the rest.

**The process tree.** A runtime leads a new process group (its group id is its process id).
`interrupt` sends SIGINT to the runtime alone, never the group, so the tool server it started keeps
running; both runtimes stop the turn and stay up. `shutdown(grace)` closes the input and waits up to
`grace` for the runtime to finish on its own, then sends SIGTERM to the group and waits up to `grace`
again, then sends SIGKILL; `grace` past 60 s is refused. Whenever the runtime has ended, by itself or
not, whatever is left in its group is killed with SIGKILL before the runtime is collected: the check
that it ended leaves it uncollected (`waitid` with `WNOWAIT`), so its process id, the group's id,
cannot be reused by another process before the group signal. The group is signalled three times,
5 ms apart: a process the group was forking at the first signal can appear after it (seen on macOS,
where a test caught such a survivor) and the next pass kills it. Dropping a `RuntimeProcess` kills the
group and the runtime and collects it, and signals nothing when the runtime can no longer be checked
as this process's child. Out of reach: a process that leaves the group on purpose (`setsid`); if it
keeps an output pipe open, that pipe's reader thread waits for it. A process stuck in an
uninterruptible system call delays the drop's final wait until the call returns.

**Input and output.** Each stream has its own named thread. Standard output is read as lines of at
most `max_line_bytes`: a longer line is counted, never held, and reported as `Dropped::TooLong`; a
line that is not UTF-8 is `Dropped::NotText`; both are logged by length alone. At most
`queued_lines` wait to be read, after which the reader, and then the runtime, wait. `send_line`
queues a line for the input thread and returns at once: a runtime that stops reading fills the queue
and the next line is refused with `InputBacklog`, so an adapter reading output and writing input from
one thread cannot deadlock. The error stream keeps its last `stderr_tail_bytes`; `exit_error` carries
at most the last `MAX_DETAIL_BYTES` (4 KiB) of it. A thread the system refuses to start is a typed
error, and the runtime it was for is killed.

**Folders.** `Workspace::open` makes the app-owned root with mode 0700 or refuses one that is a link,
someone else's, or readable by others; give it a folder inside the application's own data folder,
since only the root itself is checked, not the folders above it. Each conversation gets
`conversation-<id>` with `home`, `config`, `work` and `tmp`, all 0700, checked again on every open;
its contents are kept for a resume. `remove_conversation` removes the whole folder, refusing a link or
a folder not private to this user; `remove_dir_all` never follows a link inside it. Stop the runtime
first. A probe folder that could not be removed stays as `probe-<pid>-<n>` and is logged.

**The Claude adapter** (`claude`, AR-201, after the AR-196 spike's note). `ClaudeSession::start`
writes the tool server configuration and starts

```sh
claude -p --bare --input-format stream-json --output-format stream-json --verbose \
  --include-partial-messages --mcp-config <conversation>/claude-mcp.json --strict-mcp-config \
  --tools "" --allowedTools "mcp__archforge__*" --permission-mode dontAsk \
  --model <model> --session-id <uuid>      # or --resume <uuid>
```

with `CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1` and the key as `ANTHROPIC_API_KEY`, its only way
in. The model is named on every start, as a resume does not keep it; the session id is the
conversation's UUID, and the transcripts stay under the conversation's `CLAUDE_CONFIG_DIR`. The
configuration names one server:

```json
{"mcpServers":{"archforge":{"command":"<abs>/archforge-cli",
  "args":["mcp","--bridge","<port>","--read-only","--origin","<label>"],
  "env":{"ANTHROPIC_API_KEY":"","HOME":"<the person's home>"}}}}
```

`--read-only` goes only when "Allow changes" is off; switching it is a shutdown and a new start with
`Start::Resume`, which rewrites the file (not yet verified live: the spike's round-trip item 5).
Claude hands its whole environment to the server, so every secret variable is blanked in `env`
(verified live later: item 4). The bridge finds the control token in the settings folder it derives
from `HOME` (or `XDG_CONFIG_HOME`), and takes no token path yet, while `claude` gets a private
`HOME`; `env` therefore gives the bridge the person's own (`BridgeHome::from_process`), and a start
without it is refused. AR-203 can drop that once `archforge-cli mcp` takes the token file or settings
folder itself. A configuration that holds the key (pasted into the origin label, say) is refused
before it is written, as `SecretExposed`; an earlier private file of this user is replaced by an
atomic rename, anything else at the file's place (a link among them) is removed first, and the new
file must come out a regular file only this user can read.

`send` writes `{"type":"user","message":{"role":"user","content":…},"parent_tool_use_id":null}`
and refuses a second message while a turn runs. `stop` writes
`{"type":"control_request","request_id":"archforge-stop-<n>","request":{"subtype":"interrupt"}}`;
when that cannot be sent, when Claude answers it with an error, or on a second stop, SIGINT goes to
`claude` alone. Both end the turn and leave it up. The outcome is read from `result.is_error` and
the category fields, never `result.subtype` (which says `success` for failed calls): a synthetic
`assistant` message's `error` (with `api_error`) or a rejected `rate_limit_event` before the
`result`, else the result's `api_error`/`api_error_status`, else why the last retry happened.
`authentication_failed` and `cloud_credential_error` are `InvalidCredential`; `rate_limit` and
`billing_error` `RateLimited`; `model_not_found` and `overloaded` `ModelUnavailable`;
`connection_failed` `Network`; the rest `Other`. A failed result after a stop, or with
`terminal_reason: aborted_streaming`, is `Stopped`. `api_retry` is `Retrying` (no HTTP status and
`unknown`: `Network`). `init`, which comes after the first user line, is `Started`; a tool not under
`mcp__archforge__` there is logged. An exit during a turn fails it; an exit before any output with
`unknown option` on the error stream is `MissingRuntime`; the exit code decides nothing else.

Every line is hostile: empty, not JSON, nested past serde_json's 128 levels, not an object, untyped,
of an unknown type or a known type without its fields becomes `Ignored` and is counted
(`skipped`); ids and names are cut to 256 bytes, tool input to 64 KiB, summaries to 2 KiB, tool
lists to 4096 names, remembered tool calls to 1024 a turn, each cut logged by its length. Each line
is passed through `redact` before it is parsed, so no event, cut, log line or kept state holds any
part of a key the runtime writes whole in a line (a key's characters are never escaped in JSON); a
key split across two text deltas is caught only in the message they make up. `next_event` reads at
most 256 lines a call and never past its timeout, so a runtime writing only lines that mean nothing
cannot hold the caller; once the output closes it waits up to half a second for the exit status. A
line is parsed into a whole `serde_json::Value`, which may take several times the line's size: keep
`max_line_bytes` at its default or below for Claude. Claude's own
`control_request`s (permission prompts) are not expected with `dontAsk` and are skipped as unknown.

**The Codex adapter** (`codex`, AR-202, after the AR-196 spike's note). One `codex app-server`
process per conversation, driven over JSON-RPC 2.0 on its standard input and output (one object a
line, no `jsonrpc` field). `CodexSession::start` starts

```sh
codex app-server --strict-config \
  -c cli_auth_credentials_store="ephemeral"   # "keyring" for ChatGPT
  -c forced_login_method="api"                # "chatgpt"
  -c history.persistence="none" -c analytics.enabled=false -c check_for_update_on_startup=false \
  -c web_search="disabled" -c approval_policy="on-request" -c sandbox_mode="read-only" \
  -c sqlite_home="<conversation>/config/codex-state" -c log_dir="<conversation>/config/codex-log" \
  -c mcp_servers.archforge.command="<abs>/archforge-cli" \
  -c mcp_servers.archforge.args=["mcp","--bridge","<port>","--read-only","--origin","<label>"] \
  -c mcp_servers.archforge.env={HOME="<the person's home>"} \
  -c mcp_servers.archforge.startup_timeout_sec=10 -c mcp_servers.archforge.tool_timeout_sec=60 \
  -c mcp_servers.archforge.default_tools_approval_mode="approve" \
  --disable shell_tool --disable unified_exec --disable apps --disable plugins --disable multi_agent \
  --disable view_image --disable image_generation --disable browser_use --disable computer_use \
  --disable goals --disable hooks --disable skill_search --disable tool_suggest --disable sleep_tool \
  --disable memories --disable shell_snapshot
```

Every value is a TOML basic string with quotes, backslashes and control characters escaped, so no
label can end it. `--strict-config` makes Codex refuse an unknown key or feature before reading its
input; the whole line was accepted by codex-cli 0.166.0 offline. `--read-only` goes only when "Allow
changes" is off; the origin label is the conversation's. `apply_patch` has no switch: the read-only
sandbox and the declined approvals hold it back. The bridge's tool calls are not asked about
(`approve`): its own `--read-only` is the guard.

*Folders.* `CODEX_HOME` is the workspace's account folder (`Workspace::account(RuntimeKind::Codex)`,
`account-codex/config`), the same for every conversation: Codex names its keyring entry for the
ChatGPT sign-in after that folder's path, so a folder per conversation would need a sign-in per
conversation. The account folder holds only Codex's installation id, its bundled skills and helper
binaries. Everything a conversation leaves behind is in the conversation's own folder, deleted with
it: Codex's state and log databases (`sqlite_home`, `log_dir`, checked offline to move every
`*.sqlite` there), and the conversation itself. Threads are `ephemeral`, so Codex writes no thread
history under `CODEX_HOME`; Archforge keeps its own copy, the person's messages and the answers in
order, in `codex-history.json` in the conversation's folder (written after every turn, atomically,
0600, at most 4096 messages and 4 MiB, the oldest dropped first and logged; the person's text is
scrubbed of the key). `Start::Resume` gives a new thread that copy with `thread/inject_items`
before the first turn; `Start::ResumeFrom(History)` primes it from the project's record (AR-206)
when the folder has none, as on another computer (the folder's copy then starts from it). Tool calls
are not in the copy.

*Sign-in.* `SignIn::ApiKey(key)`: the key goes once, in the `account/login/start
{"type":"apiKey"}` line on standard input, built in a buffer that is overwritten when dropped;
with the in-memory store nothing is written (no `auth.json`). The key is in no argument, variable
or file; it is on the process's scrub list, so every output line is scrubbed of it before it is
read, as the error stream's tail is, and a command line holding it (a key pasted into the label) is
refused as `SecretExposed`. `SignIn::ChatGpt(KeyringAccess)`: Codex keeps the sign-in in the system
keyring; `KeyringAccess::from_process` passes the session bus variables on Linux, nothing on
macOS. Neither variant puts a secret in Codex's environment, so its tool server cannot inherit one.

*The handshake and turns.* `start` sends `initialize` (with `clientInfo.name` `archforge`), the
`initialized` notification, the key's login and `account/read`, and returns at once. The
`initialize` answer's `userAgent` (`archforge/0.166.0 …`) is a second version check: past the range
ends the session as `MissingRuntime`, newer than tested is logged. No thread is started until the
first `send`, as `thread/start` already reaches the network: a sign-in that is missing (no account)
or refused fails the turn as `InvalidCredential` with no thread started. Then `thread/start
{cwd, approvalPolicy:"on-request", sandbox:"read-only", ephemeral:true, model?,
developerInstructions?}`, the earlier messages on a resume, and `turn/start {threadId, input,
model?}`. `Started` comes with the thread (its id and model) and again with each tool server state.
`item/agentMessage/delta` is `TextDelta`, a completed `agentMessage` `MessageDone`, an
`mcpToolCall` item `ToolStarted`/`ToolFinished`, `error` with `willRetry` `Retrying`, and
`turn/completed` `TurnFinished` (`completed`, `interrupted` → `Stopped`, `failed`), with the tokens
of every `thread/tokenUsage/updated` in the turn summed. `stop` sends `turn/interrupt {threadId,
turnId}` once Codex named the turn; a message still waiting for its thread is dropped as stopped.
A turn's notifications (`turn/*`, `item/*`, `error`, `thread/tokenUsage/updated`) count only for
the open turn whose `turn/start` was sent: those for another thread, an ended or abandoned turn (the
last 64 are remembered), or before the turn was asked for change nothing, neither the events nor
the copy; those that come before Codex named the turn are held (at most 1024) and then given to the
turn it names. When Codex answers `turn/start` without naming the turn, a held `turn/started`
that names one names it, else the first held notification that does, else the first later one; with none within one request timeout the turn fails as
`Other` (stopped when a stop was asked for) and its thread is left. A `turn/start` past its deadline fails the turn (as stopped when a stop was asked
for), and its thread is left: the next message starts a fresh thread with the copy, so it can never
join a turn Codex started after all; a late answer names that turn, which is interrupted while the
current turn goes on. A `thread/inject_items` past its deadline leaves its thread too, so the
earlier messages are never given twice.
There is no signal fallback: SIGINT ends `codex app-server`. Shutdown closes the input (Codex exits
0), then terminates and kills as for every runtime.

*Requests and errors.* Every request has its own deadline (`request_timeout`), as Codex never
answers a line it cannot read: a handshake request past it ends the session, a thread or turn
request fails the turn, both as `Other` ("did not answer … within"). At most 64 wait at once. Codex
answers every error with -32600; one whose message says `unknown variant` (a method this Codex
lacks) is `MissingRuntime`, as is a start that stops before its first line with `unknown
configuration field` or `unexpected argument`. Approvals and questions from Codex are declined in
the shape each takes and logged by method (`decision: decline` for commands and files, `action:
decline` for elicitations, an empty grant for permissions, no answers for questions, `denied` for
the legacy approvals); any other request gets a JSON-RPC error, so none blocks a turn. The trait
needs no permission event for that, so it is unchanged. Categories come from `codexErrorInfo`:
`unauthorized` is `InvalidCredential` with a key and `SignInExpired` with ChatGPT;
`usageLimitExceeded`, `rateLimitExceeded`, `sessionBudgetExceeded` `RateLimited`;
`modelNotFound`, `serverOverloaded`, `flexUnavailable`, `policyCapacityUnavailable`
`ModelUnavailable`; `httpConnectionFailed`, `responseStreamConnectionFailed`,
`responseStreamDisconnected`, `responseTooManyFailedAttempts` `Network` (with HTTP 401/403, 429
or 404 as those); the rest `Other`. The message is for the log and the detail only.

Every line is hostile, as for Claude: empty, not JSON, too deep, not an object, neither a request,
an answer nor a notification, an answer to no waiting request, an unknown notification or a known
one without its fields becomes `Ignored` and is counted; names are cut to 256 bytes, tool input to
64 KiB, summaries to 2 KiB, error words to 1 KiB, method names over 128 bytes are malformed. Each
line is scrubbed of the key before it is read. `next_event` reads at most 256 lines a call and never
past its timeout or the next request deadline.

*The account operations* (`CodexAccounts`, for AR-204's `AgentAccounts`). Each opens its own
`codex app-server` in the account folder (its home, work and temporary folders), without a tool
server and never starting a thread, so nothing reaches a model; its state and log databases go to a
folder of its own under the account's temporary folder, removed when it drops, so nothing it logs
outlives it. Every call waits for its own answer
at most the request timeout, so call it on a worker. `open` with `SignIn::ChatGpt` uses the keyring
store; with `SignIn::ApiKey` the in-memory one and gives the key at once (a refused key is an
`InvalidCredential` error from `open`): that is the API-key check, and the key is not kept, since
each conversation gives it again. `read(refresh)` is `account/read` (`refreshToken` renews a ChatGPT
sign-in, reaching the sign-in service only); `check` reads and, for ChatGPT, renews once: a renewal
refused with words of an expired or revoked sign-in reads as `Expired`. `login_chatgpt` and
`login_device_code` answer the address to open, or the code and the address, and queue the same as
`AccountEvent::LoginStarted`; an address must be `https` on `openai.com` or `chatgpt.com` (or below)
without a user part, else it is refused and never queued; a device code is letters, digits and
hyphens. Keep the process running until `LoginCompleted`: Codex receives the browser's answer on its
own local callback. `cancel_login`, `logout`, `rate_limits` and `models` (every page, hidden models
left out, at most 10 pages and 1000 models) complete the set. ChatGPT calls on an API-key process
are refused (`InvalidSetting`), so a key never meets the keyring.

What a real round trip still has to show (spike items 6 to 11): an API-key turn with the bridge and
no `auth.json`; the ChatGPT sign-in in the browser and with a device code under a private `HOME`
(whether the macOS keychain and the Linux secret service are reached), sign-out, and what an
expired sign-in looks like; whether an MCP tool call still raises an elicitation with `approve`;
`turn/interrupt` timing; `thread/inject_items` with the copy's messages; `account/rateLimits/read`
for both kinds; that the switched-off tools stay off; that no state or log database holds the key
after an API-key login; and whether processes sharing one ChatGPT sign-in renew it without
spending each other's renewal.

**Tests.** `cargo test -p archforge-agents`: unit tests beside the code, and `tests/runtimes.rs`
against fake runtimes (shell scripts that answer `--version` and then do what a test needs) for
discovery order and the override, relative PATH entries, links, untrusted installers' folders, the
locale filter, the version decisions, a failing and a hanging probe, the cleared environment, the
canary key, the line cap, the error tail cap, the input backlog, SIGINT, the shutdown steps, the group
killed on drop and on exit, and the private folders; `tests/claude.rs` for the Claude adapter, over
the spike's transcripts in `tests/fixtures/` (every Claude one and the hostile lines) and against fake
`claude` scripts that replay them: the events of a turn, the error categories, the command line and
configuration for both settings, the canary key, stop by control request and its SIGINT fallback, an
exit during a turn. `tests/codex.rs` for the Codex adapter, over every Codex transcript of the spike
and the hostile lines, against the generated 0.166.0 schema (`tests/fixtures/codex/schema/` holds the part the
tests read: the message unions and the shapes of the definitions used, cut as its `$comment` says,
credited to OpenAI Codex under Apache-2.0 in the `NOTICE` beside it and in `ATTRIBUTION.md`;
every method sent, read or declined exists there, and every line the adapter wrote fits its
method's parameters or its request's response), and against fake `codex` scripts that answer the
handshake and replay the transcripts through `spawn`: a turn end to end, the resume and the primed
resume, stop by `turn/interrupt`, a stop before the thread, the declines, a request timeout, a
missing and a refused sign-in, a Codex out of range, an exit during a turn, the canary key (only in
the login line; not in arguments, environment, any file, events, errors, `Debug` or logs, and no
`auth.json`), the command line for both settings, the account folder, and the account calls
(device code shown and cancelled, browser sign-in, check, rate limits, models over pages, sign-out,
a foreign address refused, expiry, the API-key check, a call timeout). Each crate has one real
round trip, `#[ignore]`d: it needs a real key or sign-in and the owner's OK. Unix only.

**Non-goals.** Windows: every entry point answers `AgentError::NotAvailable` ("not available on this
system yet"). Storing conversations is the project's (AR-206); the Settings page (AR-204) calls
`CodexAccounts`. The bridge's token lookup through `HOME` (the spike's open question 6) is bridged by the
Claude adapter's configuration for now.
