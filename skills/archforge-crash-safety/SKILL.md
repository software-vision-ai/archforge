---
name: archforge-crash-safety
description: "Use when writing or reviewing non-test Rust in Archforge, reading anything from outside the process (settings, files, fonts, arguments), writing a file, or investigating a crash. A crash loses someone's work, so this outranks feature work."
---

# Never crash

People will trust Archforge with their models. This outranks feature work.

## In non-test code
- No `unwrap()`, `expect()`, `panic!`, `unreachable!`, `todo!` or `unimplemented!`. Every crate root
  (each `lib.rs` and each binary's `main.rs`) carries the `#![deny(...)]` line `AGENTS.md` gives, and
  the gate's lint step runs clippy with `-D warnings`. A new crate copies that line.
- Errors are `Result` with a message that names the offending value and the accepted choices, passed
  up with `?`. An unfinished feature says "not available yet"; it never panics.
- Input-derived numbers are hostile: `get()` rather than `[i]`, checked or saturating arithmetic,
  `try_from` rather than `as`, slices only at character boundaries, and a cap on total bytes before
  anything is allocated. Every loop driven by input has a budget or a seen-set.
- A request out of range is refused, not clamped.

## The patterns already in the code
- The panic hook in `apps/archforge/src/main.rs` sends a panic's message and place to the log, and
  nothing on that path can panic again.
- Preferences (`apps/archforge/src/prefs.rs`): a file too long, damaged or not text is set aside
  under a new name and said in the log; it is never overwritten unseen. The defaults take over.
- Writes go through `craft_atomic::atomic_write`: a temporary file in the same folder, synchronised,
  then renamed over the target, so a crash leaves the old file or the new one.
- The graphics marker (`apps/archforge/src/graphics.rs`) records a start-up that crashed, so the
  next start skips that graphics choice.
- A font egui cannot load is left out and said in the log (`crates/ui-egui/src/fonts.rs`,
  `crates/ui-egui/src/ux/mod.rs`); text waits for `ux::ready`.
- The interface never touches the file system or the operating system: the host injects
  `Services` (`crates/ui-egui/src/lib.rs`, `apps/archforge/src/services.rs`).

## Proving a crash fix
Every crash fix ships a regression test that failed before the fix, in the test file of its area
(`skills/archforge-tests/SKILL.md`). Start it from the smallest input that reproduces the crash.

## When someone says the app crashed
Read the log first: `logs/archforge.log` in the settings folder (`apps/archforge/src/app_dirs.rs`),
rotated as `apps/archforge/src/logging.rs` describes. The panic hook puts the message and its
source line there.
