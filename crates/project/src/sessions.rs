//! The project's own sessions on its server, so a rename or a delete can let go of the branch first.
//!
//! Dolt refuses to rename or delete a branch that any other session has as its database, idle or
//! not, and only forcing it skips that, which also passes over another program's session (and for a
//! rename overwrites a branch of the new name). A dropped connection is no help: its quit is not
//! waited for, and the server drops the session some time later, as it also does after a `KILL`. So
//! every connection of the project is a [`Session`] the project knows by its connection id, with
//! the databases it may be using. Each one leaves its database before it closes, which the server
//! has done once the reply comes. Before a rename or a delete, [`Sessions::let_go`] ends the ones
//! still on the branch and waits until the server's process list no longer holds them, within
//! [`LET_GO_BUDGET`] of wall-clock time; a session that still holds the branch after that is another
//! program's, unless one of the project's own entered during the hold, and Dolt's refusal stands.

use std::collections::{BTreeSet, HashMap};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use mysql::prelude::Queryable;

use crate::Error;

/// How long a rename or a delete may take to end its own sessions and see them dropped, all requests
/// included, and how long a new session waits for one on its database to finish: a bound, not a
/// guess, as the server says when it is done.
const LET_GO_BUDGET: Duration = Duration::from_secs(10);
/// The pause between two looks at the process list.
const LOOK_AGAIN: Duration = Duration::from_millis(5);
/// The database a session moves to before it closes: it is on no branch there.
const NO_BRANCH: &str = "information_schema";
/// Why a release stopped when its budget was spent.
const LATE: &str = "the server did not let go of this Archforge's own sessions on the branch in time";

/// What the project knows of its sessions.
#[derive(Debug, Default)]
struct Known {
    /// By connection id, the databases each may be using, lower case as the server compares
    /// branches: the one it last moved to, and one it was asked to move to if no answer came.
    sessions: HashMap<u32, BTreeSet<String>>,
    /// The database, lower case, whose branch is being renamed or deleted: no session moves to it
    /// meanwhile.
    holding: Option<String>,
    /// Whether a session moved to that database during the hold, once its wait ran out, whether it
    /// is still there or not.
    entered: bool,
}

/// The register and the signal that a rename or a delete has let go.
#[derive(Debug, Default)]
struct Register {
    known: Mutex<Known>,
    done: Condvar,
}

impl Register {
    /// The register, also after a thread panicked holding it: each change is a single step.
    fn lock(&self) -> MutexGuard<'_, Known> {
        self.known.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// How to open the project's sessions, and the register of those open. Clones share the register.
#[derive(Debug, Clone)]
pub(crate) struct Sessions {
    connector: archforge_dolt::Connector,
    register: Arc<Register>,
}

/// A connection the project knows of. It derefs to the connection; a change of database goes
/// through [`Session::use_database`], so the register says where it may be.
#[derive(Debug)]
pub(crate) struct Session {
    conn: mysql::Conn,
    id: u32,
    register: Arc<Register>,
    /// Whether it has left its database to close: it is used no more.
    closing: bool,
}

/// A rename's or a delete's hold on its branch's database: no session moves to it until this is
/// dropped.
pub(crate) struct Hold {
    register: Arc<Register>,
}

impl Hold {
    /// Whether a session of the project's moved to the database during the hold: a refusal then
    /// may be the project's own doing, even if that session has left since.
    pub(crate) fn entered(&self) -> bool {
        self.register.lock().entered
    }
}

/// What a release asks over its own connection, and the answer, as rows or why there are none.
type Answer = Result<Vec<mysql::Row>, String>;

/// The connection a release's requests run over, on a thread of its own: see
/// [`Sessions::let_go_within`]. Dropping this lets the thread end.
struct Release {
    asks: std::sync::mpsc::Sender<String>,
    answers: std::sync::mpsc::Receiver<Answer>,
    until: Instant,
    /// Told of each answer, on the thread letting go.
    answered: Box<dyn Fn()>,
}

/// What a test sees of a release, and where it can slow it down; production watches nothing.
pub(crate) struct Watch {
    /// Run on the release's thread as a request reaches it, before the budget is looked at.
    pub(crate) received: Box<dyn Fn() + Send>,
    /// Run on the release's thread just before a request is started.
    pub(crate) started: Box<dyn Fn() + Send>,
    /// Run on the thread letting go after each answer.
    pub(crate) answered: Box<dyn Fn()>,
}

impl Watch {
    /// Watches nothing.
    pub(crate) fn none() -> Watch {
        Watch { received: Box::new(|| {}), started: Box::new(|| {}), answered: Box::new(|| {}) }
    }
}

impl Release {
    /// Starts the thread, which connects first and then answers each request in turn, while the
    /// budget lasts: a request it comes to once the budget is spent is not started, and the thread
    /// ends.
    fn start(connector: archforge_dolt::Connector, until: Instant, watch: Watch) -> Result<Release, String> {
        let (asks, asked) = std::sync::mpsc::channel::<String>();
        let (answer, answers) = std::sync::mpsc::channel::<Answer>();
        let Watch { received, started, answered } = watch;
        let work = move || {
            let mut conn = connector.connect().map_err(|e| e.to_string());
            for query in asked {
                received();
                if Instant::now() >= until {
                    break;
                }
                started();
                let rows = match conn.as_mut() {
                    Ok(c) => c.query::<mysql::Row, _>(query).map_err(|e| e.to_string()),
                    Err(e) => Err(e.clone()),
                };
                if answer.send(rows).is_err() {
                    break;
                }
            }
        };
        let spawned = std::thread::Builder::new().name("archforge-release".into()).spawn(work);
        spawned.map_err(|e| format!("no thread could end this Archforge's own sessions: {e}"))?;
        Ok(Release { asks, answers, until, answered })
    }

    /// Asks `query` and waits for its answer until the budget is spent; once it is, nothing more is
    /// asked.
    fn ask(&self, query: String) -> Answer {
        let ended = || if Instant::now() >= self.until { LATE.to_string() } else { "the release's connection has ended".to_string() };
        if Instant::now() >= self.until {
            return Err(LATE.into());
        }
        self.asks.send(query).map_err(|_| ended())?;
        let answer = match self.answers.recv_timeout(self.until.saturating_duration_since(Instant::now())) {
            Ok(answer) => answer,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => Err(LATE.into()),
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => Err(ended()),
        };
        (self.answered)();
        answer
    }
}

impl Sessions {
    /// The sessions of the server `connector` reaches, none open yet.
    pub(crate) fn new(connector: archforge_dolt::Connector) -> Sessions {
        Sessions { connector, register: Arc::default() }
    }

    /// A new session, known before it uses any database.
    pub(crate) fn open(&self) -> Result<Session, Error> {
        let conn = self.connector.connect().map_err(Error::Server)?;
        let id = conn.connection_id();
        self.register.lock().sessions.insert(id, BTreeSet::new());
        Ok(Session { conn, id, register: self.register.clone(), closing: false })
    }

    /// Ends every session of the project's that may be using `database` and waits until the server
    /// has dropped them, within [`LET_GO_BUDGET`]. Until the hold it gives is dropped, a session asked
    /// to move to `database` waits. A read on the branch at that moment fails.
    pub(crate) fn let_go(&self, database: &str) -> Result<Hold, String> {
        self.let_go_within(database, LET_GO_BUDGET, Watch::none())
    }

    /// [`Sessions::let_go`] within `budget`, `watch` seeing its requests. The requests go over a
    /// connection of their own on a thread of their own, so the budget bounds the whole release, the
    /// connection and every answer included, whatever the server does: once it is spent, the
    /// release sends no request, its thread starts none it was sent, and it waits for nothing more. A request left running ends at the server's answer or at the
    /// connection's own timeout, and its thread with it; a `KILL` left running may still end its
    /// session, which was on the branch.
    pub(crate) fn let_go_within(&self, database: &str, budget: Duration, watch: Watch) -> Result<Hold, String> {
        let until = Instant::now() + budget;
        let database = database.to_ascii_lowercase();
        let ended: Vec<u32> = {
            let mut known = self.register.lock();
            known.holding = Some(database.clone());
            known.entered = false;
            known.sessions.iter().filter(|(_, dbs)| dbs.contains(&database)).map(|(id, _)| *id).collect()
        };
        let held = Hold { register: self.register.clone() };
        if ended.is_empty() {
            return Ok(held);
        }
        let release = Release::start(self.connector.clone(), until, watch)?;
        for id in &ended {
            // An id the server no longer has is no error (measured on 2.7.5).
            release.ask(format!("KILL {id}")).map_err(|e| format!("this Archforge's own session on the branch could not be ended: {e}"))?;
        }
        loop {
            // Read as rows, not numbers: an id that is not one fails the listing, never a panic.
            let unlisted = |why: String| format!("the server's sessions cannot be listed: {why}");
            let rows = release.ask("SELECT ID FROM information_schema.processlist".into()).map_err(unlisted)?;
            let listed = rows
                .iter()
                .map(|r| r.get_opt::<u64, usize>(0).and_then(Result::ok).ok_or_else(|| unlisted("an id is not a number".into())))
                .collect::<Result<BTreeSet<u64>, String>>()?;
            let gone = |id: &u32| ended.contains(id) && !listed.contains(&u64::from(*id));
            self.register.lock().sessions.retain(|id, _| !gone(id));
            if ended.iter().all(gone) {
                return Ok(held);
            }
            let left = until.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Err(LATE.into());
            }
            std::thread::sleep(LOOK_AGAIN.min(left));
        }
    }

    /// The ids of the sessions known now, for a test to count.
    #[cfg(feature = "testing")]
    pub(crate) fn known(&self) -> usize {
        self.register.lock().sessions.len()
    }
}

impl Session {
    /// Leaves the database to close, once: see the drop.
    fn leave(&mut self) {
        if !std::mem::replace(&mut self.closing, true) && self.conn.query_drop(format!("USE `{NO_BRANCH}`")).is_ok() {
            self.register.lock().sessions.remove(&self.id);
        }
    }

    /// This session as it is once closed but before the server has dropped it, for a test to hold.
    #[cfg(feature = "testing")]
    pub(crate) fn closed(mut self) -> Session {
        self.leave();
        self
    }

    /// Moves this session to `database`, once no rename or delete holds it; the register has it
    /// first, so one that starts meanwhile ends this session. A hold that lasts past
    /// [`LET_GO_BUDGET`] is waited out no longer, and the hold records that a session entered: the
    /// rename or delete then meets this session as Dolt reports it, never as another program's.
    pub(crate) fn use_database(&mut self, database: &str) -> Result<(), mysql::Error> {
        let name = database.to_ascii_lowercase();
        {
            let until = Instant::now() + LET_GO_BUDGET;
            let mut known = self.register.lock();
            while known.holding.as_ref() == Some(&name) {
                let left = until.saturating_duration_since(Instant::now());
                if left.is_zero() {
                    break;
                }
                known = self.register.done.wait_timeout(known, left).map(|(k, _)| k).unwrap_or_else(|poisoned| poisoned.into_inner().0);
            }
            if known.holding.as_ref() == Some(&name) {
                known.entered = true;
            }
            known.sessions.entry(self.id).or_default().insert(name.clone());
        }
        self.conn.query_drop(format!("USE `{database}`"))?;
        if let Some(dbs) = self.register.lock().sessions.get_mut(&self.id) {
            dbs.retain(|db| *db == name);
        }
        Ok(())
    }
}

impl std::ops::Deref for Session {
    type Target = mysql::Conn;

    /// The connection.
    fn deref(&self) -> &mysql::Conn {
        &self.conn
    }
}

impl std::ops::DerefMut for Session {
    /// The connection.
    fn deref_mut(&mut self) -> &mut mysql::Conn {
        &mut self.conn
    }
}

impl Queryable for Session {
    /// [`Queryable::query_iter`] on the connection.
    fn query_iter<Q: AsRef<str>>(&mut self, query: Q) -> mysql::Result<mysql::QueryResult<'_, '_, '_, mysql::Text>> {
        self.conn.query_iter(query)
    }

    /// [`Queryable::prep`] on the connection.
    fn prep<Q: AsRef<str>>(&mut self, query: Q) -> mysql::Result<mysql::Statement> {
        self.conn.prep(query)
    }

    /// [`Queryable::close`] on the connection.
    fn close(&mut self, stmt: mysql::Statement) -> mysql::Result<()> {
        self.conn.close(stmt)
    }

    /// [`Queryable::exec_iter`] on the connection.
    fn exec_iter<S, P>(&mut self, stmt: S, params: P) -> mysql::Result<mysql::QueryResult<'_, '_, '_, mysql::Binary>>
    where
        S: mysql::prelude::AsStatement,
        P: Into<mysql::Params>,
    {
        self.conn.exec_iter(stmt, params)
    }

    /// [`Queryable::exec_batch`] on the connection.
    fn exec_batch<S, P, I>(&mut self, stmt: S, params: I) -> mysql::Result<()>
    where
        S: mysql::prelude::AsStatement,
        P: Into<mysql::Params>,
        I: IntoIterator<Item = P>,
    {
        self.conn.exec_batch(stmt, params)
    }
}

impl Drop for Session {
    /// Leaves the database, then forgets the session: once the server has answered, it is on no
    /// branch, however long it takes to drop it. One that cannot leave stays known, for a rename or
    /// a delete to end, until the server no longer lists it.
    fn drop(&mut self) {
        self.leave();
    }
}

impl Drop for Hold {
    /// Lets sessions move to the database again. Renames and deletes run one at a time, on the
    /// writer, so the database held is this one's.
    fn drop(&mut self) {
        self.register.lock().holding = None;
        self.register.done.notify_all();
    }
}
