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

- The gate is `cargo xtask ci`. Its first step moves the craft-libs crates to the newest commit of
  their main branch, so a change is always checked against the latest shared crates; never pin a
  revision of craft-libs or craft-fonts. Run targeted checks such as `cargo test -p <crate>` while
  working and the gate before proposing a change; a change is proposed only from a green gate.
- Look at every change to the user interface in a screenshot (`cargo xtask shot`, `light` or
  `dark`), not only in the tests.
- Build output, logs, screenshots and plans never enter the repository; the ignored `plan/`
  folder holds them.

## Rules for every change

- Verify by running the built application and reading its output, and cover the change with
  tests. Every crash fix ships a regression test that failed before the fix.
- Production code never fails unrecoverably. None of the panicking shortcuts is used: unwrapping or
  expecting on fallible values, explicit panics, unreachable markers, the todo and unimplemented
  macros. Every crate root (each `lib.rs` and each binary's `main.rs`) carries
  `#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unreachable,
  clippy::todo, clippy::unimplemented)]`, so integration tests, examples and benches stay exempt.
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
- The user interface never touches the file system or the operating system itself: the host
  injects `Services`. Only `crates/ui-egui` and the application may use the toolkit.
- The interface draws through `ux`: no literal colour, font or icon outside it, and every control
  has an automation id.
- Every string the user sees goes through the catalogues, with a row in every one.
- Behaviour, icons and wording are our own: never copy another product's, and never name another
  modelling product in the repository.
- A lint added by a toolchain release is fixed in code at once, never by pinning the toolchain.
- Hot files (the translation catalogues in `crates/ui-egui/src/i18n/`, `xtask/src/layers.rs`,
  `ATTRIBUTION.toml`) are edited surgically, with new rows placed beside related entries and never
  appended at the end of the file.
- Fonts, samples and other binaries stay out of the repository; every file under `assets/` has its
  row in `ATTRIBUTION.toml`.

## Working on something: read its skill first

| Working on | Read |
|---|---|
| Licences, dependencies, assets, the clean room, the name | `skills/archforge-ground-rules/SKILL.md` |
| Non-test Rust, input, writing files, a crash | `skills/archforge-crash-safety/SKILL.md` |
| What the interface draws or says, translations | `skills/archforge-ui/SKILL.md` |
| Writing or running tests, a failing gate | `skills/archforge-tests/SKILL.md` |

`skills/README.md` says how the skills are kept true to the code.

## Commits

A commit holds one focused change. Its subject says what changed, in the imperative; the body says
why, and the measured result where there is one.
