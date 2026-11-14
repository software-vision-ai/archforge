# archforge-dolt

**Purpose.** The project database's server: runs `dolt sql-server` for one project folder beside the
application, reachable only by this user with a password made for each start, connects to it, stops
it, and recovers a server a crashed Archforge left behind: at the next opening of its project, and
for every project at the application's start-up (but a record without a process id, below). On unix each server also has a watchdog, a small
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
is ready) owns the server through a value whose drop stops it.

A server that would not stop then (hardly possible on unix, where a kill follows the polite
signal) may still run, without a watchdog. Its run folder stays, and so does its record if it had one: the project is
refused as open elsewhere, naming that server, until the server has ended or that Archforge has,
and recovery or the start-up sweep then clears both, stopping the server first if it still runs.
Without a record (its write failed, or another start's record was there) the folder stays for a
server nothing finds, as in the moment that follows.

One moment stays open: an Archforge
killed between a server's start and its record, which needs the server's process id and so cannot
come first. That is the record's write to disk, measured at 5 to 10 ms on macOS. A server left so
has neither record nor watchdog, so no sweep finds it, and its run folder stays. Recovery treats
it as any server it did not record: once it holds the project's lock, and if its program's file is
named `dolt`, the project is refused as open elsewhere, naming the server's process id, until that
process is stopped.

A record may also name no process id: the record a start writes before its server runs, which
AR-457's second part makes every start write, holding the record's lock until the record is
complete. The opening of its project settles such a record before anything else:

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
  run folder stay for the next opening.

The start-up sweep leaves a record without a process id alone for now (AR-459), so a server of a
start its Archforge did not finish runs on until its project is opened. A record of the earlier
format, with a process id always, is a complete record and is read as one; nothing is migrated,
since the application is not released.

Every change of a server record (writing one, removing one, setting a damaged one aside) holds
that record's lock, an operating-system lock on a file of its own beside the record (the record's
name ending in `.lock`), so a change that looks before it acts (remove this record only if it is
still the one read; write one only if none is there) sees no other change in between, from this
Archforge or another of a build that has this lock. A record is never moved from its name to be
compared, so no start finds a name free that is not. Reading takes no lock: a record appears and
goes whole. The lock goes with its holder, so an Archforge killed in a change leaves the lock file
and no lock; the lock files stay, one empty file for each record name ever used.

A start takes the lock before it starts its server and holds it until the record is written: a
wait for the lock, or a start that cannot have it, comes before there is a server, and the moment
named above is no longer than it was. A change waits at most two seconds for the lock: behind a
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
| `stop_orphans` | `pub fn stop_orphans(opts: &ServerOptions) -> Result<usize, Error>` (at start-up: stops every recorded server whose Archforge is gone, forgets the records of servers that are gone, leaves a record without a process id to its project's opening; returns how many it stopped) |
| `Server::stop` | `pub fn stop(self) -> Result<(), Error>` (dropping a server stops it too) |
| `Server::{port, socket, password, data_dir, pid}` | the running server's facts |
| `Error` | one enum; every variant names the value it is about |

**Tests.** Unit tests for the pure parts (parsers, the recovery rules, path and folder checks) and
for the watchdog against a stand-in process, and integration tests in `tests/server.rs` against a
real dolt 2.6 or later, measured on macOS; the Linux and Windows runs are still to be done.

**Non-goals.** The project's tables (the project database and its settings come with creating a
project), packaging dolt, TLS.
