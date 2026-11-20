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
        picker_unavailable: Some(std::sync::Arc::new(picker_unavailable)),
        find_projects: Some(std::sync::Arc::new(find_projects)),
        ids: Some(std::sync::Arc::new(fresh_id)),
        support: support(),
        agents: Some(agents),
        chats: Some(chats),
        clock: Some(std::sync::Arc::new(clock)),
        ..with_pickers(&asks)
    }
}

/// The shell's folder pickers, asked through `asks`, and the withdrawal of a pick it lets go of.
fn with_pickers(asks: &Asks) -> Services {
    let (project, folder, withdraw) = (asks.clone(), asks.clone(), asks.clone());
    Services {
        pick_project: Some(std::sync::Arc::new(move |start: Option<&std::path::Path>| pick_project(&project, start))),
        pick_folder: Some(std::sync::Arc::new(move |title: &str| pick_folder(&folder, title))),
        withdraw_pick: Some(std::sync::Arc::new(move || withdraw.withdraw())),
        ..Services::default()
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
pub struct Asks(std::sync::Arc<std::sync::Mutex<Queue>>);

/// One folder pick asked for: its number in the order asked, its dialog, and where its answer goes.
type Ask = (u64, FileDialog, Sender<Picked>);

/// The picks asked for and not yet taken by the window, and which of all those asked the shell
/// has withdrawn.
#[derive(Default)]
struct Queue {
    asked: Vec<Ask>,
    /// The number of the last pick asked for.
    last: u64,
    /// Every pick numbered up to this one was withdrawn: never shown unless it already was.
    withdrawn: u64,
}

impl Asks {
    /// Queues `dialog`; its answer comes on the channel returned, on a later frame.
    fn ask(&self, dialog: FileDialog) -> Receiver<Picked> {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut queue = self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        queue.last = queue.last.saturating_add(1);
        let number = queue.last;
        queue.asked.push((number, dialog, tx));
        rx
    }

    /// Withdraws the pick asked for last, and any before it, which the shell has let go of: one
    /// not yet shown never is. The shell asks for one pick at a time, after the last one was
    /// answered or let go of, so the last asked is the one it lets go of.
    pub fn withdraw(&self) {
        let mut queue = self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        queue.withdrawn = queue.last;
        queue.asked.clear();
    }
}

/// The window's folder pickers, through craft-dialogs: the picks the shell asked for are shown at
/// the end of the frame, one at a time, and answered on a later frame without the window waiting.
/// On macOS the picker is a panel of its own beside the window.
pub struct Pickers {
    asks: Asks,
    /// The pick asked for while a picker is on screen, shown once it closes or is given up, unless
    /// withdrawn meanwhile; a newer ask takes its place.
    waiting: Option<Ask>,
    dialogs: Dialogs<Sender<Picked>>,
    /// Makes the queue that takes the place of one whose picker never answered.
    fresh: Box<dyn Fn() -> Dialogs<Sender<Picked>>>,
    /// Whether a picker stays in front of the window until it closes, so that the user working in
    /// the window means none is up: everywhere but macOS, whose picker is a panel beside the window.
    in_front: bool,
    /// Tests: a picker counts as on screen, as a real one does until the user closes it.
    #[cfg(test)]
    held: bool,
    /// Tests: `Some` makes the pickers hang, as a hung desktop portal's do: the pick handed to one
    /// is kept here, never answered, until the picker is given up with its queue.
    #[cfg(test)]
    hung: Option<Option<Sender<Picked>>>,
}

impl Pickers {
    /// Pickers whose answers call `wake`, which runs a frame of the window to take them.
    pub fn new(wake: craft_dialogs::Wake) -> Self {
        let fresh = wake.clone();
        Self {
            asks: Asks::default(),
            waiting: None,
            dialogs: Dialogs::new(wake),
            fresh: Box::new(move || Dialogs::new(fresh.clone())),
            in_front: !cfg!(target_os = "macos"),
            #[cfg(test)]
            held: false,
            #[cfg(test)]
            hung: None,
        }
    }

    /// The queue the shell's pickers ask through (`native`'s `asks`).
    pub fn asks(&self) -> Asks {
        self.asks.clone()
    }

    /// Hands the answers that have come to the picks that asked for them, then gives up on a
    /// picker that never answered, as [`Self::give_up_if_left`] says. Called before the shell's
    /// logic, which takes them in the same frame; a pick the shell has let go of (its dialog
    /// closed, the window quitting) has no use for its answer.
    pub fn answer(&mut self, ctx: &egui::Context) {
        for (tx, answer) in self.dialogs.take_answers() {
            let _ = tx.send(picked(answer));
        }
        self.give_up_if_left(ctx);
    }

    /// Gives up on the picker on screen when the user works in the window again (a pointer or key
    /// pressed while it shows and has the focus) where a picker stays in front of the window until
    /// it closes: none is up then, so it never appeared or went without answering, as a hung
    /// desktop portal does. craft-dialogs cannot say whether its picker is up, nor give one up, so
    /// its queue is left behind with the picker's worker and a fresh one shows the picks from then
    /// on. The pick it held ends as a cancel: the shell, its channel closed, frees Open project, Pin
    /// a folder and Change… in this frame, and an answer the old picker gives later reaches nothing.
    /// No time limit ends a pick: only a press in the window does, which a picker in front of it
    /// keeps from the window; a desktop that lets the window take presses while its picker shows
    /// gives that picker up too, and what is chosen in it is ignored. A hidden window's logic reads
    /// the input of the last frame it showed, so it gives nothing up.
    fn give_up_if_left(&mut self, ctx: &egui::Context) {
        let pressed = |i: &egui::InputState| {
            i.pointer.any_pressed() || i.events.iter().any(|e| matches!(e, egui::Event::Key { pressed: true, repeat: false, .. }))
        };
        let in_use = |i: &egui::InputState| i.viewport().visible() != Some(false) && i.focused && pressed(i);
        if !self.in_front || !self.on_screen() || !ctx.input(in_use) {
            return;
        }
        log::warn!("the folder picker did not answer and the window is in use again: the pick is given up as a cancel");
        self.dialogs = (self.fresh)();
        #[cfg(test)]
        {
            self.held = false;
            self.hung = None;
        }
    }

    /// Shows the pick asked for, at the end of the window's frame, with the window as `parent`.
    /// The shell asks for one pick at a time, after the last one was answered or let go of (its
    /// New project closed, say), so a newer ask means one still waiting was let go of: only the
    /// newest is kept, and it waits, with the open picker brought to the front, until that picker
    /// closes, or is given up ([`Self::answer`]). A waiting pick the shell withdraws is dropped and
    /// never shown. A picker already on screen stays until the user closes it, withdrawn or not:
    /// craft-dialogs cannot close one.
    pub fn show(&mut self, parent: Option<&eframe::Frame>) {
        let (asked, withdrawn) = {
            let mut queue = self.asks.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            (std::mem::take(&mut queue.asked), queue.withdrawn)
        };
        if self.waiting.as_ref().is_some_and(|(number, ..)| *number <= withdrawn) {
            self.waiting = None;
        }
        if let Some(newest) = asked.into_iter().last() {
            self.waiting = Some(newest);
            self.dialogs.raise();
        }
        if !self.on_screen()
            && let Some((_, dialog, tx)) = self.waiting.take()
        {
            #[cfg(test)]
            if let Some(hung) = &mut self.hung {
                *hung = Some(tx);
                return;
            }
            self.dialogs.ask(dialog, tx);
        }
        self.dialogs.show_pending(parent);
    }

    /// Shows the picks asked for, as [`Self::show`] does, when the window is hidden (minimized or
    /// covered). eframe then runs only the window's logic, with no drawing, so a pick asked from the
    /// menu bar meanwhile would otherwise wait until the window shows again. A window that shows, or
    /// whose state is unknown, is drawn, and its drawing shows them; eframe decides the same way.
    pub fn show_if_hidden(&mut self, ctx: &egui::Context, parent: Option<&eframe::Frame>) {
        if ctx.input(|i| i.viewport().visible()) == Some(false) {
            self.show(parent);
        }
    }

    /// Whether a picker is on screen, or its answer not yet collected.
    fn on_screen(&self) -> bool {
        #[cfg(test)]
        if self.held || self.hung.as_ref().is_some_and(Option::is_some) {
            return true;
        }
        self.dialogs.is_open()
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
        pickers.answer(&egui::Context::default());
        assert_eq!(open.try_recv(), Err(TryRecvError::Empty));
        pickers.show(None);
        assert_eq!(shown.borrow().as_slice(), [Request::File(FileDialog::folder().directory(&chosen))]);
        assert_eq!(open.try_recv(), Err(TryRecvError::Empty), "answered in the frame that showed it");
        assert!(woken.load(std::sync::atomic::Ordering::SeqCst) > 0, "the answer wakes the window");
        pickers.answer(&egui::Context::default());
        assert_eq!(open.try_recv(), Ok(Picked::Folder(chosen.clone())));

        // Pin a folder, cancelled: nothing chosen, under the title the shell gave.
        let pin = pick_folder(&asks, "Pin a folder");
        pickers.show(None);
        pickers.answer(&egui::Context::default());
        assert_eq!(pin.try_recv(), Ok(Picked::Nothing));
        let Some(Request::File(dialog)) = shown.borrow().last().cloned() else { panic!("no file dialog shown") };
        assert_eq!((dialog.kind, dialog.title.as_deref()), (craft_dialogs::Kind::Folder, Some("Pin a folder")));

        // Choose project location, with no picker to show.
        let location = pick_folder(&asks, "Choose project location");
        pickers.show(None);
        pickers.answer(&egui::Context::default());
        assert_eq!(location.try_recv(), Ok(Picked::Unavailable("No folder picker could open; Archforge's log says why.".into())));

        // A pick asked after one still waiting means the shell let that one go: only the newest
        // shows, and it answers.
        let gone = pick_project(&asks, Some(&chosen));
        drop(gone);
        let kept = pick_folder(&asks, "Pin a folder");
        pickers.show(None);
        pickers.answer(&egui::Context::default());
        assert_eq!(kept.try_recv(), Ok(Picked::Folder(chosen.clone())));
        assert_eq!(shown.borrow().len(), 4, "the pick let go of was never shown");
        let Some(Request::File(dialog)) = shown.borrow().last().cloned() else { panic!("no file dialog shown") };
        assert_eq!(dialog.title.as_deref(), Some("Pin a folder"), "the newest pick shows");
        assert_eq!(answers.borrow().len(), 1, "one answer left for a pick never shown");
    }

    /// A pick asked while a picker is on screen waits for it; withdrawn by the shell before that
    /// picker closes (its New project closed, a quit going ahead), it is never shown, and neither is
    /// one withdrawn in the frame it was asked. A withdrawal once a pick is shown changes nothing:
    /// its answer still comes. Picks asked after a withdrawal show.
    #[test]
    fn a_waiting_pick_the_shell_withdraws_is_never_shown_and_a_shown_one_still_answers() {
        use std::cell::RefCell;
        use std::rc::Rc;
        use std::sync::mpsc::TryRecvError;

        use craft_dialogs::Request;

        archforge_ui_egui::i18n::set_language("en");
        let chosen = std::env::temp_dir();
        let mut pickers = Pickers::new(std::sync::Arc::new(|| {}));
        let shown = Rc::new(RefCell::new(Vec::<Option<String>>::new()));
        let (seen, folder) = (shown.clone(), chosen.clone());
        pickers.dialogs.set_override(Some(Box::new(move |request: &Request| {
            let Request::File(dialog) = request else { return Answer::Unavailable("not a file dialog".into()) };
            seen.borrow_mut().push(dialog.title.clone());
            Answer::Paths(vec![folder.clone()])
        })));
        let services = with_pickers(&pickers.asks());
        let (pick_folder, withdraw) = (services.pick_folder.clone().unwrap(), services.withdraw_pick.clone().unwrap());

        // A picker on screen for a pick the shell already let go of; it asks Pin a folder meanwhile.
        pickers.held = true;
        let pin = pick_folder("Pin a folder");
        pickers.show(None);
        pickers.answer(&egui::Context::default());
        assert!(shown.borrow().is_empty(), "shown over the picker on screen");
        assert_eq!(pin.try_recv(), Err(TryRecvError::Empty));
        // The shell lets it go too, then the picker on screen closes: nothing shows.
        withdraw();
        drop(pin);
        pickers.held = false;
        pickers.show(None);
        pickers.answer(&egui::Context::default());
        assert!(shown.borrow().is_empty(), "a withdrawn pick showed");

        // The next pick shows; withdrawn once shown, its answer still comes.
        let location = pick_folder("Choose project location");
        pickers.show(None);
        assert_eq!(shown.borrow().as_slice(), [Some("Choose project location".to_string())]);
        withdraw();
        pickers.answer(&egui::Context::default());
        assert_eq!(location.try_recv(), Ok(Picked::Folder(chosen.clone())));

        // Withdrawn in the frame it was asked, before the window took it: never shown.
        let gone = pick_folder("Pin a folder");
        withdraw();
        drop(gone);
        pickers.show(None);
        pickers.answer(&egui::Context::default());
        assert_eq!(shown.borrow().len(), 1, "a pick withdrawn in its own frame showed");

        // A withdrawal holds only for the picks asked before it.
        let open = (services.pick_project.clone().unwrap())(Some(&chosen));
        pickers.show(None);
        pickers.answer(&egui::Context::default());
        assert_eq!(open.try_recv(), Ok(Picked::Folder(chosen.clone())));
        assert_eq!(shown.borrow().len(), 2);
    }

    /// The handoff: New project's Change… picker stays on screen after the shell let go of it (New
    /// project closed), and Home's Open project asks meanwhile. The Open pick waits, frame after
    /// frame, without being withdrawn or lost; once the old picker closes, it shows without another
    /// ask, and its answer reaches the Open request.
    #[test]
    fn a_pick_asked_while_an_old_picker_is_on_screen_shows_once_it_closes_and_answers() {
        use std::cell::RefCell;
        use std::rc::Rc;
        use std::sync::mpsc::TryRecvError;

        use craft_dialogs::Request;

        archforge_ui_egui::i18n::set_language("en");
        let chosen = std::env::temp_dir();
        let mut pickers = Pickers::new(std::sync::Arc::new(|| {}));
        let shown = Rc::new(RefCell::new(Vec::<FileDialog>::new()));
        let (seen, folder) = (shown.clone(), chosen.clone());
        pickers.dialogs.set_override(Some(Box::new(move |request: &Request| {
            let Request::File(dialog) = request else { return Answer::Unavailable("not a file dialog".into()) };
            seen.borrow_mut().push(dialog.clone());
            Answer::Paths(vec![folder.clone()])
        })));
        let services = with_pickers(&pickers.asks());
        let (pick_project, withdraw) = (services.pick_project.clone().unwrap(), services.withdraw_pick.clone().unwrap());

        // Change…'s picker on screen; New project closes, which lets its pick go.
        pickers.held = true;
        withdraw();
        // Home's Open project asks, and the shell keeps waiting for it over several frames.
        let open = pick_project(Some(&chosen));
        for _ in 0..3 {
            pickers.answer(&egui::Context::default());
            pickers.show(None);
            assert!(shown.borrow().is_empty(), "shown over the picker on screen");
            assert_eq!(open.try_recv(), Err(TryRecvError::Empty));
        }

        // The old picker closes: the Open pick shows in the next frame, asked once, and answers.
        pickers.held = false;
        pickers.answer(&egui::Context::default());
        pickers.show(None);
        assert_eq!(shown.borrow().as_slice(), [FileDialog::folder().directory(&chosen)], "the Open pick did not show once");
        pickers.answer(&egui::Context::default());
        assert_eq!(open.try_recv(), Ok(Picked::Folder(chosen.clone())));
        pickers.show(None);
        assert_eq!(shown.borrow().len(), 1, "the Open pick showed again");
    }

    /// A hidden window (minimized or covered) runs only its logic, so a pick asked then, from the
    /// menu bar, shows from the logic and answers. A window that shows, or whose state is unknown,
    /// is drawn, and the drawing shows the pick, never the logic.
    #[test]
    fn a_pick_asked_while_the_window_is_hidden_shows_from_its_logic() {
        use std::cell::Cell;
        use std::rc::Rc;

        archforge_ui_egui::i18n::set_language("en");
        let chosen = std::env::temp_dir();
        let mut pickers = Pickers::new(std::sync::Arc::new(|| {}));
        let shown = Rc::new(Cell::new(0_usize));
        let (seen, folder) = (shown.clone(), chosen.clone());
        pickers.dialogs.set_override(Some(Box::new(move |_: &craft_dialogs::Request| {
            seen.set(seen.get() + 1);
            Answer::Paths(vec![folder.clone()])
        })));
        let asks = pickers.asks();
        let ctx = egui::Context::default();
        let window = |minimized: Option<bool>, occluded: Option<bool>| {
            let mut input = egui::RawInput::default();
            let root = input.viewports.entry(egui::ViewportId::ROOT).or_default();
            (root.minimized, root.occluded) = (minimized, occluded);
            input
        };
        let cases = [
            ("minimized", window(Some(true), Some(false)), true),
            ("covered", window(Some(false), Some(true)), true),
            ("showing", window(Some(false), Some(false)), false),
            ("unknown", window(None, None), false),
        ];
        for (state, input, hidden) in cases {
            let open = pick_project(&asks, Some(&chosen));
            let before = shown.get();
            let _ = ctx.run_logic(&input, |ctx| pickers.show_if_hidden(ctx, None));
            assert_eq!(shown.get() - before, usize::from(hidden), "a {state} window's logic");
            if !hidden {
                pickers.show(None);
                assert_eq!(shown.get() - before, 1, "a {state} window's drawing");
            }
            pickers.answer(&egui::Context::default());
            assert_eq!(open.try_recv(), Ok(Picked::Folder(chosen.clone())), "a {state} window's pick");
        }
    }

    /// One frame's input: the window focused or not, and the events in it.
    fn input(focused: bool, events: Vec<egui::Event>) -> egui::RawInput {
        egui::RawInput { focused, events, ..Default::default() }
    }

    /// A pointer button pressed in the window.
    fn press() -> egui::Event {
        egui::Event::PointerButton {
            pos: egui::pos2(40.0, 40.0),
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        }
    }

    /// A key going down in the window, or let go; egui marks one already down as a repeat.
    fn key(pressed: bool) -> egui::Event {
        egui::Event::Key { key: egui::Key::A, physical_key: None, pressed, repeat: false, modifiers: egui::Modifiers::NONE }
    }

    /// Pickers whose first picker never answers, as a hung desktop portal's does, and whose fresh
    /// queues answer every pick with `chosen`; with the count of pickers shown and of queues made.
    fn hanging_pickers(chosen: &std::path::Path) -> (Pickers, std::rc::Rc<std::cell::Cell<usize>>, std::rc::Rc<std::cell::Cell<usize>>) {
        let mut pickers = Pickers::new(std::sync::Arc::new(|| {}));
        let (shown, made) = (std::rc::Rc::new(std::cell::Cell::new(0_usize)), std::rc::Rc::new(std::cell::Cell::new(0_usize)));
        let (seen, folder, making) = (shown.clone(), chosen.to_path_buf(), made.clone());
        pickers.fresh = Box::new(move || {
            making.set(making.get() + 1);
            let (seen, folder) = (seen.clone(), folder.clone());
            let mut dialogs = Dialogs::new(std::sync::Arc::new(|| {}));
            dialogs.set_override(Some(Box::new(move |_: &craft_dialogs::Request| {
                seen.set(seen.get() + 1);
                Answer::Paths(vec![folder.clone()])
            })));
            dialogs
        });
        pickers.dialogs = (pickers.fresh)();
        pickers.hung = Some(None);
        (pickers, shown, made)
    }

    /// A picker that never answers, where pickers stay in front of the window: the window waits,
    /// with Open project and Pin a folder off, through idle frames and through presses while it
    /// has not the focus. A press in the focused window gives the picker up: the pick ends as a
    /// cancel, saying nothing, Open and Pin are free in that frame, the answer the old picker
    /// would give later reaches nothing, and the next pick shows a picker of its own and answers.
    #[test]
    fn a_picker_that_never_answers_is_given_up_once_the_window_is_used_again() {
        archforge_ui_egui::i18n::set_language("en");
        let sandbox = crate::test_host::Sandbox::named("pick", None);
        let chosen = std::env::temp_dir();
        let (mut pickers, shown, made) = hanging_pickers(&chosen);
        pickers.in_front = true;
        let mut shell = archforge_ui_egui::Shell::new(Services { projects: Some(sandbox.host.clone()), ..with_pickers(&pickers.asks()) });
        let ctx = egui::Context::default();
        let frame = |pickers: &mut Pickers, shell: &mut archforge_ui_egui::Shell, input: egui::RawInput| {
            // As eframe runs a shown window's logic: in the frame's pass, with its input.
            ctx.run_ui(input, |ui| {
                pickers.answer(ui.ctx());
                shell.logic(ui.ctx());
            })
            .textures_delta
            .clear();
            pickers.show(None);
        };
        let queued = |pickers: &Pickers| pickers.asks.0.lock().map(|q| q.asked.len()).unwrap_or_default();

        shell.pin_from_picker();
        pickers.show(None);
        assert!(pickers.hung.as_ref().is_some_and(Option::is_some), "the pick was not handed to the picker");
        for (what, events, focused) in
            [("an idle frame", vec![], true), ("a press elsewhere", vec![press()], false), ("a key elsewhere", vec![key(true)], false)]
        {
            frame(&mut pickers, &mut shell, input(focused, events));
            assert!(!shell.can_open_project(), "Open free after {what}");
            shell.pin_from_picker();
            assert_eq!(queued(&pickers), 0, "Pin asked again after {what}");
        }

        frame(&mut pickers, &mut shell, input(true, vec![press()]));
        assert_eq!((made.get(), pickers.hung.is_none()), (2, true), "the hung picker's queue was kept");
        assert!(shell.can_open_project(), "Open still off");
        assert_eq!((shell.picker_error.as_deref(), shell.state.pinned.len()), (None, 0), "a pick given up is a cancel");
        shell.pin_from_picker();
        assert_eq!(queued(&pickers), 1, "Pin still off");
        frame(&mut pickers, &mut shell, input(true, vec![]));
        frame(&mut pickers, &mut shell, input(true, vec![]));
        assert_eq!((shown.get(), shell.state.pinned.as_slice()), (1, [chosen].as_slice()), "the next pick's picker");
    }

    /// Only a press, or a key going down, in the focused window gives a picker up, and only where
    /// pickers stay in front of the window: a key held down since before the picker showed (the
    /// Enter that asked for it, say) repeating, or let go, is not the user back in the window, and
    /// neither is the press a hidden window's logic still reads from the last frame it showed. On
    /// macOS the picker is a panel beside the window, which keeps working, so a press there leaves
    /// the panel waiting for its answer.
    #[test]
    fn only_a_press_in_the_focused_window_gives_up_and_never_beside_a_macos_panel() {
        let chosen = std::env::temp_dir();
        let cases = [
            ("a key going down", true, vec![], Some(vec![key(true)]), true),
            ("a key held down", true, vec![key(true)], Some(vec![key(true)]), false),
            ("a key let go", true, vec![key(true)], Some(vec![key(false)]), false),
            ("the pointer moving", true, vec![], Some(vec![egui::Event::PointerMoved(egui::pos2(1.0, 1.0))]), false),
            ("a press beside a macOS panel", false, vec![], Some(vec![press()]), false),
            ("a hidden window's last press", true, vec![press()], None, false),
        ];
        for (what, in_front, before, events, gives_up) in cases {
            let (mut pickers, ..) = hanging_pickers(&chosen);
            pickers.in_front = in_front;
            let ctx = egui::Context::default();
            let frame = |pickers: &mut Pickers, events| ctx.run_ui(input(true, events), |ui| pickers.answer(ui.ctx())).textures_delta.clear();
            frame(&mut pickers, before);
            let pin = pick_folder(&pickers.asks(), "Pin a folder");
            pickers.show(None);
            match events {
                Some(events) => frame(&mut pickers, events),
                None => {
                    let mut minimized = input(true, vec![]);
                    minimized.viewports.entry(egui::ViewportId::ROOT).or_default().minimized = Some(true);
                    let _ = ctx.run_logic(&minimized, |ctx| pickers.answer(ctx));
                }
            }
            let closed = pin.try_recv() == Err(std::sync::mpsc::TryRecvError::Disconnected);
            assert_eq!((closed, pickers.on_screen()), (gives_up, !gives_up), "{what}");
        }
    }

    /// The handoff over a picker that never answers: New project's Change… picker hangs, New project
    /// closes, and Open project asks meanwhile, so its pick waits. A press in the window gives the
    /// hung picker up, and the waiting Open pick shows once, in a picker of its own, and answers.
    #[test]
    fn a_pick_waiting_behind_a_picker_given_up_shows_once_and_answers() {
        let chosen = std::env::temp_dir();
        let (mut pickers, shown, _) = hanging_pickers(&chosen);
        pickers.in_front = true;
        let services = with_pickers(&pickers.asks());
        let (pick_folder, pick_project, withdraw) =
            (services.pick_folder.clone().unwrap(), services.pick_project.clone().unwrap(), services.withdraw_pick.clone().unwrap());
        let ctx = egui::Context::default();
        let frame = |pickers: &mut Pickers, events| {
            ctx.run_ui(input(true, events), |ui| pickers.answer(ui.ctx())).textures_delta.clear();
            pickers.show(None);
        };

        drop(pick_folder("Choose project location"));
        pickers.show(None);
        withdraw();
        let open = pick_project(Some(&chosen));
        frame(&mut pickers, vec![]);
        frame(&mut pickers, vec![]);
        assert_eq!((shown.get(), open.try_recv()), (0, Err(std::sync::mpsc::TryRecvError::Empty)), "shown over the hung picker");

        frame(&mut pickers, vec![press()]);
        frame(&mut pickers, vec![]);
        assert_eq!(open.try_recv(), Ok(Picked::Folder(chosen.clone())));
        frame(&mut pickers, vec![]);
        assert_eq!(shown.get(), 1, "the waiting pick did not show once");
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
