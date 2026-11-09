//! The current branch and the branches of a project. Every connection uses the current branch, so
//! every read and write follows it; the list says of each branch whether it holds unsaved work.

use std::fmt;
use std::sync::{Arc, Mutex};

use mysql::prelude::Queryable;

use crate::branch_name::{SafeBranch, UnsafeBranch};
use crate::chat::{head, is_commit};
use crate::creation::{self, BranchStore, CreationRecord, CreationRecovery, Found};
use crate::model::{ModelSchema, database_connection, database_name, sql, unsaved_tables};
use crate::{DATABASE, Error, Project};
use archforge_mof::Uuid;
use archforge_mof::rows::Rows;

/// The most branches listed.
pub const MAX_BRANCHES: usize = 200;
/// The most bytes read of a branch's name: one more than a usable name can have, so a longer one
/// is refused as too long and not cut into a name that fits.
const NAME_BYTES: usize = 513;

/// The current branch, shared by the project and its readers so they all follow it.
pub(crate) type CurrentBranch = Arc<Mutex<SafeBranch>>;

/// The branch `current` holds now.
pub(crate) fn branch_now(current: &CurrentBranch) -> SafeBranch {
    current.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).clone()
}

/// The branches over one connection, as recovery asks of them.
struct OpenBranches(Result<mysql::Conn, String>);

impl OpenBranches {
    /// Renames the branch; what the answer is worth is for the caller to read back.
    fn rename(&mut self, from: &SafeBranch, to: &SafeBranch) -> Result<(), String> {
        let c = self.0.as_mut().map_err(|e| e.clone())?;
        c.exec_drop("CALL DOLT_BRANCH('-m', ?, ?)", (from.stored(), to.stored())).map_err(|e| e.to_string())
    }
}

impl BranchStore for OpenBranches {
    /// Reads the head. Only a byte-exact name counts: `DOLT_BRANCH('-D')` is exact, and a record
    /// naming `FOO` must not reach a branch `foo`.
    fn head(&mut self, branch: &SafeBranch) -> Result<Option<String>, String> {
        let c = self.0.as_mut().map_err(|e| e.clone())?;
        let query =
            format!("SELECT LEFT(CAST(name AS CHAR), {NAME_BYTES}), LEFT(CAST(hash AS CHAR), 64) FROM dolt_branches WHERE LOWER(name) = LOWER(?)");
        let rows: Vec<mysql::Row> = c.exec(query, (branch.stored(),)).map_err(|e| e.to_string())?;
        let exact = |row: &&mysql::Row| row.get_opt::<String, usize>(0).and_then(Result::ok).as_deref() == Some(branch.stored());
        let Some(row) = rows.iter().find(exact) else { return Ok(None) };
        let hash = row.get_opt::<String, usize>(1).and_then(Result::ok).unwrap_or_default();
        if is_commit(&hash) { Ok(Some(hash)) } else { Err("its head does not read as a commit".into()) }
    }

    /// Deletes the branch, whatever it holds.
    fn delete(&mut self, branch: &SafeBranch) -> Result<(), String> {
        let c = self.0.as_mut().map_err(|e| e.clone())?;
        c.exec_drop("CALL DOLT_BRANCH('-D', ?)", (branch.stored(),)).map_err(|e| e.to_string())
    }
}

/// A step of making a branch, as a failure names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreationStep {
    CheckName,
    CheckVersion,
    WriteRecord,
    MakeBranch,
    PrepareModel,
    /// Reading the source's unsaved rows and its head.
    ReadWork,
    /// Writing those rows to the new branch.
    CopyWork,
    RemoveConversations,
    ChatCommit,
    RemoveRecord,
}

/// A branch that was made. Its chat commit marks it finished, so nothing after that undoes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BranchCreated {
    /// The record is gone and the branch is listed.
    Listed,
    /// The record could not be removed. The branch is complete but left out of the list, and
    /// another creation is refused, until the next opening removes the record.
    RecordStays { why: String },
}

/// Why a branch was not made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BranchCreationError {
    /// A record from an earlier creation remains, until the next opening clears it.
    RecordRemains,
    /// A branch of that name exists, in any case.
    NameTaken,
    /// The start is not a commit hash.
    NotACommit,
    /// The version holds these scripts, which Archforge does not run.
    Scripted { what: &'static str },
    /// `step`, before the chat commit, failed, and the project is as it was: no branch, no record.
    Failed { step: CreationStep, why: String },
    /// `step`, before the chat commit, failed and cleaning up did too. The record stays, so the
    /// next opening finishes it.
    NotCleanedUp { step: CreationStep, why: String, cleaning: String },
}

impl fmt::Display for BranchCreationError {
    /// The reason in English, for logs; the dialog words its own.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BranchCreationError::RecordRemains => f.write_str("the last new branch is not cleaned up yet"),
            BranchCreationError::NameTaken => f.write_str("a branch of that name exists"),
            BranchCreationError::NotACommit => f.write_str("the start is not a commit hash"),
            BranchCreationError::Scripted { what } => write!(f, "the version holds {what}, which Archforge does not run"),
            BranchCreationError::Failed { step, why } => write!(f, "{step:?} failed: {why}"),
            BranchCreationError::NotCleanedUp { step, why, cleaning } => write!(f, "{step:?} failed: {why}; cleaning up failed: {cleaning}"),
        }
    }
}

impl std::error::Error for BranchCreationError {}

/// Why a branch was not renamed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenameBranchError {
    /// `main` keeps its name, in any case.
    Main,
    /// No branch has exactly this name.
    NoSuchBranch,
    /// A branch has the new name, in any case, this one included.
    NameTaken,
    /// A creation record names one of the two, until the next opening clears it.
    RecordRemains,
    /// The rename did not happen; Dolt's own words, such as another session being on the branch.
    Failed(String),
    /// The reply was lost and where the branch is could not be read. Until the same rename is asked
    /// again or the project closes, nothing is written to either name if it is the branch in use.
    CannotTell(String),
}

impl fmt::Display for RenameBranchError {
    /// The reason in English, for logs; the dialog words its own.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RenameBranchError::Main => f.write_str("main cannot be renamed"),
            RenameBranchError::NoSuchBranch => f.write_str("there is no such branch"),
            RenameBranchError::NameTaken => f.write_str("a branch of that name exists"),
            RenameBranchError::RecordRemains => f.write_str("the last new branch is not cleaned up yet"),
            RenameBranchError::Failed(why) | RenameBranchError::CannotTell(why) => f.write_str(why),
        }
    }
}

impl std::error::Error for RenameBranchError {}

/// Why a branch was not deleted, or whether it was cannot be said.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeleteBranchError {
    /// `main` stays, in any case.
    Main,
    /// The branch in use stays, in any case.
    Current,
    /// A creation record names it: the next opening deals with it.
    RecordRemains,
    /// No branch has exactly this name.
    NoSuchBranch,
    /// An unclear rename holds the project until it is settled or closes.
    Held,
    /// It is still there, or what it held could not be read; nothing was deleted.
    Failed(String),
    /// Whether it went cannot be told. Asking for the same delete again settles it.
    CannotTell(String),
}

impl fmt::Display for DeleteBranchError {
    /// The reason in English, for logs; the dialog words its own.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DeleteBranchError::Main => f.write_str("main cannot be deleted"),
            DeleteBranchError::Current => f.write_str("the branch in use cannot be deleted"),
            DeleteBranchError::RecordRemains => f.write_str("the last new branch is not cleaned up yet"),
            DeleteBranchError::NoSuchBranch => f.write_str("there is no such branch"),
            DeleteBranchError::Held => f.write_str("whether a branch was renamed cannot be told"),
            DeleteBranchError::Failed(why) | DeleteBranchError::CannotTell(why) => f.write_str(why),
        }
    }
}

impl std::error::Error for DeleteBranchError {}

/// A step that did not go on: `clean` is false when a test stops as a crash would.
struct Halted {
    step: CreationStep,
    why: String,
    clean: bool,
}

/// Told of each step before it runs; a test's answer stands in for that step failing or a stop.
type StepWatch<'a> = &'a mut dyn FnMut(CreationStep) -> Result<(), Halted>;

/// Maps a failure of `step` to its halt.
fn failed<E: fmt::Display>(step: CreationStep) -> impl Fn(E) -> Halted {
    move |e| Halted { step, why: e.to_string(), clean: true }
}

/// One branch of the project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListedBranch {
    /// The name as the project holds it.
    pub name: String,
    /// The name to use, or why it cannot be opened.
    pub usable: Result<SafeBranch, UnsafeBranch>,
    /// Whether connections use this branch now.
    pub current: bool,
    /// Whether it holds changes to the model that are not saved: conversations do not count. None
    /// when it holds changes that cannot be looked at, as its name cannot be used.
    pub unsaved: Option<bool>,
}

/// The branches of a project, by name.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Branches {
    pub listed: Vec<ListedBranch>,
    /// Whether there are more than the list holds.
    pub cut: bool,
}

impl Project {
    /// The branch every connection of this project uses.
    pub fn current_branch(&self) -> SafeBranch {
        branch_now(&self.branch)
    }

    /// Points every later connection at `branch`, and nothing more: the caller has already done
    /// what leaving one branch for another takes.
    pub fn set_current_branch(&self, branch: SafeBranch) {
        *self.branch.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = branch;
    }

    /// Moves this project to `branch`, which has passed [`Project::check_branch`]: connections
    /// follow it, and the change ledger starts again, as on opening.
    pub fn move_to_branch(&self, branch: SafeBranch) {
        self.set_current_branch(branch);
        self.forget_changes();
    }

    /// Runs `statement` on `branch`, for a test to shape a branch with.
    #[cfg(feature = "testing")]
    pub fn run_on_branch(&self, branch: &str, statement: &str) -> Result<(), String> {
        let mut c = self.server().connector().connect().map_err(|e| e.to_string())?;
        c.query_drop(format!("USE `project/{branch}`")).map_err(|e| e.to_string())?;
        c.query_drop(statement).map_err(|e| e.to_string())
    }

    /// The head of `branch`, for a test to record as a start.
    #[cfg(feature = "testing")]
    pub fn head_of(&self, branch: &str) -> Result<String, String> {
        let mut c = self.server().connector().connect().map_err(|e| e.to_string())?;
        c.query_drop(format!("USE `project/{branch}`")).map_err(|e| e.to_string())?;
        crate::chat::head(&mut c)
    }

    /// Runs `query` on `branch` and gives its first column as text, for a test to look with.
    #[cfg(feature = "testing")]
    pub fn first_on_branch(&self, branch: &str, query: &str) -> Result<String, String> {
        let mut c = self.server().connector().connect().map_err(|e| e.to_string())?;
        c.query_drop(format!("USE `project/{branch}`")).map_err(|e| e.to_string())?;
        let row: Option<mysql::Row> = c.query_first(query).map_err(|e| e.to_string())?;
        row.and_then(|r| r.get_opt::<String, usize>(0)).and_then(Result::ok).ok_or_else(|| "no text".to_string())
    }

    /// [`Project::create_branch`], halted just before `step`: a stop as a crash would, or a failure.
    #[cfg(feature = "testing")]
    pub fn create_branch_halted(
        &self,
        name: &SafeBranch,
        start: Option<&str>,
        schema: &ModelSchema,
        halt: (CreationStep, bool),
    ) -> Result<BranchCreated, BranchCreationError> {
        let (step, clean) = halt;
        let mut watch = |s: CreationStep| if s == step { Err(Halted { step, why: "halted".into(), clean }) } else { Ok(()) };
        match start {
            Some(start) => self.create_branch_watched(name, start, None, schema, &mut watch),
            None => self.create_branch_from_work_watched(name, schema, &mut watch),
        }
    }

    /// [`Project::rename_branch`] with its answer and its read-back shaped by a test: whether the
    /// rename lands, whether its reply comes, and whether where the branch is can be read.
    #[cfg(feature = "testing")]
    pub fn rename_branch_faulted(
        &self,
        from: &SafeBranch,
        to: &SafeBranch,
        lands: bool,
        answered: bool,
        readable: bool,
    ) -> Result<(), RenameBranchError> {
        let ask = |store: &mut OpenBranches| {
            let done = if lands { store.rename(from, to) } else { Ok(()) };
            done.and_then(|()| if answered { Ok(()) } else { Err("the reply was lost".to_string()) })
        };
        self.rename_steps(from, to, ask, || if readable { self.where_is(from, to) } else { Err("it cannot be read".to_string()) })
    }

    /// Renames the branch `from` to `to`, on the writer only. A branch this project is on stays
    /// current under its new name. Conversations and unsaved work go with it, and a rename does not
    /// move the switch generation, so jobs already asked for still run.
    ///
    /// Names are compared as Dolt stores them, where `foo` and `Foo` are one name, so a rename to
    /// the same stored name in another case is refused as taken. That is no limit on what a person
    /// types: [`SafeBranch::typed`] encodes anything not lower case, so `foo` and `Foo` from the
    /// dialog are two different stored names and both allowed.
    pub fn rename_branch(&self, from: &SafeBranch, to: &SafeBranch) -> Result<(), RenameBranchError> {
        self.rename_steps(from, to, |store| store.rename(from, to), || self.where_is(from, to))
    }

    /// The rename, from and to, whose outcome could not be told.
    fn unclear(&self) -> std::sync::MutexGuard<'_, Option<(SafeBranch, SafeBranch)>> {
        self.unclear_rename.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Whether a rename of the branch in use has an unknown outcome.
    pub(crate) fn branch_is_unclear(&self) -> bool {
        self.unclear().as_ref().is_some_and(|(from, _)| from.same_branch(&self.current_branch()))
    }

    /// Whether `from` and `to` exist, over a connection of its own, as the one that asked may be
    /// the one that was lost, and on `main`, which no unclear rename can be.
    fn where_is(&self, from: &SafeBranch, to: &SafeBranch) -> Result<(bool, bool), String> {
        let mut store = OpenBranches(database_connection(&self.server().connector(), &SafeBranch::default()).map_err(|e| e.to_string()));
        Ok((store.head(from)?.is_some(), store.head(to)?.is_some()))
    }

    /// The rename has happened: a branch in use stays current under its new name.
    fn renamed(&self, from: &SafeBranch, to: &SafeBranch) {
        if from.same_branch(&self.current_branch()) {
            self.set_current_branch(to.clone());
        }
    }

    /// [`Project::rename_branch`], `ask` making the rename and `read` saying which of the names
    /// exist. Dolt's success is taken as the rename done. Its failure is read back, as the reply
    /// can be what was lost: only "still at the old name" is a failure, and not being able to tell
    /// holds the project until the same rename is asked for again or it closes.
    fn rename_steps(
        &self,
        from: &SafeBranch,
        to: &SafeBranch,
        ask: impl FnOnce(&mut OpenBranches) -> Result<(), String>,
        read: impl Fn() -> Result<(bool, bool), String>,
    ) -> Result<(), RenameBranchError> {
        let failed = |why: String| RenameBranchError::Failed(why);
        if from.same_branch(&SafeBranch::default()) {
            return Err(RenameBranchError::Main);
        }
        if self.unclear().as_ref().is_some_and(|(was, then)| was == from && then == to) {
            match read() {
                Ok((true, false)) => *self.unclear() = None,
                Ok((false, true)) => {
                    *self.unclear() = None;
                    self.renamed(from, to);
                    return Ok(());
                }
                other => return Err(RenameBranchError::CannotTell(other.err().unwrap_or_else(|| "neither name, or both, can be found".into()))),
            }
        } else if self.branch_is_unclear() {
            return Err(RenameBranchError::CannotTell("an earlier rename of this branch is not settled".into()));
        }
        if let Found::Good(record) = creation::read(self.path())
            && (record.branch.same_branch(to) || record.branch.same_branch(from))
        {
            return Err(RenameBranchError::RecordRemains);
        }
        // Over a connection on `main`, which stays good whatever becomes of the branch. Dolt refuses
        // while another session is on the branch (measured); that comes back as a failure.
        let mut store = OpenBranches(self.connection_to(&SafeBranch::default()).map_err(|e| e.to_string()));
        if store.head(from).map_err(failed)?.is_none() {
            return Err(RenameBranchError::NoSuchBranch);
        }
        if self.branch_name_taken(to).map_err(|e| failed(e.to_string()))? {
            return Err(RenameBranchError::NameTaken);
        }
        let Err(why) = ask(&mut store) else {
            self.renamed(from, to);
            return Ok(());
        };
        match read() {
            Ok((false, true)) => {
                self.renamed(from, to);
                Ok(())
            }
            Ok((true, false)) => Err(failed(why)),
            _ => {
                *self.unclear() = Some((from.clone(), to.clone()));
                Err(RenameBranchError::CannotTell(why))
            }
        }
    }

    /// [`Project::delete_branch`] shaped by a test: whether the delete lands, whether where the
    /// branch is can be read, and `asked` set if the delete was asked for at all.
    #[cfg(feature = "testing")]
    pub fn delete_branch_faulted(
        &self,
        branch: &SafeBranch,
        lands: bool,
        readable: bool,
        asked: &std::cell::Cell<bool>,
    ) -> Result<Vec<Uuid>, DeleteBranchError> {
        let ask = |store: &mut OpenBranches| {
            asked.set(true);
            if lands { store.delete(branch) } else { Err("the reply was lost".to_string()) }
        };
        self.delete_steps(branch, ask, || if readable { self.branch_is_there(branch) } else { Err("it cannot be read".to_string()) })
    }

    /// Deletes the branch for good, on the writer only, and answers the conversations that held
    /// only there, for their private folders to be let go: none unless the branch is seen gone.
    /// A read on the branch at that moment fails; it never answers for something else. No reader
    /// starts on it, as readers follow the current branch, which this refuses.
    pub fn delete_branch(&self, branch: &SafeBranch) -> Result<Vec<Uuid>, DeleteBranchError> {
        self.delete_steps(branch, |store| store.delete(branch), || self.branch_is_there(branch))
    }

    /// Whether `branch` exists, over a connection of its own on `main`.
    fn branch_is_there(&self, branch: &SafeBranch) -> Result<bool, String> {
        let mut store = OpenBranches(database_connection(&self.server().connector(), &SafeBranch::default()).map_err(|e| e.to_string()));
        Ok(store.head(branch)?.is_some())
    }

    /// The delete whose outcome is unknown.
    fn unclear_delete(&self) -> std::sync::MutexGuard<'_, Option<(SafeBranch, Vec<Uuid>)>> {
        self.unclear_delete.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// [`Project::delete_branch`], `ask` deleting and `read` saying whether the branch exists. The
    /// read decides, whatever the answer was. Not being able to tell keeps what it held, so
    /// that asking again answers as the first would have.
    fn delete_steps(
        &self,
        branch: &SafeBranch,
        ask: impl FnOnce(&mut OpenBranches) -> Result<(), String>,
        read: impl Fn() -> Result<bool, String>,
    ) -> Result<Vec<Uuid>, DeleteBranchError> {
        let failed = |why: String| DeleteBranchError::Failed(why);
        if branch.same_branch(&SafeBranch::default()) {
            return Err(DeleteBranchError::Main);
        }
        if branch.same_branch(&self.current_branch()) {
            return Err(DeleteBranchError::Current);
        }
        if let Found::Good(record) = creation::read(self.path())
            && record.branch.same_branch(branch)
        {
            return Err(DeleteBranchError::RecordRemains);
        }
        let earlier = self.unclear_delete().clone().filter(|(was, _)| was == branch);
        if let Some((_, chats)) = earlier {
            match read() {
                Ok(false) => {
                    *self.unclear_delete() = None;
                    return Ok(chats);
                }
                Ok(true) => *self.unclear_delete() = None,
                Err(why) => return Err(DeleteBranchError::CannotTell(why)),
            }
        }
        let connected = self.connection_to(&SafeBranch::default());
        if matches!(connected, Err(Error::BranchUnclear { .. })) {
            return Err(DeleteBranchError::Held);
        }
        let mut store = OpenBranches(connected.map_err(|e| e.to_string()));
        if store.head(branch).map_err(failed)?.is_none() {
            return Err(DeleteBranchError::NoSuchBranch);
        }
        let chats = self.version_reader().chats_only_on(branch).map_err(|e| failed(e.to_string()))?;
        let asked = ask(&mut store);
        match read() {
            Ok(false) => Ok(chats),
            Ok(true) => Err(failed(asked.err().unwrap_or_else(|| "the branch is still there".into()))),
            Err(why) => {
                *self.unclear_delete() = Some((branch.clone(), chats));
                Err(DeleteBranchError::CannotTell(why))
            }
        }
    }

    /// Makes the branch `name` at the version `start`, with no conversations. On the writer only.
    /// A version holding triggers, procedures or events is refused first, as an opening refuses
    /// them, so none can run. Then, in this order: the record, the branch, its model tables, the
    /// inherited conversations removed, the chat commit that marks it finished, and the record
    /// removed. If anything fails up to that commit the branch is deleted and then the record
    /// removed; after it the branch is finished and stays, whatever becomes of the record.
    ///
    /// The screen that asks for this comes with AR-292.
    pub fn create_branch(&self, name: &SafeBranch, start: &str, schema: &ModelSchema) -> Result<BranchCreated, BranchCreationError> {
        self.create_branch_watched(name, start, None, schema, &mut |_| Ok(()))
    }

    /// Makes the branch `name` at the head, with the source's unsaved rows and no conversations,
    /// as [`Project::create_branch`] does, and the rows copied once its model tables are prepared.
    /// On the writer only, once what waits is written. The source keeps its own unsaved work and
    /// its conversations, committed or not: only the model's rows are read from it.
    pub fn create_branch_from_work(&self, name: &SafeBranch, schema: &ModelSchema) -> Result<BranchCreated, BranchCreationError> {
        self.create_branch_from_work_watched(name, schema, &mut |_| Ok(()))
    }

    /// [`Project::create_branch_from_work`], telling `watch` of each step.
    fn create_branch_from_work_watched(
        &self,
        name: &SafeBranch,
        schema: &ModelSchema,
        watch: StepWatch,
    ) -> Result<BranchCreated, BranchCreationError> {
        let refused = |why: String| BranchCreationError::Failed { step: CreationStep::ReadWork, why };
        let start = self.model_connection().map_err(|e| e.to_string()).and_then(|mut c| head(&mut c)).map_err(refused)?;
        let work = self.read_model(schema).map_err(|e| refused(e.to_string()))?;
        self.create_branch_watched(name, &start, Some(&work), schema, watch)
    }

    /// [`Project::create_branch`], telling `watch` of each step. A record that remains goes first,
    /// so that a branch it hides is reported as that and not as a name taken.
    fn create_branch_watched(
        &self,
        name: &SafeBranch,
        start: &str,
        work: Option<&Rows>,
        schema: &ModelSchema,
        watch: StepWatch,
    ) -> Result<BranchCreated, BranchCreationError> {
        let refused = |step: CreationStep, e: Error| BranchCreationError::Failed { step, why: e.to_string() };
        if creation::read(self.path()) != Found::None {
            return Err(BranchCreationError::RecordRemains);
        }
        let record = CreationRecord::new(name.clone(), self.current_branch(), start).ok_or(BranchCreationError::NotACommit)?;
        if self.branch_name_taken(name).map_err(|e| refused(CreationStep::CheckName, e))? {
            return Err(BranchCreationError::NameTaken);
        }
        // As an opening checks a branch, and of the commit itself: only looking, so nothing is written
        // to a version whose scripts could run, and there is nothing to undo.
        match crate::checked_database(self.server(), self.path(), &format!("{DATABASE}/{}", record.start)) {
            Ok(_) => {}
            Err(Error::Scripted { what, .. }) => return Err(BranchCreationError::Scripted { what }),
            Err(e) => return Err(refused(CreationStep::CheckVersion, e)),
        }
        if !creation::write(self.path(), &record).map_err(|e| refused(CreationStep::WriteRecord, e))? {
            return Err(BranchCreationError::RecordRemains);
        }
        match self.build_branch(&record, work, schema, watch) {
            Ok(()) => Ok(BranchCreated::Listed),
            Err(Halted { step, why, clean: false }) => Err(BranchCreationError::Failed { step, why }),
            Err(Halted { step: CreationStep::RemoveRecord, why, .. }) => Ok(BranchCreated::RecordStays { why }),
            Err(Halted { step, why, .. }) => Err(self.undo_creation(name, step, why)),
        }
    }

    /// Whether a branch has this name in any case, among all of them, as Dolt treats such names as one.
    fn branch_name_taken(&self, name: &SafeBranch) -> Result<bool, Error> {
        let mut c = self.model_connection()?;
        let rows: Vec<mysql::Row> =
            c.exec("SELECT 1 FROM dolt_branches WHERE LOWER(name) = LOWER(?) LIMIT 1", (name.stored(),)).map_err(sql("cannot list the branches"))?;
        Ok(!rows.is_empty())
    }

    /// The steps after the record is written.
    fn build_branch(&self, record: &CreationRecord, work: Option<&Rows>, schema: &ModelSchema, watch: StepWatch) -> Result<(), Halted> {
        let name = &record.branch;
        watch(CreationStep::MakeBranch)?;
        let made = self
            .model_connection()
            .and_then(|mut c| c.exec_drop("CALL DOLT_BRANCH(?, ?)", (name.stored(), record.start.as_str())).map_err(sql("cannot make the branch")));
        made.map_err(failed(CreationStep::MakeBranch))?;
        watch(CreationStep::PrepareModel)?;
        let mut c = self.connection_to(name).and_then(|c| self.prepare_on(c, schema)).map_err(failed(CreationStep::PrepareModel))?;
        if let Some(work) = work {
            watch(CreationStep::CopyWork)?;
            self.write_model_on(&mut c, schema, work).map_err(failed(CreationStep::CopyWork))?;
        }
        watch(CreationStep::RemoveConversations)?;
        self.empty_chats(&mut c).map_err(failed(CreationStep::RemoveConversations))?;
        watch(CreationStep::ChatCommit)?;
        self.commit_new_branch(&mut c).map_err(failed(CreationStep::ChatCommit))?;
        watch(CreationStep::RemoveRecord)?;
        creation::remove_seen_gone(self.path()).map_err(failed(CreationStep::RemoveRecord))
    }

    /// After `step` failed: the creation is undone, and the answer says whether it could be.
    fn undo_creation(&self, name: &SafeBranch, step: CreationStep, why: String) -> BranchCreationError {
        let mut store = OpenBranches(self.model_connection().map_err(|e| e.to_string()));
        match creation::undo(self.path(), &mut store, name) {
            Ok(()) => BranchCreationError::Failed { step, why },
            Err(cleaning) => BranchCreationError::NotCleanedUp { step, why, cleaning },
        }
    }

    /// Cleans up after a branch creation that stopped, and logs what it did. For an opening, before
    /// anything lists or reads branches; see the `creation` module for why this is safe.
    pub fn recover_creation(&self) -> CreationRecovery {
        let mut store = OpenBranches(self.model_connection().map_err(|e| e.to_string()));
        let outcome = creation::recover(self.path(), &self.current_branch(), &mut store);
        if outcome != CreationRecovery::NoRecord {
            log::warn!("the unfinished branch creation of {}: {outcome:?}", self.path().display());
        }
        outcome
    }

    /// The project's branches, at most `max` of them. A name that cannot be used is listed with
    /// its reason rather than left out. Refused when `max` is over [`MAX_BRANCHES`], before
    /// anything is read: a dirty branch costs its own statements.
    pub fn branches(&self, max: usize) -> Result<Branches, Error> {
        if max > MAX_BRANCHES {
            return Err(Error::TooLarge { what: "a list", size: max, max: MAX_BRANCHES, unit: "branches" });
        }
        let mut c = self.model_connection()?;
        let current = self.current_branch();
        // What a record names is left out in the query, so the cap counts only what is listed. No
        // name is empty; a damaged record cannot say which branch it meant; the branch in use shows.
        let hidden = match creation::read(self.path()) {
            Found::Good(record) if !record.branch.same_branch(&current) => record.branch.stored().to_string(),
            _ => String::new(),
        };
        let query = format!(
            "SELECT LEFT(CAST(name AS CHAR), {NAME_BYTES}), dirty FROM dolt_branches WHERE LOWER(name) <> LOWER(?) ORDER BY name LIMIT {}",
            max.saturating_add(1)
        );
        let rows: Vec<mysql::Row> = c.exec(query, (hidden,)).map_err(sql("cannot list the branches"))?;
        let mut listed = Vec::new();
        for row in rows.iter().take(max) {
            let (name, dirty) = self.branch_row(row)?;
            let usable = SafeBranch::found(&name);
            let is_current = usable.as_ref().is_ok_and(|b| b.same_branch(&current));
            let unsaved = match &usable {
                _ if !dirty => Some(false),
                Err(_) => None,
                Ok(branch) => Some(self.model_changed_on(&mut c, branch)?),
            };
            listed.push(ListedBranch { name, usable, current: is_current, unsaved });
        }
        Ok(Branches { listed, cut: rows.len() > max })
    }

    /// Whether `branch`'s own working set holds changes other than conversations.
    fn model_changed_on(&self, c: &mut mysql::Conn, branch: &SafeBranch) -> Result<bool, Error> {
        c.query_drop(format!("USE `{}`", database_name(branch))).map_err(sql("cannot use the branch"))?;
        Ok(!unsaved_tables(c, self.path())?.is_empty())
    }

    /// A row of `dolt_branches` as its name and whether anything on it is uncommitted.
    fn branch_row(&self, row: &mysql::Row) -> Result<(String, bool), Error> {
        let damaged = || Error::Damaged { path: self.path().to_path_buf(), why: "a branch in its list cannot be read".into() };
        let name = match row.as_ref(0) {
            Some(mysql::Value::Bytes(b)) => std::str::from_utf8(b).map_err(|_| damaged())?.to_string(),
            _ => return Err(damaged()),
        };
        let dirty = match row.as_ref(1) {
            Some(mysql::Value::Int(n)) => *n != 0,
            Some(mysql::Value::Bytes(b)) => b.as_slice() != b"0",
            _ => return Err(damaged()),
        };
        Ok((name, dirty))
    }
}
