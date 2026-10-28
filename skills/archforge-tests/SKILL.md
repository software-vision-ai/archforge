---
name: archforge-tests
description: "Use when writing, placing or running tests in Archforge: a regression test for a bug or crash, a harness test for a screen, a catalogue test, a task-runner test, or when the gate fails a step you did not touch."
---

# Where tests go and how to run them

## Placement
- A test goes into the existing test file of its area, never a new file named after the bug:
  - the shell and its screens: `crates/ui-egui/src/tests.rs`
  - translations: `crates/ui-egui/src/i18n/tests.rs`
  - the desktop app (arguments, preferences, graphics): the `tests` module in `apps/archforge/src/main.rs`
  - the task runner: a `tests` module in the `xtask/src` file it checks
  - the project database's server: unit tests beside the code in `crates/dolt/src`, and
    `crates/dolt/tests/server.rs` against a real dolt
- A screen test builds the real `Shell` in an `egui_kittest` harness, draws it in both appearances
  and finds controls by automation id (`harness` and `click` in `crates/ui-egui/src/tests.rs`).
  Every harness pins the language with `set_language`; `every_harness_pins_the_language` fails a
  harness that does not.
- A test that touches the disk works in its own folder in the temporary directory, named with the
  process id, and removes it at the end.
- Every crash fix ships a regression test that failed before the fix
  (`skills/archforge-crash-safety/SKILL.md`).

## Rules
- Test the behaviour the user sees, through the public items, not the private steps.
- No test depends on another's state, on the wall clock, or on fonts installed on the machine; the
  installed-font tests give their own sources.
- A failing test lands with its fix in the same change.

## Running
- While working: `cargo test -p archforge-ui-egui`, `cargo test -p archforge` or `cargo test -p xtask`,
  with a name filter after it.
- Before proposing a change: the gate, `cargo xtask ci`. Its steps are update (craft-libs to their
  newest main), format, lint, test, assets, skills, layers, portable, brands and deny. It stops at the
  first failure and prints a table of steps.
- The test step needs dolt 2.6 or later on the PATH or in `ARCHFORGE_DOLT`; without it the
  server tests fail and say how to install it. A test that starts a server stops it on every path,
  so a failed run leaves no server behind.
- The portable step needs the `wasm32-unknown-unknown` target (`rustup target add wasm32-unknown-unknown`)
  and a cargo from rustup on `PATH`; the deny step needs cargo-deny at the version `xtask/src/main.rs`
  names; both the update and deny steps need the network.
- The brands step runs only where `FORGE_BRANDS` names a list; the gate says when it skipped it.
- `cargo xtask stats` prints the tests and lines per crate.
