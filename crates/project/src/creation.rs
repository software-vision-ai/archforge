//! The branch creation record: a file in the project folder, beside the [`crate::MARKER`], naming
//! the branch being made, the branch it is made from and the commit it starts at. This module
//! writes, reads and removes such a file and sets a damaged one aside. Nothing calls it yet.
//!
//! The file is a claim, read as one. Only the exact shape, with every name passing
//! [`SafeBranch::found`], counts as a record; anything else is damaged, and is not to be acted on.
//! The start commit is kept so that an unfinished branch can be told from a finished one that has
//! moved on. It is no defence against someone who can edit the folder: they can write a record
//! naming any branch and the commit it is at, and they could as well delete the database.
//!
//! It is four fixed lines, not JSON. This crate has no JSON reader outside its tests, and a repeated
//! key in JSON can be read without complaint, the last one winning, so a file could say two things.
//! Here a repeated, unknown or reordered line is simply the wrong shape.
//!
//! Writing and removing flush what they do, the folder included. The tests show those calls are
//! made and in which order; no test can show that a flush reached the disk, or that the disk kept
//! it through a power cut. For that this rests on the operating system and the disk.

use std::fs::{self, File, OpenOptions};
use std::io::{self, ErrorKind, Read, Write};
use std::path::{Path, PathBuf};

use crate::chat::is_commit;
use crate::{Error, SafeBranch, UnsafeBranch};

/// The record's file in the project folder.
pub(crate) const RECORD: &str = ".creating-branch";
/// Where a record is made before it takes its name; never read as a record.
const TEMPORARY: &str = ".creating-branch.tmp";
/// A record's first line, which says this is one and of which shape.
const FIRST_LINE: &str = "archforge branch creation 1";
/// More than the longest good record; a longer file is damaged and is not read in full.
const MAX_BYTES: u64 = 2048;
/// The most `.damaged-<n>` names tried.
const DAMAGED_TRIES: u32 = 100;

/// A branch being made, as the record says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CreationRecord {
    pub(crate) branch: SafeBranch,
    pub(crate) source: SafeBranch,
    pub(crate) start: String,
}

/// Why a record cannot be used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RecordDamage {
    /// A link, a folder or anything else that is not a regular file.
    NotAFile,
    TooLong,
    NotText,
    /// Not the lines a record has: one is missing, extra, repeated or out of order.
    Shape,
    Branch(UnsafeBranch),
    Source(UnsafeBranch),
    /// The start is not a commit hash.
    Start,
    /// It is there and could not be read.
    Unreadable(ErrorKind),
}

/// What the project folder holds.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Found {
    None,
    Good(CreationRecord),
    Damaged(RecordDamage),
}

/// A step whose order makes a record safe, so that tests can watch and fail it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Step {
    Temporary,
    FlushTemporary,
    Link,
    Remove,
    FlushFolder,
}

/// Told of each step before it runs; an error stands in for the step's own.
type Watch<'a> = &'a mut dyn FnMut(Step) -> io::Result<()>;

/// Runs `action` as `step`.
fn step(watch: Watch, step: Step, action: impl FnOnce() -> io::Result<()>) -> io::Result<()> {
    watch(step)?;
    action()
}

impl CreationRecord {
    /// A record of `branch` made from `source` at the commit `start`; none if `start` is no hash.
    pub(crate) fn new(branch: SafeBranch, source: SafeBranch, start: &str) -> Option<CreationRecord> {
        is_commit(start).then(|| CreationRecord { branch, source, start: start.to_string() })
    }

    /// The record as the file holds it.
    fn text(&self) -> String {
        format!("{FIRST_LINE}\nbranch {}\nsource {}\nstart {}\n", self.branch.stored(), self.source.stored(), self.start)
    }

    /// The record `text` holds, in exactly the shape [`CreationRecord::text`] gives.
    fn parse(text: &str) -> Result<CreationRecord, RecordDamage> {
        let mut lines = text.strip_suffix('\n').ok_or(RecordDamage::Shape)?.split('\n');
        if lines.next() != Some(FIRST_LINE) {
            return Err(RecordDamage::Shape);
        }
        let mut field = |label: &str| lines.next().and_then(|l| l.strip_prefix(label)).ok_or(RecordDamage::Shape);
        let (branch, source, start) = (field("branch ")?, field("source ")?, field("start ")?);
        if lines.next().is_some() {
            return Err(RecordDamage::Shape);
        }
        let branch = SafeBranch::found(branch).map_err(RecordDamage::Branch)?;
        let source = SafeBranch::found(source).map_err(RecordDamage::Source)?;
        CreationRecord::new(branch, source, start).ok_or(RecordDamage::Start)
    }
}

/// Reads the record in `folder`, changing nothing: what to do with a damaged one is the caller's.
pub(crate) fn read(folder: &Path) -> Found {
    let path = folder.join(RECORD);
    let unreadable = |e: io::Error| if e.kind() == ErrorKind::NotFound { Found::None } else { Found::Damaged(RecordDamage::Unreadable(e.kind())) };
    let meta = match fs::symlink_metadata(&path) {
        Ok(meta) => meta,
        Err(e) => return unreadable(e),
    };
    if !meta.is_file() {
        return Found::Damaged(RecordDamage::NotAFile);
    }
    let mut bytes = Vec::new();
    if let Err(e) = File::open(&path).and_then(|f| f.take(MAX_BYTES + 1).read_to_end(&mut bytes)) {
        return unreadable(e);
    }
    if bytes.len() as u64 > MAX_BYTES {
        return Found::Damaged(RecordDamage::TooLong);
    }
    match String::from_utf8(bytes) {
        Err(_) => Found::Damaged(RecordDamage::NotText),
        Ok(text) => CreationRecord::parse(&text).map_or_else(Found::Damaged, Found::Good),
    }
}

/// Writes `record` into `folder` unless something has its name: whole and flushed under a temporary
/// name, linked to its own name, which fails if it is taken, and the folder flushed. Answers whether
/// it was written; something already there is left as it was. An error means no creation may go on:
/// a record linked but not flushed is removed, and if even that fails it stays, naming no branch yet.
pub(crate) fn write(folder: &Path, record: &CreationRecord) -> Result<bool, Error> {
    write_watched(folder, record, &mut |_| Ok(()))
}

/// [`write`], telling `watch` of each step.
fn write_watched(folder: &Path, record: &CreationRecord, watch: Watch) -> Result<bool, Error> {
    let (path, temporary) = (folder.join(RECORD), folder.join(TEMPORARY));
    // A stop can leave one behind; it was never the record.
    clear(&temporary).map_err(|e| Error::io("cannot clear", &temporary, e))?;
    let made = step(watch, Step::Temporary, || File::create_new(&temporary)?.write_all(record.text().as_bytes()))
        .and_then(|()| step(watch, Step::FlushTemporary, || OpenOptions::new().write(true).open(&temporary)?.sync_all()));
    let linked = made.map(|()| step(watch, Step::Link, || fs::hard_link(&temporary, &path)));
    if let Err(e) = clear(&temporary) {
        log::warn!("the temporary record {} stays until the next creation: {e}", temporary.display());
    }
    match linked.map_err(|e| Error::io("cannot write", &temporary, e))? {
        Ok(()) => {}
        Err(e) if e.kind() == ErrorKind::AlreadyExists => return Ok(false),
        Err(e) => return Err(Error::io("cannot write", &path, e)),
    }
    if let Err(e) = step(watch, Step::FlushFolder, || flush_folder(folder)) {
        // Not known to last, so no creation may go on under it. One left here names no branch yet.
        if let Err(r) = fs::remove_file(&path) {
            log::warn!("the record {} stays until the project opens again: {r}", path.display());
        }
        return Err(Error::io("cannot flush", folder, e));
    }
    Ok(true)
}

/// Removes the record from `folder` and flushes that, as a record that came back would delete a
/// finished branch. A record already gone is no error, and the folder is flushed all the same: a
/// stop before the first flush leaves exactly that.
pub(crate) fn remove(folder: &Path) -> Result<(), Error> {
    remove_watched(folder, &mut |_| Ok(()))
}

/// [`remove`], telling `watch` of each step.
fn remove_watched(folder: &Path, watch: Watch) -> Result<(), Error> {
    let path = folder.join(RECORD);
    step(watch, Step::Remove, || clear(&path)).map_err(|e| Error::io("cannot remove", &path, e))?;
    step(watch, Step::FlushFolder, || flush_folder(folder)).map_err(|e| Error::io("cannot flush", folder, e))
}

/// Moves whatever has the record's name, a link or folder too, to a free `.damaged-<n>` name and
/// says which. The name is claimed first, by making something there exclusively, so an earlier
/// damaged record is never replaced.
pub(crate) fn set_aside(folder: &Path) -> Result<PathBuf, Error> {
    let path = folder.join(RECORD);
    let is_dir = fs::symlink_metadata(&path).map_err(|e| Error::io("cannot set aside", &path, e))?.is_dir();
    for n in 1..=DAMAGED_TRIES {
        let aside = folder.join(format!("{RECORD}.damaged-{n}"));
        match if is_dir { fs::create_dir(&aside) } else { File::create_new(&aside).map(drop) } {
            Ok(()) => {}
            Err(e) if e.kind() == ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(Error::io("cannot claim", &aside, e)),
        }
        return match fs::rename(&path, &aside) {
            Ok(()) => Ok(aside),
            Err(e) => {
                let unclaimed = if is_dir { fs::remove_dir(&aside) } else { fs::remove_file(&aside) };
                if let Err(r) = unclaimed {
                    log::warn!("the claimed name {} stays: {r}", aside.display());
                }
                Err(Error::io("cannot set aside", &path, e))
            }
        };
    }
    Err(Error::io("cannot set aside", &path, io::Error::other("every damaged name is taken")))
}

/// Removes the file at `path`, which may be gone already.
fn clear(path: &Path) -> io::Result<()> {
    fs::remove_file(path).or_else(|e| if e.kind() == ErrorKind::NotFound { Ok(()) } else { Err(e) })
}

/// Makes the folder's list of files last.
#[cfg(unix)]
fn flush_folder(folder: &Path) -> io::Result<()> {
    File::open(folder)?.sync_all()
}

/// Does nothing: a folder cannot be opened to flush it here, so the file system alone decides when
/// its list of files lasts.
#[cfg(not(unix))]
fn flush_folder(_folder: &Path) -> io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    const HASH: &str = "0123456789abcdefghijklmnopqrstuv";

    /// A folder of its own, claimed exclusively and removed on every path.
    struct Folder(PathBuf);

    impl Folder {
        /// Claims a new folder.
        fn new() -> Folder {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            loop {
                let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
                let path = std::env::temp_dir().join(format!("af-record-{now}-{}", NEXT.fetch_add(1, Ordering::SeqCst)));
                match fs::create_dir(&path) {
                    Err(e) if e.kind() == ErrorKind::AlreadyExists => continue,
                    made => return made.map(|()| Folder(path)).unwrap(),
                }
            }
        }

        /// The record's path.
        fn record(&self) -> PathBuf {
            self.0.join(RECORD)
        }

        /// Whether the temporary file is there.
        fn temporary(&self) -> bool {
            self.0.join(TEMPORARY).exists()
        }
    }

    impl Drop for Folder {
        /// Removes the folder and all in it.
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    /// The branch named `name`.
    fn branch(name: &str) -> SafeBranch {
        SafeBranch::found(name).unwrap()
    }

    /// A record of the branch `name`.
    fn record(name: &str) -> CreationRecord {
        CreationRecord::new(branch(name), branch("from"), HASH).unwrap()
    }

    /// The text of a record, each part as given.
    fn text(name: &str, source: &str, start: &str) -> String {
        format!("{FIRST_LINE}\nbranch {name}\nsource {source}\nstart {start}\n")
    }

    /// A record is read field by field, the longest names too; no file is no record.
    #[test]
    fn a_good_record_is_read_field_by_field() {
        let folder = Folder::new();
        assert_eq!(read(&folder.0), Found::None, "no record, no cleanup");
        fs::write(folder.record(), text("made", "from", HASH)).unwrap();
        let Found::Good(found) = read(&folder.0) else { panic!("a good record") };
        assert_eq!((found.branch.stored(), found.source.stored(), found.start.as_str()), ("made", "from", HASH));
        let (long, longer) = ("a".repeat(512), "b".repeat(512));
        fs::write(folder.record(), text(&long, &longer, HASH)).unwrap();
        assert_eq!(read(&folder.0), Found::Good(CreationRecord::new(branch(&long), branch(&longer), HASH).unwrap()));
    }

    /// Every file that is not exactly a record is damaged, by its own reason, and stays to be set aside.
    #[test]
    fn a_hostile_record_is_damaged_by_its_own_reason() {
        use RecordDamage::*;
        use UnsafeBranch::{Character, Empty, LooksLikeCommit, NotABranchName};
        let good = |b: &str, s: &str, t: &str| text(b, s, t).into_bytes();
        let cases: Vec<(&str, Vec<u8>, RecordDamage)> = vec![
            ("empty", Vec::new(), Shape),
            ("not a record", b"{ \"branch\": \"main\" }\n".to_vec(), Shape),
            ("at the cap", vec![b'x'; 2048], Shape),
            ("past the cap", vec![b'x'; 2049], TooLong),
            ("a good record past the cap", [good("a", "b", HASH), vec![b'\n'; 2048]].concat(), TooLong),
            ("not text", b"\xff\xfe\n".to_vec(), NotText),
            ("no last line break", good("a", "b", HASH).split_last().unwrap().1.to_vec(), Shape),
            ("another version", format!("archforge branch creation 2\nbranch a\nsource b\nstart {HASH}\n").into_bytes(), Shape),
            ("a field missing", format!("{FIRST_LINE}\nbranch a\nstart {HASH}\n").into_bytes(), Shape),
            ("a field repeated", format!("{FIRST_LINE}\nbranch a\nbranch c\nsource b\nstart {HASH}\n").into_bytes(), Shape),
            ("fields out of order", format!("{FIRST_LINE}\nsource b\nbranch a\nstart {HASH}\n").into_bytes(), Shape),
            ("a field unknown", [good("a", "b", HASH), b"also x\n".to_vec()].concat(), Shape),
            ("windows line ends", format!("{FIRST_LINE}\r\nbranch a\r\nsource b\r\nstart {HASH}\r\n").into_bytes(), Shape),
            ("empty branch", good("", "b", HASH), Branch(Empty)),
            ("a path", good("../x", "b", HASH), Branch(Character('/'))),
            ("backtick", good("a`b", "b", HASH), Branch(Character('`'))),
            ("quote", good("a'b", "b", HASH), Branch(Character('\''))),
            ("space after", good("a ", "b", HASH), Branch(Character(' '))),
            ("hash shaped", good(HASH, "b", HASH), Branch(LooksLikeCommit)),
            ("head", good("HEAD", "b", HASH), Branch(NotABranchName)),
            ("bad source", good("a", "b`c", HASH), Source(Character('`'))),
            ("empty source", good("a", "", HASH), Source(Empty)),
            ("short start", good("a", "b", "abc"), Start),
            ("capitals in the start", good("a", "b", &HASH.to_uppercase()), Start),
            ("a letter past v", good("a", "b", &format!("w{}", &HASH[1..])), Start),
            ("a statement as the start", good("a", "b", "'; DROP DATABASE project; --"), Start),
        ];
        for (name, bytes, why) in cases {
            let folder = Folder::new();
            fs::write(folder.record(), &bytes).unwrap();
            assert_eq!(read(&folder.0), Found::Damaged(why), "{name}");
            assert_eq!(fs::read(folder.record()).unwrap(), bytes, "{name}: reading changes nothing");
        }
    }

    /// A link is not followed, however good the record it reaches, and a folder is no record.
    #[cfg(unix)]
    #[test]
    fn a_link_or_folder_is_damaged() {
        let (folder, elsewhere) = (Folder::new(), Folder::new());
        fs::write(elsewhere.record(), text("made", "from", HASH)).unwrap();
        std::os::unix::fs::symlink(elsewhere.record(), folder.record()).unwrap();
        assert_eq!(read(&folder.0), Found::Damaged(RecordDamage::NotAFile));
        let aside = set_aside(&folder.0).unwrap();
        assert!(fs::symlink_metadata(&aside).unwrap().is_symlink(), "the link moved, not what it reaches");
        assert!(matches!(read(&elsewhere.0), Found::Good(_)));
        std::os::unix::fs::symlink(&elsewhere.0, folder.record()).unwrap();
        assert!(fs::symlink_metadata(set_aside(&folder.0).unwrap()).unwrap().is_symlink(), "a link to a folder is a link");
        fs::create_dir(folder.record()).unwrap();
        fs::write(folder.record().join("inside"), b"x").unwrap();
        assert_eq!(read(&folder.0), Found::Damaged(RecordDamage::NotAFile));
        assert!(set_aside(&folder.0).unwrap().join("inside").is_file());
        assert_eq!(read(&folder.0), Found::None);
    }

    /// A file that is there and cannot be read is damaged, not absent.
    #[cfg(unix)]
    #[test]
    fn an_unreadable_record_is_damaged() {
        use std::os::unix::fs::PermissionsExt;
        let folder = Folder::new();
        fs::write(folder.record(), text("made", "from", HASH)).unwrap();
        fs::set_permissions(folder.record(), fs::Permissions::from_mode(0o000)).unwrap();
        // The superuser reads it all the same, so there is nothing to show.
        if File::open(folder.record()).is_err() {
            assert_eq!(read(&folder.0), Found::Damaged(RecordDamage::Unreadable(ErrorKind::PermissionDenied)));
        }
    }

    /// What is written is read back as it was, the longest names too.
    #[test]
    fn a_record_written_reads_back() {
        let folder = Folder::new();
        assert!(write(&folder.0, &record("made")).unwrap());
        assert_eq!(fs::read(folder.record()).unwrap(), text("made", "from", HASH).into_bytes());
        assert_eq!(read(&folder.0), Found::Good(record("made")));
        assert!(!folder.temporary());
        remove(&folder.0).unwrap();
        assert_eq!(read(&folder.0), Found::None);
        let longest = CreationRecord::new(branch(&"a".repeat(512)), branch(&"b".repeat(512)), HASH).unwrap();
        assert!(write(&folder.0, &longest).unwrap());
        assert_eq!(read(&folder.0), Found::Good(longest));
    }

    /// Whatever has the record's name stays as it was, and no creation goes on.
    #[test]
    fn a_second_record_is_refused_and_the_first_kept() {
        let folder = Folder::new();
        assert!(write(&folder.0, &record("first")).unwrap());
        assert!(!write(&folder.0, &record("second")).unwrap());
        assert_eq!(read(&folder.0), Found::Good(record("first")));
        assert!(!folder.temporary());
        let held = Folder::new();
        fs::create_dir(held.record()).unwrap();
        assert!(!write(&held.0, &record("made")).unwrap());
        assert!(held.record().is_dir());
    }

    /// The steps run in order, each seeing what the one before it left. This shows the flushes are
    /// asked for and when; no test can show that one reached the disk, or that the disk kept it.
    #[test]
    fn a_write_flushes_the_whole_record_before_it_has_its_name_and_the_folder_after() {
        let folder = Folder::new();
        let mut steps = Vec::new();
        let whole = record("made").text().into_bytes();
        let temporary = folder.0.join(TEMPORARY);
        let written = write_watched(&folder.0, &record("made"), &mut |s| {
            assert_eq!(folder.temporary(), matches!(s, Step::FlushTemporary | Step::Link), "{s:?}");
            if matches!(s, Step::FlushTemporary | Step::Link) {
                assert_eq!(fs::read(&temporary).unwrap(), whole, "{s:?}: the whole record");
            }
            assert_eq!(folder.record().exists(), s == Step::FlushFolder, "{s:?}");
            steps.push(s);
            Ok(())
        });
        assert!(written.unwrap());
        assert_eq!(steps, [Step::Temporary, Step::FlushTemporary, Step::Link, Step::FlushFolder]);
    }

    /// A record not known to be on disk does not stay to be acted on.
    #[test]
    fn a_write_that_fails_at_any_step_leaves_no_record() {
        for failing in [Step::Temporary, Step::FlushTemporary, Step::Link, Step::FlushFolder] {
            let folder = Folder::new();
            let answer = write_watched(&folder.0, &record("made"), &mut |s| if s == failing { Err(io::Error::other("refused")) } else { Ok(()) });
            assert!(answer.is_err(), "{failing:?}");
            assert_eq!(read(&folder.0), Found::None, "{failing:?}");
            assert!(!folder.temporary(), "{failing:?}");
        }
    }

    /// What a stop leaves under the temporary name is no record and does not hold up the next.
    #[test]
    fn a_temporary_file_left_by_a_stop_is_no_record() {
        let folder = Folder::new();
        fs::write(folder.0.join(TEMPORARY), record("stopped").text()).unwrap();
        assert_eq!(read(&folder.0), Found::None);
        assert!(write(&folder.0, &record("next")).unwrap());
        assert_eq!(read(&folder.0), Found::Good(record("next")));
        let blocked = Folder::new();
        fs::create_dir(blocked.0.join(TEMPORARY)).unwrap();
        assert!(write(&blocked.0, &record("made")).is_err(), "what cannot be cleared stops it");
        assert_eq!(read(&blocked.0), Found::None);
    }

    /// Removal is flushed after it, repeats safely, and a failure says so.
    #[test]
    fn a_removal_is_flushed_and_repeats_safely() {
        let folder = Folder::new();
        assert!(write(&folder.0, &record("made")).unwrap());
        let refuse = |failing: Step| move |s: Step| if s == failing { Err(io::Error::other("refused")) } else { Ok(()) };
        assert!(remove_watched(&folder.0, &mut refuse(Step::Remove)).is_err());
        assert_eq!(read(&folder.0), Found::Good(record("made")), "a failed removal keeps the record");
        let mut steps = Vec::new();
        for _ in 0..2 {
            let removed = remove_watched(&folder.0, &mut |s| {
                assert_eq!(steps.is_empty(), folder.record().exists(), "{s:?}");
                steps.push(s);
                Ok(())
            });
            removed.unwrap();
        }
        assert_eq!(steps, [Step::Remove, Step::FlushFolder, Step::Remove, Step::FlushFolder]);
        assert!(remove_watched(&folder.0, &mut refuse(Step::FlushFolder)).is_err(), "an unflushed removal is a failed one");
    }

    /// Set aside whole under a free name; an earlier one is never replaced.
    #[test]
    fn a_damaged_record_never_replaces_an_earlier_one() {
        let folder = Folder::new();
        let earlier = folder.0.join(format!("{RECORD}.damaged-1"));
        fs::write(&earlier, b"earlier").unwrap();
        fs::write(folder.record(), b"broken").unwrap();
        let aside = set_aside(&folder.0).unwrap();
        assert_eq!(aside, folder.0.join(format!("{RECORD}.damaged-2")));
        assert_eq!((fs::read(&earlier).unwrap(), fs::read(&aside).unwrap()), (b"earlier".to_vec(), b"broken".to_vec()));
        assert_eq!(read(&folder.0), Found::None);
        for n in 3..=DAMAGED_TRIES {
            fs::write(folder.0.join(format!("{RECORD}.damaged-{n}")), b"").unwrap();
        }
        fs::write(folder.record(), b"again").unwrap();
        assert!(set_aside(&folder.0).is_err(), "every name taken");
        assert_eq!(fs::read(folder.record()).unwrap(), b"again");
    }
}
