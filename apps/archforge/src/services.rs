//! The host services the shell may call: the installed fonts, the account's name, the folder new
//! projects go in, the folder picker, and the projects themselves.

use std::path::PathBuf;

use archforge_ui_egui::{Folder, Services};

pub fn native(projects: std::sync::Arc<crate::projects::Projects>) -> Services {
    Services {
        // The installed fonts, scanned and read on the shell's font worker.
        fonts: Some(std::sync::Arc::new(crate::fonts::SystemFonts::installed())),
        user_name: user_name(),
        projects_folder: projects_folder(),
        projects: Some(projects),
        pick_project: Some(std::sync::Arc::new(pick_project)),
        pick_folder: Some(std::sync::Arc::new(pick_folder)),
        find_projects: Some(std::sync::Arc::new(find_projects)),
        ids: Some(std::sync::Arc::new(fresh_id)),
    }
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

/// How deep below a pinned folder Home's search looks for projects, and the most it lists.
const FIND_DEPTH: usize = 2;
const FIND_LIMIT: usize = 200;

/// The projects in Home's pinned folders, looked for on a thread of their own so a slow disk
/// never holds the window; the answer is sent once. A thread that cannot start sends nothing, and
/// the search keeps to the recent projects.
fn find_projects(folders: Vec<PathBuf>) -> std::sync::mpsc::Receiver<Vec<PathBuf>> {
    let (tx, rx) = std::sync::mpsc::channel();
    let started = std::thread::Builder::new().name("find-projects".into()).spawn(move || {
        let mut found: Vec<PathBuf> = Vec::new();
        for folder in folders {
            let room = FIND_LIMIT.saturating_sub(found.len());
            if room == 0 {
                break;
            }
            for project in archforge_project::find(&folder, FIND_DEPTH, room) {
                if !found.contains(&project) {
                    found.push(project);
                }
            }
        }
        let _ = tx.send(found);
    });
    if let Err(e) = started {
        log::warn!("the pinned folders could not be searched for projects: {e}");
    }
    rx
}

/// The platform's folder picker, for Pin a folder on Home: it starts in the user's Documents.
fn pick_folder() -> Option<PathBuf> {
    let mut dialog = rfd::FileDialog::new().set_title(archforge_ui_egui::i18n::tr("Pin a folder"));
    if let Some(documents) = projects_folder().and_then(|f| f.path.parent().map(PathBuf::from)) {
        dialog = dialog.set_directory(documents);
    }
    dialog.pick_folder()
}

/// A version 7 id from the clock, a process-wide counter and the platform's randomness: the
/// counter in the tail makes every id of this process distinct whatever the clock and the
/// randomness do (RFC 9562's monotonic counter), the two random bytes before it keep ids of two
/// processes apart, and the time keeps them sorted. A failing randomness source is said once.
fn fresh_id() -> archforge_mof::Uuid {
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

/// The full name of the account the app runs under, for Home's avatar: on macOS what `id -F`
/// prints, on Linux the name field of the account's passwd entry, elsewhere the login name. None
/// when it cannot be read; the avatar is then hidden.
pub fn user_name() -> Option<String> {
    let name = full_name().or_else(|| std::env::var("USER").or_else(|_| std::env::var("USERNAME")).ok())?;
    let name = name.trim();
    // A name is a line of a few words; anything else is not shown.
    (!name.is_empty() && name.len() <= 256 && !name.contains('\n')).then(|| name.to_string())
}

#[cfg(target_os = "macos")]
fn full_name() -> Option<String> {
    let out = std::process::Command::new("/usr/bin/id").arg("-F").output().ok()?;
    if !out.status.success() || out.stdout.len() > 1024 {
        return None;
    }
    String::from_utf8(out.stdout).ok()
}

#[cfg(target_os = "linux")]
fn full_name() -> Option<String> {
    use std::io::Read as _;
    let user = std::env::var("USER").ok()?;
    let mut text = String::new();
    std::fs::File::open("/etc/passwd").ok()?.take(1 << 20).read_to_string(&mut text).ok()?;
    let gecos = text.lines().map(|l| l.split(':').collect::<Vec<_>>()).find(|f| f.first() == Some(&user.as_str()))?.get(4)?.to_string();
    gecos.split(',').next().map(str::to_string).filter(|n| !n.trim().is_empty())
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn full_name() -> Option<String> {
    None
}
