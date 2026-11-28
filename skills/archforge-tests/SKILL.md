---
name: archforge-tests
description: "Use when writing, placing or running tests in Archforge: a regression test for a bug or crash, a harness test for a screen, a catalogue test, a task-runner test, or when the gate fails a step you did not touch."
---

# Where tests go and how to run them

## Placement
- A test goes into the existing test file of its area, never a new file named after the bug:
  - the shell and its screens: `crates/ui-egui/src/tests.rs`, on the fake notation (its model
    `archforge_notations::fake`, behind `archforge-notations`' feature `testing`, and the sketch's
    screens part, `SCREENS` in that file), never ER or UML (ADR 0005); the gate's `layers` step
    refuses a link from the shell to a notation crate and a notation's command name in its files
    (rules 3 and 5 of `xtask/src/notation_rules.rs`)
  - the fake notation itself, its rows saved and opened again: `crates/notations/tests/fake.rs`
  - UML's screens without the shell, what UML's own code draws or decides (its refusal words, its
    hooks, its Properties bodies and editors), against the shell's interfaces' test context:
    `crates/uml-ui/src/tests.rs`
  - UML's window tests, the whole window with the notations the app registers (ADR 0005, whole-app
    behaviour): the app's `apps/archforge/tests/uml_window/`, through ui-egui's `testing` and UML's
    screens' `snapshot`, in the module of their area beside `main.rs` (the
    canvas, its tools and tabs, and relationships; Properties and its keys; Models; profiles and
    stereotypes; Chat; the menus and the rail's panels; the views of a version; UML's refusals in
    the chosen language)
  - ER's screens without the shell, what ER's own code decides (its refusal words), against the
    shell's interfaces' test context: `crates/er-ui/src/tests.rs`
  - ER's window tests, the whole window with the notations the app registers, and the tests that
    ask both notations: the app's `apps/archforge/tests/er_window/`, through ui-egui's `testing`
    and the notations' screens' `snapshot`, in the module of their area beside
    `main.rs` (ER's canvas, its tools, relationships, tables and Generate; ER's Properties; ER's
    refusals that take a row of the shell's catalogue; both notations asked; the Model menu)
  - every offscreen screen's automation ids, in both appearances: the `tests` module in
    `apps/archforge/src/offscreen.rs`, against `apps/archforge/src/offscreen/automation-ids.tsv`;
    a change that renames, adds or removes an id on purpose updates the screen's rows from the
    listing the failing test prints, and a new screen, a row of the `screens!` list in the same
    file (its one list of screens, `offscreen::SCREENS`), gets its rows under its name
  - translations: the shell's catalogues in `crates/ui-egui/src/i18n/tests.rs`, each notation's
    registering in its screens crate;
    the tests across every catalogue (every literal has a row, every row is read and is in the
    catalogue of the code that reads it, no key twice, the model words) in the app, the one crate
    that knows every catalogue: `apps/archforge/src/catalogues.rs`; the machinery (lookups, plural rules, registering, the build-time row check) in
    `crates/shell-api/src/i18n/tests.rs`
  - the desktop app (arguments, preferences, graphics): the `tests` module in `apps/archforge/src/lib.rs`
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
  - the process and network scan of the interface's crates (the shell, its interfaces and the
    notations' screens): the `tests` modules in `xtask/src/shell_io.rs` and `xtask/src/layers.rs`
  - the notation rules (who links a model or screens crate, a notation of both parts, no notation
    command in the shared crates): the `tests` module in `xtask/src/notation_rules.rs`
  - the project store on disk: `crates/project/tests/`, on the fake notation; ER's and UML's own
    models saved and opened again: the app's `apps/archforge/tests/saved_model.rs`; the agent's
    tools over the real command table: the app's `apps/archforge/tests/agent_tools.rs`
  - the diagram-kind scan: the `tests` module in `xtask/src/kind_literals.rs`; its table of kinds
    against what the notations register: `crates/notations/tests/notations.rs`
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
  and finds controls by automation id (`harness` and `click` in `crates/ui-egui/src/testing.rs`,
  behind the feature `testing` for the app's window tests as well).
  Every harness pins the language with `set_language`; `every_harness_pins_the_language` fails a
  harness that does not.
- A notation's screens are tested without the shell against the shell's interfaces' test context
  (`crates/shell-api/src/testing.rs`, behind the feature `testing`, ADR 0005): a `TestContext` for
  the notations given owns what the narrow context borrows and keeps every `Request` made, and
  `harness` draws a screen over it in one language, pinned each frame, and one appearance, with
  `centre` and `click` to press a control by automation id. Its own tests, over a stub notation,
  are in the same file.
- A screen test scrolls to a control before it presses it, at the design's window height, and never
  makes the window taller to reach it. A panel's scrolled body lists a control only while part of it
  is inside the body (`record_visible` in `crates/shell-api/src/widgets.rs`), so a control under the
  fold or scrolled out above is not found, and one an edge cuts is found with a centre no press
  reaches. What a click
  adds above a control (a refusal under a field) moves it too: find it again after every press.
  `reached_in_properties` in `crates/ui-egui/src/tests.rs` wheels the Properties body, down to its
  end and then up to its top, until the control's centre is inside it.
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
- Before proposing a change: the gate, `cargo xtask ci`. Its steps are format, lint, test, assets,
  skills, layers, portable, generated, kinds and deny. It stops at the first failure and prints a
  table of steps. It builds with `--locked` and never changes `Cargo.lock`, so there is nothing to put
  back after it; a manifest changed without its lock fails it before the first step. The layers
  step runs the notation rules (`xtask/src/notation_rules.rs`, `AGENTS.md`, "Notations are
  plugins"), and a test's link counts as a production one: a link to a model crate from any crate
  but `archforge-notations`, that notation's screens crate or the app fails it, so does a link to
  a screens crate from any crate but the app, and so does a notation's command name in a test of
  the shell, its interfaces, the project store or the command-line tool. Such a test belongs in
  the app's tests (ADR 0005).
- The test step needs dolt 2.6 or later on the PATH or in `ARCHFORGE_DOLT`; without it the
  server and project tests fail and say so. A test that starts a server stops it on every path,
  so a failed run leaves no server behind: in the app, every test over the real project host
  takes it from `test_host::Sandbox` (`apps/archforge/src/test_host.rs`), whose drop stops the
  host as the application does at exit and removes the test's folder, also when the test panics.
- The portable step needs the `wasm32-unknown-unknown` target, which `rust-toolchain.toml` lists so
  rustup installs it with the pinned version, and a cargo from rustup on `PATH` (one from a package
  manager ignores the toolchain file); the deny step needs cargo-deny at the version `xtask/src/main.rs`
  names, and the network.
- `cargo xtask stats` prints the tests and lines per crate.
