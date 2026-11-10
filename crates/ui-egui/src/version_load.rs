//! Steps between versions and entries to one (`docs/design/versioning.md`, section 3): at most one
//! read is under way, and the newest version asked for is the one that shows. An answer overtaken
//! never shows, but goes into a small cache under its project and hash, so that coming back to it
//! needs no read. Only reads made here fill the cache, and a restore or a branch switch empties it
//! when it starts, the read then under way included. The version in view stays until the next has
//! arrived, and one that cannot be read or built leaves it.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};

use archforge_mof::rows::{Cell, Rows};

use crate::hold::HoldRefused;
use crate::i18n::tr;
use crate::{Shell, VersionRead, VersionReadError};

/// The most versions the cache holds: the design's last few.
const MOST_KEPT: usize = 8;
/// The most bytes the versions in the cache come to, as a single read may be 384 MiB.
const MOST_KEPT_BYTES: usize = 256 << 20;

/// What a read answers: a version's rows, or why they could not be read.
type Answer = VersionRead;

/// Where a version goes once it has been read.
enum Then {
    /// To the replacement entering it, under its hold.
    Enter(Sender<Answer>),
    /// In place of the version in view, with what the chip then says of it.
    Step(Option<String>),
}

/// The one read under way, of the version `commit` of `project`.
struct Reading {
    project: PathBuf,
    commit: String,
    answer: Receiver<Answer>,
    /// False once a restore or a branch switch has started: the answer then goes unused.
    keep: bool,
}

/// The newest version asked for.
struct Wanted {
    project: PathBuf,
    commit: String,
    then: Then,
}

/// A version in the cache, with what it comes to in bytes.
struct Kept {
    project: PathBuf,
    commit: String,
    rows: Arc<Rows>,
    bytes: usize,
}

/// The read under way, the newest version asked for, and the versions kept for viewing again.
#[derive(Default)]
pub(crate) struct Loads {
    reading: Option<Reading>,
    wanted: Option<Wanted>,
    /// Least recently used first.
    kept: VecDeque<Kept>,
    bytes: usize,
}

/// What `rows` come to in memory, counted as the read's own budget counts them.
fn bytes_of(rows: &Rows) -> usize {
    let text = |c: &Cell| if let Cell::Text(t) = c { t.len() + 16 } else { 0 };
    let tables = rows.tables.iter().flatten().map(|row| size_of::<Vec<Cell>>() + row.iter().map(|c| size_of::<Cell>() + text(c)).sum::<usize>());
    let junctions = rows.junctions.iter().flatten().map(|entry| size_of_val(entry) + text(&entry.2));
    tables.chain(junctions).fold(0, usize::saturating_add)
}

impl Loads {
    /// Whether a read is under way or a version waits to be read or shown: the window must look
    /// again for the answer, as no other event will bring it.
    pub(crate) fn waiting(&self) -> bool {
        self.reading.is_some() || self.wanted.is_some()
    }

    /// Calls off the version asked for. A read under way goes on, as it cannot be stopped, and its
    /// answer is kept but not shown.
    pub(crate) fn call_off(&mut self) {
        self.wanted = None;
    }

    /// The version a step has asked for and that has not arrived.
    pub(crate) fn wanted_commit(&self) -> Option<&str> {
        self.wanted.as_ref().filter(|w| matches!(w.then, Then::Step(_))).map(|w| w.commit.as_str())
    }

    /// Empties the cache, and keeps the read under way from filling it or being used.
    pub(crate) fn clear(&mut self) {
        self.kept.clear();
        self.bytes = 0;
        if let Some(read) = self.reading.as_mut() {
            read.keep = false;
        }
    }

    /// Where the version `commit` of `project` sits in the cache.
    fn find(&self, project: &Path, commit: &str) -> Option<usize> {
        self.kept.iter().position(|k| k.project == project && k.commit == commit)
    }

    /// Keeps `rows`, of `bytes`, as the most recently used, those least recently used going until
    /// both limits hold. A version over the byte limit alone is not kept, and none goes for it. A
    /// read starts only for what the cache does not hold, so a version never arrives twice.
    fn keep(&mut self, project: &Path, commit: &str, rows: Arc<Rows>, bytes: usize) {
        if bytes > MOST_KEPT_BYTES {
            return;
        }
        self.kept.push_back(Kept { project: project.to_path_buf(), commit: commit.to_string(), rows, bytes });
        self.bytes = self.bytes.saturating_add(bytes);
        while self.kept.len() > MOST_KEPT || self.bytes > MOST_KEPT_BYTES {
            let Some(gone) = self.kept.pop_front() else { break };
            self.bytes = self.bytes.saturating_sub(gone.bytes);
        }
    }

    /// The rows kept for the version asked for, which become the most recently used.
    fn kept_wanted(&mut self) -> Option<Arc<Rows>> {
        let wanted = self.wanted.as_ref()?;
        let entry = self.kept.remove(self.find(&wanted.project, &wanted.commit)?)?;
        let rows = entry.rows.clone();
        self.kept.push_back(entry);
        Some(rows)
    }

    /// The commits the cache holds, least recently used first, for the tests.
    #[cfg(test)]
    pub(crate) fn kept_commits(&self) -> Vec<&str> {
        self.kept.iter().map(|k| k.commit.as_str()).collect()
    }
}

impl Shell {
    /// Asks for the version `commit` from the version in view, overtaking the one asked for before;
    /// the version in view itself calls that one off. Refused without a host or a project.
    pub(crate) fn step_to(&mut self, commit: &str, label: Option<String>) -> Result<(), HoldRefused> {
        if self.viewing() == Some(commit) {
            self.loads.call_off();
            return Ok(());
        }
        let (Some(_), Some(open)) = (&self.services.projects, &self.project) else {
            return Err(HoldRefused::NoProject);
        };
        self.loads.wanted = Some(Wanted { project: open.path.clone(), commit: commit.to_string(), then: Then::Step(label) });
        self.read_wanted();
        Ok(())
    }

    /// Asks for the version `commit` of `project` to enter it, under the hold already taken, by the
    /// same one read as a step. What is returned answers the replacement that enters it.
    pub(crate) fn read_for_entry(&mut self, project: PathBuf, commit: &str) -> Receiver<Answer> {
        let (to, from) = channel();
        self.loads.wanted = Some(Wanted { project, commit: commit.to_string(), then: Then::Enter(to) });
        self.read_wanted();
        from
    }

    /// Starts the read of the version asked for, when none is under way and the cache lacks it.
    fn read_wanted(&mut self) {
        let (None, Some(wanted), Some(host)) = (&self.loads.reading, &self.loads.wanted, &self.services.projects) else {
            return;
        };
        if self.loads.find(&wanted.project, &wanted.commit).is_none() {
            let answer = host.read_version(&wanted.project, wanted.commit.clone());
            self.loads.reading = Some(Reading { project: wanted.project.clone(), commit: wanted.commit.clone(), answer, keep: true });
        }
    }

    /// Once a frame: takes the answer of the read under way, shows the version asked for if it has
    /// come or is kept, and otherwise starts its read. A kept version waits for this, so it never
    /// replaces what shows part way through drawing a frame.
    pub(crate) fn drive_loads(&mut self) {
        let answer = self.loads.reading.as_ref().and_then(|r| match r.answer.try_recv() {
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(Err(tr("The reading stopped without an answer, so nothing was changed.").into())),
            Ok(answer) => Some(answer),
        });
        if let Some(answer) = answer {
            self.take_answer(answer);
        }
        if let Some(rows) = self.loads.kept_wanted() {
            self.deliver(Ok(rows));
        }
        self.read_wanted();
    }

    /// The read under way answered. Unless a restore or a branch switch has started since, its rows
    /// are kept, or a lasting reason is noted on its row, of the project shown only, as that holds
    /// whoever asked. They or the failure go on only when this is still the version asked for.
    fn take_answer(&mut self, answer: Answer) {
        let Some(read) = self.loads.reading.take().filter(|r| r.keep) else { return };
        let shown = self.project.as_ref().is_some_and(|o| o.path == read.project);
        match &answer {
            Ok(rows) => self.loads.keep(&read.project, &read.commit, rows.clone(), bytes_of(rows)),
            Err(why) if shown && !matches!(why, VersionReadError::Failed(_)) => {
                let reason = (*why != VersionReadError::Shape).then(|| why.to_string());
                self.versions.unavailable.insert(read.commit.clone(), reason);
            }
            Err(_) => {}
        }
        if self.loads.wanted.as_ref().is_some_and(|w| w.project == read.project && w.commit == read.commit) {
            self.deliver(answer);
        }
    }

    /// Notes on the row of `commit` that its rows cannot be built, which holds of them whoever
    /// asks. A host that gives no ids says nothing of the rows, so that marks nothing.
    pub(crate) fn mark_unbuilt(&mut self, commit: String, why: &str) {
        if self.services.ids.is_some() {
            self.versions.unavailable.insert(commit, Some(why.to_string()));
        }
    }

    /// Hands `answer` to what asked for the newest version. A step that failed stays on the version
    /// in view and says why in the toast above the toolbar; one that cannot be built cannot ever
    /// be, so its row says so.
    fn deliver(&mut self, answer: Answer) {
        let Some(Wanted { commit, then, .. }) = self.loads.wanted.take() else { return };
        match (then, answer) {
            // Nothing receives this once the person has gone back, and the rows are kept anyway.
            (Then::Enter(to), answer) => drop(to.send(answer)),
            (Then::Step(_), Err(why)) => self.canvas.say_refused_step(Some(why.to_string())),
            (Then::Step(label), Ok(rows)) => {
                if let Err(why) = self.show_step(&rows, commit.clone(), label) {
                    self.mark_unbuilt(commit, &why);
                    self.canvas.say_refused_step(Some(why));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const P: &str = "/p";

    /// A cache of `commits` under the project `/p`, each of one byte, oldest first.
    fn kept(commits: &[&str]) -> Loads {
        let mut loads = Loads::default();
        commits.iter().for_each(|c| loads.keep(Path::new(P), c, Arc::default(), 1));
        loads
    }

    /// Asks `loads` for `commit` of `project`, and answers from the cache if it holds it.
    fn hit(loads: &mut Loads, project: &str, commit: &str) -> bool {
        loads.wanted = Some(Wanted { project: project.into(), commit: commit.into(), then: Then::Step(None) });
        loads.kept_wanted().is_some()
    }

    /// Past the entry limit the least recently used goes, and using a version makes it the newest.
    #[test]
    fn the_entry_limit_evicts_the_least_recently_used() {
        let commits: Vec<String> = (0..MOST_KEPT).map(|n| format!("h{n}")).collect();
        let mut loads = kept(&commits.iter().map(String::as_str).collect::<Vec<_>>());
        assert!(hit(&mut loads, P, "h0"), "h0 is used, so h1 is now the oldest");
        loads.keep(Path::new(P), "new", Arc::default(), 1);
        assert_eq!(loads.kept.len(), MOST_KEPT);
        assert!(!hit(&mut loads, P, "h1") && hit(&mut loads, P, "h0") && hit(&mut loads, P, "new"));
    }

    /// The byte limit holds with few entries, and what goes stops counting. One version over the
    /// limit alone is not kept, and none goes for it.
    #[test]
    fn the_byte_limit_evicts_the_least_recently_used() {
        let mut loads = Loads::default();
        let half = MOST_KEPT_BYTES / 2;
        loads.keep(Path::new(P), "a", Arc::default(), half);
        loads.keep(Path::new(P), "b", Arc::default(), half);
        assert!(hit(&mut loads, P, "a"), "a is used, so b is now the oldest");
        loads.keep(Path::new(P), "huge", Arc::default(), MOST_KEPT_BYTES + 1);
        assert_eq!(loads.kept_commits(), ["b", "a"], "too big for the cache, and nothing went for it");
        loads.keep(Path::new(P), "c", Arc::default(), 1);
        assert_eq!(loads.kept_commits(), ["a", "c"], "few entries, but over the byte limit");
        assert_eq!(loads.bytes, half + 1);
    }

    /// A key has two parts, and each is checked: the same hash in another project and another hash
    /// in the same project both miss.
    #[test]
    fn the_cache_tells_projects_and_hashes_apart() {
        let mut loads = kept(&["h"]);
        assert!(!hit(&mut loads, "/other", "h"), "the same hash in another project");
        assert!(!hit(&mut loads, P, "i"), "another hash in the same project");
        assert!(hit(&mut loads, P, "h"));
    }

    /// Emptying leaves nothing to find and nothing counted, and bars the read under way.
    #[test]
    fn clearing_empties_the_cache_and_bars_the_read_under_way() {
        let mut loads = kept(&["h"]);
        loads.reading = Some(Reading { project: P.into(), commit: "r".into(), answer: channel().1, keep: true });
        loads.clear();
        assert!(!hit(&mut loads, P, "h") && loads.bytes == 0 && loads.reading.as_ref().is_some_and(|r| !r.keep));
    }

    /// A read's text counts in a table and in a junction, as what it comes to depends on it.
    #[test]
    fn the_size_of_rows_counts_their_text() {
        let long = || Cell::Text(Arc::from("x".repeat(1000)));
        let table = Rows { tables: vec![vec![vec![long()]]], junctions: vec![] };
        let junction = Rows { tables: vec![], junctions: vec![vec![(archforge_mof::Uuid([0; 16]), 0, long())]] };
        assert!(bytes_of(&table) > 1000 && bytes_of(&junction) > 1000);
        assert_eq!(bytes_of(&Rows::default()), 0);
    }
}
