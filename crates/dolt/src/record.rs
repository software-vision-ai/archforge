//! A record per running server, in the user's settings: which process serves which project, in
//! which run folder, started by which Archforge. Recovery recognises a leftover server by it. A
//! start writes it before its server starts, without the server's process id, and completes it
//! once the server runs, holding the record's lock from before the one to after the other.
//!
//! Every change of a record (writing, removing, setting a damaged one aside) holds that record's
//! lock, an operating-system lock on a file of its own beside the record, so a change that looks
//! before it acts sees no other change in between, from this process or another. One lock for
//! each project: a start that has stalled holds up no other project's record. The lock goes with
//! its holder, a killed one too. Reading takes no lock: a record appears and goes whole. Where
//! the file system has no locks a change fails, naming the lock file, and changes nothing.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::Error;
use crate::recovery::valid_pid;

/// The most of a record read; a larger file is set aside. No record is written that would be
/// larger once it names a process id, whichever.
const CAP: u64 = 4096;

/// Whether a record of `len` bytes is within [`CAP`]: the one test of size, for reading and for
/// writing.
fn fits(len: usize) -> bool {
    u64::try_from(len).is_ok_and(|n| n <= CAP)
}

/// The longest a change waits for the lock: two seconds, less in this crate's own tests. A longer
/// wait means the holder has stalled, and the change fails instead of hanging with it.
pub(crate) const LOCK_WAIT: Duration = Duration::from_millis(if cfg!(test) { 100 } else { 2000 });

/// The lock of one project's record, held until it is dropped or the process ends, however it
/// ends.
#[derive(Debug)]
pub(crate) struct Held {
    lock: std::fs::File,
    record: PathBuf,
}

impl Drop for Held {
    /// Closing the file unlocks it too, but on Windows not at once.
    fn drop(&mut self) {
        let _ = self.lock.unlock();
    }
}

/// Takes the lock of the record in `file`, a file beside it, waiting at most `wait` for it.
pub(crate) fn hold(file: &Path, wait: Duration) -> Result<Held, Error> {
    let path = file.with_extension("lock");
    // A link, a pipe or a folder under the lock's name could block the open or lead it elsewhere.
    if std::fs::symlink_metadata(&path).is_ok_and(|m| !m.is_file()) {
        return Err(Error::io("cannot lock", &path, std::io::Error::other("it is not a plain file")));
    }
    // Opened for reading too: a file opened only to append cannot be locked on Windows.
    let lock = std::fs::File::options().read(true).append(true).create(true).open(&path).map_err(|e| Error::io("cannot open", &path, e))?;
    let asked = Instant::now();
    loop {
        match lock.try_lock() {
            Ok(()) => return Ok(Held { lock, record: file.to_path_buf() }),
            Err(std::fs::TryLockError::WouldBlock) if asked.elapsed() < wait => {
                #[cfg(test)]
                tests::waiting();
                std::thread::sleep(Duration::from_millis(2));
            }
            Err(std::fs::TryLockError::WouldBlock) => {
                let stalled = std::io::Error::new(std::io::ErrorKind::TimedOut, format!("another change of the server record took over {wait:?}"));
                return Err(Error::io("cannot lock", &path, stalled));
            }
            Err(std::fs::TryLockError::Error(e)) => return Err(Error::io("cannot lock", &path, e)),
        }
    }
}

/// Runs `change` holding the lock of the record in `file`, waiting at most `wait` for it.
fn locked<T>(file: &Path, wait: Duration, change: impl FnOnce() -> T) -> Result<T, Error> {
    let _held = hold(file, wait)?;
    Ok(change())
}

/// What a record's file holds.
enum Found {
    Absent,
    /// No record that can be used, and why.
    Bad(&'static str),
    Good(Record),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Record {
    pub data_dir: String,
    /// The server's process id; none while its start is under way, and in the record of a start
    /// that ended before it completed it. A record of the earlier format always has one.
    pub server_pid: Option<u32>,
    /// The checked run root the run folder was made in, and the run folder.
    pub run_root: String,
    pub run_dir: String,
    /// The server's command line exactly as started.
    pub command: Vec<String>,
    pub owner_pid: u32,
    /// The owner's start time as the identity helper reads it.
    pub owner_started: String,
}

/// FNV-1a, 64 bits: a stable name for a project's record, the same in every build.
pub(crate) fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// The record file of the project at `data_dir`.
pub(crate) fn path(records: &Path, data_dir: &str) -> PathBuf {
    records.join(format!("{:016x}.json", fnv1a64(data_dir.as_bytes())))
}

/// The record of the project at `data_dir`, if there is one. A damaged or oversized record, or one
/// about another project, is set aside and counts as none; one that cannot be set aside (no
/// lock to be had, too many set aside before it, a move that fails) is an error.
pub(crate) fn read(records: &Path, data_dir: &str) -> Result<Option<Record>, Error> {
    read_file(&path(records, data_dir), Some(data_dir))
}

/// Every record in `records`, for the sweep at start. A file that cannot be read, or holds no
/// record and cannot be set aside, is left out and logged: the sweep goes on with the others.
pub(crate) fn all(records: &Path) -> Result<Vec<(PathBuf, Record)>, Error> {
    let entries = match std::fs::read_dir(records) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(Error::io("cannot list", records, e)),
    };
    let mut out = Vec::new();
    for e in entries.flatten() {
        let p = e.path();
        if p.extension().is_some_and(|x| x == "json") {
            match read_file(&p, None) {
                Ok(Some(r)) => out.push((p, r)),
                Ok(None) => {}
                Err(e) => log::warn!("the server record {} is left out of the sweep: {e}", p.display()),
            }
        }
    }
    Ok(out)
}

/// The record in `file`, if it holds one. A file that holds none that can be used is set aside:
/// under the lock and after a second look, so a record another start wrote meanwhile stays.
fn read_file(file: &Path, project: Option<&str>) -> Result<Option<Record>, Error> {
    let found = match load(file, project)? {
        Found::Bad(_) => locked(file, LOCK_WAIT, || {
            #[cfg(test)]
            tests::in_change(file);
            let again = load(file, project)?;
            if let Found::Bad(why) = &again {
                set_aside(file, why)?;
            }
            Ok(again)
        })??,
        found => found,
    };
    Ok(if let Found::Good(r) = found { Some(r) } else { None })
}

/// Reads `file` and changes nothing. A record must name `project`, when one is given.
fn load(file: &Path, project: Option<&str>) -> Result<Found, Error> {
    let f = match std::fs::File::open(file) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Found::Absent),
        Err(e) => return Err(Error::io("cannot read", file, e)),
    };
    let mut bytes = Vec::new();
    f.take(CAP + 1).read_to_end(&mut bytes).map_err(|e| Error::io("cannot read", file, e))?;
    if !fits(bytes.len()) {
        return Ok(Found::Bad("it is too large"));
    }
    Ok(match serde_json::from_slice::<Record>(&bytes) {
        Ok(r) if project.is_some_and(|p| p != r.data_dir) => Found::Bad("it names another project"),
        Ok(r) if r.server_pid.is_none_or(valid_pid) && valid_pid(r.owner_pid) && Path::new(&r.run_root).is_absolute() => Found::Good(r),
        _ => Found::Bad("it cannot be read"),
    })
}

/// Moves a record that cannot be used out of the way, under a new name, and says so in the log.
/// The caller holds the lock. One that cannot be moved is an error: it still holds the name.
fn set_aside(file: &Path, why: &str) -> Result<(), Error> {
    let Some(target) = (1..100).map(|n| file.with_extension(format!("json.damaged-{n}"))).find(|t| !t.exists()) else {
        return Err(Error::io("cannot set aside", file, std::io::Error::other(format!("{why}, and too many were set aside before it"))));
    };
    std::fs::rename(file, &target).map_err(|e| Error::io("cannot set aside", file, e))?;
    log::warn!("the server record {} was set aside as {}: {why}", file.display(), target.display());
    Ok(())
}

/// The records folder of `held`, if it is the lock of `r`'s own record.
fn own_folder<'a>(held: &'a Held, r: &Record) -> Result<&'a Path, Error> {
    let file = &held.record;
    let own = file.parent().filter(|d| path(d, &r.data_dir) == *file);
    own.ok_or_else(|| Error::io("cannot write", file, std::io::Error::other("the lock held is another record's")))
}

/// Writes `r` in full under a new temporary name in `records` and synchronises it. Returns that
/// name. The name is made exclusively, so nothing under it, a link included, is written through.
/// A record the reader would refuse for its size once it names the largest process id there is
/// is refused first, and nothing is written.
fn write_tmp(records: &Path, r: &Record) -> Result<PathBuf, Error> {
    use std::io::Write;
    let json = |r: &Record| serde_json::to_vec_pretty(r).map_err(|e| Error::io("cannot write", records, std::io::Error::other(e)));
    let largest = json(&Record { server_pid: Some(u32::MAX), ..r.clone() })?.len();
    if !fits(largest) {
        return Err(Error::RecordTooLarge { project: r.data_dir.clone(), len: largest, max: CAP });
    }
    let bytes = json(r)?;
    let tmp = records.join(format!(".record-{}.tmp", crate::secret::random_name(8)?));
    // A name that is taken is another file's: only one this call made is removed again.
    let mut f = std::fs::File::create_new(&tmp).map_err(|e| Error::io("cannot write", &tmp, e))?;
    match f.write_all(&bytes).and_then(|()| f.sync_all()) {
        Ok(()) => Ok(tmp),
        Err(e) => {
            remove(&tmp);
            Err(Error::io("cannot write", &tmp, e))
        }
    }
}

/// Writes a project's record unless one is there already, atomically and exclusively: the record
/// is written in full under a temporary name, then linked to its own name, which fails if another
/// start got there first. The caller holds the record's lock, `held`, taken before the server
/// the record is about was started. Returns whether it was written.
pub(crate) fn write_new(held: &Held, r: &Record) -> Result<bool, Error> {
    let file = &held.record;
    let tmp = write_tmp(own_folder(held, r)?, r)?;
    #[cfg(test)]
    tests::in_change(file);
    let linked = std::fs::hard_link(&tmp, file);
    remove(&tmp);
    match linked {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(false),
        Err(e) => Err(Error::io("cannot write", file, e)),
    }
}

/// Completes the record `pending`, which a start wrote before its server ran, with the server's
/// process id `server_pid`, under the lock `held` the start has held since. Only while the name
/// still holds `pending`, read in place: the complete record is written in full under a temporary
/// name and renamed over it, so a reader finds the one or the other whole, and no link under the
/// name is written through. Anything else under the name fails the completion and stays.
pub(crate) fn complete(held: &Held, pending: &Record, server_pid: u32) -> Result<(), Error> {
    let file = &held.record;
    if !matches!(load(file, Some(&pending.data_dir))?, Found::Good(r) if r == *pending) {
        return Err(Error::io("cannot complete", file, std::io::Error::other("it is no longer this start's record")));
    }
    let done = Record { server_pid: Some(server_pid), ..pending.clone() };
    let tmp = write_tmp(own_folder(held, &done)?, &done)?;
    std::fs::rename(&tmp, file).map_err(|e| {
        remove(&tmp);
        Error::io("cannot write", file, e)
    })
}

/// The record under the lock `held`, as it is: read only, nothing set aside. A file that holds no
/// record of the project at `data_dir` is an error.
pub(crate) fn look(held: &Held, data_dir: &str) -> Result<Option<Record>, Error> {
    match load(&held.record, Some(data_dir))? {
        Found::Absent => Ok(None),
        Found::Good(r) => Ok(Some(r)),
        Found::Bad(why) => Err(Error::io("cannot read", &held.record, std::io::Error::other(why))),
    }
}

/// Removes the project's record only while it still names `server_pid`, so a start never removes
/// the record of a server another start made. Returns whether the record's name is free now.
pub(crate) fn remove_if(records: &Path, data_dir: &str, server_pid: u32) -> bool {
    remove_while(&path(records, data_dir), |r| r.data_dir == data_dir && r.server_pid == Some(server_pid))
}

/// Removes the record in `file` only while `is_it` holds for it. The look and the removal happen
/// under the lock, so the record removed is the one looked at: one another start wrote since the
/// caller read its own is never removed, and never moved. A record that cannot be locked or
/// looked at stays, which the log says. Returns whether the name is free afterwards: not with
/// another record or a damaged file under it, nor when that could not be found out.
pub(crate) fn remove_while(file: &Path, is_it: impl Fn(&Record) -> bool) -> bool {
    let gone = locked(file, LOCK_WAIT, || remove_looked(file, &is_it));
    gone.and_then(|looked| looked).unwrap_or_else(|e| {
        log::warn!("the server record {} stays: {e}", file.display());
        false
    })
}

/// As [`remove_while`], for the record under the lock `held`, which the caller holds already: a
/// second take of that lock would wait for the caller itself.
pub(crate) fn remove_held(held: &Held, is_it: impl Fn(&Record) -> bool) -> bool {
    remove_looked(&held.record, &is_it).unwrap_or_else(|e| {
        log::warn!("the server record {} stays: {e}", held.record.display());
        false
    })
}

/// The look and the removal of [`remove_while`], for a caller that holds the lock.
fn remove_looked(file: &Path, is_it: &impl Fn(&Record) -> bool) -> Result<bool, Error> {
    match load(file, None)? {
        Found::Good(r) if is_it(&r) => {
            #[cfg(test)]
            tests::in_change(file);
            remove(file);
            Ok(!file.exists())
        }
        found => Ok(matches!(found, Found::Absent)),
    }
}

pub(crate) fn remove(file: &Path) {
    if let Err(e) = std::fs::remove_file(file)
        && e.kind() != std::io::ErrorKind::NotFound
    {
        log::warn!("the server record {} could not be removed: {e}", file.display());
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::cell::RefCell;

    thread_local! {
        /// What a test does in the middle of the next change, with the lock held: once.
        pub(crate) static IN_CHANGE: RefCell<Option<Box<dyn FnOnce(&Path)>>> = const { RefCell::new(None) };
        /// What a test does once the next change finds the lock held and starts to wait: once.
        pub(crate) static WAITING: RefCell<Option<Box<dyn FnOnce()>>> = const { RefCell::new(None) };
    }

    /// Runs the hook a test set for a change's wait for the lock, once.
    pub(crate) fn waiting() {
        if let Some(hook) = WAITING.with(|h| h.borrow_mut().take()) {
            hook();
        }
    }

    /// Runs the hook a test set for the middle of a change, once, with the record's file.
    pub(crate) fn in_change(file: &Path) {
        if let Some(hook) = IN_CHANGE.with(|h| h.borrow_mut().take()) {
            hook(file);
        }
    }

    /// Writes `r` as a start does: under its record's lock, taken first.
    pub(crate) fn write_new(records: &Path, r: &Record) -> Result<bool, Error> {
        super::write_new(&hold(&path(records, &r.data_dir), LOCK_WAIT)?, r)
    }

    /// Sets the next change's hook to try another change right there, where a second start or a
    /// second removal used to get in, and to note in `refused` that it had to wait.
    fn another_change_tries(refused: &std::rc::Rc<std::cell::Cell<bool>>) {
        let refused = refused.clone();
        IN_CHANGE.with(|h| *h.borrow_mut() = Some(Box::new(move |file: &Path| refused.set(locked(file, Duration::ZERO, || ()).is_err()))));
    }

    /// A record of the project at `data_dir`, for the tests.
    pub(crate) fn rec(data_dir: &str) -> Record {
        Record {
            data_dir: data_dir.into(),
            server_pid: Some(7),
            run_root: "/t".into(),
            run_dir: "/t/af-abcdefgh".into(),
            command: vec!["dolt".into()],
            owner_pid: 8,
            owner_started: "x".into(),
        }
    }

    #[test]
    fn fnv_matches_its_published_vectors() {
        assert_eq!(fnv1a64(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a64(b"a"), 0xaf63_dc4c_8601_ec8c);
        assert_eq!(fnv1a64(b"foobar"), 0x8594_4171_f739_67e8);
    }

    #[test]
    fn records_round_trip_and_bad_ones_are_set_aside() {
        let dir = crate::home::tests::scratch("records");
        assert!(write_new(&dir, &rec("/p/a")).unwrap());
        assert_eq!(read(&dir, "/p/a").unwrap(), Some(rec("/p/a")));
        // A second start's record never replaces the first's, and is never removed by it.
        let mut other = rec("/p/a");
        other.server_pid = Some(99);
        assert!(!write_new(&dir, &other).unwrap());
        remove_if(&dir, "/p/a", 99);
        assert_eq!(read(&dir, "/p/a").unwrap(), Some(rec("/p/a")));
        assert_eq!(all(&dir).unwrap().len(), 1);
        std::fs::write(path(&dir, "/p/b"), "{ damaged").unwrap();
        assert_eq!(read(&dir, "/p/b").unwrap(), None);
        // A record filed under another project's name is not trusted.
        std::fs::write(path(&dir, "/p/c"), serde_json::to_vec(&rec("/p/other")).unwrap()).unwrap();
        assert_eq!(read(&dir, "/p/c").unwrap(), None);
        std::fs::write(path(&dir, "/p/d"), vec![b' '; 5000]).unwrap();
        assert_eq!(read(&dir, "/p/d").unwrap(), None);
        // A process id kill would read as a group, or a relative run root, is not trusted.
        let mut bad = rec("/p/e");
        bad.server_pid = Some(0);
        std::fs::write(path(&dir, "/p/e"), serde_json::to_vec(&bad).unwrap()).unwrap();
        assert_eq!(read(&dir, "/p/e").unwrap(), None);
        let aside = std::fs::read_dir(&dir).unwrap().flatten().filter(|e| e.file_name().to_string_lossy().contains(".damaged-")).count();
        assert_eq!(aside, 4);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// What used to get in between a removal's look and its act (another removal, another start's
    /// write) and between a write's check and its link now waits: nothing changes in the middle.
    #[test]
    fn no_other_change_gets_into_the_middle_of_a_change() {
        let dir = crate::home::tests::Scratch::new("lock");
        let refused = std::rc::Rc::new(std::cell::Cell::new(false));
        another_change_tries(&refused);
        assert!(write_new(&dir.0, &rec("/p/a")).unwrap());
        assert!(refused.replace(false), "a change during a write waits");
        another_change_tries(&refused);
        remove_if(&dir.0, "/p/a", 7);
        assert!(refused.get(), "a change during a removal waits");
        assert_eq!(read(&dir.0, "/p/a").unwrap(), None);
        // The lock went with each change: the next one does not wait.
        assert!(write_new(&dir.0, &rec("/p/a")).unwrap());
    }

    /// A removal that read an earlier record leaves the one there now where it is: it is never
    /// moved, so no start finds its name free meanwhile.
    #[test]
    fn a_record_not_meant_stays_under_its_name() {
        let dir = crate::home::tests::Scratch::new("forget-back");
        assert!(write_new(&dir.0, &rec("/p/a")).unwrap());
        IN_CHANGE.with(|h| *h.borrow_mut() = Some(Box::new(|_: &Path| {})));
        remove_if(&dir.0, "/p/a", 99);
        assert!(IN_CHANGE.with(|h| h.borrow_mut().take()).is_some(), "nothing was removed");
        assert_eq!(read(&dir.0, "/p/a").unwrap(), Some(rec("/p/a")));
        let names: Vec<String> = std::fs::read_dir(&dir.0).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        assert_eq!(names.len(), 2, "the record and the lock file: {names:?}");
    }

    /// A stalled holder fails the other changes of its record, which change nothing, and holds up
    /// no other project's record. A holder that is only slow is waited for.
    #[test]
    fn a_change_that_cannot_have_the_lock_changes_nothing() {
        let dir = crate::home::tests::Scratch::new("lock-held");
        assert!(write_new(&dir.0, &rec("/p/a")).unwrap());
        std::fs::write(path(&dir.0, "/p/b"), "{ damaged").unwrap();
        let (a, b) = (hold(&path(&dir.0, "/p/a"), LOCK_WAIT).unwrap(), hold(&path(&dir.0, "/p/b"), LOCK_WAIT).unwrap());
        let before = std::fs::read_dir(&dir.0).unwrap().count();
        assert!(matches!(hold(&path(&dir.0, "/p/a"), LOCK_WAIT), Err(Error::Io { .. })));
        assert!(!remove_if(&dir.0, "/p/a", 7));
        assert!(read(&dir.0, "/p/b").is_err(), "the damaged record is not moved");
        assert_eq!(all(&dir.0).unwrap(), vec![(path(&dir.0, "/p/a"), rec("/p/a"))]);
        assert!(write_new(&dir.0, &rec("/p/c")).unwrap() && remove_if(&dir.0, "/p/c", 7), "another project's goes on");
        assert_eq!(std::fs::read_dir(&dir.0).unwrap().count(), before + 1, "nothing was added but the other project's lock");
        // The holder lets go only once the next change has found the lock held.
        WAITING.with(|h| *h.borrow_mut() = Some(Box::new(move || drop(a))));
        assert!(remove_if(&dir.0, "/p/a", 7));
        assert!(WAITING.with(|h| h.borrow().is_none()), "the removal waited");
        assert_eq!(read(&dir.0, "/p/a").unwrap(), None);
        drop(b);
    }

    /// Not a plain file under the lock's name: never opened. A lock is good for its own record.
    #[test]
    fn a_lock_that_is_not_a_plain_file_or_not_the_records_is_refused() {
        let dir = crate::home::tests::Scratch::new("lock-odd");
        std::fs::create_dir(path(&dir.0, "/p/a").with_extension("lock")).unwrap();
        let refused = write_new(&dir.0, &rec("/p/a"));
        assert!(matches!(&refused, Err(Error::Io { source, .. }) if source.to_string() == "it is not a plain file"), "{refused:?}");
        let other = super::write_new(&hold(&path(&dir.0, "/p/b"), LOCK_WAIT).unwrap(), &rec("/p/c"));
        assert!(matches!(&other, Err(Error::Io { source, .. }) if source.to_string() == "the lock held is another record's"), "{other:?}");
        assert_eq!(std::fs::read_dir(&dir.0).unwrap().count(), 2, "the two locks: no record, no temporary file");
    }

    /// A damaged record that cannot be set aside fails the read: it still holds the name. A
    /// record that cannot be looked at is not reported gone.
    #[test]
    fn a_record_that_cannot_be_set_aside_or_looked_at_is_not_passed_over() {
        let dir = crate::home::tests::Scratch::new("stuck");
        let file = path(&dir.0, "/p/a");
        std::fs::write(&file, "{ damaged").unwrap();
        for n in 1..100 {
            std::fs::write(file.with_extension(format!("json.damaged-{n}")), "").unwrap();
        }
        assert!(matches!(read(&dir.0, "/p/a"), Err(Error::Io { what, .. }) if what == "cannot set aside"));
        assert!(file.exists());
        assert!(!remove_if(&dir.0, "/p/a", 7), "the damaged file still holds the name");
        std::fs::create_dir(path(&dir.0, "/p/b")).unwrap();
        assert!(!remove_if(&dir.0, "/p/b", 7));
    }

    /// A start's record names no process until it is completed under the lock held since, and
    /// completing puts a new file under the name: a second name for the first file still reads as
    /// the first record. A name that no longer holds that record is not completed, and a removal
    /// under the lock held takes only the record meant. No temporary file is left.
    #[test]
    fn a_record_is_completed_by_a_new_file_and_only_its_own() {
        let dir = crate::home::tests::Scratch::new("complete");
        let pending = Record { server_pid: None, ..rec("/p/a") };
        let held = hold(&path(&dir.0, "/p/a"), LOCK_WAIT).unwrap();
        assert!(super::write_new(&held, &pending).unwrap());
        std::fs::hard_link(path(&dir.0, "/p/a"), dir.0.join("first")).unwrap();
        complete(&held, &pending, 7).unwrap();
        assert_eq!((look(&held, "/p/a").unwrap(), read_file(&dir.0.join("first"), None).unwrap()), (Some(rec("/p/a")), Some(pending.clone())));
        let again = complete(&held, &pending, 9);
        assert!(matches!(&again, Err(Error::Io { what, .. }) if what == "cannot complete"), "{again:?}");
        assert!(!remove_held(&held, |r| *r == pending), "the name holds another record");
        assert_eq!(look(&held, "/p/a").unwrap(), Some(rec("/p/a")));
        assert!(remove_held(&held, |r| *r == rec("/p/a")) && look(&held, "/p/a").unwrap().is_none());
        assert_eq!(std::fs::read_dir(&dir.0).unwrap().count(), 2, "the lock and the second name: no temporary file");
    }

    /// The writer and the reader take the same sizes. The record of a real canonical project
    /// folder, as long as this platform lets a test make one, takes exactly the most a record may
    /// take once it names the largest process id there is. It is written without a process id,
    /// read, completed with the largest valid one, read again and removed under the lock. One a
    /// byte longer is refused before anything is written. Linux makes a folder that long; macOS
    /// stops near 1,024 bytes (its PATH_MAX), so there another field makes up the rest. The path
    /// is measured as the record holds it: its folder has a name JSON escapes where the platform
    /// allows one, and on Windows its separators and verbatim prefix are escaped.
    #[test]
    fn a_record_at_the_limit_is_written_completed_read_and_removed_and_a_byte_more_is_refused() {
        let (dir, deep) = (crate::home::tests::Scratch::new("size"), crate::home::tests::Scratch::new("deep"));
        let root = deep.0.canonicalize().unwrap();
        let pending =
            Record { data_dir: String::new(), server_pid: None, run_root: root.to_str().unwrap().into(), owner_started: String::new(), ..rec("") };
        let largest = |r: &Record| serde_json::to_vec_pretty(&Record { server_pid: Some(u32::MAX), ..r.clone() }).unwrap().len();
        // What the limit leaves once the record names `data_dir` as written, escapes included.
        let room = |data_dir: &str| usize::try_from(CAP).unwrap().checked_sub(largest(&Record { data_dir: data_dir.into(), ..pending.clone() }));
        assert_eq!(room(&"\"".repeat(room("").unwrap())), None, "a path is measured as the record holds it");
        // The deepest canonical folder whose record fits that can be made, below a name JSON
        // escapes where the platform allows one.
        let (mut project, mut name) = (root.join("q\"\\"), 255);
        if std::fs::create_dir(&project).is_err() {
            project = root.clone();
        }
        while name > 0 {
            let next = project.join("d".repeat(name));
            let made = next.to_str().is_some_and(|t| room(t).is_some()) && std::fs::create_dir(&next).is_ok();
            if made && next.canonicalize().is_ok_and(|c| c == next) {
                project = next;
            } else {
                if made {
                    let _ = std::fs::remove_dir(&next);
                }
                name -= 1;
            }
        }
        let data_dir = project.to_str().unwrap().to_string();
        assert!(cfg!(windows) || data_dir.contains("q\"\\"), "a name JSON escapes is in the path: {data_dir}");
        let at =
            |extra: usize| Record { owner_started: "x".repeat(room(&data_dir).unwrap_or(0) + extra), data_dir: data_dir.clone(), ..pending.clone() };
        let (edge, pid) = (at(0), u32::try_from(i32::MAX).unwrap());
        assert_eq!(largest(&edge), 4096, "{} bytes of the path are a real folder's", data_dir.len());
        let (file, done) = (path(&dir.0, &data_dir), Record { server_pid: Some(pid), ..edge.clone() });
        let held = hold(&file, LOCK_WAIT).unwrap();
        assert!(super::write_new(&held, &edge).unwrap());
        assert_eq!(look(&held, &data_dir).unwrap(), Some(edge.clone()));
        complete(&held, &edge, pid).unwrap();
        assert_eq!((std::fs::metadata(&file).unwrap().len(), look(&held, &data_dir).unwrap()), (CAP, Some(done.clone())));
        assert_eq!(read(&dir.0, &data_dir).unwrap(), Some(done.clone()));
        assert!(remove_held(&held, |r| *r == done) && !file.exists(), "the record at the limit is removed");
        let refused = super::write_new(&held, &at(1));
        assert!(matches!(&refused, Err(Error::RecordTooLarge { len, max: CAP, .. }) if *len == 4097), "{refused:?}");
        assert_eq!(std::fs::read_dir(&dir.0).unwrap().count(), 1, "the lock alone: nothing of the longer record");
    }

    /// A record of the earlier format, with a process id always, reads as a complete record; one
    /// whose process id is left out or null names no process.
    #[test]
    fn a_record_of_the_earlier_format_is_a_complete_record() {
        let dir = crate::home::tests::Scratch::new("format");
        let old =
            r#"{"data_dir":"/p/a","server_pid":7,"run_root":"/t","run_dir":"/t/af-abcdefgh","command":["dolt"],"owner_pid":8,"owner_started":"x"}"#;
        std::fs::write(path(&dir.0, "/p/a"), old).unwrap();
        assert_eq!(read(&dir.0, "/p/a").unwrap(), Some(rec("/p/a")));
        let held = hold(&path(&dir.0, "/p/a"), LOCK_WAIT).unwrap();
        assert_eq!(look(&held, "/p/a").unwrap(), Some(rec("/p/a")));
        assert!(look(&held, "/p/b").is_err(), "a record of another project is none of this one's");
        drop(held);
        for pid in ["", r#""server_pid":null,"#] {
            std::fs::write(path(&dir.0, "/p/a"), old.replace(r#""server_pid":7,"#, pid)).unwrap();
            assert_eq!(read(&dir.0, "/p/a").unwrap(), Some(Record { server_pid: None, ..rec("/p/a") }));
        }
    }

    /// A start that wrote its record over a damaged one between a reader's first look and its
    /// second keeps it: the reader sets nothing aside and reads the new record.
    #[test]
    fn a_record_written_over_a_damaged_one_is_not_set_aside() {
        let dir = crate::home::tests::Scratch::new("second-look");
        std::fs::write(path(&dir.0, "/p/a"), "{ damaged").unwrap();
        IN_CHANGE.with(|h| *h.borrow_mut() = Some(Box::new(|file: &Path| std::fs::write(file, serde_json::to_vec(&rec("/p/a")).unwrap()).unwrap())));
        assert_eq!(read(&dir.0, "/p/a").unwrap(), Some(rec("/p/a")));
        assert_eq!(all(&dir.0).unwrap().len(), 1);
    }
}
