# AGENTS.md: rules for every contributor and agent

This file holds the rules for changing Archforge, for people and coding agents alike; `CLAUDE.md`
only points here. The rules are short; the skill the table at the end names for your work has the
detail. `CONTRIBUTING.md` says how to propose a change.

## Building and checking

```sh
cargo run -p archforge                    # the application
cargo xtask ci                            # the gate: every check, in order
cargo xtask shot dark                     # an offscreen screenshot and a small copy to read
```

- The gate is `cargo xtask ci`. It never changes `Cargo.lock`: the task runner's build and every
  cargo command of the gate that resolves dependencies run with `--locked`, so a manifest changed
  without its lock fails the gate. craft-libs moves only through `cargo xtask bump`, which moves
  its crates to the newest commit of its main branch and nothing else, in a pull request of its
  own (`CONTRIBUTING.md`); never pin a revision of craft-libs or craft-fonts. Run targeted checks
  such as `cargo test -p <crate>` while working and the gate before proposing a change; a change
  is proposed only from a green gate.
- Look at every change to the user interface in a screenshot (`cargo xtask shot`, `light` or
  `dark`), not only in the tests.
- A change to a flow a user sees updates the user manual's chapter for it (`docs/manual/`) in the
  same change. The manual's pictures are rendered, never committed: a chapter names a snapshot
  screen, and `cargo xtask manual` draws them under `plan/manual/`.
- Build output, logs, screenshots and plans never enter the repository; the ignored `plan/`
  folder holds them.

## Rules for every change

- Verify by running the built application and reading its output, and cover the change with
  tests. Every crash fix ships a regression test that failed before the fix.
- Production code never fails unrecoverably. None of the panicking shortcuts is used: unwrapping or
  expecting on fallible values, explicit panics, unreachable markers, the todo and unimplemented
  macros. The workspace lints (`[workspace.lints.clippy]` in `Cargo.toml`) deny them, every
  member takes them with `[lints] workspace = true`, and the gate's lint step refuses a member
  without that table. `#[test]` and `#[cfg(test)]` code may unwrap, expect and panic
  (`clippy.toml`); an integration test whose helpers do allows only those lints, at its top, with
  the reason.
- A size declared in input is never trusted for preallocation. Total bytes are capped, not only
  item counts.
- Every input-derived integer uses checked, saturating or fallible arithmetic and conversion. It is
  never narrowed by a cast, never used to index or slice directly, and strings are sliced only at
  character boundaries.
- When a caller asks for something specific and it is out of range, it is refused rather than
  clamped, because a clamped operation reports success for a different result.
- A failed operation leaves the document and its history untouched: everything is validated, then
  applied.
- A silent fallback is a bug. An unknown format, target, parameter key or feature produces a typed
  error that names the offending value and the accepted choices.
- A hang is a crash. Every loop driven by untrusted data has a budget or a seen-set.
- Writes are atomic: a temporary file in the same directory, synchronised, then renamed over the
  target.
- A diagram kind is written out once, as the constant of the crate that defines it
  (`archforge_er::DIAGRAM_KIND`, `archforge_uml::STRUCTURE_DIAGRAM`), and named through it
  everywhere else; the gate's `kinds` step (`xtask/src/kind_literals.rs`) refuses it written out.
  A new kind gets its row in `xtask/diagram-kinds.tsv`, which a notations test holds to the kinds
  registered.
- The user interface never touches the file system or the operating system itself: the host
  injects `Services`. Only `crates/ui-egui`, the shell, `crates/shell-api`, its interfaces, a
  notation's screens crate (`crates/er-ui`, `crates/uml-ui`) and the application (`apps/archforge`,
  its library target `apps/archforge/src/lib.rs` included) may use the toolkit. The gate's `layers`
  step refuses the toolkit in a library crate below the shell's interfaces' layer, 9
  (`TOOLKIT_MIN_LAYER` in `xtask/src/layers.rs`), and process or network code in the shell, its
  interfaces and the notations' screens (`xtask/src/shell_io.rs`).
- The interface draws through `ux` (`crates/shell-api/src/ux/`, re-exported by the shell as
  `crate::ux` with the shell's own app icon and fonts, and by each notation's screens crate with the
  icons only that notation draws, `crates/er-ui/src/ux/icons.rs` and
  `crates/uml-ui/src/ux/icons.rs`; an icon the shell or both notations draw is in
  `crates/shell-api/src/ux/icons.rs`): no
  literal colour, font or icon outside it, and every control has an automation id.
- Every string the user sees goes through the catalogues, with a row in every language: in the
  shell's catalogues (`crates/ui-egui/src/i18n/`), in the shell's interfaces' own
  (`crates/shell-api/src/i18n/`) when only their code says it, or in a notation's own
  (`crates/er-ui/src/i18n/er/`, `crates/uml-ui/src/i18n/uml/`) when only that notation's screens
  say it. The app's catalogue tests (`apps/archforge/src/catalogues.rs`), the one crate that knows
  every catalogue, fail a literal without its row and a row in another crate's catalogue than the
  code that reads it.
- Behaviour, icons and wording are our own: never copy another product's.
- The Rust version is the one `rust-toolchain.toml` pins. A lint added by a newer release is fixed
  in code in the change that raises it, never by allowing the lint or by holding the version back.
- Hot files (the translation catalogues in `crates/ui-egui/src/i18n/`, `crates/shell-api/src/i18n/`, `crates/er-ui/src/i18n/` and `crates/uml-ui/src/i18n/`, `xtask/src/layers.rs`,
  `ATTRIBUTION.toml`) are edited surgically, with new rows placed beside related entries and never
  appended at the end of the file.
- Fonts, samples and other binaries stay out of the repository (the app icon's one picture,
  `assets/app-icon/icon.png`, is the exception); every file under `assets/` has its
  row in `ATTRIBUTION.toml`.

## Notations are plugins

A notation is a model part and a screens part, and the shell knows none (`forge-ux/adr/0002`,
"Notations are plugins"; the architecture decision records are in the forge-ux repository's
`adr/` folder). Its model part is composed in `archforge-notations` (`crates/notations`), its
screens part registered in the app's screens registry (`SCREENS` in `apps/archforge/src/lib.rs`),
and `the_model_and_screens_registries_hold_the_same_notations` there fails when the two list
different notations (`forge-ux/adr/0003`, "Two registries"). The shell is handed both as values,
`Shell::new(services, notations, screens)`, so the shared crates (`crates/ui-egui`,
`crates/shell-api`, `crates/project` and the command-line tool, `apps/archforge-cli`) name no
notation. A question the shell's interfaces cannot answer yet gets a new hook in `NotationUi` or
`DiagramKind` (`crates/shell-api/src/notation_ui.rs`, `crates/shell-api/src/kinds.rs`), which every
notation answers for itself, never a special case in a shared screen.

The gate's `layers` step enforces this with the notation rules (`xtask/src/notation_rules.rs`,
`forge-ux/adr/0004`, "Layer rules are enforced by the gate, without exceptions", and
`forge-ux/adr/0005`, "Tests follow the plugin"). They read `cargo metadata` over every kind of
dependency, so a test's link counts as a production one; there is no exception list. Each refusal
names its rule and its records, and says where the code belongs:

1. rule 1: "only archforge-notations, the notation's own screens crate … and the app link the
   model crate …, in production or in tests";
2. rule 2: "only the app links a notation's screens crate, in production or in tests";
3. rule 3: "ui-egui, shell-api, project and the command-line tool link no notation crate, in
   production or in tests";
4. rule 4: "the model crate has no screens crate …" and "the screens crate has no model
   crate …": "a notation is a model part and a screens part" (a model crate on layer 7 of the
   layer table takes a screens crate `<model>-ui` on layer 10);
5. rule 5: "a shared crate names no notation command, in production or in tests": a scan of every
   file of the shared crates, catalogues and tests included, for a command id under a model
   crate's name (`er.…`, `uml.…`) and a Model menu row a screens crate declares (`model.…`).

So tests follow the plugin: the shell's and the project store's run on the fake notation
(`archforge_notations::fake`, behind `archforge-notations`' feature `testing`); a notation's screens
are tested in its own crate against the shell's interfaces' test context
(`archforge_shell_api::testing`); whole-app behaviour with ER and UML is tested in the app
(`apps/archforge/tests/`). `skills/archforge-tests/SKILL.md` says where each test goes.

## Pull requests and the `deep-review` label

PR-Agent reviews a pull request from this repository once, when it is opened, reopened or marked
ready for review, unless it changes no code file (`CONTRIBUTING.md`, "Code review"); pull requests
from forks get no PR-Agent review.
CodeRabbit reviews only pull requests labelled `deep-review`, and its reviews are few, so the label
goes by what a change does, not by which files it touches.

- Apply `deep-review` when you create the pull request (`gh pr create --label deep-review`) only if
  it changes behaviour across one of these boundaries:
  - a saved format: project files, the database schema, chat storage, or a metamodel change that
    alters what a project stores;
  - a security boundary: the control channel's token and server, secrets, how agent programs are
    started, what files the application may touch, or a workflow's permissions or secrets;
  - the contract agents use: a command, a parameter or an MCP tool added, renamed or removed, or
    what a refusal means changed;
  - the life of processes: starting and stopping Dolt servers, their locks and watchdog, and the
    supervision of agent programs.
- Don't apply it to a refactor that keeps behaviour (code moved, hooks added, names changed, proven
  by tests and snapshots), to interface work and wording, to a fix inside one notation's rules, or
  to scaffolding, documentation, test-only changes, dependency bumps or exploratory work.
- On a labelled pull request, gather the fixes and push once each time it is ready for review:
  CodeRabbit reviews a push only when asked (`@coderabbitai review`).

## Working on something: read its skill first

| Working on | Read |
|---|---|
| Licences, dependencies, assets, our own work, the name | `skills/archforge-ground-rules/SKILL.md` |
| Non-test Rust, input, writing files, a crash | `skills/archforge-crash-safety/SKILL.md` |
| How the interface looks and behaves: the design decisions so far | `skills/archforge-design/SKILL.md` |
| What the interface draws or says, translations | `skills/archforge-ui/SKILL.md` |
| Writing or running tests, a failing gate | `skills/archforge-tests/SKILL.md` |

`skills/README.md` says how the skills are kept true to the code.

## Commits

A commit holds one focused change. Its subject says what changed, in the imperative; the body says
why, and the measured result where there is one.
