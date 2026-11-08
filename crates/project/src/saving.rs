//! What a save did to the history, told by the branch's head before and after it.

use crate::Error;

/// A save that finished.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Saved {
    /// A new version; its hash, unless the head could not be read back after it.
    Version { hash: Option<String> },
    /// Nothing had changed; a retry whose version already exists ends here, and makes no second one.
    NothingNew,
}

/// What is known about the history after a save failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AfterFailure {
    /// No version was made.
    NoVersion,
    /// The commit reported a failure, yet the head moved to this version.
    Version(String),
    /// The head could not be read, or the reply was lost and the commit may still be running.
    CannotTell,
}

/// A save that failed, and what it left.
#[derive(Debug)]
pub struct SaveFailure {
    pub error: Error,
    pub version: AfterFailure,
}

/// Judges a failed commit by the heads around it (`None` when unreadable). A head that did not move
/// proves no version only when the server itself refused: after a lost reply it may still be running.
pub(crate) fn after_failed_commit(before: Option<&str>, after: Option<&str>, reply_lost: bool) -> AfterFailure {
    match (before, after) {
        (Some(b), Some(a)) if b != a => AfterFailure::Version(a.to_string()),
        (Some(_), Some(_)) if !reply_lost => AfterFailure::NoVersion,
        _ => AfterFailure::CannotTell,
    }
}

/// Whether `e` means the server's answer never arrived, rather than the server refusing.
pub(crate) fn reply_lost(e: &mysql::Error) -> bool {
    !matches!(e, mysql::Error::MySqlError(_))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An unreadable head tells nothing, and only an error the server sent counts as its answer.
    #[test]
    fn a_failed_commit_is_judged_by_the_head_and_by_whether_the_server_answered() {
        assert_eq!(after_failed_commit(Some("a"), Some("b"), false), AfterFailure::Version("b".into()), "a moved head is proof either way");
        for (before, after) in [(None, Some("a")), (Some("a"), None), (None, None)] {
            assert_eq!(after_failed_commit(before, after, false), AfterFailure::CannotTell, "{before:?} {after:?}");
        }
        assert!(!reply_lost(&mysql::Error::MySqlError(mysql::MySqlError { state: "HY000".into(), code: 1105, message: "no".into() })));
        assert!(reply_lost(&mysql::Error::IoError(std::io::Error::from(std::io::ErrorKind::TimedOut))));
    }
}
