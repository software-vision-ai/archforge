# archforge-agents

**Purpose.** The agent runtimes a person installs beside Archforge, Claude Code (`claude`) and OpenAI
Codex (`codex`): found on this computer, checked against the versions Archforge accepts, and run as
supervised child processes in private folders. Archforge never bundles them. The adapters that
speak each runtime's protocol (AR-201, AR-202) build on this crate; the AR-196 spike's note records
how the runtimes behave.

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
| `Workspace` | `open(&Path) -> Result<Workspace, _>` (absolute; made with 0700, an existing one must be private and is never changed), `conversation(&ConversationId) -> Result<ConversationFolder, _>` (makes or checks), `remove_conversation(&ConversationId) -> Result<bool, _>` |
| `ConversationId`, `ConversationFolder` | 1 to 64 of `[a-z0-9-]`; `path()`, `home()`, `config()`, `work()`, `tmp()` |
| `spawn` | `pub fn spawn(&Runtime, &ConversationFolder, SpawnRequest) -> Result<RuntimeProcess, AgentError>` |
| `SpawnRequest`, `Limits` | `args`, `vars`, `secret_vars: Vec<(String, Secret)>`, `input_secrets` (secrets that go over standard input, never into the environment: refused in the command line and plain variables, kept only to scrub), `config_dir` (a shared private settings folder in place of the conversation's `config`), `limits`; `max_line_bytes` (8 MiB), `stderr_tail_bytes` (16 KiB), `queued_lines` (16), each refused at zero or past its maximum, and `max_line_bytes × (queued_lines + 1)` refused past `MAX_BUFFERED_BYTES` (256 MiB) |
| `RuntimeProcess` | `send_line` (never waits), `close_input`, `next_event(timeout)`, `try_next_event`, `interrupt`, `status`, `shutdown(grace)` (waits up to twice `grace`, so on a worker), `terminate` (never waits), `try_reap` (never waits), `add_input_secret` (a secret sent over the input after the start, scrubbed from then on), `stderr_tail`, `stderr_closed`, `exit_error(Exit)`, `pid`; kills its group on drop |
| `OutputEvent`, `Dropped`, `Exit` | `Line`, `Dropped(TooLong \| NotText { bytes })`, `ReadFailed`, `End`; `code`, `signal` |
| `AgentError`, `ErrorKind` | every failure; `kind()` gives `MissingRuntime`, `InvalidCredential`, `SignInExpired`, `RateLimited`, `Network`, `ModelUnavailable` or `Other`; `AgentError::reported(kind, detail)` for the adapters |
| `RuntimeProcess::redact` | `fn redact(&self, String) -> String`: the runtime's secrets replaced, for what an adapter shows from standard output |
| `AgentSession` | the neutral trait every adapter implements: `send(&mut self, &str)`, `stop(&mut self)`, `next_event(&mut self, Duration) -> Option<SessionEvent>`, `shutdown(&mut self, Duration) -> Result<Exit, _>` |
| `SessionEvent` | `Started { session_id, model, tools, servers }` (Codex: the thread's id, no tool list, and again when a tool server's state changes), `TextDelta`, `MessageDone`, `ToolStarted { id, server, tool, input }`, `ToolFinished { id, tool, ok, summary }`, `Retrying { attempt, max_attempts, delay_ms, kind }`, `TurnFinished { outcome, usage }`, `Ignored(Skipped)`, `Ended { exit, error }` |
| `TurnOutcome`, `Usage`, `ToolServer`, `Skipped` | `Completed`, `Stopped`, `Failed(AgentError)`; token counts without a cost; a tool server's name and state; why a line was skipped |
| `claude::ClaudeSession` | `start(&Runtime, &ConversationFolder, ClaudeConfig, Secret) -> Result<ClaudeSession, _>`, `AgentSession`, `session_id`, `skipped`, `turn_running`, `process` |
| `claude::ClaudeConfig` | `session: SessionId` (the conversation's UUID), `start: Start` (`New`, `Resume`), `model: Model`, `bridge: Option<Bridge { cli, port, token_file, origin, allow_changes }>` (`None`: no tool server at all), `limits` |
| `claude::command_line`, `mcp_config`, `write_mcp_config` | the arguments; the tool server configuration as JSON; that configuration written to `claude-mcp.json` in the conversation's folder (atomic, 0600) |
| `claude::ClaudeParser`, `kind_of` | stream-json lines to events, usable alone; an error category to its `ErrorKind` |
| `codex::CodexSession` | `start(&Runtime, &ConversationFolder, &CodexHome, CodexConfig, Auth) -> Result<CodexSession, _>`, `AgentSession`, `history`, `damaged_copy`, `copy_error`, `skipped`, `turn_running`, `process` |
| `codex::CodexConfig`, `Auth`, `ChatGptStore`, `CredentialStore` | `start: Start`, `model: Option<Model>`, `bridge: Option<Bridge>` (`None`: no tool server), `instructions` (64 KiB), `limits` (`max_line_bytes` at least 8 MiB), `request_timeout` (10 ms to 600 s, default 30 s); `Auth::ApiKey(Secret)` or `Auth::ChatGpt(ChatGptStore::{Keyring, File})`; `CredentialStore::{Ephemeral, Keyring, File}` |
| `codex::CodexHome` | `open(&Workspace)`: `codex-home` in the workspace, 0700, Archforge's private `CODEX_HOME` |
| `codex::CodexAccount` | `start(&Runtime, &Workspace, &CodexHome, CredentialStore, Limits, request_timeout, timeout)`, `read`, `login_api_key`, `login_chatgpt`, `login_device_code`, `cancel_login`, `logout`, `models`, `next_event`, `shutdown`, `folder`, `skipped`, `process` |
| `codex::command_line`, `kind_of`, `RpcError`, `EXPIRED_SIGN_IN_MESSAGES` | the arguments; a `codexErrorInfo` to its `ErrorKind`; an error answer (`unknown_method`, `expired_sign_in`); Codex's exact refresh sentences |
| `codex::history` | the conversation's copy: `History`, `COPY_FILE`, the budget constants, `LARGEST_COPY_BYTES`, `MAX_COPY_FILE_BYTES` |

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

**Keys.** A key reaches a runtime only as a `secret_vars` entry, through its environment, or as an
`input_secrets` entry, which never enters the environment and which the adapter writes to the
runtime's standard input itself (Codex's API key, owner decision): a request whose command line or
plain variables hold either kind of secret's value is refused, naming the variable and never the
value. `SpawnRequest`'s and `RuntimeProcess`'s `Debug` never show a value (`Secret` redacts
itself). The `Command` that held copies of the environment is dropped right after the start; std
frees those copies without overwriting them, which this crate cannot change. The process keeps its
secrets only to scrub the error stream's tail, where every occurrence becomes `<redacted>`, and a
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
where a test caught such a survivor) and the next pass kills it. `terminate` ends the runtime without
waiting: its input is closed, then SIGTERM and SIGKILL go once each to its group and to the runtime
itself, with no pause, and `try_wait` collects it if it has already ended; `try_reap` collects it
later the same way (one SIGKILL to what is left of the group first), so no zombie stays. Neither
signals a runtime already collected. Dropping a `RuntimeProcess` kills the
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
conversation's UUID, and the transcripts stay under the conversation's `CLAUDE_CONFIG_DIR`. With a
bridge, the configuration names one server:

```json
{"mcpServers":{"archforge":{"command":"<abs>/archforge-cli",
  "args":["mcp","--bridge","<port>","--token-file","<abs token file>","--read-only","--origin","<label>"],
  "env":{"ANTHROPIC_API_KEY":""}}}}
```

and without one (a conversation without agent access, a test message) none: `{"mcpServers":{}}`.
`--token-file` is the file the application granted this conversation alone
(`archforge_control::Server::grant`), so the bridge needs no home folder of its own. `--read-only`
goes only when "Allow changes" is off, a second layer over the read-only role of the conversation's
token, which the control channel enforces; switching it is a shutdown and a new start with
`Start::Resume`, which rewrites the file (not yet verified live: the spike's round-trip item 5).
Claude hands its whole environment to the server, so every secret variable is blanked in `env`
(verified live later: item 4). A configuration that holds the key (pasted into the origin label, say) is refused
before it is written, as `SecretExposed`; whatever stands at the file's place, a link among them, is
removed first, and the new file must come out a regular file only this user can read.

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

**The Codex adapter** (`codex`, AR-202, after the AR-196 spike's note). `CodexSession::start` starts

```sh
codex app-server --strict-config -c cli_auth_credentials_store="<store>" \
  -c history.persistence="none" -c analytics.enabled=false -c web_search="disabled" \
  --disable shell_tool --disable unified_exec --disable apps --disable plugins --disable multi_agent \
  --disable view_image --disable image_generation --disable browser_use --disable computer_use \
  --disable goals --disable hooks --disable skill_search --disable tool_suggest --disable sleep_tool \
  -c mcp_servers.archforge.command="<abs>/archforge-cli" \
  -c mcp_servers.archforge.args=["mcp","--bridge","<port>","--token-file","<abs token file>","--read-only","--origin","<label>"] \
  -c mcp_servers.archforge.startup_timeout_sec=10 -c mcp_servers.archforge.tool_timeout_sec=60
```

in the conversation's folder (its `home`, `work` and `tmp`), with `CODEX_HOME` at Archforge's private
`codex-home` (`CodexHome`, 0700, shared by every Codex process of the person). The bridge goes in
`-c` values, not a `config.toml`, as `CODEX_HOME` is shared; the arguments follow the Claude
adapter's rules, every value is checked (no control character, absolute UTF-8 paths), and without a
bridge no `mcp_servers` value is given at all. That `-c mcp_servers.*` works like the spike's `config.toml` is not yet verified live.

*Stores (owner decisions).* An API key (`Auth::ApiKey`) goes to Codex over its standard input in
`account/login/start {"type":"apiKey"}`, with `cli_auth_credentials_store="ephemeral"`: never a file,
the keyring, the environment or the command line (it is an `input_secrets` entry, so it is refused
there and scrubbed from the error tail and `redact`; an account process adds the key it is given to
what it scrubs before sending it, as Codex may echo it). The line is built in one buffer of its final
size and overwritten when dropped; a key with characters JSON would escape is refused first. A
ChatGPT sign-in is kept in the system keyring (`ChatGptStore::Keyring`) or, where no usable keyring
exists, in Codex's own files in `codex-home` (`ChatGptStore::File`); the application chooses.

*Requests and bounds.* Every request has its own timeout (`request_timeout`). The session's methods
never wait, but `next_event` (at most its timeout) and `shutdown` (at most twice its grace). The one
exception to the timeout: when a turn is accepted or ends, `next_event` writes the conversation's copy
within the call (one file of at most `LARGEST_COPY_BYTES`, synchronised with its folder). A timeout
too long for the clock waits as long as the clock allows, never zero.

| Call | Requests | Bound |
|---|---|---|
| `CodexSession::start` | `initialize`, queued; `initialized` and, for a key, `account/login/start` follow from `next_event` | none waited for; a handshake request past its timeout ends the session |
| `send` | for a turn without a ready thread `thread/start`, then the copy in `thread/inject_items` batches (as many messages as fit half the line limit, at least one; one after another; at most 16 with the default 8 MiB limit; none for an empty copy), then `turn/start` | each its own request timeout |
| `stop` | at most one `turn/interrupt` a turn, tied to the turn's number; none for a turn not sent yet (it ends at once as stopped); for a turn Codex has not named, sent once it is named | its request timeout; refused or timed out, the turn has one more request timeout to end (send stays refused), then the session ends naming `Start::Resume`; a stop answer for a turn no longer open is ignored |
| `next_event` | none of its own; releases threads (below) | at most its timeout, at most 256 lines; an exit wait is at most the smaller of 500 ms and the time left |
| `CodexAccount::start` | `initialize`, then `initialized` | the smaller of `timeout` and `request_timeout` |
| `read`, `login_api_key`, `cancel_login`, `logout` | one each | the smaller of `timeout` and `request_timeout` |
| `login_chatgpt`, `login_device_code` | one, or two when Archforge refuses the answer: the sign-in is then cancelled with `account/login/cancel`, only when Codex's raw `loginId` is a UUID (it is sent back exactly as given, never a cut or shown form) | both within the smaller of `timeout` and `request_timeout` |
| `models` | `model/list` page by page, at most 64 pages and 4096 models | each page its request timeout, all within `timeout` |
| `CodexAccount::next_event` | none | at most its timeout, at most 256 lines |

*Zero timeout.* `next_event(Duration::ZERO)`, on a session or an account process, reads every line
already received without waiting, up to the 256-line budget, so it never answers `None` while an event
waits behind lines that mean nothing. (A line still in the pipe that the reader thread has not taken
yet is not "received".)

*Answers.* `account/read`: `requiresOpenaiAuth` must be a boolean, else the answer is unreadable; a
missing or null `account` is signed out. An expired ChatGPT sign-in is `SignInExpired` only for code
-32603 with one of Codex 0.166.0's exact sentences (`EXPIRED_SIGN_IN_MESSAGES`, quoted from
`codex-rs/login/src/auth/manager.rs`); no loose word counts. An error is an unknown method (a
missing runtime) only when the first unknown variant its message names is the request's own method,
so an unknown parameter value is not. `model/list`: a missing or null `nextCursor` ends the list, any
other is passed back unchanged; an empty, non-text, repeated or over 64 KiB cursor fails the call.
Sign-in addresses must be `https` with a host and no user part, printable ASCII, at most 8 KiB;
device codes 1 to 32 letters, digits and hyphens; login ids UUIDs. A cancelled sign-in's end is
`LoginOutcome::Cancelled` (Codex says "Login was not completed" for a device code and "Login server
error: Login was not completed" for the browser); only a ChatGPT sign-in whose start timed out is
cancelled when it starts late. The `Debug` of everything holding a
sign-in address, a device code, an email, a key or a message shows lengths only. A turn ends by
`turn/completed`'s status as the spike's error mapping says, `unauthorized` being `InvalidCredential`
with a key and `SignInExpired` with ChatGPT. Approvals and elicitations are declined, a permission
request gets an empty grant, any other request of Codex's a JSON-RPC error.

*Ending.* When a call's Codex ends first, the call fails with the end's reason (the exit code or
signal and the error tail) and `Ended` stays queued; every later call of an account process fails
with that reason too. When Codex closes its output, its exit status and whole error tail are awaited
for up to 500 ms, so `Ended` carries them (also when a `shutdown` came after Codex had ended or closed
its output on its own); only when the exit
is not seen in that time is the reason generic, and Codex, its output closed but running, is then
terminated (a session or an account process alike). When Archforge must end a session (an
unreadable, refused or timed-out handshake, a refused key, a refused stop's grace spent), its input is
closed, it gets 500 ms to end, then `RuntimeProcess::terminate`, and `Ended` comes within 500 ms more
carrying the original reason (without an exit status if the system has not collected it yet: it is
collected later, without waiting, in `next_event`, `shutdown` or the drop). None of it blocks a caller
past its timeout. An account process whose start fails is dropped. Dropping a session or an account
process kills Codex's group and collects it as `RuntimeProcess`'s drop does, which waits for a
process stuck in an uninterruptible system call.

*Threads.* Every thread is started `ephemeral` and Codex runs with `history.persistence="none"`, so
no thread history is kept in the shared `CODEX_HOME`; whether Codex's own databases there (the spike
saw `logs_2.sqlite`, `state_5.sqlite` and `memories_1.sqlite` appear after one ephemeral thread) hold
any of a conversation is not verified yet and needs a real round trip. A thread a timeout abandons is released with `thread/unsubscribe`: after a
`thread/start` timeout the request is remembered (at most 16, each for ten request timeouts) and the
thread a late answer names is released; after a `thread/inject_items` timeout the thread is released at
once; after a `turn/start` timeout the old thread is kept only until a late turn on it (named by the
late answer or `turn/started`) is interrupted, then released, or released when its request is
forgotten after ten request timeouts or when the late answer names no turn; a turn accepted without a
name and asked to stop is ended as stopped after a request timeout, its thread released. Tool server
states that arrive before `thread/start`'s answer are held (at most 64 and 1 MiB), delivered only when
they name the answered thread, and dropped when the start fails; the servers are reported with
`Started` once the thread is ready, and again on each change.

*The conversation's copy* (`codex-history.json` in the conversation's folder, 0600). Codex's threads
keep nothing, so Archforge keeps the conversation's text messages (not tool calls); `Start::Resume`
reads them and hands them to the new thread before the next question, and `Start::New` refuses a folder
that already holds a copy (AR-206 gives every conversation a fresh folder). The copy's snapshot is taken
before the question is added and put back when the turn was never sent or `turn/start` was refused; the
question is kept once `turn/start` is accepted, and also when it was sent but timed out or Codex ended
(Codex may have received it). The budget is 4096 messages and 4 MiB of text, each message counted as
its text plus 64 bytes and cut at 1 MiB (a longer question is refused); the oldest go first, the open
turn's question coming on top until the turn's first answer or its end. So a copy holds at most 4097
messages, and the longest file is 31,455,920 bytes (`LARGEST_COPY_BYTES`: four 1 MiB-less-64 answers
and a 1 MiB question, every byte a control character JSON writes in six); the reader takes up to
32 MiB. A copy of exactly 4097 is trimmed to 4096; one with more, or one unreadable, too long or not a
regular file, is damaged and set aside whole as `codex-history.json.damaged-<n>`, its name claimed first
with an exclusive create (a file, or a folder for a folder), so an earlier damaged copy is never
replaced; the conversation goes on without it and `damaged_copy` names it. Before parsing, a copy's
commas and opening brackets outside strings are counted (three a message), so a file of tiny values
cannot make the parser build millions of them; the file is opened without following a link or
blocking, and must be a regular file on the open handle. The copy is written when a turn is accepted
(the question) and when it ends (its answers), so a crash during a turn loses that turn's answers but
not its question; a failed write is logged and kept for `copy_error` until a later write succeeds.
Every write goes through
`write_private_file`: a new temporary file beside the target (exclusive, 0600, never following a
link), written and synchronised, checked to be the very file written (device and inode), a regular
0600 file of this user, then renamed over the target, so a link at the target's place is replaced, never
followed; a failure before the rename removes only the temporary file, and a failed folder
synchronisation after it is a logged warning.

*Account processes* run in a scratch folder `codex-<pid>-<n>` of the workspace (0700, home, config,
work and tmp inside), made exclusively (a taken name is skipped, up to 64 tries) and owned from the
moment it exists: a failed start or the drop removes that folder alone.

*Not swept.* A process that crashes leaves its `codex-<pid>-<n>` scratch folder, and a write cut short
between its temporary file's creation and its rename leaves `.<name>.tmp-<pid>-<n>`; nothing removes
either later. Damaged copies are kept. What Codex itself writes to `codex-home` (its logs and state
databases) is its own. A conversation's folder goes with `Workspace::remove_conversation`, its copy with
it.

**Tests.** `cargo test -p archforge-agents`: unit tests beside the code, and `tests/runtimes.rs`
against fake runtimes (shell scripts that answer `--version` and then do what a test needs) for
discovery order and the override, relative PATH entries, links, untrusted installers' folders, the
locale filter, the version decisions, a failing and a hanging probe, the cleared environment, the
canary key, the line cap, the error tail cap, the input backlog, SIGINT, the shutdown steps, the group
killed on drop and on exit, and the private folders; `tests/claude.rs` for the Claude adapter, over
the spike's transcripts in `tests/fixtures/` (every Claude one and the hostile lines) and against fake
`claude` scripts that replay them: the events of a turn, the error categories, the command line and
configuration for both settings, the canary key, stop by control request and its SIGINT fallback, an
exit during a turn. Its one real round trip is `#[ignore]`d: it needs a real key and the owner's OK.
`tests/codex.rs` for the Codex adapter, over copies of the spike's Codex transcripts in
`tests/fixtures/codex/` (their note lines kept; `docs/agent-runtimes/README.md` says how they were
sanitised) and fake `codex` scripts that replay them or answer as a test needs; every fixture message
the contract covers is validated against `schema/codex-app-server-0.166.0.contract.json`, a 65 KB cut
of Codex 0.166.0's generated schema (Apache-2.0, credited in its `$comment` and in NOTICE), as is
every request and notification the adapter writes in those tests. No test runs a real `codex`, uses a real key or reads `~/.codex`. Unix only.

**Agent access and its limits** (AR-203). The application gives each conversation with agent access
its own token, read-only unless that conversation allows changes, in a file of its own beside the
others in Archforge's settings folder; the control channel refuses every change a read-only token
asks for. That promise assumes the runtime has no tool that reads files: a model that could read a
sibling `control-<port>-<hex>.token` of a conversation that allows changes could use that token.
Claude runs with `--tools ""` and only the bridge's tools allowed, and Codex with its shell, apps and
plugins disabled, so neither has one as configured here; a runtime version that adds a file-reading
tool outside these switches would break it. Token files are removed when their conversation ends,
when agent access closes and on quit; a crash leaves them behind. Their tokens are dead with the
process that minted them (a later start mints new ones on a new port), and nothing sweeps the files.

**Non-goals.** Windows: every entry point answers `AgentError::NotAvailable` ("not available on this
system yet"). Storing conversations in the project (AR-206) and the Chat screen come later; a Codex
turn's token use is not read yet (its `usage` is zero).
