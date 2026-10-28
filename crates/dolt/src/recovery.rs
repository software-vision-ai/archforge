//! What to do about what an earlier start left behind, decided from facts gathered first: a pure
//! function, so every rule has its test. Nothing is stopped unless its owner is dead and the
//! server is proven ours; any helper failure while gathering the facts touches nothing.

use std::path::Path;

use crate::Error;

/// The project's lock file, `.dolt/sql-server.info`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Lock {
    Absent,
    Damaged,
    Pid(u32),
}

/// What a process named in the lock or the record is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Proc {
    /// No such process, or a zombie.
    Gone,
    /// The server this crate recorded, its owner dead.
    OursOwnerDead,
    /// The server this crate recorded, its owner alive.
    OursOwnerAlive,
    /// Another dolt sql-server.
    OtherDoltServer,
    /// Something else, for instance a process that took a reused id.
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Action {
    Proceed,
    /// Remove the record and its run folder, then proceed.
    Clean,
    /// Stop the server with this id (checking again before each signal), clean, then proceed.
    Stop(u32),
    OpenElsewhere(u32),
    DamagedLock,
}

/// The rules of the spec, in order.
pub(crate) fn decide(lock: &Lock, holder: Proc, record: Option<(u32, Proc)>) -> Action {
    match (lock, holder) {
        (Lock::Damaged, _) => Action::DamagedLock,
        (Lock::Pid(pid), Proc::OursOwnerDead) => Action::Stop(*pid),
        (Lock::Pid(pid), Proc::OursOwnerAlive | Proc::OtherDoltServer) => Action::OpenElsewhere(*pid),
        // No lock, its process gone, or a process dolt takes the lock over from: the record decides.
        _ => match record {
            None => Action::Proceed,
            Some((_, Proc::Gone | Proc::Other | Proc::OtherDoltServer)) => Action::Clean,
            Some((pid, Proc::OursOwnerDead)) => Action::Stop(pid),
            Some((pid, Proc::OursOwnerAlive)) => Action::OpenElsewhere(pid),
        },
    }
}

/// The lock file's process id, read with a cap: `pid:port:uuid` as dolt writes it.
pub(crate) fn read_lock(data_dir: &Path) -> Result<Lock, Error> {
    use std::io::Read;
    let file = data_dir.join(".dolt").join("sql-server.info");
    // A link to a pipe or a device could block the read: only a plain file is a lock.
    match std::fs::symlink_metadata(&file) {
        Ok(m) if !m.is_file() => return Ok(Lock::Damaged),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Lock::Absent),
        _ => {}
    }
    let f = match std::fs::File::open(&file) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Lock::Absent),
        Err(e) => return Err(Error::io("cannot read", &file, e)),
    };
    let mut bytes = Vec::new();
    f.take(1025).read_to_end(&mut bytes).map_err(|e| Error::io("cannot read", &file, e))?;
    Ok(parse_lock(&bytes))
}

pub(crate) fn parse_lock(bytes: &[u8]) -> Lock {
    if bytes.len() > 1024 {
        return Lock::Damaged;
    }
    let Ok(text) = std::str::from_utf8(bytes) else { return Lock::Damaged };
    let parts: Vec<&str> = text.trim().split(':').collect();
    match parts.as_slice() {
        [pid, port, uuid] if port.parse::<u16>().is_ok() && !uuid.is_empty() => {
            pid.parse().ok().filter(|p| valid_pid(*p)).map(Lock::Pid).unwrap_or(Lock::Damaged)
        }
        _ => Lock::Damaged,
    }
}

/// A process id `kill` takes as one process: 0 means the caller's group and anything above
/// `i32::MAX` wraps to "every process", so neither is ever accepted from a file.
pub(crate) fn valid_pid(pid: u32) -> bool {
    pid >= 1 && i32::try_from(pid).is_ok()
}

#[cfg(test)]
mod tests {
    use super::Proc::*;
    use super::*;

    #[test]
    fn every_rule_of_the_table() {
        let none = None;
        // 1: a damaged lock touches nothing, whatever else holds.
        assert_eq!(decide(&Lock::Damaged, Gone, Some((5, OursOwnerDead))), Action::DamagedLock);
        // 2: the lock's holder is a live dolt server.
        assert_eq!(decide(&Lock::Pid(9), OursOwnerDead, Some((9, OursOwnerDead))), Action::Stop(9));
        assert_eq!(decide(&Lock::Pid(9), OursOwnerAlive, Some((9, OursOwnerAlive))), Action::OpenElsewhere(9));
        assert_eq!(decide(&Lock::Pid(9), OtherDoltServer, none), Action::OpenElsewhere(9));
        assert_eq!(decide(&Lock::Pid(9), OtherDoltServer, Some((5, Gone))), Action::OpenElsewhere(9));
        // 3: no lock, a dead holder, or a holder that is not dolt: the record decides.
        for (lock, holder) in [(Lock::Absent, Gone), (Lock::Pid(9), Gone), (Lock::Pid(9), Other)] {
            assert_eq!(decide(&lock, holder, none), Action::Proceed);
            assert_eq!(decide(&lock, holder, Some((5, Gone))), Action::Clean);
            assert_eq!(decide(&lock, holder, Some((5, Other))), Action::Clean, "a reused process id");
            assert_eq!(decide(&lock, holder, Some((5, OursOwnerDead))), Action::Stop(5), "an orphan from before the lock");
            assert_eq!(decide(&lock, holder, Some((5, OursOwnerAlive))), Action::OpenElsewhere(5));
        }
    }

    #[test]
    fn lock_files_are_read_strictly() {
        assert_eq!(parse_lock(b"25040:52894:197fddbc-45da-4905-ad36-d5affdf64bcd"), Lock::Pid(25040));
        assert_eq!(parse_lock(b"25040:52894:uuid\n"), Lock::Pid(25040));
        for bad in [&b""[..], b"x:1:u", b"1:99999:u", b"1:2", b"1:2:", &[0xff, 0xfe][..]] {
            assert_eq!(parse_lock(bad), Lock::Damaged, "{bad:?}");
        }
        assert_eq!(parse_lock(&vec![b'1'; 1025]), Lock::Damaged);
        assert_eq!(parse_lock(b"0:52894:u"), Lock::Damaged, "0 is the caller's own process group");
        assert_eq!(parse_lock(b"4294967295:52894:u"), Lock::Damaged, "it would wrap to every process");
        assert_eq!(parse_lock(b"2147483647:52894:u"), Lock::Pid(2_147_483_647));
    }
}
