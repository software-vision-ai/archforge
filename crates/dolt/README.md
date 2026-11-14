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
needs no server comes before the server's start, and what follows (completing the record, the wait
until it is ready) owns the server through a value whose drop stops it.

**Records.** A project has one record file in the records folder, named by a hash of the project
folder. It holds the project folder, the run folder and its checked root, the server's command line
exactly as started (its configuration lies in the run folder, whose random name makes the line
unique to that start), the Archforge that started it (process id and start time) and the server's
process id. A start writes it twice:

1. *Before the server starts*, without a process id, which is not known yet: written in full under a
   temporary name, synchronised, then linked to the record's name. The link fails if a record is
   there, so of two starts one gives way, before it has started anything.
2. *Once the server runs*, with its process id: again written in full under a temporary name and
   synchronised, then renamed over the first. A record is never edited in place, so a reader finds
   one or the other whole.

Removing a record compares before it moves anything: the record is read where it is, and only the
one meant is taken aside under another name, compared again there, and removed or put back. So a
removal that read an earlier record of the project leaves the record of a start under way where it
is, and that start's name stays its own. One race stays, known from reading the code, not from a
run: a record replaced by a new start's between that read and the move is aside for a moment. A
start that completes its record in that moment writes it all the same, and its first record stays
beside it under the other name; one that removes its record then misses it, the record comes back,
and the project is refused as open elsewhere until that Archforge ends; and a third start may find
the name free and start beside the first, where the project's lock lets one server live.

The watchdog starts after the second. A start that fails removes its record once its server is
stopped, or was never started; a server that would not stop keeps the record it has. What an
Archforge killed during a start leaves, and what finds it:

| Killed | Left behind | The next opening of the project | The start-up sweep |
|---|---|---|---|
| before the first record | a run folder, nothing else | starts | nothing to find (the run folder stays) |
| after the first record, before the server | a record without a process id; no server | removes the record and the run folder, starts | leaves the record |
| after the server started, before the second record | that record; a server with no watchdog | stops the server once it holds the project's lock, see below | leaves both |
| after the second record, before the watchdog | a complete record; a server with no watchdog | stops the server | stops the server |
| later | as above, with a watchdog | the watchdog has stopped the server; forgets the record | the same |

A record without a process id whose Archforge still runs (same process id, same start time, as for
a complete record) is a start under way: the project is refused as open elsewhere, naming that
Archforge, and the sweep leaves it. When its Archforge is gone, the project's opening looks at the
process the project's lock names: if its command line is exactly the recorded one, it is the server
that start left, and it is stopped as any recorded server is, checked again before each signal. No
process is ever looked up by name, and none is signalled on the word of a record alone. With a
lock that is absent, or names a process that is gone or is not a dolt server, the record and its
run folder are removed and the start goes on; a damaged lock, or one that names another dolt
server, refuses the project as before and leaves the record.

Still open (the second part of AR-457): the sweep does nothing about a record without a process id,
so a server left in that moment (the second record's write to disk, measured at 5 to 10 ms on
macOS) runs on until its project is opened. And a server left so that does not hold the project's
lock yet when the project is opened is not found: its run folder goes with the record, so it
loses its socket, the new start races it, and once it holds the lock, and if its program's file is
named `dolt`, the project is refused as open elsewhere, naming the server's process id, until that
process is stopped. A record without a process id left under another name by the race above stays
until the sweep handles such records.

A record written before this format, with a process id from the start, is a complete record and is
read as one. The application is not released, so nothing is migrated. Windows runs the same code;
like the rest of the crate it is not run there yet.

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
