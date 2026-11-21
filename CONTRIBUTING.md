# Contributing to Archforge

Thank you for helping. Archforge is at its start: the window shows Home, and the screens behind it
come one at a time. Small, focused changes are the easiest to review.

## Before you start

- For anything larger than a fix, open an issue first and say what you want to change and why, so
  the work fits the design before you spend time on it.
- Read `AGENTS.md`: it holds the rules every change follows, whether a person or a coding agent
  writes it.
- Taking part means agreeing to the code of conduct, `CODE_OF_CONDUCT.md`.

## Making a change

1. Build and run the application: `cargo run -p archforge` (see `README.md` for the toolchain and
   the Linux packages).
2. Make one focused change, with a test that fails without it.
3. For a change to the user interface, look at it: `cargo xtask shot` and `cargo xtask shot dark`
   write screenshots under `plan/shots/`.
4. Run the gate, `cargo xtask ci`, until it is green.
5. Open a pull request. Describe what changed, why, and how you checked it; attach before and after
   screenshots for interface changes. The Gate workflow (`.github/workflows/gate.yml`) runs the
   same `cargo xtask ci` on the pull request, and a pull request merges only when that run is green.

## The Rust version

The Rust version lives in one place: `rust-toolchain.toml` at the repository's root, with the
formatter, the lints and the web target the gate needs. rustup reads it wherever cargo runs in the
checkout, so the gate on your machine, the Gate workflow and the release jobs build with the same
compiler, whatever your default toolchain is; the first cargo command installs it. Use the cargo
rustup provides: one from a package manager ignores the file.

Raising the version is one change to that file's `channel`. Cargo does not read the file, so the
same commit sets `rust-version` in `Cargo.toml` to the same number, and fixes in code the lints the
new release adds. Neither workflow names a version of its own.

## Translations

The interface strings live in `crates/ui-egui/src/i18n/<language>.tsv`, keyed by the English text.
A new string needs a row in every catalogue; the tests say which rows are missing. Place new rows
beside related entries rather than at the end of the file.

## Licence of contributions

Unless you say otherwise, a contribution you submit is licensed under the MIT licence or the
Apache License, Version 2.0, at your option, as the rest of the code is. Do not add fonts,
images or other assets without their licence and a row in `ATTRIBUTION.toml`.
