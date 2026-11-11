//! The project's versions, read from the current branch's log for History: the commits Save made,
//! newest first, a page at a time.

use std::collections::{BTreeSet, VecDeque};

use archforge_mof::Uuid;
use mysql::prelude::Queryable;

use crate::DATABASE;
use crate::branch_name::SafeBranch;
use crate::branches::{CurrentBranch, branch_now};
use crate::chat::{id_at, is_commit, limits};
use crate::model::{database_connection, sql, unsaved_tables};
use crate::saving::{is_agent, is_version, shown_author};
use crate::{Error, Project};

/// How many commits at the old end of a line are not versions: Dolt's root and the creation.
const NOT_VERSIONS: usize = 2;
/// The most versions one page may ask for.
const MOST_IN_A_PAGE: usize = 200;
/// The most commits of a line that are read: a longer one is refused, never listed in part.
const MOST_COMMITS: usize = 200_000;
/// The most characters kept of a name or a message, which only a commit from elsewhere can exceed.
const TEXT_CHARS: usize = 1000;

/// One version, as History lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Version {
    pub hash: String,
    /// What the person typed; none for the automatic `Saved <time>`.
    pub description: Option<String>,
    /// Who to show; none for a version with nobody recorded.
    pub author: Option<String>,
    /// Whether an agent asked for this save.
    pub by_agent: bool,
    /// When it was saved: RFC 3339 in UTC, to the second.
    pub date: String,
    /// The nearest earlier version on this line, which is not the Dolt parent when a chat commit
    /// lies between; none for the first version.
    pub previous: Option<String>,
}

/// A page of versions, newest first.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Versions {
    pub versions: Vec<Version>,
    /// Whether older versions follow this page.
    pub more: bool,
}

/// Reads a project's versions, and the model each holds, over connections of its own, holding
/// nothing of the open project, so a long read never stands in the way of a write. It fails once
/// the project has closed.
#[derive(Debug, Clone)]
pub struct VersionReader {
    connector: archforge_dolt::Connector,
    path: std::path::PathBuf,
    /// The project's current branch, followed as it moves.
    branch: CurrentBranch,
}

/// One commit as the log gives it.
struct Commit {
    hash: String,
    name: String,
    email: String,
    date: String,
    message: String,
}

/// The page being gathered as the log goes by.
struct Gathering<'a> {
    /// The version the page starts after, until it has gone by.
    after: Option<&'a str>,
    limit: usize,
    page: Versions,
}

impl Gathering<'_> {
    /// Takes the next commit of the line, older than the last; true once the page is complete.
    fn take(&mut self, commit: Commit) -> bool {
        if !is_version(&commit.email) {
            return false;
        }
        if let Some(after) = self.after {
            if after == commit.hash {
                self.after = None;
            }
            return false;
        }
        if let Some(newer) = self.page.versions.last_mut() {
            newer.previous = Some(commit.hash.clone());
        }
        if self.page.versions.len() == self.limit {
            self.page.more = true;
            return true;
        }
        self.page.versions.push(Version {
            author: shown_author(&commit.name, &commit.email).map(str::to_string),
            by_agent: is_agent(&commit.email),
            description: (!is_automatic(&commit.message)).then_some(commit.message),
            date: commit.date,
            previous: None,
            hash: commit.hash,
        });
        false
    }
}

impl Project {
    /// A reader of this project's versions and their models, for a thread of its own.
    pub fn version_reader(&self) -> VersionReader {
        VersionReader { connector: self.server().connector(), path: self.path().to_path_buf(), branch: self.branch.clone() }
    }

    /// [`VersionReader::versions`], on this project.
    pub fn versions(&self, after: Option<&str>, limit: usize) -> Result<Versions, Error> {
        self.version_reader().versions(after, limit)
    }

    /// [`VersionReader::branches`], on this project.
    pub fn branches(&self, max: usize) -> Result<crate::Branches, Error> {
        self.version_reader().branches(max)
    }
}

/// What deleting a branch would take with it, as its confirmation says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BranchCounts {
    /// The versions no other branch has. Chat commits are not versions.
    pub versions_only_here: u64,
    /// The conversations on the branch, committed or not.
    pub conversations: u64,
    /// Whether it holds unsaved changes to the model, as the branch row's dot says.
    pub unsaved: bool,
}

/// The text in cell `i`.
fn text_at(row: &mysql::Row, i: usize) -> Option<String> {
    row.get_opt::<String, usize>(i).and_then(Result::ok)
}

impl VersionReader {
    /// What deleting `branch` would take with it, read for that branch whichever one is current.
    pub fn branch_counts(&self, branch: &SafeBranch) -> Result<BranchCounts, Error> {
        self.branch_counts_after(branch, || ())
    }

    /// [`VersionReader::branch_counts`], with `meanwhile` run once its connection is on the branch.
    /// A branch deleted from under it makes the read fail; it never answers for something else.
    #[cfg(feature = "testing")]
    pub fn branch_counts_interrupted(&self, branch: &SafeBranch, meanwhile: impl FnOnce()) -> Result<BranchCounts, Error> {
        self.branch_counts_after(branch, meanwhile)
    }

    /// The conversations on `branch`, committed or not, that are on the head of no other branch.
    /// A conversation can reach another branch only through shared history, as a branch made
    /// outside Archforge keeps its source's, so the heads are enough, and an unsafe name no bar.
    pub(crate) fn chats_only_on(&self, branch: &SafeBranch) -> Result<Vec<Uuid>, Error> {
        let mut c = database_connection(&self.connector, branch)?;
        let others = self.other_heads(&mut c, branch)?;
        let mut only = self.chat_ids(&mut c)?;
        for head in others {
            c.query_drop(format!("USE `{DATABASE}/{head}`")).map_err(sql("cannot read another branch's conversations"))?;
            let there = self.chat_ids(&mut c)?;
            only.retain(|id| !there.contains(id));
        }
        Ok(only.into_iter().collect())
    }

    /// The conversations of the database `c` is on.
    fn chat_ids(&self, c: &mut mysql::Conn) -> Result<BTreeSet<Uuid>, Error> {
        if !self.holds_chats(c)? {
            return Ok(BTreeSet::new());
        }
        let most = limits::CHATS_PER_PROJECT + 1;
        let rows: Vec<mysql::Row> = c.query(format!("SELECT `id` FROM `chat` LIMIT {most}")).map_err(sql("cannot read the conversations"))?;
        if rows.len() == most {
            return Err(Error::Damaged { path: self.path.clone(), why: "it holds too many conversations to tell which go with a branch".into() });
        }
        rows.iter().map(|r| id_at(r, 0).map_err(|why| Error::Damaged { path: self.path.clone(), why })).collect()
    }

    /// Whether the database `c` is on holds the conversations' table: an early one does not.
    fn holds_chats(&self, c: &mut mysql::Conn) -> Result<bool, Error> {
        let held: Option<mysql::Row> = c
            .query_first("SELECT 1 FROM information_schema.tables WHERE table_schema = DATABASE() AND table_name = 'chat'")
            .map_err(sql("cannot list the branch's tables"))?;
        Ok(held.is_some())
    }

    /// [`VersionReader::branch_counts`], with `meanwhile` run once its connection is on the branch.
    fn branch_counts_after(&self, branch: &SafeBranch, meanwhile: impl FnOnce()) -> Result<BranchCounts, Error> {
        let mut c = database_connection(&self.connector, branch)?;
        meanwhile();
        let others = self.other_heads(&mut c, branch)?;
        let versions_only_here = self.versions_only_on(&mut c, branch, &others)?;
        let conversations = self.conversations_held(&mut c)?;
        Ok(BranchCounts { versions_only_here, conversations, unsaved: !unsaved_tables(&mut c, &self.path)?.is_empty() })
    }

    /// The heads of every other branch, the same head once. By hash, not name: a branch whose name
    /// is outside the safe set is an other branch too, and a hash is always safe in a statement.
    fn other_heads(&self, c: &mut mysql::Conn, branch: &SafeBranch) -> Result<BTreeSet<String>, Error> {
        let rows: Vec<mysql::Row> =
            c.query("SELECT CAST(name AS CHAR), LEFT(CAST(hash AS CHAR), 64) FROM dolt_branches").map_err(sql("cannot list the branches"))?;
        let mut heads = BTreeSet::new();
        for row in &rows {
            if text_at(row, 0).is_none_or(|name| !name.eq_ignore_ascii_case(branch.stored())) {
                heads.insert(text_at(row, 1).filter(|hash| is_commit(hash)).ok_or_else(|| self.damaged_log())?);
            }
        }
        Ok(heads)
    }

    /// The versions on `branch` and on none of the `others`. With no other branch, its whole line,
    /// less the oldest commits, which are never versions.
    fn versions_only_on(&self, c: &mut mysql::Conn, branch: &SafeBranch, others: &BTreeSet<String>) -> Result<u64, Error> {
        let quoted: Vec<String> = others.iter().map(|hash| format!("'{hash}'")).collect();
        let not = if quoted.is_empty() { String::new() } else { format!(", '--not', {}", quoted.join(", ")) };
        let query = format!(
            "SELECT LEFT(CAST(email AS CHAR), {TEXT_CHARS}), CAST(COUNT(*) AS CHAR) FROM dolt_log('{}'{not}) GROUP BY email",
            branch.stored()
        );
        let rows: Vec<mysql::Row> = c.query(query).map_err(sql("cannot read the branch's versions"))?;
        let mut versions = 0;
        for row in &rows {
            let count = text_at(row, 1).and_then(|n| n.parse::<u64>().ok()).ok_or_else(|| self.damaged_log())?;
            if text_at(row, 0).is_some_and(|email| is_version(&email)) {
                versions += count;
            }
        }
        Ok(if others.is_empty() { versions.saturating_sub(NOT_VERSIONS as u64) } else { versions })
    }

    /// The conversations on the branch `c` is on; a version from before the first holds no table.
    fn conversations_held(&self, c: &mut mysql::Conn) -> Result<u64, Error> {
        if !self.holds_chats(c)? {
            return Ok(0);
        }
        let row: Option<mysql::Row> = c.query_first("SELECT CAST(COUNT(*) AS CHAR) FROM `chat`").map_err(sql("cannot count the conversations"))?;
        row.and_then(|r| text_at(&r, 0)).and_then(|n| n.parse().ok()).ok_or_else(|| self.damaged_log())
    }

    /// The name Dolt gives the branch a connection to this project is on now.
    pub fn active_branch(&self) -> Result<String, Error> {
        let mut c = database_connection(&self.connector, &branch_now(&self.branch))?;
        let name: Option<String> = c.query_first("SELECT active_branch()").map_err(sql("cannot read the current branch"))?;
        name.ok_or_else(|| self.damaged_log())
    }

    /// The project this reader reads.
    pub(crate) fn path(&self) -> &std::path::Path {
        &self.path
    }

    /// A connection of this reader's own, using the project's database.
    pub(crate) fn model_connection(&self) -> Result<mysql::Conn, Error> {
        database_connection(&self.connector, &branch_now(&self.branch))
    }

    /// The branch the project is on now.
    pub fn current_branch(&self) -> SafeBranch {
        branch_now(&self.branch)
    }

    /// The current branch's versions, newest first: at most `limit`, starting after the version
    /// `after`, so a save made meanwhile does not shift a later page. Refused when the page asked
    /// for is too large, when `after` is not a version of this line, or when the line is too long
    /// to read whole.
    pub fn versions(&self, after: Option<&str>, limit: usize) -> Result<Versions, Error> {
        self.versions_on(&branch_now(&self.branch), after, limit)
    }

    /// [`VersionReader::versions`] of `branch`, whichever branch is current.
    pub fn versions_on(&self, branch: &SafeBranch, after: Option<&str>, limit: usize) -> Result<Versions, Error> {
        if limit > MOST_IN_A_PAGE {
            return Err(Error::TooLarge { what: "a page of versions", size: limit, max: MOST_IN_A_PAGE, unit: "versions" });
        }
        let mut c = database_connection(&self.connector, branch)?;
        let query = format!(
            "SELECT commit_hash, LEFT(committer, {TEXT_CHARS}), email, DATE_FORMAT(date, '%Y-%m-%dT%H:%i:%sZ'), LEFT(message, {TEXT_CHARS}) \
             FROM dolt_log LIMIT {}",
            MOST_COMMITS + 1
        );
        let rows = c.query_iter(query).map_err(sql("cannot read the project's versions"))?;
        // A commit is known not to be one of the oldest only once that many have come after it.
        let mut held = VecDeque::with_capacity(NOT_VERSIONS + 1);
        let mut gathering = Gathering { after, limit, page: Versions::default() };
        for (seen, row) in rows.enumerate() {
            if seen == MOST_COMMITS {
                return Err(Error::TooLarge { what: "the project's history", size: seen + 1, max: MOST_COMMITS, unit: "commits" });
            }
            let row = row.map_err(sql("cannot read the project's versions"))?;
            held.push_back(self.commit_of(&row)?);
            if held.len() > NOT_VERSIONS
                && let Some(commit) = held.pop_front()
                && gathering.take(commit)
            {
                break;
            }
        }
        match gathering.after {
            Some(commit) => Err(Error::NotAVersion { commit: commit.to_string() }),
            None => Ok(gathering.page),
        }
    }

    /// A row of the log as a commit; a cell that is not text means the log is not as Dolt writes it.
    fn commit_of(&self, row: &mysql::Row) -> Result<Commit, Error> {
        let text = |i: usize| match row.as_ref(i) {
            Some(mysql::Value::Bytes(b)) => std::str::from_utf8(b).map(str::to_string).map_err(|_| self.damaged_log()),
            _ => Err(self.damaged_log()),
        };
        Ok(Commit { hash: text(0)?, name: text(1)?, email: text(2)?, date: text(3)?, message: text(4)? })
    }

    /// The error for a log that cannot be read as commits.
    fn damaged_log(&self) -> Error {
        Error::Damaged { path: self.path.clone(), why: "a commit in its history cannot be read".into() }
    }
}

/// Whether `message` is exactly the automatic `Saved <RFC 3339>`, so "Saved the day" stays a
/// description.
fn is_automatic(message: &str) -> bool {
    const SHAPE: &[u8] = b"dddd-dd-ddTdd:dd:ddZ";
    message
        .strip_prefix("Saved ")
        .is_some_and(|time| time.len() == SHAPE.len() && time.bytes().zip(SHAPE).all(|(b, s)| if *s == b'd' { b.is_ascii_digit() } else { b == *s }))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Only the exact shape counts, and the time the application writes has it.
    #[test]
    fn only_the_exact_saved_and_a_time_is_automatic() {
        assert!(is_automatic(&format!("Saved {}", crate::timestamp(1_761_670_800))));
        for typed in ["Saved the day", "Saved", "Saved 2026-11-08", "Saved 2026-11-08T10:00:00Z now", "saved 2026-11-08T10:00:00Z"] {
            assert!(!is_automatic(typed), "{typed}");
        }
    }
}
