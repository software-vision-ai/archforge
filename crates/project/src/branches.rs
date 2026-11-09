//! The current branch and the branches of a project. Every connection uses the current branch, so
//! every read and write follows it; the list says of each branch whether it holds unsaved work.

use std::sync::{Arc, Mutex};

use mysql::prelude::Queryable;

use crate::branch_name::{SafeBranch, UnsafeBranch};
use crate::chat::is_commit;
use crate::creation::{self, BranchStore, CreationRecovery};
use crate::model::{database_name, sql, unsaved_tables};
use crate::{Error, Project};

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
        let query = format!("SELECT LEFT(CAST(name AS CHAR), {NAME_BYTES}), dirty FROM dolt_branches ORDER BY name LIMIT {}", max.saturating_add(1));
        let rows: Vec<mysql::Row> = c.query(query).map_err(sql("cannot list the branches"))?;
        let current = self.current_branch();
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
