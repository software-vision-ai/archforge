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

## Code review

Pull requests get AI review in two tiers:

- **PR-Agent** reviews every pull request once, when it is opened, reopened or marked ready for
  review, with the project's own model keys (`.github/workflows/pr-agent.yml`). It looks only at
  the changed code. Documentation, the lockfile, generated code, assets and other non-code files
  are left out, and a pull request without code gets no review. It does not review later pushes.
  The owner, a member or a collaborator can ask for a new review by commenting `/review` on the
  pull request. Pull requests from forks get no PR-Agent review, neither when they open nor on a
  `/review` comment.
- **CodeRabbit** reviews only pull requests labelled `deep-review`, once (`.coderabbit.yaml`).
  `AGENTS.md` says which changes take the label. Later pushes are reviewed when someone comments
  `@coderabbitai review`. That comment is also the way to start a review if adding the label to
  an open pull request does not start one.

## The Rust version

The Rust version lives in one place: `rust-toolchain.toml` at the repository's root, with the
formatter, the lints and the web target the gate needs. rustup reads it wherever cargo runs in the
checkout, so the gate on your machine, the Gate workflow and the release jobs build with the same
compiler, whatever your default toolchain is; the first cargo command installs it. Use the cargo
rustup provides: one from a package manager ignores the file.

Raising the version is one change to that file's `channel`. Cargo does not read the file, so the
same commit sets `rust-version` in `Cargo.toml` to the same number, and fixes in code the lints the
new release adds. Neither workflow names a version of its own.

## Cargo.lock and craft-libs

The gate never changes `Cargo.lock`. The task runner's build and every cargo command of the gate
that resolves dependencies run with `--locked`, so a manifest changed without its lock stops the
gate at once, with cargo's `cannot update the lock file … because --locked was passed to prevent
this`. Its hint to drop `--locked` is not the fix here:

- For a dependency you added or changed, run `cargo build` (or `cargo update -p <crate>`), check
  that the lock's diff holds only that dependency and what it brings in, and commit `Cargo.lock`
  with the change.
- craft-libs, whose crates follow its main branch, moves only through `cargo xtask bump`. It moves
  every craft-libs crate in `Cargo.lock` to the newest commit of that branch, prints each crate's
  old and new commit, and keeps every other package as the lock had it; when the newest craft-libs
  needs another package added, removed or at another version, it puts the lock back and names
  them. Run it on an up-to-date `main`, run the gate, and open a pull request for the bump alone.
  A bump stopped part way (Ctrl-C) can leave `cargo update`'s lock behind;
  `git checkout -- Cargo.lock` puts it back.
  A change that needs a newer craft-libs waits for that pull request to merge.

## Translations

The interface strings live in `crates/ui-egui/src/i18n/<language>.tsv`, keyed by the English text;
a string only one notation's screens say lives in that notation's catalogues, `i18n/er/` or
`i18n/uml/`. A new string needs a row in every language; the tests say which rows are missing. Place new rows
beside related entries rather than at the end of the file.

## Licence of contributions

Unless you say otherwise, a contribution you submit is licensed under the MIT licence or the
Apache License, Version 2.0, at your option, as the rest of the code is. Do not add fonts,
images or other assets without their licence and a row in `ATTRIBUTION.toml`.
