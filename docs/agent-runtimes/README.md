# Agent runtimes for Chat: protocols and lifecycle (AR-196)

The spike for story AR-173 (chat with the Claude and Codex agents). It records how Archforge drives
the two agent programs a person installs, `claude` (Claude Code) and `codex` (OpenAI Codex), as
child processes, and how they reach the model through Archforge's MCP server
`archforge-cli mcp --bridge <port>` ([Working with an agent](../manual/07-agents.md)). The adapter
subtasks AR-201 (Claude) and AR-202 (Codex) and the supervisor AR-200 build on it.

Tested on macOS with Claude Code **2.1.317** and codex-cli **0.166.0**, on 2026-11-06.

## How this was checked

Nothing here sent a prompt to a model, ran a sign-in, or read an existing credential or the
person's own runtime settings. Every run had:

- a cleared environment: `env -i` with only `PATH=/usr/bin:/bin`, `HOME` set to an empty temporary
  folder, and a private empty `CLAUDE_CONFIG_DIR` or `CODEX_HOME`;
- for Claude, `ANTHROPIC_BASE_URL=http://127.0.0.1:9` (a closed local port) and
  `CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1`, so no request could leave the computer. Where a run
  needed a key to get past the "not logged in" check, the key was a made-up canary string
  (`sk-ant-api03-CANARY-…`) that only ever met the closed local port;
- for Codex, only requests that answer without an account: `initialize`, `account/read`,
  `model/list`, `config/read`, `mcpServerStatus/list`, error probes, and one `thread/start` with the
  Archforge bridge configured.

What was observed is marked **observed**; what comes from the providers' documentation is marked
**doc**; what needs a real key or sign-in is listed under
[To verify with a real round trip](#to-verify-with-a-real-round-trip-needs-the-owners-ok).

## Lifecycle at a glance

```text
                Claude (claude -p, stream-json)          Codex (codex app-server, JSON-RPC)
                ---------------------------------        -----------------------------------
 find           PATH, known folders; `claude --version`   PATH, known folders; `codex --version`
                -> "2.1.317 (Claude Code)", exit 0        -> "codex-cli 0.166.0", exit 0
                below the floor -> MissingRuntime         below the floor -> MissingRuntime
                     |                                         |
 spawn          env (cleared first): PATH, locale,        env: PATH, HOME, CODEX_HOME=<private>
                HOME=<private>, TMPDIR=<private>,         args: app-server --strict-config -c ...
                CLAUDE_CONFIG_DIR=<private>,              cwd: private per-conversation folder
                CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1,
                ANTHROPIC_API_KEY
                cwd: private per-conversation folder
                args: see "Claude: the command line"
                     |                                         |
 handshake      none; the process waits on stdin.         -> initialize {clientInfo}
                The first user line triggers               <- result {userAgent "archforge/0.166.0 ...",
                <- system/init (session_id, tools,            codexHome, platformOs}
                   mcp_servers[].status, model,           -> initialized (notification)
                   apiKeySource, claude_code_version)     -> account/read  <- {account|null, requiresOpenaiAuth}
                check: archforge "connected",             -> model/list    <- {data[], nextCursor}
                tools only mcp__archforge__*              (sign in if needed: account/login/start)
                     |                                    -> thread/start {cwd, sandbox, approvalPolicy,
                     |                                        ephemeral, developerInstructions}
                     |                                    <- result {thread.id} + thread/started
                     |                                    <- mcpServer/startupStatus/updated
                     |                                         starting -> ready | failed
                     |                                         |
 turn           -> {"type":"user","message":{...}}        -> turn/start {threadId, input[], model}
                <- system/status "requesting"             <- result {turn} + turn/started
                <- stream_event (text_delta, tool input)  <- item/started, item/agentMessage/delta,
                <- assistant (text, tool_use)                item/completed (agentMessage, mcpToolCall)
                <- user (tool_result)                     <- server requests (approvals): answer
                <- system/api_retry (while retrying)         decline unless allowed
                <- result (one per user line)             <- error {willRetry}, account/rateLimits/updated
                                                          <- turn/completed {status}
                     |                                         |
 stop           SIGINT to the pid (doc), or               -> turn/interrupt {threadId, turnId}
                control_request interrupt (observed)      <- result {}
                <- user "[Request interrupted by user]"   <- turn/completed {status "interrupted"} (doc)
                <- result error_during_execution,
                   terminal_reason "aborted_streaming"
                the process stays up for the next turn
                     |                                         |
 exit           close stdin -> exits; the code varies     close stdin -> exit 0 (observed)
                (0 or 1 seen); `result` decides the turn  SIGTERM -> exit 0 (observed)
                SIGTERM -> 143, no result (doc)           SIGINT -> killed by the signal (observed)
                resume later: --resume <session_id>       resume later: thread/resume (not ephemeral)
```

## Claude Code

### The command line

```sh
# CONVERSATION: the conversation's private folder; MODEL: an alias or a full model name;
# SESSION_ID: the conversation's UUID. For a stored conversation, --resume replaces --session-id.
claude -p --bare \
  --input-format stream-json --output-format stream-json --verbose --include-partial-messages \
  --mcp-config "$CONVERSATION/claude-mcp.json" --strict-mcp-config \
  --tools "" --allowedTools "mcp__archforge__*" \
  --permission-mode dontAsk \
  --model "$MODEL" \
  --session-id "$SESSION_ID"
```

The tool server configuration in `$CONVERSATION/claude-mcp.json` (`MCP_CONFIG_FILE`), as the Claude adapter writes it
(`crates/agents/src/claude/mod.rs`; `HOME` and `XDG_CONFIG_HOME` are the person's own, the latter
only where it is set, as on some Linux systems):

```json
{
  "mcpServers": {
    "archforge": {
      "command": "/opt/archforge/bin/archforge-cli",
      "args": ["mcp", "--bridge", "47000", "--read-only", "--origin", "Chat 1"],
      "env": {
        "ANTHROPIC_API_KEY": "",
        "HOME": "/home/person",
        "XDG_CONFIG_HOME": "/home/person/.config"
      }
    }
  }
}
```

`--read-only` is left out only once "Allow changes to the model" is on; `--origin` labels the
conversation's changes in the history.

| Flag | What it does (observed unless marked) |
|---|---|
| `-p` | Non-interactive mode. Required by every flag below. |
| `--bare` | Skips hooks, plugins sync, CLAUDE.md discovery, auto-memory, keychain reads. Auth is only `ANTHROPIC_API_KEY` (or an `apiKeyHelper` via `--settings`); OAuth and the keychain are never read (`claude --help`). This is the "no user settings" mode. |
| `--input-format stream-json` | One JSON user message per stdin line. Refused without `--output-format stream-json`: `Error: --input-format=stream-json requires output-format=stream-json.`, exit 1. |
| `--output-format stream-json --verbose` | One JSON event per stdout line. Without `--verbose`: `Error: When using --print, --output-format=stream-json requires --verbose`, exit 1. |
| `--include-partial-messages` | Adds `stream_event` lines (raw model stream events) for live text. |
| `--mcp-config <file or JSON>` | The configuration above. Observed (the spike ran before `--read-only` and the `env` block were used, so without them): the server connected (`mcp_servers:[{"name":"archforge","status":"connected","source":"dynamic"}]`) and its 151 tools were listed. |
| `--strict-mcp-config` | Only the servers from `--mcp-config` (no project `.mcp.json`, no user servers). Use it: a stray project file is otherwise a second source. |
| `--tools ""` | Removes every built-in tool (Bash, Edit, Read, WebFetch, Task, …). Observed: with it the init `tools` list held only `mcp__archforge__*`; without it, bare mode listed `Bash`, `Edit`, `Read`. MCP tools are not affected (doc). |
| `--allowedTools "mcp__archforge__*"` | Pre-approves the server's tools so `dontAsk` does not deny them. The wildcard is safe because read-only is enforced by the bridge, not by this list: a conversation's server runs `archforge-cli mcp --bridge <port> --read-only` (AR-198, [Working with an agent](../manual/07-agents.md#read-only-sessions-and-labelled-changes)), which lists and allows only the reads and refuses every other tool with JSON-RPC error -32004 before the call reaches Archforge. Only "Allow changes to the model" drops `--read-only`, which means restarting the session with a new `--mcp-config` and `--resume`. |
| `--permission-mode dontAsk` | A call that would otherwise need approval is denied instead; calls covered by `--allowedTools` or allow rules, and calls that need no approval, still run (doc). Choices in 2.1.317: `acceptEdits, auto, bypassPermissions, manual, dontAsk, plan`; an unknown value exits 1 with the list. `--permission-prompts none` (2.1.259+) is the equivalent switch for who answers. |
| `--model` | An alias (`sonnet`, `opus`, `haiku`, `fable`) or a full name; the init event names the resolved model (`sonnet` resolved to `claude-sonnet-5-5`). The model is **not** remembered across `--resume`: a resumed run without `--model` used the default model, so pass it on every spawn. `--fallback-model` takes a list (doc). |
| `--session-id <uuid>` | Fixes the conversation id; a non-UUID exits 1 with `Error: Invalid session ID. Must be a valid UUID.` |
| `--resume <uuid>` | Continues a stored conversation under the same `session_id` (observed). An unknown id: stderr `No conversation found with session ID: …` and a `result` with `subtype:"error_during_execution"`, `errors:[…]`, exit 1. |
| `--no-session-persistence` | Nothing is written under the config folder, so the conversation cannot be resumed (observed). |
| `--restricted` | Not needed with `--tools ""`; it removes code-running tools and ignores settings files, but keeps the file tools. |

Terms and naming (doc, [Agent SDK overview](https://code.claude.com/docs/en/agent-sdk/overview)):
products built on it fall under the Commercial Terms; third-party apps may not offer claude.ai
sign-in or its rate limits unless approved, so API-key only, as decided. The interface may say
"Claude Agent" or "Powered by Claude", not "Claude Code".

`--max-budget-usd` (print mode) is a possible cost guard; `--setting-sources` and `--settings` are
not needed with `--bare`.

### Environment and isolation

- **The key**: `ANTHROPIC_API_KEY` in the child's environment. The init event reports
  `apiKeySource:"ANTHROPIC_API_KEY"` when it is present and `"none"` when it is not (observed). In
  `-p` the key is always used when present (doc,
  [authentication](https://code.claude.com/docs/en/authentication)).
- **The key reaches the MCP server too (observed)**: Claude passes its whole environment to the
  stdio MCP server it starts, plus the server's `env` block. With the canary key set, the
  `archforge-cli` child saw `ANTHROPIC_API_KEY`, `ANTHROPIC_BASE_URL`, `CLAUDE_CONFIG_DIR`,
  `CLAUDE_CODE_SESSION_ID`, `CLAUDE_PROJECT_DIR`, `CLAUDECODE`, `HOME`, `PATH`, `PWD`. The bridge
  does not log its environment; the Claude adapter blanks the key in the server's `env` block
  (`"ANTHROPIC_API_KEY": ""`), and whether that keeps it out entirely is still to verify with a
  real round trip.
- **`CLAUDE_CONFIG_DIR`** moves the whole settings tree. After the runs the private folder held
  `.claude.json`, `backups/`, `plugins/store/…` and `projects/<cwd with / as ->/<session-id>.jsonl`,
  the transcripts `--resume` reads. The person's own folder is never touched.
- **`HOME`**: `claude` itself runs with a private `HOME` (the adapter gives each conversation its
  own home, temporary and config folders), but the bridge must see the person's real one:
  `archforge-cli` finds the control token in the platform settings folder it derives from `HOME`
  (or `XDG_CONFIG_HOME` on Linux), and Claude passes its own environment on to the server. So the
  server's `env` block sets `HOME` to the person's home folder, and `XDG_CONFIG_HOME` to theirs
  where it is set (the example above). Without that override the bridge connects and lists its
  tools, but every call fails to find the token. The adapter refuses to start a session when the
  person's home folder is unknown. Once the bridge takes the token file or settings folder as an
  argument (AR-203), the override goes.
- **Working directory**: give each conversation a private empty folder. One of the startup probes
  run inside the repository checkout began connecting the repository's `.mcp.json` server before it
  shut down (an `mcp-logs-archforge` log appeared: "Skipping connection (Claude Code is shutting
  down)"); which probe, bare or not, was not pinned down. `--strict-mcp-config` and a private
  working folder close that door either way.
- **Traffic**: `CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1` turns off updates, telemetry, error
  reports and feature-flag fetches (doc, [environment
  variables](https://code.claude.com/docs/en/env-vars)); the init event then says
  `analytics_disabled:true`.

### stdin

```json
{"type":"user","message":{"role":"user","content":"Which entities are in Shop?"},"parent_tool_use_id":null}
```

Content may also be an array of content blocks (doc,
[streaming input](https://code.claude.com/docs/en/agent-sdk/streaming-vs-single-mode)). A line that
is not JSON is reported on stderr (`Error parsing streaming input line (type=unknown, 15 chars):
SyntaxError`) and skipped; the process goes on (observed).

The SDK control protocol works on the same stdin (observed, not in the public docs):

```json
{"type":"control_request","request_id":"req-1","request":{"subtype":"interrupt"}}
```

answered by `{"type":"control_response","response":{"subtype":"success","request_id":"req-1","response":{"still_queued":[]}}}`.

### stdout events

| `type` / `subtype` | When | Fields the adapter reads |
|---|---|---|
| `system` / `ui_invalidate` | First line after start (observed, undocumented) | ignore |
| `system` / `init` | After the **first user line**, not at start (observed) | `session_id`, `model`, `tools[]`, `mcp_servers[]{name,status}` (`pending/connected/failed/needs-auth/disabled`), `permissionMode`, `apiKeySource`, `claude_code_version`, `capabilities[]` (includes `interrupt_receipt_v1`) |
| `system` / `status` | `status:"requesting"` before each model request (observed) | progress |
| `stream_event` | With `--include-partial-messages`: `event` is a raw stream event (`message_start`, `content_block_start/delta/stop` with `text_delta` or `input_json_delta`, `message_delta`, `message_stop`) (doc) | live text, tool input |
| `assistant` | A complete model message: `message.content[]` with `text` and `tool_use{id,name,input}` blocks | text, tool calls (`name` is `mcp__archforge__<tool>`) |
| `assistant` with `error` | A synthetic message (`message.model:"<synthetic>"`, `is_api_error_message:true`) carrying `error` (category, below) and sometimes `api_error` (observed `connection_failed`) | error category |
| `user` | `message.content[]` with `tool_result{tool_use_id, content, is_error?}`; also `[Request interrupted by user]` after a stop | tool results |
| `system` / `api_retry` | Before each retry: `attempt`, `max_retries` (10), `retry_delay_ms`, `error_status` (HTTP status or null), `error` (category) (observed) | "retrying n of 10" |
| `rate_limit_event` | `rate_limit_info{status allowed/allowed_warning/rejected, resetsAt}` (doc) | rate limit |
| `result` | Once per user line: `subtype` (`success`, `error_during_execution`, `error_max_turns`, `error_max_budget_usd`, …), `is_error`, `result` (text), `errors[]`, `terminal_reason` (observed `api_error`, `aborted_streaming`), `api_error_status`, `api_error`, `total_cost_usd`, `usage`, `permission_denials[]`, `session_id` | end of turn |
| `control_response` | Answer to a `control_request` | interrupt acknowledged |

Two traps, both observed:

- `result.subtype` is `"success"` even when the turn failed on the API: the "not logged in" and the
  "connection refused" results both had `subtype:"success"` with `is_error:true` and
  `terminal_reason:"api_error"`. Decide on `is_error` and the categories, never on `subtype` alone.
- The error **category** fields are the contract; the texts (`Not logged in · Please run /login`,
  `API Error: Connection refused — a firewall or proxy may be blocking it (ECONNREFUSED)`) are for
  logs only.

The categories in `assistant.error` and `api_retry.error` (doc,
[headless](https://code.claude.com/docs/en/headless)): `authentication_failed`,
`oauth_org_not_allowed`, `account_on_hold`, `billing_error`, `rate_limit`, `overloaded`,
`invalid_request`, `model_not_found`, `server_error`, `max_output_tokens`,
`cloud_credential_error`, `unknown`.

### Retries, stop and exit (observed)

- With the API unreachable, Claude retried 10 times with growing delays (0.5 s, 1.2 s, 2.3 s, 4.9 s,
  9 s, 19 s, then about 34 s each), every `api_retry` with `error:"unknown"` and
  `error_status:null`, and gave up after **172 s** with `assistant.error:"server_error"`,
  `api_error:"connection_failed"`. The Chat panel should show the retries and offer Stop rather
  than wait silently.
- **SIGINT** during a turn: within 11 ms a `user` line `[Request interrupted by user]` and a
  `result` with `subtype:"error_during_execution"`, `terminal_reason:"aborted_streaming"`. The
  process stays up and takes the next user line. Signal the `claude` pid only, never its process
  group, so the MCP server is not hit.
- **`control_request` interrupt**: the same two lines, preceded by the `control_response`. With no
  turn running it is answered and nothing else happens.
- **Exit**: closing stdin ends the process. Exit code 1 followed a run whose last result was an
  error (keyless, interrupt by control request), but the SIGINT run exited 0 although its last result
  was also an error. The exit code is not a turn outcome; the `result` lines are. An exit without a
  final `result` for an open turn is a crash (category Other).

## Codex app-server

### Starting it

`cli_auth_credentials_store` is `"keyring"` for a ChatGPT sign-in and `"ephemeral"` for an API key
(see [Accounts and sign-in](#accounts-and-sign-in)); `CODEX_HOME_DIR` is the private folder.

```sh
CODEX_HOME="$CODEX_HOME_DIR" codex app-server --strict-config \
  -c cli_auth_credentials_store='"keyring"' \
  -c history.persistence='"none"' -c analytics.enabled=false \
  -c web_search='"disabled"' \
  --disable shell_tool --disable unified_exec --disable apps --disable plugins \
  --disable multi_agent --disable view_image --disable image_generation \
  --disable browser_use --disable computer_use --disable goals --disable hooks \
  --disable skill_search --disable tool_suggest --disable sleep_tool
```

The Archforge server goes in `$CODEX_HOME_DIR/config.toml` (observed working without `--read-only`,
`--origin` and `env`, which came later):

```toml
[mcp_servers.archforge]
command = "/opt/archforge/bin/archforge-cli"
# --read-only is left out only after "Allow changes to the model".
args = ["mcp", "--bridge", "47000", "--read-only", "--origin", "Chat 1"]
# The person's own folders, so the bridge finds the control token (XDG_CONFIG_HOME only where set).
env = { HOME = "/home/person", XDG_CONFIG_HOME = "/home/person/.config" }
startup_timeout_sec = 10
tool_timeout_sec = 60
```

Which environment Codex passes to a stdio server by default is not documented, so the `env` table
names the folders explicitly, as the Claude configuration does.

Further keys (doc, [config reference](https://learn.chatgpt.com/docs/config-file/config-reference)):
`env`, `env_vars`, `cwd`, `enabled`, `enabled_tools`, `disabled_tools`, `required`, and
`default_tools_approval_mode` / `tools.<tool>.approval_mode` (`auto`, `prompt`, `writes`,
`approve`; only `writes`, "prompts for tools that aren't marked read-only", is described).

`--strict-config` matters: without it an unknown `-c` key is accepted silently (observed:
`-c tools.apply_patch=false` and `-c include_apply_patch_tool=false` were taken and changed
nothing). With it, `codex app-server` refuses the same key before reading stdin: stderr
`Error: unknown configuration field \`tools.apply_patch\` in -c/--config override`, exit 1. The
command line above passes under `--strict-config` and answers the handshake (observed).
`codex debug` refuses the flag itself.

### The wire

JSON-RPC 2.0 over stdio, one JSON object per line, **without the `"jsonrpc"` field** (observed, and
[app-server](https://learn.chatgpt.com/docs/app-server)). Notifications carry `emittedAtMs`.

Observed in 0.166.0:

- A request before `initialize`: `{"error":{"code":-32600,"message":"Not initialized"},"id":0}`.
- A second `initialize`: `{"error":{"code":-32600,"message":"Already initialized"},"id":2}`.
- An unknown method: code **-32600** (not -32601), message `Invalid request: unknown variant
  \`no/such/method\`, expected one of …` with the full method list. A method an older Codex lacks
  arrives the same way, which is how "too old" shows at run time.
- A line that is not JSON: no answer at all, only a stderr log line
  `ERROR codex_app_server_transport::transport: Failed to deserialize JSONRPCMessage`. Every request
  therefore needs its own timeout.
- `turn/interrupt` for an unknown thread: `{"code":-32600,"message":"thread not found: …"}`.
- `account/rateLimits/read` without an account: `-32600 codex account authentication required to
  read rate limits`.
- stderr carries tracing logs with colour escapes even in a cleared environment; read it for the
  log only, never parse it for state.
- Responses carry fields the stable schema does not list (`workspaceRouting`, `environments`,
  `runtimeWorkspaceRoots`); the parser must ignore unknown fields.

### Handshake (observed)

```jsonl
{"id":1,"method":"initialize","params":{"clientInfo":{"name":"archforge","title":"Archforge","version":"0.13.0"},"capabilities":{"experimentalApi":false,"requestAttestation":false}}}
{"id":1,"result":{"userAgent":"archforge/0.166.0 (Mac OS 27.2.0; arm64) unknown (archforge; 0.13.0)","codexHome":"/tmp/archforge-agent/codex-home","platformFamily":"unix","platformOs":"macos"}}
{"method":"initialized"}
```

The `userAgent` carries the Codex version after `clientInfo.name/`, a second version check that
needs no extra process. A `remoteControl/status/changed` notification follows `initialize`; it
names the computer's host name and an installation id (sanitised in the fixtures).

### Accounts and sign-in

- `account/read` with nothing stored: `{"account":null,"requiresOpenaiAuth":true}` (observed).
- **An API key in the environment is not read** (observed): with `OPENAI_API_KEY` or
  `CODEX_API_KEY` set, `account/read` still answered `account:null`. The documented way is
  `account/login/start {"type":"apiKey","apiKey":"…"}`, which "stores it for API requests" (doc).
  The owner accepted this route with the ephemeral store on 2026-11-06 (owner question 2).
- ChatGPT: `account/login/start {"type":"chatgpt"}` returns `{loginId, authUrl}`; the app opens
  `authUrl`, Codex runs the local callback (port 1455) and sends `account/login/completed
  {loginId, success, error}` and `account/updated {authMode, planType}` (doc).
  `{"type":"chatgptDeviceCode"}` returns `{loginId, verificationUrl, userCode}` for a sign-in
  without a callback (doc). `account/login/cancel {loginId}` and `account/logout` complete the set.
  `chatgptAuthTokens` (the host supplies tokens) needs `experimentalApi` and is the route the "Sign
  in with ChatGPT" developer programme uses.
- **Credential store** (`cli_auth_credentials_store`, doc,
  [auth](https://learn.chatgpt.com/docs/auth)): `file` (`CODEX_HOME/auth.json`), `keyring` (the
  system store; fails when unavailable), `auto` (keyring, else file), `ephemeral` (in memory for
  this process only). With a private `CODEX_HOME` and `keyring`, ChatGPT tokens stay in Codex's own
  keyring entry, as decided. `forced_login_method = "chatgpt" | "api"` pins one mode.
- **Rate limits**: `account/rateLimits/read` returns `{ordinaryUsageAllowed, rateLimits{primary,
  secondary{usedPercent, windowDurationMins, resetsAt}, credits, planType, rateLimitReachedType},
  rateLimitsByLimitId}`; `account/rateLimits/updated` sends sparse updates to merge (schema).
- **Models**: `model/list` answers without an account (observed) with `{data[{id, model,
  displayName, description, hidden, isDefault, supportedReasoningEfforts, defaultReasoningEffort,
  inputModalities, …}], nextCursor}`; the list reflects the bundled catalogue, and what an account
  may actually use shows only on a turn.

### Threads and turns

- `thread/start {cwd, approvalPolicy, sandbox, ephemeral, developerInstructions, model, config}`
  answered within 0.1 s with the thread (`id`, `ephemeral:true`, `model`, `status:{type:"idle"}`,
  `path:null`) and `thread/started`, then `mcpServer/startupStatus/updated` `starting` → `ready`
  for `archforge` 0.2 s later (observed). `mcpServerStatus/list` then lists the server's tools with
  their annotations. A `failed` status carries `error` and `failureReason`.
- **`thread/start` already goes on the network** (observed): even with no account it tried to open
  the Responses WebSocket (`failed to connect to Responses WebSocket` on stderr). Opening Settings
  must not start a thread.
- `turn/start {threadId, input:[{"type":"text","text":"…","text_elements":[]}], model, effort}`;
  then `turn/started`, `item/started`/`item/completed` for `userMessage`, `mcpToolCall{server, tool,
  arguments, status, result, error, readOnlyHint}` and `agentMessage`, `item/agentMessage/delta
  {itemId, delta}`, and `turn/completed {turn{status: completed | interrupted | failed, error}}`
  (doc and schema; see the doc-derived fixtures).
- Turn snapshots, per the public source (`codex-rs/app-server`, `turn_processor.rs` and
  `bespoke_event_handling.rs`): the `turn/start` response has `startedAt`, `completedAt` and
  `durationMs` null, no items and `itemsView:"notLoaded"`; `turn/started` carries `startedAt`
  (seconds); `turn/completed` carries the times and, when the turn produced a final agent message,
  that one item with `itemsView:"summary"`, otherwise no items and `"notLoaded"`. Item times are in
  milliseconds in two places: `item/started` carries `startedAtMs` and `item/completed` carries
  `completedAtMs` (both required), and every `ThreadItem` also carries its own `startedAtMs` and
  `completedAtMs`, each `null` when not recorded (in the generated schema they sit beside the
  variants, at the top level of `ThreadItem`; in the source the payload is flattened into a struct
  with the two times). An item in `item/started` has its start and `completedAtMs: null`; in
  `item/completed` and in a turn's summary it has both.
- `turn/interrupt {threadId, turnId}` → `{}` and `turn/completed` with `status:"interrupted"` (doc).
- `error {error{message, codexErrorInfo}, willRetry, threadId, turnId}`: with `willRetry:true` the
  turn goes on (show "retrying"); otherwise a failed `turn/completed` follows (doc).
- `thread/resume {threadId}` reopens a stored thread; an `ephemeral` thread is gone with the
  process. `thread/fork` with `ephemeral` copies one in memory.

### Approvals and tools

- Server requests: `item/commandExecution/requestApproval`, `item/fileChange/requestApproval`,
  `item/permissions/requestApproval`, `mcpServer/elicitation/request`, `item/tool/requestUserInput`.
  Archforge answers `{"decision":"decline"}` (commands, files), `{"action":"decline","content":null,
  "_meta":null}` (elicitations) and an empty grant for permissions, unless the person allowed it;
  `serverRequest/resolved` follows. An unanswered request blocks the turn, so the adapter answers
  every request it does not understand with a decline or a JSON-RPC error.
- **What the model sees** (`codex debug prompt-input`, offline, observed): by default the
  top-level tools are `exec` (a JavaScript "code mode" that calls nested tools: `exec_command`,
  `write_stdin`, `apply_patch`, `view_image`, the MCP resource tools and every MCP tool),
  `wait`, `request_user_input`, `sleep` and six multi-agent `collaboration.*` tools. With the
  switches above, `exec_command`, `write_stdin`, `view_image`, `sleep` and the collaboration tools
  are gone, but **`apply_patch` stays** (no switch found). The read-only sandbox and the declined
  file-change approvals are what keep it from writing; Archforge's own `--read-only` bridge (AR-198)
  keeps the model safe either way.
- `sandbox: "read-only"` gives `{"type":"readOnly","networkAccess":false}` (observed).
  `approvalPolicy`: `untrusted`, `on-request`, `never`, or `granular{…}` (schema); the docs call
  `untrusted` retired, so use `on-request` and decline.

### What `CODEX_HOME` collects

After one ephemeral thread the private folder held `config.toml`, `installation_id`,
`state_5.sqlite`, `logs_2.sqlite`, `memories_1.sqlite`, `goals_1.sqlite`, `queue_1.sqlite`,
`shell_snapshots/`, `skills/.system/`, `.tmp/plugins*` (a plugin catalogue sync, which is network
traffic; `--disable plugins` should stop it) and `tmp/arg0-macos/` with `apply_patch` helpers.
Keep one private `CODEX_HOME` per person (it holds the keyring reference and logs), not per
conversation.

### The schema

`codex app-server generate-json-schema --out <dir>` and `generate-ts --out <dir>` work offline
(observed, exit 0, 4.5 MB of schema and 3.1 MB of TypeScript); `--experimental` adds the
experimental methods. The full schema is not kept in the repository. To regenerate the one these
fixtures were checked against, with codex-cli **0.166.0** installed:

```sh
codex --version                                    # codex-cli 0.166.0
SCHEMA_DIR="${SCHEMA_DIR:-$(mktemp -d)}"           # keeps a folder already set, else a new temporary one
codex app-server generate-json-schema --out "$SCHEMA_DIR"
echo "$SCHEMA_DIR/codex_app_server_protocol.schemas.json"
```

The bundle lands in that folder, `$SCHEMA_DIR/codex_app_server_protocol.schemas.json` (the last
line prints its path), beside one file per message type and the `v1/` and `v2/` folders. It is
the whole bundle (v1 and v2 definitions, non-experimental, about 750 KB). The stable protocol has 106 client requests, 1 client notification
(`initialized`), 10 server requests and 85 server notifications. Every observed message and every
doc-derived Codex fixture validated against it (checked with `jsonschema`, Draft 7, requests and
notifications by kind, responses by method). The Codex adapter's tests use a small, credited cut
of that schema in `crates/agents` once its pull request (#193) lands. The docs say the generated
output is specific to the Codex version; the subcommands are marked experimental.

## Versions

| | Claude Code | Codex |
|---|---|---|
| Read the version | `claude --version` → `2.1.317 (Claude Code)`, exit 0; also `claude_code_version` in `system/init` | `codex --version` → `codex-cli 0.166.0`, exit 0; also `userAgent` in the `initialize` result |
| Needed features and where they appeared | stream-json output 0.2.66, `--mcp-config` 0.2.75, `--disallowedTools` 0.2.82, `--replay-user-messages` 1.0.86, `--include-partial-messages` 1.0.109, `--bare` 2.1.81 with full enforcement (only CLI MCP servers) from 2.1.286, interrupt receipt 2.1.205, `--permission-prompts` 2.1.259, `api_retry.no_response` 2.1.261 (doc, changelog and reference pages) | The schema is version-specific; when `chatgptDeviceCode`, `ephemeral` threads and `model/list` first appeared was not found |
| Recommended range | **>= 2.1.286 and < 3.0.0**; tested 2.1.317. Below the floor: MissingRuntime ("update Claude Code"). Newer 2.x: accepted with a warning (owner, 2026-11-06) | **>= 0.166.0 and < 1.0.0**; tested 0.166.0. Contract tests run against the credited schema cut in `crates/agents` (#193); each tested bump regenerates it. Newer 0.x: accepted with a warning (owner, 2026-11-06), since unknown methods fail loudly (-32600) |
| Missing | spawn fails with "not found" → MissingRuntime | the same |
| Too old at run time | The version check stops it first; a `claude` that still refuses an option prints `error: unknown option '…'` on stderr and exits 1 before any event, which the adapter reports as an exit (Other) | `-32600 unknown variant` for a needed method → MissingRuntime |
| stderr | Usage errors and input-parse errors, plain text | Tracing logs with ANSI colours; malformed-input reports |

## Error mapping

This is what the adapters do: Claude in `crates/agents/src/claude/parse.rs` (`ClaudeParser::result`,
`failure`, `kind_of`) and Codex in the Codex adapter's `crates/agents/src/codex/parse.rs`
(`turn_completed`, `kind_of`, #193). A turn ends **completed**, **stopped** or **failed**; stopped is
not an error, and only a failed turn gets a neutral category. Retries (`api_retry`, an `error`
notification with `willRetry:true`) are progress: shown, never a failure by themselves.

**Claude**, at the turn's `result`, the first rule that applies:

1. `is_error:false`: completed.
2. `terminal_reason:"aborted_streaming"` (after SIGINT or the interrupt control request; its
   `subtype` is `error_during_execution`): stopped, **even when a category was recorded earlier in
   the turn**.
3. A stop was asked for and no category was recorded in the turn: stopped.
4. `is_error` missing or not a boolean: failed, Other.
5. Otherwise failed, with the first of: (a) the category recorded in the turn, from the last
   `assistant` message with `error` (through the category list below, with that message's
   `api_error`) or a `rate_limit_event` with `status:"rejected"` (RateLimited), whichever came last;
   (b) the result's `api_error:"connection_failed"`: Network; (c) the result's `api_error_status`:
   401 or 403 InvalidCredential, 429 RateLimited, 404 or 529 ModelUnavailable; (d) the kind of the
   turn's last `api_retry` (no `error_status` and category `unknown`: Network; otherwise its
   category, else its `error_status` as in (c)); (e) Other.

The category list: `authentication_failed` and `cloud_credential_error` are InvalidCredential;
`rate_limit` and `billing_error` RateLimited; `model_not_found` and `overloaded` ModelUnavailable;
`oauth_org_not_allowed`, `account_on_hold`, `invalid_request` and `max_output_tokens` Other. Any
other category (`server_error`, `unknown`, …) is Network when its `api_error` is
`connection_failed`, else its HTTP status as in (c) where it has one, else Other. So the order is:
category, then `connection_failed`, then HTTP status, then the last retry, then Other. If `claude`
ends while a turn is open, the turn fails with the end's error (an exit or a crash: Other).

**Codex**, at `turn/completed`: `status:"completed"` is completed; `"interrupted"` is stopped;
`"failed"` fails with the category of the turn's own `error`, else of the last `error` notification
with `willRetry:false`; with neither it is stopped when a stop was asked for, else Other; any other
status fails as Other. A category comes from `codexErrorInfo`: its name first, and an HTTP status
only inside the connection failures (below); anything not listed is Other. Before a turn, a sign-in
problem fails the turn when it is sent (below).

| Neutral category | Claude | Codex |
|---|---|---|
| **MissingRuntime** | Not found, not executable, or its location not safe; `--version` fails, times out or cannot be read; the version is below the floor or at or past the first unsupported major version | The same for `codex`; `initialize` refused, or the handshake's version below the floor or past the supported range; a request Archforge needs answered as an unknown method (`-32600 unknown variant`) |
| **InvalidCredential** | Category `authentication_failed` or `cloud_credential_error`; else HTTP status 401 or 403 (rule 5c or a retry's status). A missing key shows as `authentication_failed` with `apiKeySource:"none"` in `init` (observed, `Not logged in`) | `unauthorized` when signed in with an API key, or a connection failure with HTTP status 401 or 403 then; `account/login/start` refused; `account/login/completed` with `success:false`; `account/read` with `account:null`; `account/updated` with `authMode:null` for an API key |
| **SignInExpired** | Not used (API key only) | `unauthorized` when signed in with ChatGPT, or a connection failure with HTTP status 401 or 403 then; `account/updated` with `authMode:null` for a ChatGPT sign-in |
| **RateLimited** | Category `rate_limit` or `billing_error`; a `rate_limit_event` with `status:"rejected"`; else HTTP status 429 | `usageLimitExceeded`, `rateLimitExceeded`, `sessionBudgetExceeded`; a connection failure with HTTP status 429 |
| **Network** | `api_error:"connection_failed"` (observed after the retries ran out); else a last `api_retry` with no `error_status` and category `unknown` (observed) | `httpConnectionFailed`, `responseStreamConnectionFailed`, `responseStreamDisconnected`, `responseTooManyFailedAttempts` with no HTTP status or another one than 401, 403, 404 or 429 |
| **ModelUnavailable** | Category `model_not_found` or `overloaded`; else HTTP status 404 or 529 | `modelNotFound`, `serverOverloaded`, `flexUnavailable`, `policyCapacityUnavailable`; a connection failure with HTTP status 404 |
| **Other** | Category `oauth_org_not_allowed`, `account_on_hold`, `invalid_request` or `max_output_tokens`; any other category, or a failed `result` without one, that none of `connection_failed`, a mapped HTTP status (401, 403, 404, 429, 529) or the last retry places; `is_error` missing; `claude` exiting or crashing with a turn open | Every other `codexErrorInfo` (`badRequest`, `internalServerError`, `contextWindowExceeded`, `sandboxError`, `cyberPolicy`, `misalignmentPolicyViolation`, `tooManyDenials`, `other`, …); a failed turn with no error and no stop asked for; an unknown status; a request not answered in time; a refused request other than the ones above; `codex` ending with a turn open |

Not mapped to a category by either adapter: an unreadable output line (skipped and counted);
`account/rateLimits/read`, which the Codex adapter reads for Settings as limits, not as a failure;
and `account/rateLimits/updated` and `modelProvider/authRecoveryStarted` / `authRecoveryCompleted`,
which a Codex session knows and passes over.

## Fixtures

`fixtures/` holds transcripts for the adapter tests: one JSON object per line,
`{"t": seconds since start, "dir": …, "line": …}`, where `dir` is one of:

| `dir` | Meaning | `line` |
|---|---|---|
| `in` | One line written to the child's stdin | the raw line, without its line break |
| `out` | One line the child wrote to stdout | the raw line |
| `err` | One line the child wrote to stderr | the raw line |
| `close` | The recorder closed the child's stdin (end of input); nothing is written | `""` |
| `sig` | The recorder sent the child a signal | its name, such as `SIGINT` |
| `exit` | The child ended | its exit code, or `-N` when signal N ended it |
| `note` | Not part of the run: what the file is and how it was made | free text |

For `in`, `out` and `err`, `line` is exactly what crossed the pipe, so malformed input is
representable; a replay writes `in` lines, and closes stdin at a `close` record. Files named
`observed-*` were recorded from the installed programs and sanitised (paths replaced by
`/tmp/archforge-agent/…` and `/opt/archforge/bin/archforge-cli`, the host name by `host.example`,
the installation id zeroed, colour escapes removed, Codex tool tables cut to two entries, and in
`codex/observed-handshake.jsonl` the provider's wording in `model/list` (every model's
`description`, `availabilityNux.message`, `upgradeInfo.migrationMarkdown`,
`supportedReasoningEfforts[].description` and `serviceTiers[].name` and `.description`) replaced by
neutral "Fixture text: …" strings with every field kept; that file's first `note` line lists its changes).
Files named `doc-*` are written from the documentation and the schema and start with a `note` line
saying so; `synthetic-*` are made-up hostile lines.

The doc-derived tool calls use the bridge's real shapes. `er_entity_list` takes the schema's id, a
UUID, and a command tool's `tools/call` answer is one text block holding the control channel's
`engine.execute` answer as JSON, `{"result": <the command's value>, "undo": <the label of the
newest undo step, or null>}`, with `isError: false` (`crates/mcp/src/lib.rs`, `call`;
`crates/ui-egui/src/control.rs`, `engine.execute`). For `er_entity_list` the value is a list of
`{"id": <uuid>, "name": …}`.

| File | Shows |
|---|---|
| `claude/observed-argument-errors.txt` | Usage errors and the keyless runs, with exit codes |
| `claude/observed-keyless-turn.jsonl` | Full command line without a key: `ui_invalidate`, `init` (Archforge tools only), `authentication_failed`, an idle interrupt, a malformed stdin line |
| `claude/observed-network-retry-then-interrupt.jsonl` | `api_retry` x4, interrupt by control request, `aborted_streaming` |
| `claude/observed-network-retry-then-sigint.jsonl` | The same stopped by SIGINT |
| `claude/observed-network-retries-exhausted.jsonl` | 10 retries over 172 s, then `server_error` / `connection_failed` |
| `claude/observed-resume-keyless.jsonl`, `observed-resume-unknown-session.jsonl`, `observed-no-session-persistence.jsonl` | Resume and persistence |
| `claude/doc-turn-with-mcp-tool.jsonl` | Partial text, a tool call and result, the answer, a success result |
| `claude/doc-permission-denied.jsonl` | A denied tool in `dontAsk` |
| `claude/doc-api-errors.jsonl` | 401, 429 with retries and `rate_limit_event`, billing, 404 model, 529 |
| `codex/observed-handshake.jsonl` | Not initialized, malformed line, initialize, account, model list, error probes |
| `codex/observed-thread-start-with-mcp.jsonl` | `config/read`, ephemeral `thread/start`, MCP server `starting` → `ready`, `mcpServerStatus/list` |
| `codex/observed-env-api-key-not-read.jsonl`, `observed-ephemeral-store-account-read.jsonl` | `account:null` despite `OPENAI_API_KEY` in the environment; the ephemeral store |
| `codex/observed-sigint.jsonl`, `observed-sigterm.jsonl` | Exit on signals |
| `codex/observed-recommended-flags-handshake.jsonl`, `observed-strict-config-unknown-key.jsonl` | The recommended command line under `--strict-config`; an unknown key refused |
| `codex/doc-*.jsonl` | A turn with a tool call, interrupt, declined approvals, API-key, browser and device-code sign-in, logout, rate limits, and six failed turns, one per terminal error category |
| `synthetic-hostile-lines.jsonl` | Lines both parsers must survive |

The Claude adapter (AR-201) copied the Claude fixtures and the hostile lines into
`crates/agents/tests/fixtures/`, where its tests read them; a change to one copy goes to the other in
the same commit. AR-202 does the same for the Codex fixtures.

## To verify with a real round trip (needs the owner's OK)

Each needs a real key or sign-in and costs a little.

1. Claude: a full turn with a real key and the bridge to a running Archforge: event order, partial
   text, the `mcp__archforge__*` tool call and result, `total_cost_usd`; and that the tools list the
   model receives holds only the Archforge tools.
2. Claude: whether `init` comes once per process or once per user line in stream-json input.
3. Claude: real 401 (invalid key), 429 and 529 shapes; the exact `error`/`api_error` values.
4. Claude: whether `"env":{"ANTHROPIC_API_KEY":""}` in the MCP config keeps the key out of the
   bridge's environment.
5. Claude: `--resume` of a conversation that had tool calls, with a different `--mcp-config`
   (for switching "Allow changes" on).
6. Codex: API-key sign-in with `cli_auth_credentials_store="ephemeral"`, then a turn; whether
   `auth.json` stays absent.
7. Codex: ChatGPT sign-in (browser and device code) with `keyring` in a private `CODEX_HOME`, sign
   out, and what an expired sign-in looks like.
8. Codex: a turn with the bridge: `mcpToolCall` items, whether MCP tool calls raise an elicitation
   under `on-request`, and what `default_tools_approval_mode` changes.
9. Codex: `turn/interrupt` timing and the final `turn/completed`; `error` with `willRetry`.
10. Codex: `account/rateLimits/read` for a ChatGPT account and for an API key.
11. Codex: that the switched-off tools stay off in a real request and that `apply_patch` is refused
    under the read-only sandbox.

## Owner questions

Questions 1, 2, 3 and 5 were decided by the owner on 2026-11-06; 4 and 6 are still open.

1. **Codex "Sign in with ChatGPT" in Archforge. Decided (2026-11-06): offered in 0.13.0, as an
   open-source local app.** Background: the app-server documentation says app-server
   authentication "has never been permitted for commercial or hosted services"; local or
   open-source applications built on it may continue, and OpenAI recommends migrating to its "Sign
   in with ChatGPT" programme ([app-server, Auth endpoints](https://learn.chatgpt.com/docs/app-server)).
   The programme's plan usage is for open-source projects, personal projects that run locally and
   selected private apps; paid or hosted apps join a waitlist
   ([cookbook](https://developers.openai.com/cookbook/articles/sign-in-with-chatgpt),
   [developer docs](https://developers.openai.com/siwc),
   [app-server token sharing](https://developers.openai.com/siwc/token-sharing-open-source/codex-app-server),
   [waitlist](https://openai.com/form/sign-in-with-chatgpt-interest/)). Also relevant: the
   [authentication overview](https://learn.chatgpt.com/docs/auth) and the
   [Terms of Use](https://openai.com/policies/terms-of-use/), whose clause against sharing account
   credentials or making an account available to others was seen only in search results; the
   policy pages refused automated reading. Archforge is open source (MIT or Apache-2.0) and runs
   locally.
2. **How the OpenAI API key reaches Codex. Decided (2026-11-06): over stdin with
   `account/login/start {"type":"apiKey"}` and `cli_auth_credentials_store="ephemeral"`, accepted in
   place of "only through the environment".** Background: the app-server does not read
   `OPENAI_API_KEY` or `CODEX_API_KEY` from its environment (observed); the pipe is private to the
   child, nothing is written to a file and no argument is visible to other processes. The
   alternative, a custom model provider whose `env_key` names an environment variable, is not
   pursued.
3. **Runtime transcripts. Decided (2026-11-06): kept in a private folder per conversation and
   deleted with the conversation.** Background: `--resume` needs Claude's transcript files, which
   land under `CLAUDE_CONFIG_DIR/projects/<working folder>/`, so a private working folder per
   conversation keeps them apart; a non-ephemeral Codex thread writes its history under
   `CODEX_HOME`, where the person's sign-in also lives, so AR-202 settles how a thread's history is
   kept per conversation without splitting the sign-in.
4. **Codex `apply_patch`. Open.** It cannot be switched off; the read-only sandbox and declined
   approvals hold it back. Acceptable, given the bridge's own read-only mode (AR-198)?
5. **Version policy. Decided (2026-11-06): newer runtime versions than the tested ones are accepted
   with a warning within the same major version** (Claude Code 2.x, Codex 0.x, as in
   [Versions](#versions)); a new major version is refused until tested.
6. **The bridge's token lookup. Open.** Let `archforge-cli mcp` take the token file (or settings
   folder) as an argument so the children's `HOME` stops mattering, in AR-198 or AR-203?
