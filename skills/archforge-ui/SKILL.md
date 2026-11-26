---
name: archforge-ui
description: "Use when adding or changing anything Archforge draws or says: a screen, a control, a colour, a text style, an icon, a label, a tooltip, a menu entry or a translation. Covers the ux module, automation ids, the screenshot, the catalogues and adding a language."
---

# The interface and its text

The interface is `crates/ui-egui`, the shell, over its interfaces in `crates/shell-api`; the two are
the only library crates that may use the toolkit. It draws the design: when the design changes, the
code follows it, and a correction the owner makes to the look goes into the design first. `crates/ui-egui/README.md` lists the crate's public items. The
decisions taken so far about how it looks and behaves are in `skills/archforge-design/SKILL.md`.

## Draw through `ux`
- `ux` is the shell's interfaces' design (`crates/shell-api/src/ux/mod.rs`), which the shell
  re-exports as `crate::ux` (`crates/ui-egui/src/ux/mod.rs`) with what needs the shell: the app
  icon, the installing of the design's fonts, and the icons only one notation draws.
- Colours come from the palette, `ux::palette(dark)` (`crates/shell-api/src/ux/mod.rs`): `LIGHT`
  and `DARK`, one field per role. No literal colour outside that file; a new role is a new field in
  both appearances.
- A colour is a `ux::Colour`: its value is private, so only `ux` makes one (a palette role, a
  named colour such as `ux::CLEAR`, or `ux::data_colour` for a colour the document holds). A
  screen passes it on as `ux::Colour`. egui takes it where it takes `impl Into<Color32>`; where
  an egui call or field wants its own colour type (`Painter::galley`, `TextFormat`, the theme's
  `Visuals`), the colour gets `.into()` right there. A test reading a frame back compares what
  was painted, as the `Color32` egui holds, with the palette's values turned into it
  (`Color32::from(p.accent)`) or with a colour's own bytes; nothing turns a painted colour back
  into a `ux::Colour`, not even in a test.
- Text uses the design's styles, `ux::types::*`, drawn with `ux::text`, `ux::galley` or
  `ux::galley_rows`, which place a line box's top-left corner as the design does. No raw `FontId`
  in a screen. A screen checks `ux::ready` before its first text.
- Icons are strokes in `crates/shell-api/src/ux/icons.rs` when the shell or both notations draw
  them, and in `crates/ui-egui/src/ux/icons.rs` when only ER or only UML does, until that
  notation's screens crate takes them; both are drawn with `ux::icon` or `ux::icon_in` at the
  design's stroke weight.
- `crates/ui-egui/src/theme.rs` gives the toolkit's own visuals (tooltips, scroll bars) the same
  palette. Everything else is drawn by the screen itself.
- On macOS the title strip is the window's title bar: keep clear of `Shell::top_inset`.

## Icons over text
- Prefer an icon to a text label wherever an icon says it clearly: tools, toggles, option and
  segmented buttons, cardinality ends, line styles, and row or editor actions such as edit, remove,
  apply and cancel. The words go into the tooltip and the accessible name, through `tr`.
- Keep text where no icon is clear: a heading, a field label, a value, the name of a menu entry, a
  sentence. When in doubt, draw the icon and show it beside the design before falling back to text.
- An icon is a thin stroke at the design's weight and size, never a scaled-up copy; an icon without
  text always has a tooltip and an automation id.
- An icon is our own drawing in Archforge's style, never a copy of a classic tool's. Draw it at its
  natural size, so it matches its neighbours, and look at it at 1x as well as on a retina screen
  before it is final: thin gaps, such as a dashed line's, close up at 1x.

## Controls
- Every interactive control is registered with an automation id, its kind and its rectangle, as
  `control` in `crates/ui-egui/src/home.rs` does through the automation registry,
  `crates/shell-api/src/widgets.rs`. Ids are dotted, lower-case and stable:
  `<screen>.<area>.<thing>`, such as `strip.home`. The tests find controls by id, never by position.
- A control whose screen does not exist yet says "Not available yet" on hover. A canvas tool that
  is not built is left out of the toolbar instead.
- The interface never touches the file system: what it needs from the host comes through
  `Services` (`skills/archforge-crash-safety/SKILL.md`).

## Look at the result
`cargo xtask shot` and `cargo xtask shot dark` (`--size WxH` for another frame) draw the real
shell offscreen and write a screenshot and a small copy under `plan/shots/`. Read the small copy
after every visible change, in both appearances, beside the design.

## Text and translations
- Every string the user sees goes through `tr("…")`, `tr_ctx("context", "…")` for a word with
  several meanings, or `trn("{n} thing", "{n} things", n)` (the machinery is
  `crates/shell-api/src/i18n/mod.rs`; the shell's `crate::i18n` re-exports it with its
  catalogues registered). Pass the English text as a literal: the coverage test reads the
  literals in `crates/ui-egui/src`, `crates/shell-api/src` and `apps/archforge/src`. Text from a
  table goes through a function the test calls, as `home::start_texts` does.
- The catalogues are `crates/ui-egui/src/i18n/<code>.tsv`, keyed by the English text, four
  tab-separated columns: kind, context, key, text. All seven are complete, so a new string needs
  its row in every one. Rows go beside related entries, never at the end of the file. A key is in
  a catalogue once (a `str` row by context and key, a `plural` row by key alone): check that the
  row is not there already, as the coverage test refuses a second one.
- Translate the whole sentence with its punctuation; placeholders such as `{n}` may move. A row a
  model translated says so, as the catalogue header records its provenance.
- The catalogue screen test refuses hidden characters, addresses and instructions in a row.
- A new language is a catalogue plus a row in `LANGUAGES` (`crates/shell-api/src/i18n/mod.rs`)
  and in the shell's `CATALOGUES` (`crates/ui-egui/src/i18n/mod.rs`);
  `every_catalogue_in_the_folder_is_registered` fails without the rows. Japanese and Chinese draw with an installed family
  (`crates/ui-egui/src/fonts.rs`).
- Wording is plain and UK English: colour, licence, catalogue.

## Tests
A screen gets a harness test in `crates/ui-egui/src/tests.rs` that draws it in both appearances
and checks its ids: `skills/archforge-tests/SKILL.md`.
