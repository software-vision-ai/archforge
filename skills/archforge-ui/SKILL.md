---
name: archforge-ui
description: "Use when adding or changing anything Archforge draws or says: a screen, a control, a colour, a text style, an icon, a label, a tooltip, a menu entry or a translation. Covers the ux module, automation ids, the screenshot, the catalogues and adding a language."
---

# The interface and its text

The interface is `crates/ui-egui`, the only library crate that may use the toolkit. It draws the
design: when the design changes, the code follows it, and a correction the owner makes to the look
goes into the design first. `crates/ui-egui/README.md` lists the crate's public items.

## Draw through `ux`
- Colours come from the palette, `ux::palette(dark)` (`crates/ui-egui/src/ux/mod.rs`): `LIGHT` and
  `DARK`, one field per role. No literal colour outside that file; a new role is a new field in
  both appearances.
- Text uses the design's styles, `ux::types::*`, drawn with `ux::text`, `ux::galley` or
  `ux::galley_rows`, which place a line box's top-left corner as the design does. No raw `FontId`
  in a screen. A screen checks `ux::ready` before its first text.
- Icons are strokes in `crates/ui-egui/src/ux/icons.rs`, drawn with `ux::icon` or `ux::icon_in` at
  the design's stroke weight.
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

## Controls
- Every interactive control is registered with an automation id, its kind and its rectangle, as
  `control` in `crates/ui-egui/src/home.rs` does through `crates/ui-egui/src/widgets.rs`. Ids are
  dotted, lower-case and stable: `<screen>.<area>.<thing>`, such as `strip.home`. The tests find
  controls by id, never by position.
- A control whose screen does not exist yet says "Not available yet" on hover.
- The interface never touches the file system: what it needs from the host comes through
  `Services` (`skills/archforge-crash-safety/SKILL.md`).

## Look at the result
`cargo xtask shot` and `cargo xtask shot dark` (`--size WxH` for another frame) draw the real
shell offscreen and write a screenshot and a small copy under `plan/shots/`. Read the small copy
after every visible change, in both appearances, beside the design.

## Text and translations
- Every string the user sees goes through `tr("…")`, `tr_ctx("context", "…")` for a word with
  several meanings, or `trn("{n} thing", "{n} things", n)` (`crates/ui-egui/src/i18n/mod.rs`).
  Pass the English text as a literal: the coverage test reads the literals in
  `crates/ui-egui/src` and `apps/archforge/src`. Text from a table goes through a function the
  test calls, as `home::start_texts` does.
- The catalogues are `crates/ui-egui/src/i18n/<code>.tsv`, keyed by the English text, four
  tab-separated columns: kind, context, key, text. All seven are complete, so a new string needs
  its row in every one. Rows go beside related entries, never at the end of the file.
- Translate the whole sentence with its punctuation; placeholders such as `{n}` may move. A row a
  model translated says so, as the catalogue header records its provenance.
- The catalogue screen test refuses hidden characters, addresses and instructions in a row.
- A new language is a catalogue plus a row in `LANGUAGES`; `every_catalogue_in_the_folder_is_registered`
  fails without the row. Japanese and Chinese draw with an installed family
  (`crates/ui-egui/src/fonts.rs`).
- Wording is plain and UK English: colour, licence, catalogue.

## Tests
A screen gets a harness test in `crates/ui-egui/src/tests.rs` that draws it in both appearances
and checks its ids: `skills/archforge-tests/SKILL.md`.
