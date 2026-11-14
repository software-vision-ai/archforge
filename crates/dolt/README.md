# archforge-dolt

**Purpose.** The project database's server: runs `dolt sql-server` for one project folder beside the
application, reachable only by this user with a password made for each start, connects to it, stops
it, and recovers a server a crashed Archforge left behind: at the next opening of its project, and
for every project at the application's start-up. On unix each server also has a watchdog, a small
`/bin/sh` child that reads a pipe only Archforge holds open: when Archforge is gone, a kill
included, the watchdog stops the server politely at once, if the process is still that server. The
start-up sweep stays the backstop: for Windows, and for a watchdog that was killed too, could not
start, could not prove the server (no `/bin/ps`, no `C.UTF-8` locale for a command line outside
ASCII), or was held up by another program that inherited the pipe (possible on macOS only) until
that program and everything it started ended. A watchdog starts with no other start of this crate
under way, so none of them inherits its pipe; no start waits for another longer than half a second,
so a stalled start never holds up a stop or a recovery of another project.

A start that fails once its server runs stops that server before it returns: every check that
needs no server comes before the server's start, and what follows (the record, the wait until it
is ready) owns the server through a value whose drop stops it. One moment stays open: an Archforge
killed between a server's start and its record, which needs the server's process id and so cannot
come first. That is the record's write to disk, measured at 5 to 10 ms on macOS. A server left so
has neither record nor watchdog, so no sweep finds it, and its run folder stays. Recovery treats
it as any server it did not record: once it holds the project's lock, and if its program's file is
named `dolt`, the project is refused as open elsewhere, naming the server's process id, until that
process is stopped.

**Layer.** Host services, layer 9: the application uses it, the interface never depends on it, and
the portable-target check leaves it out. No toolkit.

**Binding rules.** Every helper program runs by absolute path in a cleared environment with a
budget (but the watchdog, whose work is to wait); dolt runs in Archforge's own private dolt home with usage metrics and the online version
check off, and never reads or writes the user's `~/.dolt`; nothing is stopped unless its owner is
dead and it is proven to be the server this crate started; only run folders this crate made are
removed. The password never shows in a log, an error or a debug print.

**Public API.**

| Item | Signature |
|---|---|
| `Home::prepare` | `pub fn prepare(path: &Path) -> Result<Home, Error>` (creates and checks the private dolt home) |
| `Dolt::find` | `pub fn find(home: &Home) -> Result<Dolt, Error>` (`ARCHFORGE_DOLT`, else `dolt` on the PATH; 2.6 or later within 2) |
| `Dolt::at` | `pub fn at(path: &Path, home: &Home) -> Result<Dolt, Error>` (an absolute path) |
| `ServerOptions::new` | `pub fn new(records: impl Into<PathBuf>) -> ServerOptions` (records folder; run root, budgets) |
| `Server::start` | `pub fn start(dolt: &Dolt, data_dir: &Path, opts: &ServerOptions) -> Result<Server, Error>` |
| `Server::connect` | `pub fn connect(&self) -> Result<mysql::Conn, Error>` (socket on unix, loopback on Windows) |
| `stop_orphans` | `pub fn stop_orphans(opts: &ServerOptions) -> Result<usize, Error>` (at start-up: stops every recorded server whose Archforge is gone, forgets the records of servers that are gone; returns how many it stopped) |
| `Server::stop` | `pub fn stop(self) -> Result<(), Error>` (dropping a server stops it too) |
| `Server::{port, socket, password, data_dir, pid}` | the running server's facts |
| `Error` | one enum; every variant names the value it is about |

**Tests.** Unit tests for the pure parts (parsers, the recovery rules, path and folder checks) and
for the watchdog against a stand-in process, and integration tests in `tests/server.rs` against a
real dolt 2.6 or later, measured on macOS; the Linux and Windows runs are still to be done.

**Non-goals.** The project's tables (the project database and its settings come with creating a
project), packaging dolt, TLS.
