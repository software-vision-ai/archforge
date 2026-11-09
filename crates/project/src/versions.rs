//! The project's versions, read from the current branch's log for History: the commits Save made,
//! newest first, a page at a time.

use std::collections::VecDeque;

use mysql::prelude::Queryable;

use crate::model::{database_connection, sql};
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
        VersionReader { connector: self.server().connector(), path: self.path().to_path_buf() }
    }

    /// [`VersionReader::versions`], on this project.
    pub fn versions(&self, after: Option<&str>, limit: usize) -> Result<Versions, Error> {
        self.version_reader().versions(after, limit)
    }
}

impl VersionReader {
    /// The project this reader reads.
    pub(crate) fn path(&self) -> &std::path::Path {
        &self.path
    }

    /// A connection of this reader's own, using the project's database.
    pub(crate) fn model_connection(&self) -> Result<mysql::Conn, Error> {
        database_connection(&self.connector)
    }

    /// The current branch's versions, newest first: at most `limit`, starting after the version
    /// `after`, so a save made meanwhile does not shift a later page. Refused when the page asked
    /// for is too large, when `after` is not a version of this line, or when the line is too long
    /// to read whole.
    pub fn versions(&self, after: Option<&str>, limit: usize) -> Result<Versions, Error> {
        if limit > MOST_IN_A_PAGE {
            return Err(Error::TooLarge { what: "a page of versions", size: limit, max: MOST_IN_A_PAGE, unit: "versions" });
        }
        let mut c = database_connection(&self.connector)?;
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
