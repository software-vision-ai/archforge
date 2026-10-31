//! The installed fonts: the desktop's font directories, walked and read for the shell's
//! `FontSource`. Each file is read by this module under the face limit, never mapped, so a font
//! changed on disk cannot fault the process; a collection claiming more faces than it can hold is
//! skipped before it is parsed; the walk and the faces taken are capped; fontconfig's own
//! configuration is not read.

use std::collections::BTreeSet;
use std::io::Read;
use std::path::{Path, PathBuf};

use archforge_ui_egui::{FontEntry, FontSource};

/// How deep the walk goes below a font directory.
const MAX_DEPTH: usize = 16;
/// The font files a scan takes at most.
const MAX_FILES: usize = 10_000;
/// The directory entries a walk looks at, at most (a font folder linked to the home folder).
const MAX_VISITED: usize = 200_000;
/// The faces one collection file may hold.
const MAX_FACES_PER_FILE: u32 = 256;
/// The faces a scan takes at most.
const MAX_FACES: usize = 20_000;

/// The fonts installed in a set of directories.
pub struct SystemFonts {
    dirs: Vec<PathBuf>,
}

impl SystemFonts {
    /// The platform's font directories.
    pub fn installed() -> Self {
        Self::in_dirs(platform_dirs())
    }

    pub fn in_dirs(dirs: Vec<PathBuf>) -> Self {
        Self { dirs }
    }
}

/// The directories the platform installs fonts in, the user's own included.
fn platform_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    #[cfg(target_os = "macos")]
    {
        dirs.push("/System/Library/Fonts".into());
        dirs.push("/Library/Fonts".into());
        if let Some(home) = std::env::var_os("HOME") {
            dirs.push(PathBuf::from(home).join("Library/Fonts"));
        }
    }
    #[cfg(windows)]
    {
        if let Some(windir) = std::env::var_os("WINDIR") {
            dirs.push(PathBuf::from(windir).join("Fonts"));
        }
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            dirs.push(PathBuf::from(local).join("Microsoft").join("Windows").join("Fonts"));
        }
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        dirs.push("/usr/share/fonts".into());
        dirs.push("/usr/local/share/fonts".into());
        if let Some(home) = std::env::var_os("HOME") {
            dirs.push(PathBuf::from(&home).join(".local/share/fonts"));
            dirs.push(PathBuf::from(home).join(".fonts"));
        }
    }
    dirs
}

fn is_font(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| ["ttf", "otf", "ttc", "otc"].iter().any(|x| e.eq_ignore_ascii_case(x)))
}

/// The font files under `dirs`, without loops (each directory and file once, by its canonical
/// path), at most `MAX_DEPTH` deep, `MAX_FILES` many and `MAX_VISITED` entries looked at, none
/// longer than `cap` by its length when listed; sorted.
fn font_files(dirs: &[PathBuf], cap: u64) -> Vec<PathBuf> {
    let mut seen: BTreeSet<PathBuf> = BTreeSet::new();
    let mut files: Vec<PathBuf> = Vec::new();
    let mut visited = 0usize;
    let mut stack: Vec<(PathBuf, usize)> = dirs.iter().rev().map(|d| (d.clone(), 0)).collect();
    let mut limited = None;
    'walk: while let Some((dir, depth)) = stack.pop() {
        let Ok(dir) = dir.canonicalize() else { continue };
        if !seen.insert(dir.clone()) {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            visited += 1;
            if files.len() >= MAX_FILES || visited > MAX_VISITED {
                limited = Some(if files.len() >= MAX_FILES { "font files" } else { "directory entries" });
                break 'walk;
            }
            let path = entry.path();
            // Followed through a link: a loop is caught by the seen set.
            let Ok(meta) = std::fs::metadata(&path) else { continue };
            if meta.is_dir() {
                if depth < MAX_DEPTH {
                    stack.push((path, depth + 1));
                } else {
                    limited = Some("folder depth");
                }
            } else if meta.is_file()
                && is_font(&path)
                && let Ok(file) = path.canonicalize()
                && seen.insert(file.clone())
            {
                if meta.len() <= cap {
                    files.push(file);
                } else {
                    log::info!("font file {} skipped: longer than {cap} bytes", file.display());
                }
            }
        }
    }
    if let Some(what) = limited {
        log::warn!("the font scan stopped at its limit of {what}; fonts past it are not offered");
    }
    files.sort();
    files
}

/// A file's bytes, refused when it is not a regular file or is longer than `cap` (read at most
/// one byte past it, so a file grown since it was listed is refused unread).
fn read_capped(path: &Path, cap: u64) -> Result<Vec<u8>, String> {
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    if !file.metadata().map_err(|e| e.to_string())?.is_file() {
        return Err("not a regular file".into());
    }
    let mut bytes = Vec::new();
    file.take(cap.saturating_add(1)).read_to_end(&mut bytes).map_err(|e| e.to_string())?;
    if bytes.len() as u64 > cap {
        return Err(format!("the font file is longer than {cap} bytes"));
    }
    Ok(bytes)
}

/// Whether a font file's collection header claims no more faces than it may hold: at most
/// `MAX_FACES_PER_FILE`, and no more offsets than the file has room for. A single font passes.
fn sane_collection(bytes: &[u8]) -> bool {
    if bytes.get(..4) != Some(b"ttcf") {
        return true;
    }
    let Some(count) = bytes.get(8..12).map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]])) else { return false };
    count <= MAX_FACES_PER_FILE && u64::from(count) * 4 <= (bytes.len() as u64).saturating_sub(12)
}

/// The faces of one font file, read and parsed on their own, so a bad file costs only itself.
fn faces_of(path: &Path, cap: u64) -> Result<Vec<FontEntry>, String> {
    let bytes = read_capped(path, cap)?;
    if !sane_collection(&bytes) {
        return Err(format!("its collection header claims more than {MAX_FACES_PER_FILE} faces or more than it holds"));
    }
    let len = bytes.len() as u64;
    let mut db = fontdb::Database::new();
    db.load_font_data(bytes);
    let key = path.to_string_lossy().into_owned();
    Ok(db
        .faces()
        .map(|face| FontEntry {
            families: face.families.iter().map(|(name, _)| name.clone()).collect(),
            post_script: face.post_script_name.clone(),
            weight: face.weight.0,
            italic: face.style != fontdb::Style::Normal,
            stretch: face.stretch.to_number(),
            key: key.clone(),
            index: face.index,
            len,
        })
        .collect())
}

impl FontSource for SystemFonts {
    fn scan(&self, cap: u64) -> Vec<FontEntry> {
        let mut entries = Vec::new();
        for file in font_files(&self.dirs, cap) {
            // A panic on one file skips that file, not the scan.
            match std::panic::catch_unwind(|| faces_of(&file, cap)) {
                Ok(Ok(faces)) => entries.extend(faces),
                Ok(Err(e)) => log::info!("font file {} skipped: {e}", file.display()),
                Err(_) => log::warn!("font file {} skipped: its parser stopped", file.display()),
            }
            if entries.len() >= MAX_FACES {
                log::warn!("the font scan stopped at its limit of {MAX_FACES} faces; fonts past it are not offered");
                entries.truncate(MAX_FACES);
                break;
            }
        }
        entries
    }

    fn read(&self, entry: &FontEntry, cap: u64) -> Result<Vec<u8>, String> {
        read_capped(Path::new(&entry.key), cap)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A directory of fixture fonts: egui's own bundled faces written as files, a font cut short
    /// inside its table directory, a text file named like a font, a nested directory and a
    /// link back to the top (where links can be made). No system font is read.
    fn fixture() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("archforge-fonts-{}-{:?}", std::process::id(), std::thread::current().id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("nested")).unwrap();
        let defs = egui::FontDefinitions::default();
        std::fs::write(dir.join("Hack-Regular.ttf"), (*defs.font_data["Hack"].font).as_ref()).unwrap();
        std::fs::write(dir.join("nested").join("Ubuntu-Light.TTF"), (*defs.font_data["Ubuntu-Light"].font).as_ref()).unwrap();
        std::fs::write(dir.join("truncated.ttf"), &(*defs.font_data["Hack"].font).as_ref()[..40]).unwrap();
        std::fs::write(dir.join("not-a-font.ttf"), b"this is not a font").unwrap();
        std::fs::write(dir.join("readme.txt"), b"skipped by its extension").unwrap();
        // A collection claiming four billion faces in twelve bytes, and one claiming more
        // offsets than it holds: both skipped before they are parsed.
        std::fs::write(dir.join("hostile.ttc"), [b't', b't', b'c', b'f', 0, 1, 0, 0, 0xff, 0xff, 0xff, 0xff]).unwrap();
        let mut short = b"ttcf\x00\x01\x00\x00\x00\x00\x00\x10".to_vec();
        short.extend([0u8; 20]);
        std::fs::write(dir.join("short.ttc"), short).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(&dir, dir.join("nested").join("loop")).unwrap();
        dir
    }

    #[test]
    fn a_collection_header_is_held_to_what_the_file_can_hold() {
        assert!(sane_collection(b"not a collection"));
        let mut ok = b"ttcf\x00\x01\x00\x00\x00\x00\x00\x02".to_vec();
        ok.extend([0u8; 8]);
        assert!(sane_collection(&ok));
        assert!(!sane_collection(&ok[..15]), "two offsets need eight bytes");
        assert!(!sane_collection(b"ttcf\x00\x01\x00\x00\xff\xff\xff\xff"));
        assert!(!sane_collection(b"ttcf\x00\x01"), "a header cut short");
        let mut many = b"ttcf\x00\x01\x00\x00\x00\x00\x01\x01".to_vec();
        many.extend(vec![0u8; 257 * 4]);
        assert!(!sane_collection(&many), "past the faces one file may hold");
    }

    #[test]
    fn the_scan_finds_the_fixture_families_skips_what_is_no_font_and_survives_a_loop() {
        let dir = fixture();
        let fonts = SystemFonts::in_dirs(vec![dir.clone()]);
        let entries = fonts.scan(64 << 20);
        let mut families: Vec<&str> = entries.iter().filter_map(|e| e.families.first().map(String::as_str)).collect();
        families.sort();
        assert_eq!(families, ["Hack", "Ubuntu"], "{entries:#?}");
        assert_eq!(fonts.scan(64 << 20), entries, "the same fonts give the same scan");
        let hack = entries.iter().find(|e| e.families[0] == "Hack").unwrap();
        assert_eq!((hack.weight, hack.italic, hack.stretch, hack.index), (400, false, 5, 0));
        assert_eq!(fonts.read(hack, 64 << 20).unwrap().len() as u64, hack.len);
        assert!(fonts.read(hack, 100).unwrap_err().contains("longer than 100 bytes"));
        // A file past the face limit is not scanned at all.
        assert!(fonts.scan(1000).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
