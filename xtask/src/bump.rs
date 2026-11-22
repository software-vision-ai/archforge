//! `cargo xtask bump`: moves the craft-libs crates in `Cargo.lock` to the newest commit of
//! craft-libs's main branch, and nothing else.
//!
//! The gate never updates the lock file, so craft-libs moves only here, in a pull request of its
//! own. `cargo update` re-chooses some other packages' dependency lists as it goes (which
//! `windows-sys` a crate links, for one), so the bump keeps every package that does not come from
//! craft-libs exactly as the lock had it and checks that the result still resolves. When the
//! newest main needs another package added, removed or at another version, the bump puts
//! `Cargo.lock` back as it was and names those packages: that is a dependency change to review on
//! its own.

use crate::util;

/// The git source whose crates `bump` moves: craft-libs, the only git source `deny.toml` allows.
pub const SOURCE: &str = "git+https://github.com/storytold/craft-libs";

/// What [`changed_outside`] names, first, when the lock's opening lines (its format version) changed.
const HEADER: &str = "the lock file's opening lines";

/// One `[[package]]` table of a `Cargo.lock`: its name, its version, its source (none for a crate
/// of this workspace) and the table's whole text, without the blank lines after it, so the last
/// table compares equal to itself with a table after it.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub name: String,
    pub version: String,
    pub source: Option<String>,
    pub text: String,
}

impl Entry {
    /// Whether the entry comes from `source`, a source URL without its query and commit: the URL
    /// exactly, so `craft-libs-extra` is not `craft-libs`.
    fn comes_from(&self, source: &str) -> bool {
        self.source.as_deref().and_then(|s| s.strip_prefix(source)).is_some_and(|rest| rest.is_empty() || rest.starts_with(['?', '#']))
    }

    /// The name, version and source that tell one package of a lock from another.
    fn key(&self) -> (String, String, Option<String>) {
        (self.name.clone(), self.version.clone(), self.source.clone())
    }

    /// The commit a git source locks, the part after `#`; none for any other source.
    fn commit(&self) -> Option<&str> {
        self.source.as_deref().and_then(|s| s.rsplit_once('#')).map(|(_, c)| c)
    }
}

/// The text of `lock` before its first `[[package]]` table (the comment and the format version),
/// and the tables in lock order. A table without a name or a version is an error naming it, and so
/// is any other kind of table (`[[patch.unused]]`, `[metadata]`), which would otherwise be read as
/// part of the package before it.
pub fn entries(lock: &str) -> Result<(String, Vec<Entry>), String> {
    if let Some(other) = lock.lines().find(|l| l.starts_with('[') && *l != "[[package]]") {
        return Err(format!("Cargo.lock: bump reads only [[package]] tables, and the lock has {other}"));
    }
    let mut parts = lock.split("[[package]]\n");
    let head = parts.next().unwrap_or_default().to_string();
    let mut out = Vec::new();
    for (n, text) in parts.enumerate() {
        let field = |key: &str| {
            text.lines().find_map(|l| l.strip_prefix(key).and_then(|r| r.strip_prefix(" = \"")).and_then(|r| r.strip_suffix('"')).map(str::to_string))
        };
        let (Some(name), Some(version)) = (field("name"), field("version")) else {
            return Err(format!("Cargo.lock: package table {} has no name or no version", n + 1));
        };
        out.push(Entry { name, version, source: field("source"), text: text.trim_end().to_string() });
    }
    Ok((head, out))
}

/// The names of the packages of `lock` from `source`, each once, in lock order.
pub fn packages_from(lock: &str, source: &str) -> Result<Vec<String>, String> {
    let mut out: Vec<String> = Vec::new();
    for e in entries(lock)?.1 {
        if e.comes_from(source) && !out.contains(&e.name) {
            out.push(e.name);
        }
    }
    Ok(out)
}

/// The packages outside `source` that `after` adds, drops or has at another version or source than
/// `before`, as `name version`, sorted and once each, and the lock's opening lines when they
/// changed. A package whose dependency list alone differs is not named: [`keep_others`] puts its
/// list back. Empty when only packages from `source` moved.
pub fn changed_outside(before: &str, after: &str, source: &str) -> Result<Vec<String>, String> {
    let (head_before, before) = entries(before)?;
    let (head_after, after) = entries(after)?;
    let others = |list: &[Entry]| list.iter().filter(|e| !e.comes_from(source)).map(Entry::key).collect::<Vec<_>>();
    let (before, after) = (others(&before), others(&after));
    let mut out: Vec<String> = before
        .iter()
        .filter(|k| !after.contains(k))
        .chain(after.iter().filter(|k| !before.contains(k)))
        .map(|(name, version, _)| format!("{name} {version}"))
        .collect();
    out.sort();
    out.dedup();
    if head_before != head_after {
        out.insert(0, HEADER.to_string());
    }
    Ok(out)
}

/// `after` with each package that `before` has at the same name, version and source put back as
/// `before` has it, and the names of the packages whose text that changed, as `name version` in
/// lock order. A moved craft-libs crate has a new source, so it stays as `after` has it, and so
/// does a package `before` lacks. Meant for when [`changed_outside`] is empty.
pub fn keep_others(before: &str, after: &str) -> Result<(String, Vec<String>), String> {
    let (head, before) = entries(before)?;
    let mut kept = Vec::new();
    let tables: Vec<String> = entries(after)?
        .1
        .into_iter()
        .map(|a| {
            let old = before.iter().find(|b| b.key() == a.key());
            match old {
                Some(b) if b.text != a.text => {
                    kept.push(format!("{} {}", a.name, a.version));
                    format!("[[package]]\n{}\n", b.text)
                }
                _ => format!("[[package]]\n{}\n", a.text),
            }
        })
        .collect();
    Ok((format!("{head}{}", tables.join("\n")), kept))
}

/// One line for each package from `source` whose commit differs between `before` and `after`:
/// `name old -> new` with each commit's first 12 characters, or `(none)` for a package only one
/// side has. Lock order of `after`, then the packages only `before` has.
pub fn moves(before: &str, after: &str, source: &str) -> Result<Vec<String>, String> {
    let pick = |lock: &str| entries(lock).map(|(_, list)| list.into_iter().filter(|e| e.comes_from(source)).collect::<Vec<_>>());
    let (before, after) = (pick(before)?, pick(after)?);
    let short = |e: Option<&Entry>| e.and_then(Entry::commit).map_or_else(|| "(none)".to_string(), |c| c.chars().take(12).collect());
    let mut out = Vec::new();
    for a in &after {
        let b = before.iter().find(|b| b.name == a.name);
        if b.and_then(Entry::commit) != a.commit() {
            out.push(format!("{} {} -> {}", a.name, short(b), short(Some(a))));
        }
    }
    for b in before.iter().filter(|b| !after.iter().any(|a| a.name == b.name)) {
        out.push(format!("{} {} -> (none)", b.name, short(Some(b))));
    }
    Ok(out)
}

/// What a bump reads, writes and runs: the lock file's text, a write of its bytes, `cargo update`
/// for the named packages, and the check that the lock resolves as it is. [`run`] gives the real
/// file and cargo; the tests give their own.
struct Io<'a> {
    read: &'a dyn Fn() -> Result<String, String>,
    write: &'a dyn Fn(&str) -> Result<(), String>,
    update: &'a dyn Fn(&[String]) -> Result<(), String>,
    check: &'a dyn Fn() -> Result<(), String>,
}

/// Moves the craft-libs crates in `Cargo.lock` to the newest commit of craft-libs's main, keeps
/// every other package as the lock had it, and prints which crates moved, from which commit to
/// which. Once `cargo update` has rewritten the lock, every failure writes it back as it was.
pub fn run() -> Result<(), String> {
    let path = util::root().join("Cargo.lock");
    let io = Io {
        read: &|| std::fs::read_to_string(&path).map_err(|e| format!("bump: {}: {e}", path.display())),
        write: &|text| craft_atomic::atomic_write(&path, text.as_bytes()).map_err(|e| format!("{}: {e}", path.display())),
        update: &|packages| {
            let mut cmd = util::cargo();
            cmd.arg("update");
            for p in packages {
                cmd.args(["-p", p]);
            }
            util::run(cmd).map_err(|e| {
                format!(
                    "{e}\nbump: `cargo update` could not move {}; cargo says why above (fetching the newest main needs the network)",
                    packages.join(", ")
                )
            })
        },
        check: &|| {
            let mut check = util::cargo();
            check.args(["metadata", "--locked", "--format-version", "1"]).stdout(std::process::Stdio::null());
            util::run(check)
        },
    };
    for line in bump(&io)? {
        println!("bump: {line}");
    }
    Ok(())
}

/// One bump through `io`; the lines to print. A lock checked out with CRLF line endings is read
/// with LF, as its tables are split on LF, and written with CRLF again. From `cargo update` on,
/// every failure, that update's own included (cargo may have cut the file short), writes back the
/// file as it was.
fn bump(io: &Io) -> Result<Vec<String>, String> {
    let original = (io.read)()?;
    let before = lf(&original);
    let packages = packages_from(&before, SOURCE)?;
    if packages.is_empty() {
        return Err(format!("bump: no crate in Cargo.lock comes from {SOURCE}"));
    }
    let write = |text: &str| (io.write)(&endings_of(&original, text));
    restoring(&original, &write, || {
        (io.update)(&packages)?;
        let after = lf(&(io.read)()?);
        let relisted = settle(&before, &after, &write, io.check)?;
        let mut lines = Vec::new();
        if !relisted.is_empty() {
            lines.push(format!("kept the dependency lists `cargo update` re-chose for {} as Cargo.lock had them", relisted.join(", ")));
        }
        let moved = moves(&before, &after, SOURCE)?;
        if moved.is_empty() {
            lines.push(match entries(&after)?.1.iter().find(|e| e.comes_from(SOURCE)).and_then(Entry::commit) {
                Some(commit) => format!("the craft-libs crates are at the newest main already, {commit}"),
                None => "no craft-libs crate moved".to_string(),
            });
        }
        lines.extend(moved);
        Ok(lines)
    })
}

/// `text` with CRLF line endings turned into LF.
fn lf(text: &str) -> String {
    text.replace("\r\n", "\n")
}

/// `text`, an LF lock, with the line endings of `original`, the lock as it was read: CRLF when it
/// had any, so a bump on a CRLF checkout changes only the lines it moves. `original` itself comes
/// back byte for byte.
fn endings_of(original: &str, text: &str) -> String {
    if text == original || !original.contains("\r\n") { text.to_string() } else { lf(text).replace('\n', "\r\n") }
}

/// Runs `step`, the part of a bump from `cargo update` on, which can change the lock. When it
/// fails, `before` is written back through `write`, and the error says whether that worked.
fn restoring<T>(before: &str, write: &dyn Fn(&str) -> Result<(), String>, step: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
    step().map_err(|refusal| match write(before) {
        Ok(()) => format!("{refusal}\nbump: Cargo.lock is back as it was"),
        Err(e) => format!("{refusal}\nbump: could not put the lock back as it was: {e}; `git checkout -- Cargo.lock` puts back the committed one"),
    })
}

/// Refuses `after`, the lock `cargo update` wrote, when it adds, drops or moves a package outside
/// craft-libs; otherwise writes it with every other package as `before` had it and runs `check`,
/// which says whether that lock resolves as it is. Returns the packages whose dependency lists
/// were put back. On an error the caller puts `before` back.
fn settle(
    before: &str,
    after: &str,
    write: &dyn Fn(&str) -> Result<(), String>,
    check: impl FnOnce() -> Result<(), String>,
) -> Result<Vec<String>, String> {
    let others = changed_outside(before, after, SOURCE)?;
    if others.first().is_some_and(|o| o == HEADER) {
        return Err("bump: `cargo update` wrote the lock file in another format (its opening lines changed): that comes with another cargo, \
             not with craft-libs. Run the bump with the cargo rust-toolchain.toml names, or move the format in a pull request of its own"
            .to_string());
    }
    if !others.is_empty() {
        return Err(format!(
            "bump: the newest craft-libs main also changes {}, which do not come from craft-libs. \
             Moving them is a dependency change of its own: make it by hand (`cargo update -p <crate>`), review it and say why in its pull request",
            others.join(", ")
        ));
    }
    let (kept, relisted) = keep_others(before, after)?;
    write(&kept).map_err(|e| format!("bump: {e}"))?;
    check().map_err(|e| {
        format!(
            "{e}\nbump: with the other packages kept as they were, the new craft-libs does not resolve. \
             The newest craft-libs needs a dependency change of its own: make it by hand (`cargo update -p <crate>`), review it and say why in its pull request"
        )
    })?;
    Ok(relisted)
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;

    const OLD: &str = "git+https://github.com/storytold/craft-libs?branch=main#d40c2be0aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const NEW: &str = "git+https://github.com/storytold/craft-libs?branch=main#1a2b3c4d5e6f77777777777777777777777777";
    const REGISTRY: &str = "registry+https://github.com/rust-lang/crates.io-index";

    /// A lock with a craft-libs crate at `craft`, egui at `egui`, and a workspace crate.
    fn lock(craft: &str, egui: &str) -> String {
        format!(
            "# This file is automatically @generated by Cargo.\nversion = 4\n\n\
             [[package]]\nname = \"craft-atomic\"\nversion = \"0.1.0\"\nsource = \"{craft}\"\n\n\
             [[package]]\nname = \"egui\"\nversion = \"{egui}\"\nsource = \"{REGISTRY}\"\nchecksum = \"00\"\n\n\
             [[package]]\nname = \"local\"\nversion = \"0.1.0\"\ndependencies = [\n \"craft-atomic\",\n \"egui\",\n]\n"
        )
    }

    /// Each table's name, version, source and commit are read, and the craft-libs names once each;
    /// a CRLF lock reads as its LF text and is written with CRLF again; a table without a version
    /// is an error, not an empty version, and so is a table of another kind.
    #[test]
    fn the_shared_crates_are_read_from_the_lock_once_each() {
        let twice = format!("{}\n[[package]]\nname = \"craft-atomic\"\nversion = \"0.1.0\"\nsource = \"{OLD}\"\n", lock(OLD, "0.36.2"));
        assert_eq!(packages_from(&twice, SOURCE).unwrap(), ["craft-atomic"]);
        assert!(packages_from("", SOURCE).unwrap().is_empty());
        let crlf = lock(OLD, "0.36.2").replace('\n', "\r\n");
        assert_eq!(lf(&crlf), lock(OLD, "0.36.2"), "a CRLF checkout reads as the LF lock");
        assert_eq!(endings_of(&crlf, &lock(NEW, "0.36.2")), lock(NEW, "0.36.2").replace('\n', "\r\n"), "and is written back with CRLF");
        assert_eq!(endings_of(&lock(OLD, "0.36.2"), &lock(NEW, "0.36.2")), lock(NEW, "0.36.2"), "an LF lock stays LF");
        let mixed = format!("{}{crlf}", lock(OLD, "0.36.2"));
        assert_eq!(endings_of(&mixed, &mixed), mixed, "the lock as it was is written back as it was");
        let (head, list) = entries(&lock(OLD, "0.36.2")).unwrap();
        assert!(head.ends_with("version = 4\n\n"), "{head:?}");
        assert_eq!(
            list.iter().map(|e| (e.name.as_str(), e.version.as_str())).collect::<Vec<_>>(),
            [("craft-atomic", "0.1.0"), ("egui", "0.36.2"), ("local", "0.1.0")]
        );
        assert_eq!(list[2].source, None);
        assert_eq!(list[0].commit(), Some("d40c2be0aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"));
        let unversioned = lock(OLD, "0.36.2").replace("version = \"0.36.2\"\n", "");
        assert_eq!(entries(&unversioned).unwrap_err(), "Cargo.lock: package table 2 has no name or no version");
        let patched = format!("{}\n[[patch.unused]]\nname = \"craft-x\"\nversion = \"0.1.0\"\nsource = \"{OLD}\"\n", lock(OLD, "0.36.2"));
        assert_eq!(entries(&patched).unwrap_err(), "Cargo.lock: bump reads only [[package]] tables, and the lock has [[patch.unused]]");
    }

    /// A move of the craft-libs commit alone is a bump; it is named crate by crate, old commit to new.
    #[test]
    fn a_craft_libs_move_alone_passes_and_is_named_from_commit_to_commit() {
        let (before, after) = (lock(OLD, "0.36.2"), lock(NEW, "0.36.2"));
        assert_eq!(changed_outside(&before, &after, SOURCE).unwrap(), Vec::<String>::new());
        assert_eq!(moves(&before, &after, SOURCE).unwrap(), ["craft-atomic d40c2be0aaaa -> 1a2b3c4d5e6f"]);
        assert_eq!(moves(&before, &before, SOURCE).unwrap(), Vec::<String>::new(), "nothing moved");
    }

    /// Any other package that changes, appears or goes is named, and so are the lock's opening lines.
    #[test]
    fn any_change_outside_craft_libs_is_named() {
        let before = lock(OLD, "0.36.2");
        assert_eq!(changed_outside(&before, &lock(NEW, "0.37.0"), SOURCE).unwrap(), ["egui 0.36.2", "egui 0.37.0"]);
        let added = format!("{}\n[[package]]\nname = \"zeroize\"\nversion = \"1.8.0\"\nsource = \"{REGISTRY}\"\n", lock(NEW, "0.36.2"));
        assert_eq!(changed_outside(&before, &added, SOURCE).unwrap(), ["zeroize 1.8.0"]);
        assert_eq!(changed_outside(&added, &before, SOURCE).unwrap(), ["zeroize 1.8.0"], "a package that goes is named too");
        let relisted = lock(NEW, "0.36.2").replace(" \"egui\",\n", "");
        assert_eq!(changed_outside(&before, &relisted, SOURCE).unwrap(), Vec::<String>::new(), "a dependency list alone is put back, not refused");
        let near = format!(
            "{}\n[[package]]\nname = \"craft-x\"\nversion = \"0.1.0\"\nsource = \"{}\"\n",
            lock(NEW, "0.36.2"),
            NEW.replace("craft-libs", "craft-libs-extra")
        );
        assert_eq!(
            changed_outside(&before, &near, SOURCE).unwrap(),
            ["craft-x 0.1.0"],
            "another repository whose URL starts the same is not craft-libs"
        );
        assert_eq!(packages_from(&near, SOURCE).unwrap(), ["craft-atomic"]);
        let format = lock(NEW, "0.36.2").replace("version = 4", "version = 5");
        assert_eq!(changed_outside(&before, &format, SOURCE).unwrap(), ["the lock file's opening lines"]);
    }

    /// The other packages' tables come back as the lock had them; the craft-libs tables stay new,
    /// their own dependency lists included.
    #[test]
    fn the_other_packages_are_kept_as_the_lock_had_them() {
        let before = lock(OLD, "0.36.2");
        let relisted = lock(NEW, "0.36.2").replace(" \"egui\",\n", "");
        assert_eq!(keep_others(&before, &relisted).unwrap(), (lock(NEW, "0.36.2"), vec!["local 0.1.0".to_string()]));
        let craft = format!("source = \"{NEW}\"\n");
        let grown = lock(NEW, "0.36.2").replace(&craft, &format!("{craft}dependencies = [\n \"egui\",\n]\n"));
        assert_eq!(keep_others(&before, &grown).unwrap(), (grown.clone(), Vec::new()));
        // The lock is written back from its tables: the repository's own comes back byte for byte.
        // Read as `run` reads it, so a CRLF checkout is split into its tables too.
        let real = lf(&std::fs::read_to_string(util::root().join("Cargo.lock")).unwrap());
        assert!(entries(&real).unwrap().1.len() > 100, "the repository's lock is read as its tables");
        assert_eq!(keep_others(&real, &real).unwrap(), (real.clone(), Vec::new()));
    }

    /// A craft-libs crate that only one side has is named with `(none)` on the other.
    #[test]
    fn a_craft_libs_crate_that_comes_or_goes_is_named() {
        let before = lock(OLD, "0.36.2");
        let more = format!("{}\n[[package]]\nname = \"craft-i18n\"\nversion = \"0.1.0\"\nsource = \"{NEW}\"\n", lock(NEW, "0.36.2"));
        assert_eq!(moves(&before, &more, SOURCE).unwrap(), ["craft-atomic d40c2be0aaaa -> 1a2b3c4d5e6f", "craft-i18n (none) -> 1a2b3c4d5e6f"]);
        assert_eq!(moves(&more, &before, SOURCE).unwrap(), ["craft-atomic 1a2b3c4d5e6f -> d40c2be0aaaa", "craft-i18n 1a2b3c4d5e6f -> (none)"]);
    }

    /// A bump that needs another package moved writes nothing and never runs the check; one that
    /// moves craft-libs alone writes the kept lock first, then checks it.
    #[test]
    fn settle_refuses_before_writing_and_writes_before_checking() {
        let log = RefCell::new(Vec::<String>::new());
        let write = |text: &str| -> Result<(), String> {
            log.borrow_mut().push(format!("write {}", text.len()));
            Ok(())
        };
        let check = || -> Result<(), String> {
            log.borrow_mut().push("check".to_string());
            Ok(())
        };
        let before = lock(OLD, "0.36.2");
        let refused = settle(&before, &lock(NEW, "0.37.0"), &write, check).unwrap_err();
        assert!(refused.contains("also changes egui 0.36.2, egui 0.37.0"), "{refused}");
        assert_eq!(*log.borrow(), Vec::<String>::new(), "a refusal writes nothing and checks nothing");
        let relisted = lock(NEW, "0.36.2").replace(" \"egui\",\n", "");
        assert_eq!(settle(&before, &relisted, &write, check).unwrap(), ["local 0.1.0"]);
        assert_eq!(*log.borrow(), [format!("write {}", lock(NEW, "0.36.2").len()), "check".to_string()]);
        let unresolved = settle(&before, &relisted, &write, || Err("cargo metadata failed".to_string())).unwrap_err();
        assert!(unresolved.starts_with("cargo metadata failed\nbump: with the other packages kept as they were"), "{unresolved}");
        log.borrow_mut().clear();
        let reformatted = lock(NEW, "0.37.0").replace("version = 4", "version = 5");
        let format = settle(&before, &reformatted, &write, check).unwrap_err();
        assert!(
            format.starts_with("bump: `cargo update` wrote the lock file in another format"),
            "a format change is not a dependency change: {format}"
        );
        assert_eq!(*log.borrow(), Vec::<String>::new(), "and is refused before anything is written");
    }

    /// Every failure after `cargo update` writes the old lock back, and the error says whether that
    /// worked; a success writes nothing.
    #[test]
    fn a_failed_bump_puts_the_old_lock_back() {
        let written = RefCell::new(Vec::<String>::new());
        let write = |text: &str| -> Result<(), String> {
            written.borrow_mut().push(text.to_string());
            Ok(())
        };
        assert_eq!(restoring("old", &write, || Ok("new".to_string())).unwrap(), "new");
        assert_eq!(*written.borrow(), Vec::<String>::new());
        let error = restoring("old", &write, || Err::<String, _>("bump: refused".to_string())).unwrap_err();
        assert_eq!(error, "bump: refused\nbump: Cargo.lock is back as it was");
        assert_eq!(*written.borrow(), ["old"]);
        // The whole path: the kept lock is written, its check fails, and the old lock follows it.
        let (before, relisted) = (lock(OLD, "0.36.2"), lock(NEW, "0.36.2").replace(" \"egui\",\n", ""));
        written.borrow_mut().clear();
        let error = restoring(&before, &write, || settle(&before, &relisted, &write, || Err("no".to_string())).map(|_| String::new())).unwrap_err();
        assert!(error.ends_with("pull request\nbump: Cargo.lock is back as it was"), "{error}");
        assert_eq!(*written.borrow(), [lock(NEW, "0.36.2"), before.clone()]);
        let failing = |_: &str| -> Result<(), String> { Err("disk full".to_string()) };
        let error = restoring("old", &failing, || Err::<String, _>("bump: refused".to_string())).unwrap_err();
        assert_eq!(
            error,
            "bump: refused\nbump: could not put the lock back as it was: disk full; `git checkout -- Cargo.lock` puts back the committed one"
        );
    }

    /// The whole bump against a lock held in memory: what it prints, and the lock it leaves, on a
    /// move, a refusal, a check that fails, an update that fails, a CRLF lock and a lock already new.
    #[test]
    fn a_bump_leaves_the_lock_moved_or_as_it_was() {
        /// Runs a bump on `lock`, where `cargo update` writes `updated` and then fails unless
        /// `update_ok`, and the check says `resolves`; returns the result and the lock left behind.
        fn bump_on(lock: &str, updated: &str, update_ok: bool, resolves: bool) -> (Result<Vec<String>, String>, String) {
            let file = RefCell::new(lock.to_string());
            let read = || -> Result<String, String> { Ok(file.borrow().clone()) };
            let write = |text: &str| -> Result<(), String> {
                *file.borrow_mut() = text.to_string();
                Ok(())
            };
            let update = |_: &[String]| -> Result<(), String> {
                *file.borrow_mut() = updated.to_string();
                if update_ok { Ok(()) } else { Err("bump: `cargo update` could not move craft-atomic".to_string()) }
            };
            let check = || -> Result<(), String> { if resolves { Ok(()) } else { Err("cargo metadata failed".to_string()) } };
            let result = bump(&Io { read: &read, write: &write, update: &update, check: &check });
            (result, file.into_inner())
        }
        let (old, new) = (lock(OLD, "0.36.2"), lock(NEW, "0.36.2"));
        let relisted = new.replace(" \"egui\",\n", "");
        let (result, left) = bump_on(&old, &relisted, true, true);
        assert_eq!(
            result.unwrap(),
            ["kept the dependency lists `cargo update` re-chose for local 0.1.0 as Cargo.lock had them", "craft-atomic d40c2be0aaaa -> 1a2b3c4d5e6f"]
        );
        assert_eq!(left, new, "craft-libs moved, the rest as it was");
        let (result, left) = bump_on(&old, &lock(NEW, "0.37.0"), true, true);
        assert!(result.unwrap_err().ends_with("\nbump: Cargo.lock is back as it was"));
        assert_eq!(left, old, "a refusal leaves the old lock");
        let (result, left) = bump_on(&old, &relisted, true, false);
        assert!(result.unwrap_err().starts_with("cargo metadata failed\n"));
        assert_eq!(left, old, "a failed check leaves the old lock");
        // A `cargo update` that fails, untouched lock or cut short, leaves the old lock too.
        for written in [old.as_str(), "# This file is automatically @gen"] {
            let (result, left) = bump_on(&old, written, false, true);
            assert_eq!(result.unwrap_err(), "bump: `cargo update` could not move craft-atomic\nbump: Cargo.lock is back as it was");
            assert_eq!(left, old, "a failed update leaves the old lock");
        }
        let crlf = |text: &str| text.replace('\n', "\r\n");
        let (result, left) = bump_on(&crlf(&old), &relisted, true, true);
        assert!(result.is_ok());
        assert_eq!(left, crlf(&new), "a CRLF lock stays CRLF");
        let (result, left) = bump_on(&new, &new, true, true);
        assert_eq!(result.unwrap(), ["the craft-libs crates are at the newest main already, 1a2b3c4d5e6f77777777777777777777777777"]);
        assert_eq!(left, new);
    }
}
