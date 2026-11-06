# archforge-agents

**Purpose.** The agent runtimes a person installs beside Archforge, Claude Code (`claude`) and OpenAI
Codex (`codex`): found on this computer, checked against the versions Archforge accepts, and run as
supervised child processes in private folders. Archforge never bundles them. The adapters that
speak each runtime's protocol (AR-201, AR-202) build on this crate; the AR-196 spike's note records
how the runtimes behave.

**Layer.** 10, a host service above `secrets`, whose `Secret` carries the keys: it starts processes
and makes folders, so the shell never depends on it and it is never built for the portable target.

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
| `SpawnRequest`, `Limits` | `args`, `vars`, `secret_vars: Vec<(String, Secret)>`, `limits`; `max_line_bytes` (8 MiB), `stderr_tail_bytes` (16 KiB), `queued_lines` (16), each refused at zero or past its maximum, and `max_line_bytes × (queued_lines + 1)` refused past `MAX_BUFFERED_BYTES` (256 MiB) |
| `RuntimeProcess` | `send_line` (never waits), `close_input`, `next_event(timeout)`, `try_next_event`, `interrupt`, `status`, `shutdown(grace)` (waits up to twice `grace`, so on a worker), `stderr_tail`, `exit_error(Exit)`, `pid`; kills its group on drop |
| `OutputEvent`, `Dropped`, `Exit` | `Line`, `Dropped(TooLong \| NotText { bytes })`, `ReadFailed`, `End`; `code`, `signal` |
| `AgentError`, `ErrorKind` | every failure; `kind()` gives `MissingRuntime`, `InvalidCredential`, `SignInExpired`, `RateLimited`, `Network`, `ModelUnavailable` or `Other`; `AgentError::reported(kind, detail)` for the adapters |

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

**Keys.** A key reaches a runtime only as a `secret_vars` entry, through its environment: a request
whose command line or plain variables hold a secret's value is refused, naming the variable and never
the value. `SpawnRequest`'s and `RuntimeProcess`'s `Debug` never show a value (`Secret` redacts
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
cannot be reused by another process before the group signal. Dropping a `RuntimeProcess` kills the
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

**Tests.** `cargo test -p archforge-agents`: unit tests beside the code, and `tests/runtimes.rs`
against fake runtimes (shell scripts that answer `--version` and then do what a test needs) for
discovery order and the override, relative PATH entries, links, untrusted installers' folders, the
locale filter, the version decisions, a failing and a hanging probe, the cleared environment, the
canary key, the line cap, the error tail cap, the input backlog, SIGINT, the shutdown steps, the group
killed on drop and on exit, and the private folders. Unix only.

**Non-goals.** Windows: every entry point answers `AgentError::NotAvailable` ("not available on this
system yet"). The protocols, the turn and its errors (AR-201, AR-202), storing conversations, and the
Codex sign-in over standard input come later. The bridge `archforge-cli mcp` finds its control token
through `HOME`, which a runtime's private home hides; the spike's open question 6 lets the bridge
take the token file directly.
