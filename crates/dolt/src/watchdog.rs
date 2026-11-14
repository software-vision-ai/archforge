//! Ties a server to the Archforge that started it, so a killed Archforge's server stops at once
//! (unix; elsewhere the start-up sweep alone finds it).
//!
//! The watchdog is a small `/bin/sh` child whose input is the read end of a pipe. Archforge holds
//! the only write end and never writes to it, so the watchdog's read ends only when Archforge is
//! gone, however it went, a kill included: then it stops the server politely, if the server's
//! process id still runs the command line this crate started.
//!
//! Who holds the write end decides when that is. The standard library marks it close-on-exec, so
//! no program started later inherits it, and the server is started before its watchdog. On Linux
//! the mark is set as the pipe is made. On macOS it is set just after, so a program another thread
//! starts in between would inherit the write end: [`runner::spawn`](crate::runner::spawn) keeps
//! this crate's own starts apart, and a start elsewhere in the application that hits that moment
//! delays the watchdog until that program and everything it started have ended. The start-up sweep
//! covers that case, as it does a system without `/bin/ps` or the UTF-8 locale named below, where
//! the watchdog proves nothing and so sends nothing.

use std::process::Child;

/// The name the watchdog runs under, before the server's process id: what tells it apart in a
/// process list.
#[cfg(unix)]
const NAME: &str = "archforge-server-watchdog";

/// The variable that gives the watchdog the server's command line, joined by spaces. Not an
/// argument: the watchdog's own command line would then read as a server's.
#[cfg(unix)]
const SERVER: &str = "ARCHFORGE_SERVER";

/// What the watchdog runs, given the server's process id. It outlives the terminal's hang-up and
/// interrupt, which may be what ends Archforge. Unlike the crate's other helpers it has no budget:
/// waiting is its work.
#[cfg(unix)]
const SCRIPT: &str = r#"trap '' HUP INT
while read -r line; do :; done
[ "$(/bin/ps -ww -o args= -p "$1")" = "$ARCHFORGE_SERVER" ] && exec /bin/kill -TERM "$1""#;

/// A server's watchdog, or none where there is none to have. Dropping it ends it without a signal
/// to the server: stopping the server is the caller's work.
#[derive(Debug)]
pub(crate) struct Watchdog(Option<Child>);

impl Watchdog {
    /// Starts the watchdog of the server `server` that runs `command`. A watchdog that cannot
    /// start is logged and done without: the start-up sweep still finds the server.
    #[cfg(unix)]
    pub(crate) fn tie(server: u32, command: &[String]) -> Watchdog {
        use std::process::Stdio;

        let sh = std::path::Path::new("/bin/sh");
        let mut env = crate::runner::base_env();
        env.push((SERVER.into(), command.join(" ").into()));
        // In the plain C locale `ps` rewrites every character outside ASCII, and nothing would match.
        env.push(("LC_ALL".into(), "C.UTF-8".into()));
        let mut c = crate::runner::command(sh, &env);
        c.args(["-c", SCRIPT, NAME]).arg(server.to_string());
        c.current_dir("/").stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null());
        match crate::runner::spawn(&mut c) {
            Ok(child) => Watchdog(Some(child)),
            Err(e) => {
                log::warn!("the database server {server} has no watchdog, so only the next start-up stops it if Archforge is killed: {e}");
                Watchdog(None)
            }
        }
    }

    /// No watchdog on this system.
    #[cfg(not(unix))]
    pub(crate) fn tie(_server: u32, _command: &[String]) -> Watchdog {
        Watchdog(None)
    }
}

impl Drop for Watchdog {
    /// Ends the watchdog before it can read the pipe's end, and waits for it.
    fn drop(&mut self) {
        if let Some(child) = &mut self.0 {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use std::os::unix::process::ExitStatusExt;
    use std::process::{Command, Stdio};

    use super::*;

    /// A process to watch that waits on its input, ended with the test. Its command line holds
    /// characters outside ASCII, as a server's may. Waiting for it closes its input, so it ends by
    /// itself unless a signal was sent before: one sent earlier is what ends it then.
    struct StandIn(Child);

    /// The stand-in's command line.
    fn command(last: &str) -> Vec<String> {
        ["/bin/sh", "-c", "read x", last].map(String::from).to_vec()
    }

    impl StandIn {
        /// Starts it with an input only this test holds open.
        fn start() -> std::io::Result<StandIn> {
            Command::new("/bin/sh").args(&command("é ü")[1..]).stdin(Stdio::piped()).spawn().map(StandIn)
        }
    }

    impl Drop for StandIn {
        /// Ends it, whatever the test left of it.
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    /// What Archforge's death does to the pipe: closes its write end. Waits for the watchdog to
    /// have decided and ended.
    fn archforge_dies(mut w: Watchdog) {
        if let Some(child) = &mut w.0 {
            drop(child.stdin.take());
            let _ = child.wait();
        }
    }

    /// The watchdog acts on the end of the pipe, and by a polite signal.
    #[test]
    fn once_archforge_is_gone_the_process_it_was_tied_to_is_stopped_politely() {
        let mut s = StandIn::start().unwrap();
        archforge_dies(Watchdog::tie(s.0.id(), &command("é ü")));
        assert_eq!(s.0.wait().unwrap().signal(), Some(15));
    }

    /// The process id is the same and the command line differs in its last argument only.
    #[test]
    fn a_process_that_took_the_servers_id_gets_no_signal() {
        let mut s = StandIn::start().unwrap();
        archforge_dies(Watchdog::tie(s.0.id(), &command("é u")));
        assert_eq!(s.0.wait().unwrap().signal(), None);
    }

    /// A dropped watchdog is reaped at once and never got to signal, though its process matches.
    #[test]
    fn dropping_it_ends_it_and_leaves_the_server_alone() {
        let mut s = StandIn::start().unwrap();
        let w = Watchdog::tie(s.0.id(), &command("é ü"));
        let pid = w.0.as_ref().unwrap().id().to_string();
        drop(w);
        let there = Command::new("/bin/kill").args(["-0", &pid]).stderr(Stdio::null()).status().unwrap();
        assert!(!there.success(), "the watchdog is gone, not even a zombie");
        assert_eq!(s.0.wait().unwrap().signal(), None);
    }

    /// Recovery refuses a project whose lock names a running server: a watchdog is none.
    #[test]
    fn a_watchdog_does_not_read_as_a_server() {
        let s = StandIn::start().unwrap();
        let w = Watchdog::tie(s.0.id(), &["/opt/dolt", "sql-server", "--config", "/run/server.yaml"].map(String::from));
        let pid = w.0.as_ref().unwrap().id();
        assert!(!crate::identity::identity(pid).unwrap().unwrap().is_dolt_server());
    }
}
