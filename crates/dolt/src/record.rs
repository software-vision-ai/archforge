//! A record per running server, in the user's settings: which process serves which project, in
//! which run folder, started by which Archforge. Recovery recognises a leftover server by it.

use std::io::Read;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::Error;

/// The most of a record read; a larger file is set aside.
const CAP: u64 = 4096;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Record {
    pub data_dir: String,
    pub server_pid: u32,
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
/// about another project, is set aside and counts as none.
pub(crate) fn read(records: &Path, data_dir: &str) -> Result<Option<Record>, Error> {
    let file = path(records, data_dir);
    Ok(read_file(&file)?.filter(|r| {
        let mine = r.data_dir == data_dir;
        if !mine {
            set_aside(&file, "it names another project");
        }
        mine
    }))
}

/// Every record in `records`, for the sweep at start.
pub(crate) fn all(records: &Path) -> Result<Vec<(PathBuf, Record)>, Error> {
    let entries = match std::fs::read_dir(records) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(Error::io("cannot list", records, e)),
    };
    let mut out = Vec::new();
    for e in entries.flatten() {
        let p = e.path();
        if p.extension().is_some_and(|x| x == "json")
            && let Some(r) = read_file(&p)?
        {
            out.push((p, r));
        }
    }
    Ok(out)
}

fn read_file(file: &Path) -> Result<Option<Record>, Error> {
    let f = match std::fs::File::open(file) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(Error::io("cannot read", file, e)),
    };
    let mut bytes = Vec::new();
    f.take(CAP + 1).read_to_end(&mut bytes).map_err(|e| Error::io("cannot read", file, e))?;
    if bytes.len() as u64 > CAP {
        set_aside(file, "it is too large");
        return Ok(None);
    }
    match serde_json::from_slice::<Record>(&bytes) {
        Ok(r) if crate::recovery::valid_pid(r.server_pid) && crate::recovery::valid_pid(r.owner_pid) && Path::new(&r.run_root).is_absolute() => {
            Ok(Some(r))
        }
        _ => {
            set_aside(file, "it cannot be read");
            Ok(None)
        }
    }
}

/// Moves a record that cannot be used out of the way, under a new name, and says so in the log.
fn set_aside(file: &Path, why: &str) {
    for n in 1..100 {
        let target = file.with_extension(format!("json.damaged-{n}"));
        if !target.exists() {
            match std::fs::rename(file, &target) {
                Ok(()) => log::warn!("the server record {} was set aside as {}: {why}", file.display(), target.display()),
                Err(e) => log::warn!("the server record {} could not be set aside ({why}): {e}", file.display()),
            }
            return;
        }
    }
    log::warn!("the server record {} could not be set aside ({why}): too many set aside", file.display());
}

/// Writes a project's record unless one is there already, atomically and exclusively: the record
/// is written in full under a temporary name, then linked to its own name, which fails if another
/// start got there first. Returns whether it was written.
pub(crate) fn write_new(records: &Path, r: &Record) -> Result<bool, Error> {
    use std::io::Write;
    let file = path(records, &r.data_dir);
    let bytes = serde_json::to_vec_pretty(r).map_err(|e| Error::io("cannot write", &file, std::io::Error::other(e)))?;
    let tmp = records.join(format!(".record-{}.tmp", crate::secret::random_name(8)?));
    let written = std::fs::File::create_new(&tmp).and_then(|mut f| {
        f.write_all(&bytes)?;
        f.sync_all()
    });
    let linked = written.and_then(|()| std::fs::hard_link(&tmp, &file));
    let _ = std::fs::remove_file(&tmp);
    match linked {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(false),
        Err(e) => Err(Error::io("cannot write", &file, e)),
    }
}

/// Removes the project's record only while it still names `server_pid`, so a start never removes
/// the record of a server another start made.
pub(crate) fn remove_if(records: &Path, data_dir: &str, server_pid: u32) {
    remove_while(&path(records, data_dir), |r| r.data_dir == data_dir && r.server_pid == server_pid);
}

/// Removes the record in `file` only while `is_it` holds for it. It is taken aside under a new name
/// first and compared there, so a record another start writes meanwhile is never the one removed.
/// One that is not meant goes back, or, if a newer record took its name meanwhile, stays under the
/// new name, which ends in `.json`, so the start-up sweep reads it again.
pub(crate) fn remove_while(file: &Path, is_it: impl Fn(&Record) -> bool) {
    let aside = match crate::secret::random_name(8) {
        Ok(n) => file.with_extension(format!("forget-{n}.json")),
        Err(e) => return log::warn!("the server record {} stays: {e}", file.display()),
    };
    match std::fs::rename(file, &aside) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return,
        Err(e) => return log::warn!("the server record {} stays: it could not be taken aside: {e}", file.display()),
    }
    #[cfg(test)]
    tests::after_aside(file);
    match read_file(&aside) {
        Ok(Some(r)) if is_it(&r) => remove(&aside),
        // A damaged record was set aside by the read, and one that went is gone.
        Ok(None) => {}
        Ok(Some(_)) | Err(_) => put_back(&aside, file),
    }
}

/// Puts a record taken aside back under its own name, unless a newer one took that name meanwhile.
fn put_back(aside: &Path, file: &Path) {
    match std::fs::hard_link(aside, file) {
        Ok(()) => remove(aside),
        Err(e) => log::warn!("the server record {} stays as {}: it could not be put back: {e}", file.display(), aside.display()),
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
        /// What another start does right after a record was taken aside, for the race tests.
        static AFTER_ASIDE: RefCell<Option<Box<dyn FnOnce(&Path)>>> = const { RefCell::new(None) };
    }

    /// Runs the hook a test set for the moment after a record was taken aside, once.
    pub(crate) fn after_aside(file: &Path) {
        if let Some(hook) = AFTER_ASIDE.with(|h| h.borrow_mut().take()) {
            hook(file);
        }
    }

    /// A record of the project at `data_dir`, for the tests.
    pub(crate) fn rec(data_dir: &str) -> Record {
        Record {
            data_dir: data_dir.into(),
            server_pid: 7,
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
        other.server_pid = 99;
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
        bad.server_pid = 0;
        std::fs::write(path(&dir, "/p/e"), serde_json::to_vec(&bad).unwrap()).unwrap();
        assert_eq!(read(&dir, "/p/e").unwrap(), None);
        let aside = std::fs::read_dir(&dir).unwrap().flatten().filter(|e| e.file_name().to_string_lossy().contains(".damaged-")).count();
        assert_eq!(aside, 4);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Another sweep forgets the record and another start writes its own right after this one
    /// compared: the newer record stays, the old one goes.
    #[test]
    fn a_record_written_while_one_is_forgotten_stays() {
        let dir = crate::home::tests::Scratch::new("forget-race");
        assert!(write_new(&dir.0, &rec("/p/a")).unwrap());
        let newer = Record { server_pid: 9, ..rec("/p/a") };
        let written = newer.clone();
        AFTER_ASIDE.with(|h| {
            *h.borrow_mut() = Some(Box::new(move |file: &Path| {
                remove(file);
                assert!(write_new(file.parent().unwrap(), &written).unwrap());
            }))
        });
        remove_while(&path(&dir.0, "/p/a"), |r| r == &rec("/p/a"));
        assert_eq!(read(&dir.0, "/p/a").unwrap(), Some(newer));
        assert_eq!(all(&dir.0).unwrap().len(), 1, "the old record went");
    }

    /// A record that is not the one meant goes back under its own name; when a newer one took
    /// that name meanwhile, both stay.
    #[test]
    fn a_record_not_meant_goes_back_or_stays_beside_a_newer_one() {
        let dir = crate::home::tests::Scratch::new("forget-back");
        assert!(write_new(&dir.0, &rec("/p/a")).unwrap());
        remove_if(&dir.0, "/p/a", 99);
        assert_eq!(read(&dir.0, "/p/a").unwrap(), Some(rec("/p/a")), "it went back");
        assert_eq!(all(&dir.0).unwrap().len(), 1);
        let newer = Record { server_pid: 9, ..rec("/p/a") };
        let written = newer.clone();
        AFTER_ASIDE.with(|h| *h.borrow_mut() = Some(Box::new(move |file: &Path| assert!(write_new(file.parent().unwrap(), &written).unwrap()))));
        remove_if(&dir.0, "/p/a", 99);
        assert_eq!(read(&dir.0, "/p/a").unwrap(), Some(newer));
        let kept: Vec<Record> = all(&dir.0).unwrap().into_iter().map(|(_, r)| r).filter(|r| r.server_pid == 7).collect();
        assert_eq!(kept, vec![rec("/p/a")], "the one taken aside stays where the sweep reads it");
    }
}
