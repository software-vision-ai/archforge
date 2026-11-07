//! What the operating system provides: process groups, signals, a wait that leaves the child
//! waitable, file modes and owners. Unix only; elsewhere every call says it is not available, and
//! the public entry points answer [`crate::AgentError::NotAvailable`] before reaching here.

use std::fs::{File, Metadata};
use std::io;
use std::path::Path;
use std::process::{Child, Command, ExitStatus};

use crate::Exit;

/// How often [`kill_group`] signals a group, and the pause between passes.
const KILL_PASSES: u32 = 3;
const KILL_PAUSE: std::time::Duration = std::time::Duration::from_millis(5);

/// Kills every process in the group led by `raw`, and `raw` itself in case it left the group. The
/// group is signalled more than once: a process the group was forking at the moment of the first
/// signal can come into being after it (observed on macOS) and is caught by the next pass, while its
/// parent, killed by the first, starts no more. Call it only while `raw` is not collected yet, so
/// its id cannot belong to another process or group.
pub(crate) fn kill_group(raw: u32) {
    let _ = signal_process(raw, Sig::Kill);
    for pass in 0..KILL_PASSES {
        let _ = signal_group(raw, Sig::Kill);
        if pass.saturating_add(1) < KILL_PASSES {
            std::thread::sleep(KILL_PAUSE);
        }
    }
}

/// The signals this crate sends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Sig {
    /// SIGINT: stop the current turn.
    Interrupt,
    /// SIGTERM: end, cleanly.
    Terminate,
    /// SIGKILL: end now.
    Kill,
}

#[cfg(unix)]
mod imp {
    use std::os::unix::fs::{DirBuilderExt, MetadataExt};
    use std::os::unix::process::{CommandExt, ExitStatusExt};

    use rustix::process::{Pid, Signal, WaitId, WaitIdOptions};

    use super::*;

    /// Whether this system can run the agent runtimes.
    pub(crate) const AVAILABLE: bool = true;

    /// The child's process id as the signal calls take it; `None` for an id the system could not
    /// have given (zero or past `i32`).
    fn pid(raw: u32) -> Option<Pid> {
        i32::try_from(raw).ok().and_then(Pid::from_raw)
    }

    /// The system signal for `sig`.
    fn signal(sig: Sig) -> Signal {
        match sig {
            Sig::Interrupt => Signal::INT,
            Sig::Terminate => Signal::TERM,
            Sig::Kill => Signal::KILL,
        }
    }

    /// Starts the command in a new process group whose id is the child's own process id, so a
    /// signal to the group reaches every process the runtime starts and nothing else.
    pub(crate) fn own_group(command: &mut Command) {
        command.process_group(0);
    }

    /// Sends `sig` to the process `raw` alone.
    pub(crate) fn signal_process(raw: u32, sig: Sig) -> io::Result<()> {
        let pid = pid(raw).ok_or_else(|| io::Error::other(format!("{raw} is not a process id")))?;
        rustix::process::kill_process(pid, signal(sig)).map_err(io::Error::from)
    }

    /// Sends `sig` to every process in the group led by `raw`.
    pub(crate) fn signal_group(raw: u32, sig: Sig) -> io::Result<()> {
        let pid = pid(raw).ok_or_else(|| io::Error::other(format!("{raw} is not a process id")))?;
        rustix::process::kill_process_group(pid, signal(sig)).map_err(io::Error::from)
    }

    /// Whether the child `child` has ended, without collecting it: an ended child stays a zombie,
    /// so its process id (the group's id) cannot be given to another process before the group is
    /// signalled and the child collected.
    pub(crate) fn has_ended(child: &Child) -> io::Result<bool> {
        let pid = pid(child.id()).ok_or_else(|| io::Error::other(format!("{} is not a process id", child.id())))?;
        let options = WaitIdOptions::EXITED | WaitIdOptions::NOHANG | WaitIdOptions::NOWAIT;
        loop {
            match rustix::process::waitid(WaitId::Pid(pid), options) {
                Ok(status) => return Ok(status.is_some()),
                Err(rustix::io::Errno::INTR) => continue,
                Err(e) => return Err(io::Error::from(e)),
            }
        }
    }

    /// How a collected child ended.
    pub(crate) fn exit_of(status: ExitStatus) -> Exit {
        Exit { code: status.code(), signal: status.signal() }
    }

    /// The effective user id, which owns every private folder.
    pub(crate) fn current_uid() -> u32 {
        rustix::process::geteuid().as_raw()
    }

    /// Whether `meta` describes a regular file someone may execute.
    pub(crate) fn is_executable(meta: &Metadata) -> bool {
        meta.is_file() && meta.mode() & 0o111 != 0
    }

    /// Why `meta` is not a private folder of `uid` (a real folder, not a link, owned by `uid`, with
    /// no group or other permission bits), or `None` when it is.
    pub(crate) fn private_folder_problem(meta: &Metadata, uid: u32) -> Option<String> {
        if meta.file_type().is_symlink() {
            return Some("is a link, not a folder".to_owned());
        }
        if !meta.is_dir() {
            return Some("is not a folder".to_owned());
        }
        if meta.uid() != uid {
            return Some(format!("belongs to user {}, not to this user ({uid})", meta.uid()));
        }
        if meta.mode() & 0o077 != 0 {
            return Some(format!("can be read or entered by others (mode {:o}); only its owner may (mode 700)", meta.mode() & 0o7777));
        }
        None
    }

    /// Makes the folder `path` with mode 0700, and with `parents` any missing folder above it too.
    pub(crate) fn make_private_folder(path: &Path, parents: bool) -> io::Result<()> {
        std::fs::DirBuilder::new().recursive(parents).mode(0o700).create(path)
    }

    /// Why another account could write to what `meta` describes, or `None`: it must belong to `uid`
    /// or to the system (root) and must not be writable by others; a group write bit is accepted
    /// only on what `uid` owns (Homebrew's folders are group-writable by the administrators).
    pub(crate) fn shared_write_problem(meta: &Metadata, uid: u32) -> Option<String> {
        let owner = meta.uid();
        if owner != uid && owner != 0 {
            return Some(format!("belongs to user {owner}, neither this user ({uid}) nor the system"));
        }
        let mode = meta.mode();
        if mode & 0o002 != 0 || (owner != uid && mode & 0o020 != 0) {
            return Some(format!("can be written by other users (mode {:o})", mode & 0o7777));
        }
        None
    }

    /// Creates a new file at `path` for writing: exclusively (whatever stands there, a link among
    /// them, is refused rather than opened), with mode 0600, never following a link, closed on exec.
    pub(crate) fn create_private_file(path: &Path) -> io::Result<File> {
        use std::os::unix::fs::OpenOptionsExt;
        let no_follow = i32::try_from(rustix::fs::OFlags::NOFOLLOW.bits()).map_err(|_| io::Error::other("O_NOFOLLOW does not fit an int"))?;
        std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).custom_flags(no_follow).open(path)
    }

    /// Opens the file at `path` for reading without following a link and without blocking on a
    /// pipe or a device, and refuses anything but a regular file (checked on the open handle).
    pub(crate) fn open_regular(path: &Path) -> io::Result<File> {
        use std::os::unix::fs::OpenOptionsExt;
        let flags = rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::NONBLOCK;
        let flags = i32::try_from(flags.bits()).map_err(|_| io::Error::other("the open flags do not fit an int"))?;
        let file = std::fs::OpenOptions::new().read(true).custom_flags(flags).open(path)?;
        if file.metadata()?.is_file() { Ok(file) } else { Err(io::Error::other("it is not a regular file")) }
    }

    /// Why the file at `path` is not the file `handle` describes, a regular file of `uid` with mode
    /// 0600 (the same device and inode), or `None` when it is. `path` is looked at without following
    /// a link.
    pub(crate) fn same_private_file(handle: &Metadata, path: &Path, uid: u32) -> Option<String> {
        let on_disk = match std::fs::symlink_metadata(path) {
            Ok(meta) => meta,
            Err(e) => return Some(format!("cannot be read back: {e}")),
        };
        if !on_disk.file_type().is_file() {
            return Some("is no longer a regular file".to_owned());
        }
        if on_disk.dev() != handle.dev() || on_disk.ino() != handle.ino() {
            return Some("was replaced by another file".to_owned());
        }
        if on_disk.uid() != uid {
            return Some(format!("belongs to user {}, not to this user ({uid})", on_disk.uid()));
        }
        if on_disk.mode() & 0o7777 != 0o600 {
            return Some(format!("has mode {:o}, not 600", on_disk.mode() & 0o7777));
        }
        None
    }
}

#[cfg(not(unix))]
mod imp {
    use super::*;

    /// Whether this system can run the agent runtimes.
    pub(crate) const AVAILABLE: bool = false;

    /// Not available on this system.
    fn unavailable() -> io::Error {
        io::Error::new(io::ErrorKind::Unsupported, "agent runtimes are not available on this system yet")
    }

    /// Not available on this system: the command is left as it is.
    pub(crate) fn own_group(_command: &mut Command) {}

    /// Not available on this system.
    pub(crate) fn signal_process(_raw: u32, _sig: Sig) -> io::Result<()> {
        Err(unavailable())
    }

    /// Not available on this system.
    pub(crate) fn signal_group(_raw: u32, _sig: Sig) -> io::Result<()> {
        Err(unavailable())
    }

    /// Not available on this system.
    pub(crate) fn has_ended(_child: &Child) -> io::Result<bool> {
        Err(unavailable())
    }

    /// How a collected child ended: its code alone.
    pub(crate) fn exit_of(status: ExitStatus) -> Exit {
        Exit { code: status.code(), signal: None }
    }

    /// No user ids here.
    pub(crate) fn current_uid() -> u32 {
        0
    }

    /// A regular file, as nothing else can be told here.
    pub(crate) fn is_executable(meta: &Metadata) -> bool {
        meta.is_file()
    }

    /// Not available on this system.
    pub(crate) fn private_folder_problem(_meta: &Metadata, _uid: u32) -> Option<String> {
        Some("cannot be checked on this system".to_owned())
    }

    /// Not available on this system.
    pub(crate) fn make_private_folder(_path: &Path, _parents: bool) -> io::Result<()> {
        Err(unavailable())
    }

    /// Not available on this system.
    pub(crate) fn shared_write_problem(_meta: &Metadata, _uid: u32) -> Option<String> {
        Some("cannot be checked on this system".to_owned())
    }

    /// Not available on this system.
    pub(crate) fn create_private_file(_path: &Path) -> io::Result<File> {
        Err(unavailable())
    }

    /// Not available on this system.
    pub(crate) fn open_regular(_path: &Path) -> io::Result<File> {
        Err(unavailable())
    }

    /// Not available on this system.
    pub(crate) fn same_private_file(_handle: &Metadata, _path: &Path, _uid: u32) -> Option<String> {
        Some("cannot be checked on this system".to_owned())
    }
}

pub(crate) use imp::*;
