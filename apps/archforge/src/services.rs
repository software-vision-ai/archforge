//! The host services the shell may call: the installed fonts, the folder new projects go in, the
//! folder picker, the projects themselves, where support for Archforge goes, and the agent programs
//! with their keys.

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};

use archforge_ui_egui::{Folder, Picked, Services};
use craft_dialogs::{Answer, Dialogs, FileDialog};

/// The host's services: the system's, with `agents` the host of AI Agents and Chat and `asks` the
/// queue the window's [`Pickers`] show the folder pickers from.
pub fn native(projects: std::sync::Arc<crate::projects::Projects>, agents: std::sync::Arc<crate::agents::Agents>, asks: Asks) -> Services {
    let chats = std::sync::Arc::new(crate::chats::Chats::new(projects.clone(), agents.clone()));
    projects.check_descriptions_with(agents.clone());
    Services {
        // The installed fonts, scanned and read on the shell's font worker.
        fonts: Some(std::sync::Arc::new(crate::fonts::SystemFonts::installed())),
        projects_folder: projects_folder(),
        projects: Some(projects),
        pick_project: Some(std::sync::Arc::new({
            let asks = asks.clone();
            move |start: Option<&std::path::Path>| pick_project(&asks, start)
        })),
        pick_folder: Some(std::sync::Arc::new(move |title: &str| pick_folder(&asks, title))),
        picker_unavailable: Some(std::sync::Arc::new(picker_unavailable)),
        find_projects: Some(std::sync::Arc::new(find_projects)),
        ids: Some(std::sync::Arc::new(fresh_id)),
        support: support(),
        agents: Some(agents),
        chats: Some(chats),
        clock: Some(std::sync::Arc::new(clock)),
    }
}

/// The time now and the local offset from UTC, for the times Chat shows.
fn clock() -> archforge_ui_egui::chat_words::Clock {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    archforge_ui_egui::chat_words::Clock { now, offset: local_offset() }
}

/// How often the local offset is looked at again: it changes with daylight saving time.
const OFFSET_EVERY: std::time::Duration = std::time::Duration::from_secs(15 * 60);

/// The local offset from UTC in seconds, as the system says it (`date +%z`), looked at on a thread
/// of its own the first time and every quarter of an hour after, so the window never waits for it;
/// none until it has answered, and where it cannot be told (times then say UTC).
fn local_offset() -> Option<i32> {
    use std::sync::Mutex;
    static KNOWN: Mutex<(Option<std::time::Instant>, Option<i32>)> = Mutex::new((None, None));
    let mut known = KNOWN.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let due = known.0.is_none_or(|at| at.elapsed() >= OFFSET_EVERY);
    if due {
        known.0 = Some(std::time::Instant::now());
        let looked = std::thread::Builder::new().name("archforge-offset".into()).spawn(|| {
            let offset = system_offset();
            KNOWN.lock().unwrap_or_else(std::sync::PoisonError::into_inner).1 = offset;
        });
        if let Err(e) = looked {
            log::warn!("the local time zone could not be looked at: {e}");
        }
    }
    known.1
}

/// The system's local offset from UTC now, from `date +%z` (`+0100`): Unix only.
fn system_offset() -> Option<i32> {
    #[cfg(unix)]
    {
        // The system's own program by its full path, never one found on the PATH.
        let out = match std::process::Command::new("/bin/date").arg("+%z").env("LC_ALL", "C").output() {
            Ok(out) if out.status.success() => out,
            Ok(out) => {
                log::warn!("the local time zone could not be read: /bin/date ended with {}", out.status);
                return None;
            }
            Err(e) => {
                log::warn!("the local time zone could not be read: /bin/date did not run: {e}");
                return None;
            }
        };
        let offset = parse_offset(String::from_utf8_lossy(&out.stdout).trim());
        if offset.is_none() {
            log::warn!("the local time zone could not be read: /bin/date gave no offset");
        }
        offset
    }
    #[cfg(not(unix))]
    {
        None
    }
}

/// An offset as `date +%z` writes it, `+0100` or `-0530`, in seconds; none for anything else.
#[cfg_attr(not(unix), allow(dead_code))]
fn parse_offset(text: &str) -> Option<i32> {
    let (sign, digits) = match text.as_bytes().first()? {
        b'+' => (1, text.get(1..)?),
        b'-' => (-1, text.get(1..)?),
        _ => return None,
    };
    if digits.len() != 4 || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let hours: i32 = digits.get(..2)?.parse().ok()?;
    let minutes: i32 = digits.get(2..)?.parse().ok()?;
    (hours <= 14 && minutes < 60).then_some(sign * (hours * 3_600 + minutes * 60))
}

/// The owner's approved support pages: GitHub Sponsors goes to the company's
/// organization, Ko-fi to the owner. Each is set once its page can take support; until then About
/// Archforge says it is not set up yet. The organization's Sponsors profile is not public yet.
const GITHUB_SPONSORS: Option<&str> = None;
const KO_FI: Option<&str> = Some("https://ko-fi.com/dejanwho");

fn support() -> archforge_ui_egui::support::Destinations {
    archforge_ui_egui::support::Destinations { github_sponsors: GITHUB_SPONSORS.map(str::to_string), ko_fi: KO_FI.map(str::to_string) }
}

/// The folder picks the shell asked for in this frame, each with the channel its answer goes back
/// on, until the window's [`Pickers`] show them at the frame's end. The shell asks from inside the
/// frame, where a picker that blocks lets AppKit's modal loop re-enter winit's handler (a file
/// dragged over the window) and abort the app; so nothing is shown here.
#[derive(Clone, Default)]
pub struct Asks(std::sync::Arc<std::sync::Mutex<Vec<(FileDialog, Sender<Picked>)>>>);

impl Asks {
    /// Queues `dialog`; its answer comes on the channel returned, on a later frame.
    fn ask(&self, dialog: FileDialog) -> Receiver<Picked> {
        let (tx, rx) = std::sync::mpsc::channel();
        self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).push((dialog, tx));
        rx
    }
}

/// The window's folder pickers, through craft-dialogs: the picks the shell asked for are shown at
/// the end of the frame, one at a time, and answered on a later frame without the window waiting.
/// On macOS the picker is a panel of its own beside the window.
pub struct Pickers {
    asks: Asks,
    /// The pick asked for while a picker is on screen, shown once it closes; a newer ask takes
    /// its place.
    waiting: Option<(FileDialog, Sender<Picked>)>,
    dialogs: Dialogs<Sender<Picked>>,
}

impl Pickers {
    /// Pickers whose answers call `wake`, which runs a frame of the window to take them.
    pub fn new(wake: craft_dialogs::Wake) -> Self {
        Self { asks: Asks::default(), waiting: None, dialogs: Dialogs::new(wake) }
    }

    /// The queue the shell's pickers ask through (`native`'s `asks`).
    pub fn asks(&self) -> Asks {
        self.asks.clone()
    }

    /// Hands the answers that have come to the picks that asked for them. Called before the shell's
    /// logic, which takes them in the same frame; a pick the shell has let go of (its dialog
    /// closed, the window quitting) has no use for its answer.
    pub fn answer(&mut self) {
        for (tx, answer) in self.dialogs.take_answers() {
            let _ = tx.send(picked(answer));
        }
    }

    /// Shows the pick asked for, at the end of the window's frame, with the window as `parent`.
    /// The shell asks for one pick at a time, after the last one was answered or let go of (its
    /// New project closed, say), so a newer ask means one still waiting was let go of: only the
    /// newest is kept, and it waits, with the open picker brought to the front, until that picker
    /// closes. A picker let go of while on screen stays until the user closes it: craft-dialogs
    /// cannot close one.
    pub fn show(&mut self, parent: Option<&eframe::Frame>) {
        let asked = std::mem::take(&mut *self.asks.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner));
        if let Some(newest) = asked.into_iter().last() {
            self.waiting = Some(newest);
            self.dialogs.raise();
        }
        if !self.dialogs.is_open()
            && let Some((dialog, tx)) = self.waiting.take()
        {
            self.dialogs.ask(dialog, tx);
        }
        self.dialogs.show_pending(parent);
    }
}

/// What a folder picker's answer is to the shell. A cancel is nothing chosen, which is also what a
/// Linux system without a desktop portal and without zenity answers, so the shell then asks
/// [`picker_unavailable`]. A picker that could not be shown says so in the user's language; its own
/// English reason goes to the log.
fn picked(answer: Answer) -> Picked {
    let unavailable = |why: &str| {
        log::warn!("the folder picker could not be shown: {why}");
        Picked::Unavailable(archforge_ui_egui::i18n::tr("No folder picker could open; Archforge's log says why."))
    };
    match answer {
        Answer::Paths(paths) => paths.into_iter().next().map_or(Picked::Nothing, Picked::Folder),
        Answer::Cancelled => Picked::Nothing,
        Answer::Unavailable(why) => unavailable(&why),
        Answer::Message(result) => unavailable(&format!("a folder pick answered as a message box ({result:?})")),
    }
}

/// The platform's folder picker, for Open project, asked through `asks`: it starts in the folder
/// given (a pinned folder), else in the projects folder.
fn pick_project(asks: &Asks, start: Option<&std::path::Path>) -> Receiver<Picked> {
    let mut dialog = FileDialog::folder();
    match start.filter(|s| s.is_dir()) {
        Some(start) => dialog = dialog.directory(start),
        None => {
            if let Some(folder) = projects_folder().filter(|f| f.path.is_dir()) {
                dialog = dialog.directory(folder.path);
            }
        }
    }
    asks.ask(dialog)
}

/// Why the folder picker cannot appear. Where the picker is the desktop portal's (Linux
/// and the BSDs), asked over D-Bus, a missing portal answers nothing, as a cancel does; the picker
/// library then falls back to zenity, so only with no zenity either is the picker surely missing.
/// craft-dialogs answers that case as a cancel, not as unavailable, so this is asked after every
/// cancel whatever shows the picker. Elsewhere the picker is the system's own and always appears.
fn picker_unavailable() -> Option<String> {
    #[cfg(any(target_os = "linux", target_os = "freebsd", target_os = "dragonfly", target_os = "netbsd", target_os = "openbsd"))]
    {
        use archforge_ui_egui::i18n::tr;
        // A portal once seen is there for the rest of the run: a cancel then asks nothing.
        static SEEN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        if SEEN.load(std::sync::atomic::Ordering::Relaxed) || on_path("zenity") {
            return None;
        }
        let why = || tr("No folder picker could open: install xdg-desktop-portal and a backend for your desktop.");
        let session_bus = std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_some()
            || std::env::var_os("XDG_RUNTIME_DIR").is_some_and(|dir| !dir.is_empty() && std::path::Path::new(&dir).join("bus").exists());
        if !session_bus {
            return Some(why());
        }
        match file_chooser_probe() {
            Probe::Present => {
                SEEN.store(true, std::sync::atomic::Ordering::Relaxed);
                None
            }
            Probe::Missing => Some(why()),
            Probe::Unknown => None,
        }
    }
    #[cfg(not(any(target_os = "linux", target_os = "freebsd", target_os = "dragonfly", target_os = "netbsd", target_os = "openbsd")))]
    {
        None
    }
}

/// What the portal's file chooser answered.
#[derive(Debug, PartialEq, Eq)]
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
enum Probe {
    Present,
    Missing,
    /// It could not be asked, or did not answer in time: nothing is claimed.
    Unknown,
}

/// The longest the probe may take: it runs on the window's thread after an empty pick.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
const PROBE_DEADLINE: std::time::Duration = std::time::Duration::from_millis(1500);

/// Whether `program` is an executable file on the PATH.
#[cfg(any(target_os = "linux", target_os = "freebsd", target_os = "dragonfly", target_os = "netbsd", target_os = "openbsd"))]
fn on_path(program: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|paths| std::env::split_paths(&paths).any(|dir| !dir.as_os_str().is_empty() && dir.join(program).is_file()))
}

/// Asks the desktop portal for its file chooser's version with `dbus-send`, killed past
/// [`PROBE_DEADLINE`]: an answer means a portal with a file chooser, a bus error naming the
/// service, interface or property unknown means none.
#[cfg(any(target_os = "linux", target_os = "freebsd", target_os = "dragonfly", target_os = "netbsd", target_os = "openbsd"))]
fn file_chooser_probe() -> Probe {
    use std::io::Read;
    let child = std::process::Command::new("dbus-send")
        .args([
            "--session",
            "--print-reply",
            "--reply-timeout=1000",
            "--dest=org.freedesktop.portal.Desktop",
            "/org/freedesktop/portal/desktop",
            "org.freedesktop.DBus.Properties.Get",
            "string:org.freedesktop.portal.FileChooser",
            "string:version",
        ])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn();
    let mut child = match child {
        Ok(child) => child,
        Err(e) => {
            log::warn!("dbus-send could not be run to look for the desktop portal: {e}");
            return Probe::Unknown;
        }
    };
    let started = std::time::Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() < PROBE_DEADLINE => std::thread::sleep(std::time::Duration::from_millis(20)),
            Ok(None) => {
                log::warn!("dbus-send did not answer within {} ms; the desktop portal is not judged", PROBE_DEADLINE.as_millis());
                let _ = child.kill();
                let _ = child.wait();
                return Probe::Unknown;
            }
            Err(e) => {
                log::warn!("dbus-send could not be waited for: {e}");
                let _ = child.kill();
                let _ = child.wait();
                return Probe::Unknown;
            }
        }
    };
    let mut stderr = String::new();
    if let Some(pipe) = child.stderr.take() {
        let _ = pipe.take(64 * 1024).read_to_string(&mut stderr);
    }
    if !status.success() {
        log::info!("dbus-send asking for the desktop portal ended with {status}: {}", stderr.trim());
    }
    classify(status.success(), &stderr)
}

/// What a `dbus-send` run says of the portal's file chooser: success, a bus error meaning there is
/// none, or anything else, which claims nothing.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn classify(success: bool, stderr: &str) -> Probe {
    const MISSING: [&str; 5] = ["ServiceUnknown", "UnknownInterface", "UnknownMethod", "UnknownProperty", "InvalidArgs"];
    if success {
        Probe::Present
    } else if MISSING.iter().any(|e| stderr.contains(&format!("org.freedesktop.DBus.Error.{e}"))) {
        Probe::Missing
    } else {
        Probe::Unknown
    }
}

/// How deep below a pinned folder Home's search looks for projects, the most it lists, and the
/// most folder entries one search reads in all its pinned folders together.
const FIND_DEPTH: usize = 2;
const FIND_LIMIT: usize = 200;
const FIND_BUDGET: usize = 20_000;

/// The projects in Home's pinned folders, looked for on a thread of their own so a slow disk
/// never holds the window: each folder is made canonical first, so a project found matches the
/// same project in the recent list. The answer is sent once, then the window is woken. A search
/// that saw only part of the folders says so in the log; a thread that cannot start sends
/// nothing, and the search keeps to the recent projects.
fn find_projects(folders: Vec<PathBuf>, wake: Box<dyn Fn() + Send>) -> std::sync::mpsc::Receiver<Vec<archforge_ui_egui::FoundProject>> {
    let (tx, rx) = std::sync::mpsc::channel();
    let started = std::thread::Builder::new().name("find-projects".into()).spawn(move || {
        let mut found: Vec<archforge_ui_egui::FoundProject> = Vec::new();
        let (mut budget, mut cut_short, mut unreadable) = (FIND_BUDGET, false, 0);
        for (i, folder) in folders.iter().enumerate() {
            let room = FIND_LIMIT.saturating_sub(found.len());
            if room == 0 || budget == 0 {
                cut_short = true;
                break;
            }
            let root = folder.canonicalize().unwrap_or_else(|_| folder.clone());
            let walk = archforge_project::find(&root, FIND_DEPTH, room, &mut budget);
            cut_short |= walk.cut_short;
            unreadable += walk.unreadable;
            for path in walk.projects {
                if found.iter().any(|f| f.path == path) {
                    continue;
                }
                let within = path
                    .parent()
                    .and_then(|p| p.strip_prefix(&root).ok())
                    .map(|p| p.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect())
                    .unwrap_or_default();
                found.push(archforge_ui_egui::FoundProject { path, folder: i, within });
            }
        }
        if cut_short || unreadable > 0 {
            log::warn!(
                "Home's search saw the pinned folders only in part: {} projects listed, {}, {unreadable} folders unreadable",
                found.len(),
                if cut_short { "a limit reached" } else { "no limit reached" }
            );
        }
        let _ = tx.send(found);
        wake();
    });
    if let Err(e) = started {
        log::warn!("the pinned folders could not be searched for projects: {e}");
    }
    rx
}

/// The platform's folder picker, asked through `asks` as [`pick_project`] is, headed with the
/// title the shell gives for what the folder is for (Pin a folder, Choose project location; on
/// macOS the line above the files): it starts in the user's Documents.
fn pick_folder(asks: &Asks, title: &str) -> Receiver<Picked> {
    let mut dialog = FileDialog::folder().title(title);
    if let Some(documents) = projects_folder().and_then(|f| f.path.parent().map(PathBuf::from)) {
        dialog = dialog.directory(documents);
    }
    asks.ask(dialog)
}

/// A version 7 id from the clock, a process-wide counter and the platform's randomness: the
/// counter in the tail makes every id of this process distinct whatever the clock and the
/// randomness do (RFC 9562's monotonic counter), the two random bytes before it keep ids of two
/// processes apart, and the time keeps them sorted. A failing randomness source is said once.
pub(crate) fn fresh_id() -> archforge_mof::Uuid {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    static WARNED: std::sync::Once = std::sync::Once::new();
    let millis = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
    let mut random = [0u8; 10];
    if let Err(e) = getrandom::fill(&mut random[..2]) {
        WARNED.call_once(|| log::warn!("no randomness for ids: {e}; the counter alone keeps them distinct"));
    }
    random[2..].copy_from_slice(&SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed).to_be_bytes());
    archforge_mof::Uuid::v7(u64::try_from(millis).unwrap_or(0), random)
}

/// `Documents/Archforge` in the user's home folder, where the platform has a Documents folder;
/// it is created only when the first project goes there.
pub fn projects_folder() -> Option<Folder> {
    let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from).filter(|h| h.is_absolute())?;
    let documents = home.join("Documents");
    documents.is_dir().then(|| Folder { path: documents.join("Archforge"), label: "Documents / Archforge".to_string() })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_file_chooser_is_missing_only_when_the_bus_says_so() {
        assert_eq!(classify(true, ""), Probe::Present);
        let unknown =
            "Error org.freedesktop.DBus.Error.ServiceUnknown: The name org.freedesktop.portal.Desktop was not provided by any .service files";
        assert_eq!(classify(false, unknown), Probe::Missing, "no portal");
        let no_backend = "Error org.freedesktop.DBus.Error.UnknownInterface: No such interface org.freedesktop.portal.FileChooser";
        assert_eq!(classify(false, no_backend), Probe::Missing, "a portal without a file chooser backend");
        assert_eq!(classify(false, "Error org.freedesktop.DBus.Error.NoReply: Did not receive a reply"), Probe::Unknown, "no answer claims nothing");
        assert_eq!(classify(false, "Failed to open connection to session bus"), Probe::Unknown);
    }

    /// A pick the shell asks for is shown only at the end of the frame and answered on a later one:
    /// a folder chosen is that folder, a cancel is nothing (the shell then asks why no picker
    /// appeared), and a picker that could not be shown says so in the user's words, never in
    /// craft-dialogs' English. A pick still waiting when a newer one is asked was let go of by the
    /// shell, and only the newer one shows.
    #[test]
    fn a_pick_shows_at_the_frames_end_and_its_answer_comes_on_a_later_frame() {
        use std::cell::RefCell;
        use std::rc::Rc;
        use std::sync::mpsc::TryRecvError;

        use craft_dialogs::Request;

        archforge_ui_egui::i18n::set_language("en");
        let woken = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let wake = woken.clone();
        let mut pickers = Pickers::new(std::sync::Arc::new(move || {
            wake.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }));
        let chosen = std::env::temp_dir();
        let shown = Rc::new(RefCell::new(Vec::<Request>::new()));
        let answers = Rc::new(RefCell::new(std::collections::VecDeque::from([
            Answer::Paths(vec![chosen.clone()]),
            Answer::Cancelled,
            Answer::Unavailable("the app isn't running yet, so the dialog can't be shown".into()),
            Answer::Paths(vec![chosen.clone()]),
            Answer::Cancelled,
        ])));
        let (seen, next) = (shown.clone(), answers.clone());
        pickers.dialogs.set_override(Some(Box::new(move |request: &Request| {
            seen.borrow_mut().push(request.clone());
            next.borrow_mut().pop_front().unwrap_or(Answer::Cancelled)
        })));
        let asks = pickers.asks();

        // Open project, from a pinned folder: nothing shows in the frame that asks.
        let open = pick_project(&asks, Some(&chosen));
        assert!(shown.borrow().is_empty(), "shown inside the frame");
        pickers.answer();
        assert_eq!(open.try_recv(), Err(TryRecvError::Empty));
        pickers.show(None);
        assert_eq!(shown.borrow().as_slice(), [Request::File(FileDialog::folder().directory(&chosen))]);
        assert_eq!(open.try_recv(), Err(TryRecvError::Empty), "answered in the frame that showed it");
        assert!(woken.load(std::sync::atomic::Ordering::SeqCst) > 0, "the answer wakes the window");
        pickers.answer();
        assert_eq!(open.try_recv(), Ok(Picked::Folder(chosen.clone())));

        // Pin a folder, cancelled: nothing chosen, under the title the shell gave.
        let pin = pick_folder(&asks, "Pin a folder");
        pickers.show(None);
        pickers.answer();
        assert_eq!(pin.try_recv(), Ok(Picked::Nothing));
        let Some(Request::File(dialog)) = shown.borrow().last().cloned() else { panic!("no file dialog shown") };
        assert_eq!((dialog.kind, dialog.title.as_deref()), (craft_dialogs::Kind::Folder, Some("Pin a folder")));

        // Choose project location, with no picker to show.
        let location = pick_folder(&asks, "Choose project location");
        pickers.show(None);
        pickers.answer();
        assert_eq!(location.try_recv(), Ok(Picked::Unavailable("No folder picker could open; Archforge's log says why.".into())));

        // A pick asked after one still waiting means the shell let that one go: only the newest
        // shows, and it answers.
        let gone = pick_project(&asks, Some(&chosen));
        drop(gone);
        let kept = pick_folder(&asks, "Pin a folder");
        pickers.show(None);
        pickers.answer();
        assert_eq!(kept.try_recv(), Ok(Picked::Folder(chosen.clone())));
        assert_eq!(shown.borrow().len(), 4, "the pick let go of was never shown");
        let Some(Request::File(dialog)) = shown.borrow().last().cloned() else { panic!("no file dialog shown") };
        assert_eq!(dialog.title.as_deref(), Some("Pin a folder"), "the newest pick shows");
        assert_eq!(answers.borrow().len(), 1, "one answer left for a pick never shown");
    }
}

#[cfg(test)]
mod support_tests {
    use archforge_ui_egui::support::approved;

    #[test]
    fn every_support_page_set_is_one_the_dialog_opens() {
        let d = super::support();
        assert_eq!(d.ko_fi.as_deref(), Some("https://ko-fi.com/dejanwho"));
        assert_eq!(approved(d.ko_fi.as_deref(), "https://ko-fi.com/"), d.ko_fi.as_deref());
        if let Some(github) = d.github_sponsors.as_deref() {
            assert_eq!(approved(Some(github), "https://github.com/sponsors/"), Some(github));
        }
    }
}
