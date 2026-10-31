---
name: archforge-ground-rules
description: "Use before adding a dependency, an icon, image or font, any product wording, or anything copied from elsewhere to Archforge, or when unsure whether something may be copied, read or named. Covers the clean room, licences, assets and attribution, the shared crates and the name."
---

# The clean room, licences, assets and the name

The rules a change cannot break. `AGENTS.md` lists them in one line each.

## Our own work
- Behaviour comes from Archforge's own design, the public modelling standards and what users tell
  us. Never copy another product's code, icons, artwork, help text, templates or sample models.
- Never name another modelling product anywhere in the repository: code, comments, strings, tests,
  documentation, commit messages. Say what a feature does, not whose feature it resembles.
- The optional trademark check, `cargo xtask brands`, reads the list `FORGE_BRANDS` names and fails
  on a listed name in code, comments, strings, test names, manifests and packaging sources. It does
  not read Markdown, so a skill, the README or a contributor document is checked by eye.
- The repository carries no tracker ids, no private paths and no names of private files. Plans,
  notes and screenshots belong in the ignored `plan/` folder.

## Licences and dependencies
- The code is MIT OR Apache-2.0. A dependency passes `cargo deny check` (`deny.toml`): permissive
  licences only, and git sources only from craft-libs. Say in the change which dependency you add
  and why.
- The shared crates come from craft-libs and the interface's fonts from craft-fonts, both at the
  newest commit of their main branch. Never pin a revision, tag or version of either: the gate's
  first step moves `Cargo.lock` to the newest craft-libs commit. To work on craft-libs beside
  Archforge, use the commented patch in `.cargo/config.toml` and never commit it uncommented.

## Assets
- Every file under `assets/` has an `[[asset]]` row in `ATTRIBUTION.toml` with its author, source,
  licence and hash. Every face the program draws with has a `[[face]]` row, and every licence named
  has its text under `licenses/`. `cargo xtask assets` checks the rows and rewrites
  `ATTRIBUTION.md`; never edit that file by hand.
- Fonts, samples and other binaries stay out of the repository; the one exception is the app
  icon's picture, `assets/app-icon/icon.png`, a brand mark (`licenses/brand/archforge-icon.txt`)
  that packaging scales with `cargo xtask resize`. Inter is read at build time from
  the craft-fonts checkout `CRAFT_FONTS_DIR` names (`crates/ui-egui/build.rs`); without it the
  toolkit's face stands in.
- The interface's icons are drawn in code from the design's vector paths (`crates/ui-egui/src/ux/icons.rs`),
  not shipped as files. A new icon follows the ones there: our own drawing, the design's stroke
  weight.
- `ATTRIBUTION.toml` is a hot file: a new row goes beside its relatives, never at the end.

## The name
- Archforge everywhere: crates `archforge-*` (the prefix `xtask/src/layers.rs` strips), the binary
  `archforge`, the window title, the settings folder.
- A new crate gets its row in the layer table, `TABLE` in `xtask/src/layers.rs`; the layer check
  fails without it.
