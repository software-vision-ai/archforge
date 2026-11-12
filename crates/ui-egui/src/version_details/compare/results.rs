//! What the Compare page finds (section 6 of `docs/design/versioning.md`): a version read at its
//! revision against current work, off the interface thread. The frame only copies current work's
//! objects; their rows are built with the comparison. Comparing reads rows only, so the session is
//! never replaced. While a version shows, current work is the session set aside.
//! Nothing draws what it finds yet.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, TryRecvError, channel};

use archforge_mof::Frozen;
use archforge_mof::rows::Rows;
use archforge_notations::differences::classified;

use super::Side;
use crate::version_details::{Found, about, not_compared};
use crate::version_load::{MOST_KEPT_BYTES, bytes_of};
use crate::{Shell, VersionRead};

/// The version compared with current work, and whether current work is the Before side.
pub(crate) type Sides = (String, bool);

/// What a comparison is of: its sides, and the revision of current work it took.
#[derive(Clone, PartialEq)]
pub(crate) struct Key {
    pub(crate) sides: Sides,
    pub(crate) revision: u64,
}

/// A version's rows; `keep` is false for rows too large to hold past the comparison they start.
pub(crate) struct Held {
    pub(crate) version: String,
    pub(crate) rows: Arc<Rows>,
    pub(crate) keep: bool,
}

/// The page's one read and one comparison, and what the last comparison found.
#[derive(Default)]
pub(crate) struct Results {
    about: Option<(PathBuf, u64)>,
    pub(crate) held: Option<Held>,
    pub(crate) reading: Option<(String, Receiver<VersionRead>)>,
    pub(crate) comparing: Option<(Key, Receiver<Found>)>,
    pub(crate) found: Option<(Key, Found)>,
}

/// The sides the page compares now; none without the page, or for a pair that is not a version and
/// current work.
fn sides(shell: &Shell) -> Option<Sides> {
    let pair = shell.versions.details.as_ref()?.compare.as_ref()?;
    match (&pair.before, &pair.after) {
        (Side::Version(v), Side::CurrentWork) => Some((v.hash.clone(), false)),
        (Side::CurrentWork, Side::Version(v)) => Some((v.hash.clone(), true)),
        _ => None,
    }
}

/// Once a frame, after the details have had their turn: lets go of what belonged to another project
/// or session and of the rows of a page gone, takes the answers, starts what is due, and wakes the
/// window while anything is awaited.
pub(crate) fn keep(shell: &mut Shell, ctx: &egui::Context) {
    let about = about(shell);
    if shell.versions.compare_results.about != about {
        shell.versions.compare_results = Results { about, ..Results::default() };
    }
    take_answers(shell);
    let now = sides(shell);
    if now.is_none() {
        let r = &mut shell.versions.compare_results;
        (r.held, r.found) = (None, None);
    }
    if let Some(now) = now {
        next(shell, now);
    }
    let r = &shell.versions.compare_results;
    if r.reading.is_some() || r.comparing.is_some() {
        ctx.request_repaint_after(std::time::Duration::from_millis(100));
    }
}

/// The answer on `rx`, if it has come; a read that ended without one is a failure.
fn answered<T, E: From<String>>(rx: &Receiver<Result<T, E>>) -> Option<Result<T, E>> {
    match rx.try_recv() {
        Err(TryRecvError::Empty) => None,
        Err(TryRecvError::Disconnected) => Some(Err(not_compared().into())),
        Ok(answer) => Some(answer),
    }
}

/// Lands what has come, only on the sides it is about. Rows that have only moved on still show what
/// they were, or a steady stream of edits would hide every answer; a failure for them says nothing
/// of the newest rows, which the next comparison takes.
fn take_answers(shell: &mut Shell) {
    let (now, revision) = (sides(shell), shell.current_work().map(|s| s.revision()));
    let r = &mut shell.versions.compare_results;
    if let Some(answer) = r.reading.as_ref().and_then(|(_, rx)| answered(rx))
        && let Some((version, _)) = r.reading.take()
        && let Some(sides) = now.clone().filter(|s| s.0 == version)
    {
        match answer {
            Ok(rows) => r.held = Some(Held { version, keep: bytes_of(&rows) <= MOST_KEPT_BYTES, rows }),
            Err(why) => r.found = revision.map(|revision| (Key { sides, revision }, Err(why.to_string()))),
        }
    }
    if let Some(answer) = r.comparing.as_ref().and_then(|(_, rx)| answered(rx))
        && let Some((key, _)) = r.comparing.take()
        && now.as_ref() == Some(&key.sides)
        && (answer.is_ok() || revision == Some(key.revision))
    {
        r.found = Some((key, answer));
    }
}

/// Compares the newest rows when what is shown is not of them. One comparison runs at a time, so
/// revisions that move while it runs fold into the next. A version that could not be read waits
/// for Retry, or each edit would read it again; nothing is compared while an edit is under way.
fn next(shell: &mut Shell, sides: Sides) {
    let Some(session) = shell.current_work().filter(|s| !s.interacting()) else { return };
    let key = Key { sides, revision: session.revision() };
    let r = &shell.versions.compare_results;
    let held = r.held.as_ref().filter(|h| h.version == key.sides.0);
    let known = r.found.as_ref().is_some_and(|(shown, found)| *shown == key || (shown.sides == key.sides && found.is_err() && held.is_none()));
    if known || r.comparing.is_some() {
        return;
    }
    let Some(before) = held.map(|h| h.rows.clone()) else { return read_before(shell, key.sides.0) };
    // The frame only copies the objects; their rows are built with the comparison.
    let started = compare(before, session.doc().freeze(), key.sides.1);
    let r = &mut shell.versions.compare_results;
    match started {
        Ok(answer) => r.comparing = Some((key, answer)),
        Err(why) => r.found = Some((key, Err(why.to_string()))),
    }
    r.held = r.held.take().filter(|h| h.keep);
}

/// Asks for the rows of `version`, taking turns with the details' reads: the host refuses reads past
/// its limit, and that would read as a failed comparison.
fn read_before(shell: &mut Shell, version: String) {
    let v = &shell.versions;
    if v.compare_results.reading.is_some() || v.details_comparing.is_some() || v.details_reading.is_some() {
        return;
    }
    let (Some(host), Some((path, _))) = (shell.services.projects.clone(), v.compare_results.about.clone()) else { return };
    let answer = host.read_version(&path, version.clone());
    shell.versions.compare_results.reading = Some((version, answer));
}

/// Builds the rows of `live` and compares them with `version` on a thread of its own, `live` first
/// when `swapped`; rows that cannot be built fail the comparison with their reason. The model's root
/// is no change of its own.
fn compare(version: Arc<Rows>, live: Frozen, swapped: bool) -> std::io::Result<Receiver<Found>> {
    let (tx, rx) = channel();
    let (tables, junctions) = archforge_notations::storage();
    let work = move || {
        let found = live.rows(&tables, &junctions).map_err(|e| e.to_string()).and_then(|live| {
            let (before, after) = if swapped { (&live, &*version) } else { (&*version, &live) };
            classified(before, after, crate::editing::fit_tables).map_err(|e| e.to_string())
        });
        // Model elements come first, then diagrams, then skills, each in the order found.
        let _ = tx.send(found.map(|mut items| {
            items.retain(|d| d.class != archforge_core::MODEL);
            items.sort_by_key(|d| d.area);
            Arc::new(items)
        }));
    };
    std::thread::Builder::new().name("archforge-version-compare".into()).spawn(work).map(|_| rx)
}
