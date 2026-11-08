//! What a save did to the history, told by the branch's head before and after it.

use crate::Error;
use crate::chat::CHAT_AUTHOR;

/// The email of a save a person made, or of one from before names were recorded.
const PERSON_EMAIL: &str = "archforge@localhost";
/// The email of a save an agent asked for.
const AGENT_EMAIL: &str = "agent@localhost";
/// The name of a person's save when no usable name is set.
const NO_NAME: &str = "Archforge";
/// The name of an agent's save when it has no usable label.
const NO_LABEL: &str = "Agent";
/// The most characters of a name recorded.
const NAME_CHARS: usize = 100;

/// Who a version is recorded as saved by.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Author {
    name: String,
    email: &'static str,
}

impl Author {
    /// A person, by the name from Settings; `Archforge` when it is empty or cannot be used.
    pub fn person(name: &str) -> Author {
        Author { name: usable(name).unwrap_or_else(|| NO_NAME.to_string()), email: PERSON_EMAIL }
    }

    /// An agent, by its origin label; `Agent` when it has none or it cannot be used.
    pub fn agent(label: Option<&str>) -> Author {
        Author { name: label.and_then(usable).unwrap_or_else(|| NO_LABEL.to_string()), email: AGENT_EMAIL }
    }

    /// Runs `commit` with this author's `Name <email>`. Should Dolt still refuse the name, as a
    /// version not tried might, it runs again under the plain name: a name never stops a save.
    pub(crate) fn commit_as(&self, mut commit: impl FnMut(&str) -> Result<(), mysql::Error>) -> Result<(), mysql::Error> {
        match commit(&format!("{} <{}>", self.name, self.email)) {
            Err(mysql::Error::MySqlError(e)) if e.message.to_lowercase().contains("author") => {
                let plain = if self.email == AGENT_EMAIL { NO_LABEL } else { NO_NAME };
                commit(&format!("{plain} <{}>", self.email))
            }
            other => other,
        }
    }
}

/// `raw` as a name Dolt takes, or none when nothing is left: without the characters that break its
/// `Name <email>` form (it refuses a closing parenthesis), line breaks made spaces, within the cap.
fn usable(raw: &str) -> Option<String> {
    let kept: String = raw.chars().filter(|c| !"<>()".contains(*c)).map(|c| if c.is_control() { ' ' } else { c }).take(NAME_CHARS).collect();
    let name = kept.split_whitespace().collect::<Vec<_>>().join(" ");
    (!name.is_empty()).then_some(name)
}

/// Whether a commit recorded under `email` is a version. The email decides, never the name, since
/// a person's name from Settings could be anything; chat commits are not versions.
pub fn is_version(email: &str) -> bool {
    !CHAT_AUTHOR.ends_with(&format!("<{email}>"))
}

/// The author History shows for a version recorded under `name` and `email`: nobody for plain
/// `Archforge`, which is every version from before names were recorded and any saved with no name.
pub fn shown_author<'a>(name: &'a str, email: &str) -> Option<&'a str> {
    (name != NO_NAME || email != PERSON_EMAIL).then_some(name)
}

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

    /// Names are cleaned, and what cannot be used falls back by who saved.
    #[test]
    fn a_name_is_cleaned_and_one_that_cannot_be_used_falls_back() {
        let line = |a: Author| format!("{} <{}>", a.name, a.email);
        assert_eq!(line(Author::person("Ada (Countess) <x>\nLovelace")), "Ada Countess x Lovelace <archforge@localhost>");
        assert_eq!(line(Author::person("李雷")), "李雷 <archforge@localhost>");
        assert_eq!(line(Author::person(" () ")), crate::AUTHOR, "the same author as before names were recorded");
        assert_eq!(line(Author::agent(Some("Chat 3"))), "Chat 3 <agent@localhost>");
        assert_eq!(line(Author::agent(Some("<>"))), line(Author::agent(None)));
        assert_eq!(Author::person(&"x".repeat(150)).name.chars().count(), NAME_CHARS);
    }

    /// Only a refusal of the author runs the commit again, and under the plain name.
    #[test]
    fn a_refused_name_never_stops_a_save() {
        let refusal = |message: &str| mysql::Error::MySqlError(mysql::MySqlError { state: "HY000".into(), code: 1105, message: message.into() });
        let mut lines = Vec::new();
        let run = Author::agent(Some("Chat 3")).commit_as(|line| {
            lines.push(line.to_string());
            if lines.len() > 1 { Ok(()) } else { Err(refusal("Author not formatted correctly")) }
        });
        assert!(run.is_ok());
        assert_eq!(lines, ["Chat 3 <agent@localhost>", "Agent <agent@localhost>"]);
        let mut tries = 0;
        assert!(
            Author::person("Ada")
                .commit_as(|_| {
                    tries += 1;
                    Err(refusal("nothing to commit"))
                })
                .is_err()
        );
        assert_eq!(tries, 1, "another refusal is not the name's fault");
    }

    /// The email decides, whatever the name says, and older versions show nobody.
    #[test]
    fn a_commit_is_read_by_its_email_and_older_versions_show_nobody() {
        assert!(!is_version("chat@localhost"));
        assert!(is_version("archforge@localhost"), "even when the person is named Archforge chat");
        assert_eq!(shown_author("Archforge", "archforge@localhost"), None);
        assert_eq!(shown_author("Archforge", "agent@localhost"), Some("Archforge"), "an agent labelled so is still shown");
        assert_eq!(shown_author("Ada", "archforge@localhost"), Some("Ada"));
    }

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
