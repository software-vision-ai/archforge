//! The host services the shell may call: the installed fonts, the folder new projects go in, the
//! folder picker, the projects themselves, and where support for Archforge goes.

use std::path::PathBuf;

use archforge_ui_egui::{Folder, Services};

pub fn native(projects: std::sync::Arc<crate::projects::Projects>) -> Services {
    Services {
        // The installed fonts, scanned and read on the shell's font worker.
        fonts: Some(std::sync::Arc::new(crate::fonts::SystemFonts::installed())),
        projects_folder: projects_folder(),
        projects: Some(projects),
        pick_project: Some(std::sync::Arc::new(pick_project)),
        pick_folder: Some(std::sync::Arc::new(pick_folder)),
        picker_unavailable: Some(std::sync::Arc::new(picker_unavailable)),
        find_projects: Some(std::sync::Arc::new(find_projects)),
        ids: Some(std::sync::Arc::new(fresh_id)),
        support: support(),
    }
}

/// The owner's approved support pages (AR-69, AR-70): GitHub Sponsors goes to the company's
/// organization, Ko-fi to the owner. Each is set once its page can take support; until then About
/// Archforge says it is not set up yet. The organization's Sponsors profile is not public yet.
const GITHUB_SPONSORS: Option<&str> = None;
const KO_FI: Option<&str> = Some("https://ko-fi.com/dejanwho");

fn support() -> archforge_ui_egui::support::Destinations {
    archforge_ui_egui::support::Destinations { github_sponsors: GITHUB_SPONSORS.map(str::to_string), ko_fi: KO_FI.map(str::to_string) }
}

/// The platform's folder picker, for Open project: it starts in the folder given (a pinned
/// folder), else in the projects folder.
fn pick_project(start: Option<&std::path::Path>) -> Option<PathBuf> {
    let mut dialog = rfd::FileDialog::new();
    match start.filter(|s| s.is_dir()) {
        Some(start) => dialog = dialog.set_directory(start),
        None => {
            if let Some(folder) = projects_folder().filter(|f| f.path.is_dir()) {
                dialog = dialog.set_directory(folder.path);
            }
        }
    }
    dialog.pick_folder()
}

/// Why the folder picker cannot appear. Where the picker is the desktop portal's (Linux
/// and the BSDs), asked over D-Bus, a missing portal answers nothing, as a cancel does; the picker
/// library then falls back to zenity, so only with no zenity either is the picker surely missing.
/// Elsewhere the picker is the system's own and always appears.
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

/// The platform's folder picker, headed with the title the shell gives for what the folder is for
/// (Pin a folder, Choose project location): it starts in the user's Documents.
fn pick_folder(title: &str) -> Option<PathBuf> {
    let mut dialog = rfd::FileDialog::new().set_title(title);
    if let Some(documents) = projects_folder().and_then(|f| f.path.parent().map(PathBuf::from)) {
        dialog = dialog.set_directory(documents);
    }
    dialog.pick_folder()
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
