//! A project on disk: `<name>.archforge`, the data folder of a Dolt server, with one database,
//! `project`, whose `settings` table says what the project is. Creating it makes the folder, starts
//! the server, writes the settings and commits them; opening it checks the folder, starts the
//! server and reads the settings back. The model's own tables come with the generated storage;
//! a format-2 project may still carry the `er_` tables an earlier Archforge made, unused. Its
//! conversations live in tables of its own (the `chat` module). The project stays open, its server
//! running, until it is closed or dropped.
//!
//! A folder from elsewhere is never trusted: its server settings cannot sync with a remote or run
//! events (the server crate overrides them), its own `.dolt` must hold nothing Archforge does not
//! leave there, and its settings are read with a cap on every value. A creation that did not finish
//! keeps a marker file, so it reads as unfinished, never as damaged, and is never removed for the
//! user. Host services, layer 9: the interface never depends on this crate.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unreachable, clippy::todo, clippy::unimplemented)]

mod branch_name;
mod chat;
mod description;
mod error;
mod model;
mod saving;
mod time;
mod versions;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use archforge_dolt::{Dolt, Server, ServerOptions};
use mysql::prelude::Queryable;

pub use branch_name::{BRANCH_NAME_CHARS, BranchNameProblem, SafeBranch, UnsafeBranch};
pub use chat::{
    Anchor, AnchorRole, CHAT_AUTHOR, CHAT_TABLES, ChangeKind, ChangeRecord, ChangeState, ChangedElement, Chat, ChatSummary, Conversation, Deleted,
    Listed, Message, MessageRecord, NewAnchor, NewChat, NewQuestion, NewReply, NewToolCall, NewTurn, Provider, Rerun, Role, ToolCall, ToolStatus,
    Turn, TurnChanges, TurnEnd, TurnError, TurnOutcome, TurnRecord, TurnSpan, limits,
};
pub use description::{DESCRIPTION_CHARS, DescriptionProblem, check_description};
pub use error::Error;
pub use model::{EARLIER_MODELS, ModelSchema};
pub use saving::{AfterFailure, Author, SaveFailure, Saved, is_version, shown_author};
pub use time::timestamp;
pub use versions::{Version, VersionReader, Versions};

/// The project format this Archforge writes and the newest it reads. Format 3 may hold
/// conversations (the chat tables, AR-206); a project of format 1 or 2 is raised to 3 only when it
/// first holds one, so an Archforge that reads up to 2 still opens every project never chatted in.
pub const FORMAT: u32 = 3;
/// A project folder's extension.
pub const EXTENSION: &str = archforge_names::EXTENSION;
/// The one database in a project folder.
pub const DATABASE: &str = "project";
/// The file a creation keeps in the project folder until it is finished.
pub const MARKER: &str = ".creating";
/// The settings a project holds, and the caps that reading them keeps to.
const MAX_ROWS: usize = 1000;
const MAX_KEY: usize = 64;
const MAX_VALUE: usize = 1024;
/// The author of the creation commit.
pub(crate) const AUTHOR: &str = "Archforge <archforge@localhost>";

/// What the `settings` table says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    pub format: u32,
    pub name: String,
    pub created_by: String,
    pub created_at: String,
}

/// Points a creation calls out to: for tests, and the app's cancel flag.
#[derive(Default)]
pub struct Hooks {
    /// Called with the server's process id and the new folder once the server runs; an error takes
    /// the undo path.
    pub after_start: Option<Box<dyn Fn(u32, &Path) -> Result<(), String> + Send + Sync>>,
    /// Set to stop a creation between its steps; it then undoes what it did.
    pub cancel: Option<Arc<AtomicBool>>,
}

impl Hooks {
    fn cancelled(&self) -> Result<(), Error> {
        if self.cancel.as_ref().is_some_and(|c| c.load(Ordering::SeqCst)) { Err(Error::Cancelled) } else { Ok(()) }
    }
}

/// An open project: its folder, its settings and its running server.
#[derive(Debug)]
pub struct Project {
    server: Server,
    path: PathBuf,
    settings: Settings,
    /// The state of each change link of this session's turns, now and at the last save, for Don't
    /// save to put back.
    changes: std::sync::Mutex<chat::Ledger>,
}

impl Project {
    /// Creates the project `name` in `folder` and leaves it open.
    pub fn create(dolt: &Dolt, opts: &ServerOptions, folder: &Path, name: &str, created_by: &str, now: &str) -> Result<Project, Error> {
        Project::create_with(dolt, opts, folder, name, created_by, now, &Hooks::default())
    }

    /// [`Project::create`] with hooks.
    pub fn create_with(
        dolt: &Dolt,
        opts: &ServerOptions,
        folder: &Path,
        name: &str,
        created_by: &str,
        now: &str,
        hooks: &Hooks,
    ) -> Result<Project, Error> {
        if let Some(problem) = archforge_names::problem(name) {
            return Err(Error::BadName { name: name.to_string(), problem });
        }
        let folder = usable_folder(folder)?;
        let target = folder.join(format!("{name}.{EXTENSION}"));
        match std::fs::symlink_metadata(&target) {
            Ok(m) if m.is_dir() && unfinished(&target) => {
                recover(&target, opts);
                return Err(Error::Unfinished { path: target });
            }
            Ok(_) => return Err(Error::NameTaken { path: target }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(Error::io("cannot look at", &target, e)),
        }
        make_private_dir(&target).map_err(|e| match e.kind() {
            std::io::ErrorKind::AlreadyExists => Error::NameTaken { path: target.clone() },
            _ => Error::FolderUnusable { path: folder.clone(), why: e.to_string() },
        })?;
        let made = Made { identity: identity(&target), path: target.clone() };
        if let Err(e) = std::fs::File::create_new(target.join(MARKER)) {
            return Err(undo(None, &made, Error::io("cannot write the marker in", &target, e)));
        }
        let mut server = None;
        match build(dolt, opts, &target, name, created_by, now, hooks, &mut server) {
            Ok(settings) => match (std::fs::remove_file(target.join(MARKER)), server) {
                (Ok(()), Some(server)) => Ok(Project { server, path: target, settings, changes: Default::default() }),
                (Err(e), server) => Err(undo(server, &made, Error::io("cannot remove the marker from", &target, e))),
                (Ok(()), None) => Err(undo(None, &made, Error::Damaged { path: target.clone(), why: "no server ran".into() })),
            },
            Err(e) => Err(undo(server.take(), &made, e)),
        }
    }

    /// Opens the project at `path`.
    pub fn open(dolt: &Dolt, opts: &ServerOptions, path: &Path) -> Result<Project, Error> {
        let not_a_project = |why: &str| Error::NotAProject { path: path.to_path_buf(), why: why.into() };
        text(path, "project folder").map_err(|_| not_a_project("its path is not plain text"))?;
        let meta = std::fs::symlink_metadata(path).map_err(|e| Error::io("cannot open", path, e))?;
        if !meta.is_dir() || meta.file_type().is_symlink() {
            return Err(not_a_project("it is not a folder"));
        }
        let path = path.canonicalize().map_err(|e| Error::io("cannot open", path, e))?;
        if unfinished(&path) {
            // Recovery only reads the lock and the records: nothing that writes may come before
            // the checks below.
            recover(&path, opts);
            return Err(Error::Unfinished { path });
        }
        // The database folder must be the project's own, not a link to one elsewhere.
        let database = std::fs::symlink_metadata(path.join(DATABASE));
        if !database.is_ok_and(|m| m.is_dir() && !m.file_type().is_symlink()) || !path.join(DATABASE).join(".dolt").is_dir() {
            return Err(Error::NotAProject { path, why: format!("it has no {DATABASE} database of its own") });
        }
        private(&path)?;
        own_dot_dolt(&path)?;
        let server = Server::start(dolt, &path, opts).map_err(Error::Server)?;
        match read_settings(&server, &path).and_then(|settings| unscripted(&server, &path, settings)) {
            Ok(settings) => Ok(Project { server, path, settings, changes: Default::default() }),
            Err(e) => {
                if let Err(stop) = server.stop() {
                    log::warn!("the server of the refused project {} did not stop: {stop}", path.display());
                }
                Err(e)
            }
        }
    }

    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The project's server, for the model to talk to.
    pub fn server(&self) -> &Server {
        &self.server
    }

    /// Stops the project's server.
    pub fn close(self) -> Result<(), Error> {
        self.server.stop().map_err(Error::Server)
    }
}

/// The folder new projects go in: made when missing, plain text, a folder.
fn usable_folder(folder: &Path) -> Result<PathBuf, Error> {
    let unusable = |why: String| Error::FolderUnusable { path: folder.to_path_buf(), why };
    text(folder, "folder").map_err(|_| unusable("its path is not plain text".into()))?;
    std::fs::create_dir_all(folder).map_err(|e| unusable(format!("it cannot be made: {e}")))?;
    let canon = folder.canonicalize().map_err(|e| unusable(format!("it cannot be read: {e}")))?;
    if !canon.is_dir() {
        return Err(unusable("it is not a folder".into()));
    }
    text(&canon, "folder").map_err(|_| unusable("its path is not plain text".into()))?;
    Ok(canon)
}

fn text<'a>(path: &'a Path, what: &str) -> Result<&'a str, String> {
    path.to_str().filter(|s| !s.chars().any(char::is_control)).ok_or_else(|| format!("the {what} path is not plain text"))
}

/// The most entries [`find`] reads in one folder: a folder holding more is read only that far.
const FIND_ENTRIES: usize = 2_000;

/// What [`find`] found, and whether it saw everything: the projects; whether a limit stopped it
/// short (the projects it may list, the entries it may read in all or in one folder); how many
/// folders it could not read.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Found {
    pub projects: Vec<PathBuf>,
    pub cut_short: bool,
    pub unreadable: usize,
}

/// The projects in `folder` and the folders below it, down to `depth` levels under it, at most
/// `limit` of them, in name order within each folder: folders named `<name>.archforge` whose path
/// is plain text, that are not a creation that did not finish. Every entry read spends one of
/// `budget`, shared by the calls of one search; when it runs out the walk stops. Links are not
/// followed; hidden folders, folders whose path is not plain text and projects' own folders are
/// not entered.
pub fn find(folder: &Path, depth: usize, limit: usize, budget: &mut usize) -> Found {
    let mut found = Found::default();
    find_in(folder, depth, limit, budget, &mut found);
    found
}

fn find_in(folder: &Path, depth: usize, limit: usize, budget: &mut usize, found: &mut Found) {
    let Ok(entries) = std::fs::read_dir(folder) else {
        found.unreadable += 1;
        return;
    };
    let mut dirs = Vec::new();
    for (n, entry) in entries.enumerate() {
        if n >= FIND_ENTRIES || *budget == 0 {
            found.cut_short = true;
            break;
        }
        *budget -= 1;
        let Ok(entry) = entry else { continue };
        let path = entry.path();
        let shown = path.file_name().and_then(|n| n.to_str()).is_some_and(|n| !n.starts_with('.'));
        if shown && entry.file_type().is_ok_and(|t| t.is_dir()) && text(&path, "folder").is_ok() {
            dirs.push(path);
        }
    }
    dirs.sort();
    for dir in dirs {
        if found.projects.len() >= limit || *budget == 0 {
            found.cut_short = true;
            return;
        }
        if dir.extension().is_some_and(|e| e == EXTENSION) {
            let named = dir.file_stem().is_some_and(|n| !n.is_empty());
            if named && !unfinished(&dir) {
                found.projects.push(dir);
            }
        } else if depth > 0 {
            find_in(&dir, depth - 1, limit, budget, found);
        }
    }
}

/// Whether a project folder is a creation that did not finish: it holds the marker, or it is empty
/// and named as a project (any other empty folder is simply not a project).
fn unfinished(path: &Path) -> bool {
    let named = path.extension().is_some_and(|e| e == EXTENSION);
    std::fs::symlink_metadata(path.join(MARKER)).is_ok() || (named && is_empty(path))
}

fn is_empty(path: &Path) -> bool {
    std::fs::read_dir(path).is_ok_and(|mut e| e.next().is_none())
}

/// Stops a server a crashed Archforge left on `path`, if it can prove it started it; a failure only
/// goes to the log.
fn recover(path: &Path, opts: &ServerOptions) {
    if let Err(e) = archforge_dolt::recover(path, opts) {
        log::warn!("the unfinished project {} could not be checked for a leftover server: {e}", path.display());
    }
}

/// The folder a creation made, and how to recognise it later.
struct Made {
    path: PathBuf,
    identity: Option<(u64, u64)>,
}

#[cfg(unix)]
fn identity(path: &Path) -> Option<(u64, u64)> {
    use std::os::unix::fs::MetadataExt;
    std::fs::symlink_metadata(path).ok().map(|m| (m.dev(), m.ino()))
}

/// Windows: std has no stable file id, so the marker alone proves the folder is ours.
#[cfg(not(unix))]
fn identity(_path: &Path) -> Option<(u64, u64)> {
    None
}

#[cfg(unix)]
fn make_private_dir(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::DirBuilderExt;
    std::fs::DirBuilder::new().mode(0o700).create(path)
}

#[cfg(not(unix))]
fn make_private_dir(path: &Path) -> std::io::Result<()> {
    std::fs::create_dir(path)
}

/// Undoes a creation: stops its server, then removes the folder only while it is still the one this
/// creation made (the same folder, with the marker or empty). Gives the error to report.
fn undo(server: Option<Server>, made: &Made, cause: Error) -> Error {
    if let Some(server) = server
        && let Err(e) = server.stop()
    {
        log::warn!("the server of the failed project {} did not stop, so the folder stays: {e}", made.path.display());
        return Error::Kept { path: made.path.clone(), cause: cause.to_string() };
    }
    // On unix the folder must have the identity recorded when it was made; one that could not be
    // read proves nothing. Windows has no stable file id, so the marker alone is the proof there.
    let same_identity = if cfg!(unix) { made.identity.is_some() && identity(&made.path) == made.identity } else { true };
    let same = std::fs::symlink_metadata(&made.path).is_ok_and(|m| m.is_dir() && !m.file_type().is_symlink())
        && same_identity
        && (std::fs::symlink_metadata(made.path.join(MARKER)).is_ok() || is_empty(&made.path));
    if !same {
        log::warn!("the failed project {} changed meanwhile and stays", made.path.display());
        return Error::Kept { path: made.path.clone(), cause: cause.to_string() };
    }
    if let Err(e) = std::fs::remove_dir_all(&made.path) {
        log::warn!("the failed project {} could not be removed: {e}", made.path.display());
        return Error::Kept { path: made.path.clone(), cause: format!("{cause}; removing it failed: {e}") };
    }
    cause
}

/// The steps of a creation that need the server: start it, make the database and its settings,
/// commit, and read them back.
#[allow(clippy::too_many_arguments)]
fn build(
    dolt: &Dolt,
    opts: &ServerOptions,
    target: &Path,
    name: &str,
    created_by: &str,
    now: &str,
    hooks: &Hooks,
    server: &mut Option<Server>,
) -> Result<Settings, Error> {
    hooks.cancelled()?;
    let started = Server::start(dolt, target, opts).map_err(Error::Server)?;
    let pid = started.pid();
    let s = server.insert(started);
    if let Some(after) = &hooks.after_start {
        after(pid, target).map_err(|why| Error::Damaged { path: target.to_path_buf(), why })?;
    }
    hooks.cancelled()?;
    let mut c = s.connect().map_err(Error::Server)?;
    let sql = |what: &'static str| move |e| Error::Sql { what, source: e };
    c.query_drop(format!("CREATE DATABASE `{DATABASE}`")).map_err(sql("cannot create the project database"))?;
    c.query_drop(format!("USE `{DATABASE}`")).map_err(sql("cannot use the project database"))?;
    c.query_drop(format!("CREATE TABLE settings (`key` VARCHAR({MAX_KEY}) PRIMARY KEY, `value` VARCHAR({MAX_VALUE}) NOT NULL)"))
        .map_err(sql("cannot create the settings table"))?;
    let format = FORMAT.to_string();
    let rows = [("format", format.as_str()), ("name", name), ("created_by", created_by), ("created_at", now)];
    c.exec_batch("INSERT INTO settings (`key`, `value`) VALUES (?, ?)", rows).map_err(sql("cannot write the settings"))?;

    // The commit's reply is not read: converting a row that is not one string would panic.
    c.query_drop(format!("CALL DOLT_COMMIT('-Am', 'Create the project', '--author', '{AUTHOR}')")).map_err(sql("cannot commit the project"))?;
    hooks.cancelled()?;
    drop(c);
    let settings = read_settings(s, target)?;
    if settings.name != name {
        return Err(Error::Damaged { path: target.to_path_buf(), why: "the name read back differs".into() });
    }
    Ok(settings)
}

/// The settings, read with a cap on rows, keys and values; anything that does not read cleanly is
/// a damaged project, never a panic.
fn read_settings(server: &Server, path: &Path) -> Result<Settings, Error> {
    let damaged = |why: String| Error::Damaged { path: path.to_path_buf(), why };
    let mut c = server.connect().map_err(Error::Server)?;
    c.query_drop(format!("USE `{DATABASE}`")).map_err(|e| damaged(format!("its {DATABASE} database cannot be used: {e}")))?;
    let kind: Option<mysql::Row> = c
        .exec_first("SELECT table_type FROM information_schema.tables WHERE table_schema = ? AND table_name = 'settings'", (DATABASE,))
        .map_err(|e| damaged(format!("its tables cannot be listed: {e}")))?;
    let kind: Option<String> = kind.and_then(|r| r.get_opt::<String, usize>(0)).and_then(Result::ok);
    if kind.as_deref() != Some("BASE TABLE") {
        return Err(damaged("it has no settings table".into()));
    }
    let query = format!(
        "SELECT LEFT(CAST(`key` AS CHAR), {}), LEFT(CAST(`value` AS CHAR), {}) FROM settings LIMIT {}",
        MAX_KEY + 1,
        MAX_VALUE + 1,
        MAX_ROWS + 1
    );
    let rows: Vec<mysql::Row> = c.query(query).map_err(|e| damaged(format!("its settings cannot be read: {e}")))?;
    if rows.len() > MAX_ROWS {
        return Err(damaged(format!("it has more than {MAX_ROWS} settings")));
    }
    let mut map = std::collections::BTreeMap::new();
    for row in rows {
        let cell = |i: usize| row.get_opt::<String, usize>(i).and_then(Result::ok);
        let (Some(key), Some(value)) = (cell(0), cell(1)) else { return Err(damaged("a setting is not text".into())) };
        if key.chars().count() > MAX_KEY || value.chars().count() > MAX_VALUE {
            return Err(damaged(format!("the setting {:?} is too long", key.chars().take(MAX_KEY).collect::<String>())));
        }
        map.insert(key, value);
    }
    let format = map.get("format").ok_or_else(|| damaged("it has no format".into()))?;
    let format: u32 = format.parse().map_err(|_| damaged(format!("its format {format:?} is not a number")))?;
    readable(format, FORMAT)?;
    let name = map.get("name").ok_or_else(|| damaged("it has no name".into()))?.clone();
    if let Some(problem) = archforge_names::problem(&name) {
        return Err(damaged(format!("its name cannot be a file name ({problem:?})")));
    }
    let get = |k: &str| map.get(k).cloned().unwrap_or_default();
    Ok(Settings { format, name, created_by: get("created_by"), created_at: get("created_at") })
}

/// Refuses a project in a format newer than `supported`, the newest this Archforge reads.
fn readable(format: u32, supported: u32) -> Result<(), Error> {
    if format > supported {
        return Err(Error::NewerFormat { found: format, supported });
    }
    Ok(())
}

/// Refuses a project whose database holds triggers, stored procedures or events: Archforge's own
/// writes would set them off, and they could change what it stores or what Save commits.
fn unscripted(server: &Server, path: &Path, settings: Settings) -> Result<Settings, Error> {
    let mut c = server.connect().map_err(Error::Server)?;
    c.query_drop(format!("USE `{DATABASE}`")).map_err(|e| Error::Sql { what: "cannot use the project database", source: e })?;
    for (what, table, column) in
        [("triggers", "triggers", "trigger_schema"), ("stored procedures", "routines", "routine_schema"), ("events", "events", "event_schema")]
    {
        let any: Option<mysql::Row> = c
            .exec_first(format!("SELECT 1 FROM information_schema.{table} WHERE {column} = ? LIMIT 1"), (DATABASE,))
            .map_err(|e| Error::Sql { what: "cannot list the project's scripts", source: e })?;
        if any.is_some() {
            return Err(Error::Scripted { path: path.to_path_buf(), what });
        }
    }
    Ok(settings)
}

/// Refuses a project folder others may write to, or another user's.
#[cfg(unix)]
fn private(path: &Path) -> Result<(), Error> {
    use std::os::unix::fs::MetadataExt;
    let meta = std::fs::metadata(path).map_err(|e| Error::io("cannot read", path, e))?;
    let mode = meta.mode() & 0o7777;
    let me = archforge_dolt::user_id().map_err(|e| Error::io("cannot tell this user's id for", path, e))?;
    if meta.uid() != me {
        return Err(Error::NotYours { path: path.to_path_buf() });
    }
    if mode & 0o022 != 0 {
        return Err(Error::NotPrivate { path: path.to_path_buf(), mode });
    }
    Ok(())
}

/// Windows: std cannot read a folder's owner or its access list.
#[cfg(not(unix))]
fn private(_path: &Path) -> Result<(), Error> {
    Ok(())
}

/// The project folder's own `.dolt` may hold only the lock a server leaves; anything else, such as
/// a `config.json` with server settings, is refused, as is a `.dolt` that is a link or a file.
fn own_dot_dolt(path: &Path) -> Result<(), Error> {
    let dot = path.join(".dolt");
    let foreign = |entry: String| Error::Foreign { path: path.to_path_buf(), entry };
    let meta = match std::fs::symlink_metadata(&dot) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(Error::io("cannot read", &dot, e)),
    };
    if !meta.is_dir() || meta.file_type().is_symlink() {
        return Err(foreign(".dolt is not a folder".into()));
    }
    let entries = std::fs::read_dir(&dot).map_err(|e| Error::io("cannot read", &dot, e))?;
    for e in entries {
        let e = e.map_err(|e| Error::io("cannot read", &dot, e))?;
        let name = e.file_name().to_string_lossy().into_owned();
        let plain = std::fs::symlink_metadata(e.path()).is_ok_and(|m| m.is_file());
        if name != "sql-server.info" || !plain {
            return Err(foreign(format!(".dolt/{name}")));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_format_3_project_is_refused_by_the_check_of_an_archforge_that_reads_up_to_format_2() {
        // Archforge 0.12 read up to format 2 with this same check: a project that holds
        // conversations says format 3, and is refused with the newer-format error, never misread.
        assert!(matches!(readable(3, 2), Err(Error::NewerFormat { found: 3, supported: 2 })));
        assert!(readable(2, 2).is_ok());
        assert!(readable(2, FORMAT).is_ok() && readable(3, FORMAT).is_ok());
        assert!(matches!(readable(FORMAT + 1, FORMAT), Err(Error::NewerFormat { .. })));
    }

    #[test]
    fn find_lists_finished_projects_down_to_its_depth_and_no_further() {
        let root = std::env::temp_dir().join(format!("af-find-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let dir = |p: &str| {
            let d = root.join(p);
            std::fs::create_dir_all(&d).unwrap();
            d
        };
        let file = |p: &str| std::fs::write(root.join(p), b"x").unwrap();
        dir("Shop.archforge");
        file("Shop.archforge/config.yaml");
        dir("Client/Billing.archforge");
        file("Client/Billing.archforge/config.yaml");
        dir("Client/Deep/Far.archforge");
        file("Client/Deep/Far.archforge/config.yaml");
        dir("Empty.archforge");
        dir("Half.archforge");
        file(&format!("Half.archforge/{MARKER}"));
        dir(".hidden/Secret.archforge");
        file(".hidden/Secret.archforge/config.yaml");
        dir("Shop.archforge/Inner.archforge");
        file("Shop.archforge/Inner.archforge/config.yaml");
        file("Loose.archforge");
        dir("Odd\nname/Hidden.archforge");
        file("Odd\nname/Hidden.archforge/config.yaml");
        let names = |found: &Found| found.projects.iter().map(|p| p.strip_prefix(&root).unwrap().to_string_lossy().into_owned()).collect::<Vec<_>>();
        let all = find(&root, 1, 50, &mut 1_000);
        assert_eq!(names(&all), ["Client/Billing.archforge", "Shop.archforge"], "a folder whose name is not plain text is passed");
        assert!(!all.cut_short);
        assert_eq!(names(&find(&root, 2, 50, &mut 1_000)), ["Client/Billing.archforge", "Client/Deep/Far.archforge", "Shop.archforge"]);
        assert_eq!(names(&find(&root, 0, 50, &mut 1_000)), ["Shop.archforge"]);
        let one = find(&root, 2, 1, &mut 1_000);
        assert_eq!((one.projects.len(), one.cut_short), (1, true), "the limit holds and says so");
        let mut budget = 3;
        assert!(find(&root, 2, 50, &mut budget).cut_short, "so does the budget");
        assert_eq!(budget, 0);
        assert_eq!(find(&root.join("missing"), 2, 50, &mut 1_000), Found { unreadable: 1, ..Found::default() });
        let _ = std::fs::remove_dir_all(&root);
    }
}
