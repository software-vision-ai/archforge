//! Shared helpers: the workspace root, running tools, a small TOML reader, file walking.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The workspace root: the parent of this crate's manifest directory, wherever the target
/// directory is.
pub fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from("."))
}

/// The cargo that runs this task runner.
pub fn cargo() -> Command {
    let mut c = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string()));
    c.current_dir(root());
    c
}

/// Runs a command, inheriting the terminal; Err when it fails or cannot start.
pub fn run(mut cmd: Command) -> Result<(), String> {
    let shown = format!("{cmd:?}");
    let status = cmd.status().map_err(|e| format!("{shown}: {e}"))?;
    if status.success() { Ok(()) } else { Err(format!("{shown} exited with {status}")) }
}

/// One table of a TOML file: its name, whether it is an array entry, and its key-value pairs.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Table {
    pub name: String,
    pub array: bool,
    pub values: BTreeMap<String, String>,
}

/// Reads the subset of TOML these files use: comments, `[table]`, `[[array]]` and
/// `key = value` with string, number or boolean values. Anything else is an error naming the line.
pub fn toml(text: &str) -> Result<Vec<Table>, String> {
    let mut tables = vec![Table::default()];
    for (n, raw) in text.lines().enumerate() {
        let line = strip_comment(raw).trim().to_string();
        if line.is_empty() {
            continue;
        }
        if let Some(name) = line.strip_prefix("[[").and_then(|l| l.strip_suffix("]]")) {
            tables.push(Table { name: name.trim().to_string(), array: true, values: BTreeMap::new() });
        } else if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            tables.push(Table { name: name.trim().to_string(), array: false, values: BTreeMap::new() });
        } else if let Some((k, v)) = line.split_once('=') {
            let v = v.trim();
            let value = if let Some(s) = v.strip_prefix('"').and_then(|s| s.strip_suffix('"')) {
                s.replace("\\\"", "\"").replace("\\\\", "\\")
            } else {
                v.to_string()
            };
            if let Some(t) = tables.last_mut() {
                t.values.insert(k.trim().trim_matches('"').to_string(), value);
            }
        } else {
            return Err(format!("line {}: cannot read {raw:?}", n + 1));
        }
    }
    Ok(tables)
}

fn strip_comment(line: &str) -> &str {
    let mut in_string = false;
    for (i, c) in line.char_indices() {
        match c {
            '"' => in_string = !in_string,
            '#' if !in_string => return &line[..i],
            _ => {}
        }
    }
    line
}

/// Every regular file under `dir`, sorted, as paths relative to the workspace root. Symbolic links
/// are not followed, so a link cycle cannot make the walk endless. A folder's listing that fails
/// part way is an error, so no check passes on the files listed before it alone.
pub fn walk(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&d) else { continue };
        for e in entries {
            let e = e.map_err(|e| format!("{}: the listing stopped part way: {e}", d.display()))?;
            let Ok(kind) = e.file_type() else { continue };
            let p = e.path();
            if kind.is_dir() {
                stack.push(p);
            } else if kind.is_file() {
                out.push(p.strip_prefix(root()).map(Path::to_path_buf).unwrap_or(p));
            }
        }
    }
    out.sort();
    Ok(out)
}

/// Every regular file under `dir`, sorted, as paths under `dir`, for a check that must read every
/// file in its scope. Unlike [`walk`], nothing in scope is passed over: a folder that cannot be
/// opened, a listing that fails part way and an entry whose kind cannot be read are each an error
/// naming its path. A folder whose name `skip` lists is left out before it is opened, so one that
/// cannot be opened is no error. A symbolic link is skipped and never followed, so a link cycle
/// cannot make the walk endless, and so is any other entry that is neither a folder nor a regular
/// file; [`walk_strict_all`] names them.
pub fn walk_strict(dir: &Path, skip: &[&str]) -> Result<Vec<PathBuf>, String> {
    walk_strict_all(dir, skip).map(|walked| walked.files)
}

/// What a strict walk finds: the regular files, and the entries it leaves alone (a symbolic link,
/// or anything else neither a folder nor a regular file), each sorted.
#[derive(Debug, Default, PartialEq)]
pub struct Walked {
    pub files: Vec<PathBuf>,
    pub passed_over: Vec<PathBuf>,
}

/// [`walk_strict`], naming the entries it passes over as well, for a check that refuses them: a
/// link is never followed and no other entry is opened.
pub fn walk_strict_all(dir: &Path, skip: &[&str]) -> Result<Walked, String> {
    walk_strict_with(dir, skip, |d| std::fs::read_dir(d), std::fs::DirEntry::file_type)
}

/// [`walk_strict_all`] with the folder listing (`open`) and an entry's kind (`kind_of`) given, so a
/// test can make either fail on a filesystem that never would.
fn walk_strict_with(
    dir: &Path,
    skip: &[&str],
    open: impl Fn(&Path) -> std::io::Result<std::fs::ReadDir>,
    kind_of: impl Fn(&std::fs::DirEntry) -> std::io::Result<std::fs::FileType>,
) -> Result<Walked, String> {
    let mut out = Walked::default();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let entries = open(&d).map_err(|e| format!("{}: this folder cannot be opened, so nothing under it is checked: {e}", d.display()))?;
        for e in entries {
            let e = e.map_err(|e| format!("{}: the listing stopped part way: {e}", d.display()))?;
            let p = e.path();
            let kind = kind_of(&e).map_err(|e| format!("{}: what this entry is cannot be read, so it is not checked: {e}", p.display()))?;
            if kind.is_dir() {
                if !skip.iter().any(|s| e.file_name() == *s) {
                    stack.push(p);
                }
            } else if kind.is_file() {
                out.files.push(p);
            } else {
                out.passed_over.push(p);
            }
        }
    }
    out.files.sort();
    out.passed_over.sort();
    Ok(out)
}

/// A new, empty folder of a test's own under the temp dir, removed with everything in it when
/// dropped, a panic included; folders it locked are opened again first, so they can be removed.
#[cfg(test)]
pub struct Scratch {
    pub path: PathBuf,
    locked: std::cell::RefCell<Vec<PathBuf>>,
}

#[cfg(test)]
impl Scratch {
    /// Creates the folder, named with `name`, the process id, the time and an attempt; never adopts
    /// one that already exists.
    pub fn new(name: &str) -> Result<Scratch, String> {
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.subsec_nanos()).unwrap_or(0);
        for attempt in 0..16u32 {
            let path = std::env::temp_dir().join(format!("archforge-xtask-{name}-{}-{nanos}-{attempt}", std::process::id()));
            match std::fs::create_dir(&path) {
                Ok(()) => return Ok(Scratch { path, locked: Default::default() }),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(format!("{}: {e}", path.display())),
            }
        }
        Err(format!("no free temporary folder for {name}"))
    }

    /// Writes `text` to `rel` under the folder, making the folders on the way.
    pub fn file(&self, rel: &str, text: &str) -> std::io::Result<()> {
        let path = self.path.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, text)
    }

    /// Takes every permission from the folder `rel`, so this user cannot open it; false when it
    /// still opens (as root, say), where a test of an unreadable folder shows nothing.
    #[cfg(unix)]
    pub fn lock(&self, rel: &str) -> std::io::Result<bool> {
        use std::os::unix::fs::PermissionsExt;
        let path = self.path.join(rel);
        self.locked.borrow_mut().push(path.clone());
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000))?;
        Ok(std::fs::read_dir(&path).is_err())
    }
}

#[cfg(test)]
impl Drop for Scratch {
    /// Opens the locked folders again and removes the whole folder.
    fn drop(&mut self) {
        #[cfg(unix)]
        for path in self.locked.borrow().iter() {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755));
        }
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_toml_reader_reads_tables_arrays_and_comments() {
        let t = toml("a = \"x # not a comment\" # comment\n[t]\nb = 2\n[[row]]\nc = true\n[[row]]\nc = false\n").unwrap();
        assert_eq!(t[0].values["a"], "x # not a comment");
        assert_eq!(t[1].name, "t");
        assert_eq!(t.iter().filter(|x| x.array && x.name == "row").count(), 2);
        assert!(toml("nonsense").is_err());
    }

    /// A tree of two folders deep with a file at each level: `x.rs`, `a/y.rs`, `a/b/z.rs`.
    fn tree(name: &str) -> Scratch {
        let s = Scratch::new(name).unwrap();
        for rel in ["x.rs", "a/y.rs", "a/b/z.rs"] {
            s.file(rel, "").unwrap();
        }
        s
    }

    /// A folder in scope that cannot be opened stops the strict walk with an error naming it, never
    /// a list of the files found elsewhere (CodeRabbit on #572).
    #[test]
    fn a_folder_the_strict_walk_cannot_open_is_an_error_naming_it() {
        let s = tree("unopened");
        let locked = s.path.join("a").join("b");
        let open = |d: &Path| if d == locked { Err(std::io::Error::other("refused here")) } else { std::fs::read_dir(d) };
        let walked = walk_strict_with(&s.path, &[], open, std::fs::DirEntry::file_type).map(|w| w.files);
        assert_eq!(walked, Err(format!("{}: this folder cannot be opened, so nothing under it is checked: refused here", locked.display())));
        assert_eq!(walk_strict(&s.path, &[]), Ok(vec![s.path.join("a/b/z.rs"), s.path.join("a/y.rs"), s.path.join("x.rs")]));
    }

    /// An entry whose kind cannot be read stops the strict walk with an error naming it, never a
    /// silent skip (CodeRabbit on #572).
    #[test]
    fn an_entry_whose_kind_the_strict_walk_cannot_read_is_an_error_naming_it() {
        let s = tree("unknown-kind");
        let kind_of = |e: &std::fs::DirEntry| if e.file_name() == "y.rs" { Err(std::io::Error::other("no kind")) } else { e.file_type() };
        let walked = walk_strict_with(&s.path, &[], |d| std::fs::read_dir(d), kind_of).map(|w| w.files);
        assert_eq!(walked, Err(format!("{}: what this entry is cannot be read, so it is not checked: no kind", s.path.join("a/y.rs").display())));
    }

    /// A skipped folder is left out before it is opened, so one that cannot be opened is no error;
    /// only a folder of that name is skipped.
    #[test]
    fn a_folder_the_strict_walk_skips_is_never_opened() {
        let s = tree("skipped");
        s.file("a/b.rs", "").unwrap();
        let open = |d: &Path| if d.ends_with("b") { Err(std::io::Error::other("refused here")) } else { std::fs::read_dir(d) };
        assert_eq!(
            walk_strict_with(&s.path, &["b"], open, std::fs::DirEntry::file_type).map(|w| w.files),
            Ok(vec![s.path.join("a/b.rs"), s.path.join("a/y.rs"), s.path.join("x.rs")])
        );
        assert!(walk_strict_with(&s.path, &["c"], open, std::fs::DirEntry::file_type).is_err(), "the folder is opened when not skipped");
    }

    /// A symbolic link, to a file, to a folder or to nothing, is skipped and never followed;
    /// [`walk_strict_all`] names each.
    #[cfg(unix)]
    #[test]
    fn the_strict_walk_skips_links() {
        let s = tree("links");
        let outside = Scratch::new("links-outside").unwrap();
        outside.file("far.rs", "").unwrap();
        for (target, link) in [(s.path.join("x.rs"), "to-file.rs"), (outside.path.clone(), "to-folder"), (s.path.join("gone"), "dangling.rs")] {
            std::os::unix::fs::symlink(target, s.path.join("a").join(link)).unwrap();
        }
        let files = vec![s.path.join("a/b/z.rs"), s.path.join("a/y.rs"), s.path.join("x.rs")];
        assert_eq!(walk_strict(&s.path, &[]), Ok(files.clone()));
        let passed_over = ["dangling.rs", "to-file.rs", "to-folder"].map(|n| s.path.join("a").join(n)).to_vec();
        assert_eq!(walk_strict_all(&s.path, &[]), Ok(Walked { files, passed_over }));
    }
}
