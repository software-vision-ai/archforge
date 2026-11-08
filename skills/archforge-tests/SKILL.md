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
  - the native menu bar's queue, wake and items: the `tests` module in
    `apps/archforge/src/macos_menu.rs`, without a real menu (it needs the main thread)
  - the task runner: a `tests` module in the `xtask/src` file it checks
  - the project database's server: unit tests beside the code in `crates/dolt/src`, and
    `crates/dolt/tests/server.rs` against a real dolt
  - projects on disk: `crates/project/tests/project.rs` against a real dolt, a project's model in
    `crates/project/tests/model.rs`, its conversations in `crates/project/tests/chat.rs`; the app's
    projects host, and its store of conversations (`apps/archforge/src/chats.rs`) against a real
    dolt: the `tests` module in `apps/archforge/src/projects.rs`
  - the Chat panel: `crates/ui-egui/src/tests.rs`, built through `crate::chat_fixture` over the
    agents host's fake and `chat_store::fake::Memory`, never a real program or `~/.claude` or
    `~/.codex`
  - the app's agents host (conversations, agent access, the test message, Codex's account):
    `apps/archforge/src/agents/tests.rs`, the real host against fake `claude` and `codex` scripts
    replaying `crates/agents/tests/fixtures/`, keys as canaries in a `MemoryStore`, never an
    installed program, a real key, `~/.claude` or `~/.codex`; a race on a conversation's private
    token that the host cannot drive reliably is tested beside it, in `sessions.rs`'s
    `revoke_tests` through a `#[cfg(test)]` hook
  - the control channel's tokens, roles and revocation: `crates/control/src/tests.rs`; what the
    window lets a read-only token do: the `tests` module in `crates/ui-egui/src/control.rs`
  - the shell's process and network scan: the `tests` module in `xtask/src/shell_io.rs`
  - name rules: the case table in `crates/names/src/lib.rs`
  - API keys: `crates/secrets/src/tests.rs`, against `MemoryStore`; another crate's tests use
    `archforge_secrets::MemoryStore` (feature `testing`), never the system's store
  - agent runtimes: unit tests beside the code in `crates/agents/src`, and
    `crates/agents/tests/runtimes.rs` against fake runtimes (shell scripts), never an installed
    `claude` or `codex`: give `find` a whole `HostEnv` so the machine's own PATH is not searched;
    the Claude adapter in `crates/agents/tests/claude.rs`, over the transcripts in
    `crates/agents/tests/fixtures/` and fake `claude` scripts that replay them; the Codex adapter in
    `crates/agents/tests/codex.rs`, over `crates/agents/tests/fixtures/codex/`, the schema cut in
    `crates/agents/schema/` and fake `codex` scripts, never an installed `codex` or `~/.codex`
  - the command engine: `crates/engine/tests/engine.rs` with its toy notation; a notation's own
    commands are checked with `archforge_engine::testing::round_trip` (feature `testing`)
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
- Before a release: `docs/release-checklist.md`, by hand in the running app on macOS, for what
  the gate cannot drive (the menu bar, system pickers, the browser, the real window). A change to
  such behaviour updates that list.
- Before proposing a change: the gate, `cargo xtask ci`. Its steps are update (craft-libs to their
  newest main), format, lint, test, assets, skills, layers, portable, brands and deny. It stops at the
  first failure and prints a table of steps.
- The test step needs dolt 2.6 or later on the PATH or in `ARCHFORGE_DOLT`; without it the
  server and project tests fail and say so. A test that starts a server stops it on every path,
  so a failed run leaves no server behind.
- The portable step needs the `wasm32-unknown-unknown` target (`rustup target add wasm32-unknown-unknown`)
  and a cargo from rustup on `PATH`; the deny step needs cargo-deny at the version `xtask/src/main.rs`
  names; both the update and deny steps need the network.
- The brands step runs only where `FORGE_BRANDS` names a list; the gate says when it skipped it.
- `cargo xtask stats` prints the tests and lines per crate.
