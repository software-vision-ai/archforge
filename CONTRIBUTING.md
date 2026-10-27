# Contributing to Archforge

Thank you for helping. Archforge is at its start: the window shows Home, and the screens behind it
come one at a time. Small, focused changes are the easiest to review.

## Before you start

- For anything larger than a fix, open an issue first and say what you want to change and why, so
  the work fits the design before you spend time on it.
- Read `AGENTS.md`: it holds the rules every change follows, whether a person or a coding agent
  writes it.

## Making a change

1. Build and run the application: `cargo run -p archforge` (see `README.md` for the toolchain and
   the Linux packages).
2. Make one focused change, with a test that fails without it.
3. For a change to the user interface, look at it: `cargo xtask shot` and `cargo xtask shot dark`
   write screenshots under `plan/shots/`.
4. Run the gate, `cargo xtask ci`, until it is green.
5. Open a pull request. Describe what changed, why, and how you checked it; attach before and after
   screenshots for interface changes.

## Translations

The interface strings live in `crates/ui-egui/src/i18n/<language>.tsv`, keyed by the English text.
A new string needs a row in every catalogue; the tests say which rows are missing. Place new rows
beside related entries rather than at the end of the file.

## Licence of contributions

Unless you say otherwise, a contribution you submit is licensed under the MIT licence or the
Apache License, Version 2.0, at your option, as the rest of the code is. Do not add fonts,
images or other assets without their licence and a row in `ATTRIBUTION.toml`.
