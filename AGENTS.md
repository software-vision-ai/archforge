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
  injects `Services`. Only `crates/ui-egui` and the application may use the toolkit.
- The interface draws through `ux`: no literal colour, font or icon outside it, and every control
  has an automation id. The compiler holds it: `clippy.toml` bans egui's colour and font types
  outside `ux`, the one place that makes them, and the gate's ux check refuses every type that file
  bans by name in any token there, macros included, since the ban does not see a struct literal or
  a variant's path (`xtask/src/ux_literals.rs`).
- Every string the user sees goes through the catalogues, with a row in every one.
- Behaviour, icons and wording are our own: never copy another product's.
- The Rust version is the one `rust-toolchain.toml` pins. A lint added by a newer release is fixed
  in code in the change that raises it, never by allowing the lint or by holding the version back.
- Hot files (the translation catalogues in `crates/ui-egui/src/i18n/`, `xtask/src/layers.rs`,
  `ATTRIBUTION.toml`) are edited surgically, with new rows placed beside related entries and never
  appended at the end of the file.
- Fonts, samples and other binaries stay out of the repository (the app icon's one picture,
  `assets/app-icon/icon.png`, is the exception); every file under `assets/` has its
  row in `ATTRIBUTION.toml`.

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
