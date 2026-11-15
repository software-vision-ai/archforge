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

Every start writes its server's record, without the server's process id, before it probes a port,
writes the server's configuration or starts the server, and completes the record with the process
id once the server runs, holding the record's lock from before the record is written until it is
complete. A start that finds another start's record there gives way then. One that fails before its
server runs removes its record; one that fails after its server runs and before the record is
complete stops the server and then removes the record, all still holding the lock (a record that
cannot be removed stays, which the log says, for the next start-up or opening of its project).
From the server's start on, a value whose drop stops the server owns it, so a start that fails
later (the wait until the server is ready) stops that server before it returns too. A record
takes at most 4096 bytes, written or read alike: a start whose record could take more once it
names the server's process id, whichever, fails before it writes the record or starts the server
(the paths it holds, the project folder's above all, are too long), and leaves nothing behind but
the record's lock file, as every start does.

A server that would not stop then (hardly possible on unix, where a kill follows the polite
signal) may still run, without a watchdog. Its run folder stays, and the start leaves its record,
complete or not: the project is refused as open elsewhere, naming that server, until the server has ended
or that Archforge has, and recovery (the start-up sweep too) then clears
both, stopping the server first if it still runs.

An Archforge killed after its server started and before its record is complete (the record's
completion: a write to disk) leaves a server without a watchdog and a record without a process
id. A record without a process id is that of a start under way, or of one that ended before it
completed its record. The opening of its project settles such a record before anything else, and
the start-up sweep settles it too (below):

- While the record's lock cannot be had, its start is under way: the project is refused as open
  elsewhere, naming that Archforge.
- Once it can, that start will not go on. Its server is looked for by the exact command line
  recorded, but only where that line ends in `--config` and the `server.yaml` of the recorded run
  folder, and that folder is one of this user's (a real folder, not a link) directly under the
  recorded run root, named as this crate names them: the random name makes the line that start's
  alone. Every process listed is proven again by its own command line before anything is done
  about it; no process is looked up by its name, and none is signalled on the word of a record
  alone. A record whose line is not so tied signals nothing and removes no file but itself.
- Whose Archforge still runs: a server found refuses the project, naming the server, and nothing
  goes.
- Whose Archforge is gone: the server's `server.yaml` is removed first. Where a child cut
  between its fork and its exec keeps the record's lock until its exec (an open file's lock, as
  the standard library takes it on unix, read from its documentation, not measured) the list
  finds it anyway; where it does not (a lock a file system emulates per process), the child finds
  no configuration once it runs dolt, and a dolt started without its configuration exits 1 at
  once and takes no lock (measured with dolt 2.7.3). Every server found is stopped, checked
  again before each signal; only then do the run folder and the record go. A running server
  whose run folder is removed runs on, keeping the project's lock and its port (measured with
  dolt 2.7.3), which is why it is stopped first.
- Where the configuration cannot be removed (but for one already gone), the list or a proof
  fails, or a server would not stop, the opening fails and nothing more goes: the record and the
  run folder stay for the next start-up or opening.

The start-up sweep settles such a record in the same way, with these differences. It takes the
record's lock without a wait (its removals afterwards wait as every change does): the application
starts no server of its own while it sweeps (it holds its job lock throughout), so a lock held is
a start under way in another Archforge, the child of a start between its fork and its exec, or
another change of that record, and the record is left to the next start-up or opening; so is a
record whose server's Archforge still runs, and one completed while the sweep looked. A
failure is logged and leaves the record and its run folder for the next start-up or opening, and
the sweep goes on with the next record. Every start writes its record under its project's own
name: a record under another name is a stray copy, which settling would check under another lock
than its start's, so that a live start's run folder could go with it. That file alone goes, and
nothing is signalled. A server the sweep stops this way counts among those it returns.

A record of the earlier format, with a process id always, is a complete record and is read as one;
nothing is migrated, since the application is not released.

Every change of a server record (writing one, removing one, setting a damaged one aside) holds
that record's lock, an operating-system lock on a file of its own beside the record (the record's
name ending in `.lock`), so a change that looks before it acts (remove this record only if it is
still the one read; write one only if none is there) sees no other change in between, from this
Archforge or another of a build that has this lock. A record is never moved from its name to be
compared, so no start finds a name free that is not. Reading takes no lock: a record appears and
goes whole. The lock goes with its holder, so an Archforge killed in a change leaves the lock file
and no lock; the lock files stay, one empty file for each record name ever used. An Archforge
killed while it writes a record, before the record's temporary file has been renamed or removed,
leaves that file (`.record-` and a random name, a few hundred bytes) in the records folder too,
and nothing removes it.

A start takes the lock before it writes its record, and so before its server, and holds it until
the record is complete: a wait for the lock, or a start that cannot have it, comes before there is
a record or a server. A change waits at most two seconds for the lock: behind a
holder that has stalled it fails and changes nothing. A start fails, having started no server; a
removal leaves the record to the next recovery or sweep, and says so in the log; a damaged record
stays where it is, which fails the opening of its project and leaves it out of a sweep. The lock
is each project's own, so a start that has stalled, which holds its lock for as long, holds up
that project's record only and no other project's. (What a stalled start does to other starts in
the same Archforge is the half second named above.) The same failures follow where the file system has no locks, and where the lock's name
holds anything but a plain file: no server starts with its records there. A damaged record that
cannot be set aside (its lock held, too many set aside before it, or the move itself failing)
fails the opening as well. A start that fails for a lost port tries again only when the record's
name is free.

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
| `stop_orphans` | `pub fn stop_orphans(opts: &ServerOptions) -> Result<usize, Error>` (at start-up: stops every recorded server whose Archforge is gone, forgets the records of servers that are gone, settles a record without a process id as an opening does, taking its lock without a wait; returns how many it stopped) |
| `Server::stop` | `pub fn stop(self) -> Result<(), Error>` (dropping a server stops it too) |
| `Server::{port, socket, password, data_dir, pid}` | the running server's facts |
| `Error` | one enum; every variant names the value it is about |

**Tests.** Unit tests for the pure parts (parsers, the recovery rules, path and folder checks) and
for the watchdog against a stand-in process, and integration tests in `tests/server.rs` against a
real dolt 2.6 or later, measured on macOS; the Linux and Windows runs are still to be done. The
unit tests in `src/server.rs` start real servers too. One of them runs its own test binary again
as an Archforge and kills it in the middle of a start, once its server runs and before its record
names the server, and once before the server starts: the next start-up stops the server in the
first case and removes what each start left.

**Non-goals.** The project's tables (the project database and its settings come with creating a
project), packaging dolt, TLS.
