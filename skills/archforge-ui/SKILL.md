---
name: archforge-ui
description: "Use when adding or changing anything Archforge draws or says: a screen, a control, a colour, a text style, an icon, a label, a tooltip, a menu entry or a translation. Covers the ux module, automation ids, the screenshot, the catalogues and adding a language."
---

# The interface and its text

The interface is `crates/ui-egui`, the shell, over its interfaces in `crates/shell-api`, with each
notation's screens in a crate of their own between them (`crates/er-ui`, `crates/uml-ui`); they are the
only library crates that may use the toolkit. It draws the design: when the design changes, the
code follows it, and a correction the owner makes to the look goes into the design first. `crates/ui-egui/README.md` lists the crate's public items. The
decisions taken so far about how it looks and behaves are in `skills/archforge-design/SKILL.md`.

## Draw through `ux`
- `ux` is the shell's interfaces' design (`crates/shell-api/src/ux/mod.rs`), which the shell
  re-exports as `crate::ux` (`crates/ui-egui/src/ux/mod.rs`) with what needs the shell: the app
  icon and the installing of the design's fonts; each notation's screens crate re-exports it as
  its own `crate::ux` (`crates/er-ui/src/ux/mod.rs`, `crates/uml-ui/src/ux/mod.rs`) with the icons
  only that notation draws.
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
  them, and in `crates/er-ui/src/ux/icons.rs` or `crates/uml-ui/src/ux/icons.rs` when only that
  notation does; all are drawn with `ux::icon` or `ux::icon_in` at the design's stroke weight.
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
  `control_cx` in `crates/shell-api/src/panels.rs` does through the automation registry,
  `crates/shell-api/src/widgets.rs` (`control` in `crates/ui-egui/src/home.rs` is the shell's own
  way to it). The shared controls (the `_cx` controls: a section, the fields and selects, the
  menus, a check box, the buttons, the name field, the Problems section) are in
  `crates/shell-api/src/panels.rs`, beside `control_cx`, for the shell and a notation's screens
  alike. Ids are dotted, lower-case and stable:
  `<screen>.<area>.<thing>`, such as `strip.home`. The tests find controls by id, never by position.
- A notation's screens and the shared controls reach the shell through the narrow context
  (`crates/shell-api/src/cx.rs`, built by the shell in `crates/ui-egui/src/cx.rs`), never the
  whole `Shell`: `Ctx` borrows the parts they change apart
  (`Shell::cx`), `CtxRef` is its read-only twin (`Shell::cx_ref`), and what lies outside it (the
  Models tree's rows, the centre leaving a version's details, a panel of the right side or an
  explorer of the left side shown, a new element's name opened for typing in Models, the delete
  dialog opened on a removal) is a `Request` the shell carries out once the call returns. What a
  screen does on a diagram through it (opening a diagram or a skill, selecting, placing an element
  with its lines, taking nodes off, asking before a removal: `canvas::open_cx`,
  `editing::select_cx`, `place_with_lines_cx`, `unplace_cx`, `ask_delete_cx`) runs at once, as
  functions of the interfaces over the context, beside the card geometry and the drawing helpers a
  notation's figures share with the canvas (`crates/shell-api/src/canvas.rs`). The
  interfaces `NotationUi` and `DiagramKind` hand it over: a method that reaches the shell takes
  `&mut Ctx`, or `&CtxRef` where it only reads, and the shell builds it at each call, never the
  notation. A kind's rows of the Model menu are data (`kinds::ModelRow`), from which the shell
  makes the menu's entries. The controls taking it end in `_cx`; the old ones over the whole
  shell forward to them for the shell's own screens. `NotationUi` and what a notation keeps
  (`NotationState`) are in `crates/shell-api/src/notation_ui.rs`; `DiagramKind`, with its rows
  (`MenuRow`, `ModelRow`) and the kind of a diagram (`kind_of`, `active_kind`, `diagram_icon`,
  `popover_takes_keys`, each asked of the kinds it is handed), is in
  `crates/shell-api/src/kinds.rs`, beside what a frame's drawing reads (`canvas::Drawing`) and
  what a press asks for (`editing::Action`). A notation adds a diagram kind by implementing
  `DiagramKind`; the app registers it with its notation's screens part (`ScreensPart`, `crates/shell-api/src/screens.rs`) in its static list of parts (`SCREENS`, `apps/archforge/src/lib.rs`), which it gives every shell (`Shell::new`); a test there (`the_model_and_screens_registries_hold_the_same_notations`) holds that list in step with the model registry (`archforge_notations::NOTATIONS`), one part for each notation, of its metamodel and with its diagram kinds. The refusal words
  (`panels::refusal`, `said`, `a_kind`, `engine_said::disabled`, in `crates/shell-api/src/`) and
  Chat's counts of the elements a tool row names (`chat_words::tool_words`, by
  `NotationUi::chat_kind` and `chat_count`) ask the notations they are handed: pass
  `cx.notation_uis`, or the shell's `notation_uis`. So do the hooks that say a refusal, a problem
  or what goes with a removal (`NotationUi::said`, `problems`, `remove_refusal`, `consequences`),
  for the reasons and kinds they name inside their sentences; a model's kind words
  (`model_words::kind_label`); and what an opening does to a model (`Shell::open_repair`, which
  the comparisons' threads and the host's restore take). No global list of notations or kinds
  exists: a shell takes its own from the screens parts it is given, and the tests ask the parts
  they register (`tests::SCREENS`). The delete dialog says what goes with a
  removal as the element's notation counts and words it (`NotationUi::consequences`, one finished
  sentence each, carried in `explorer::Delete`); the shell adds only the diagrams it disappears
  from and what happens to what it holds, and asks only when there is something to say.
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
  `crates/shell-api/src/i18n/mod.rs`, whose lookups register the interfaces' own catalogues; the
  shell's `crate::i18n` re-exports it with its catalogues registered). Pass the English text as a literal: the coverage test (`apps/archforge/src/catalogues.rs`) reads the
  literals in `crates/ui-egui/src`, `crates/shell-api/src`, `crates/er-ui/src`, `crates/uml-ui/src` and `apps/archforge/src`. Text from a
  table goes through a function the test calls, listed in `testing::word_lists`, as `home::start_texts` is, or for
  a notation's screens in `archforge_er_ui::snapshot::word_lists` and `archforge_uml_ui::snapshot::word_lists`.
- The catalogues are `crates/ui-egui/src/i18n/<code>.tsv`, the shell's,
  `crates/shell-api/src/i18n/<code>.tsv`, the shell's interfaces' (registered by their own
  lookups, and checked by their build script), and one table per notation, `crates/er-ui/src/i18n/er/<code>.tsv` and `crates/uml-ui/src/i18n/uml/<code>.tsv`
  (checked by that notation's screens crate's build script), which the notation hands over (`NotationUi::catalogues`). The shell's own is registered before
  the first lookup; a shell registers those of the notations it is given as it sets them up
  (`Shell::new`, from its screens parts, `i18n::register_notations`), so nothing a notation says is looked up before them.
  A test that says a notation's words without a shell makes one first, as the app does. A row only one notation's screens name goes in that notation's table;
  a row only the shell's interfaces' code names goes in theirs; a row the shell names, or more
  than one of them, goes in the shell's. The words for what the metamodel
  names (`model_words`, the change view's labels) follow their metamodel: the core's in the
  shell's table, a word only one notation's metamodel names in that notation's, which the shell
  says only for the notations it was given; the hover text of a diagram kind's tool is its
  notation's too. `every_row_is_in_the_catalogue_of_the_code_that_reads_it` holds every row to
  this. To it, a source file is a notation's code when, its tests aside, it names that notation's
  crate (`archforge_er`) and no other notation's, and the interfaces' code when it is in
  `crates/shell-api/src`; a test reading a row places it nowhere. Each is keyed by the English
  text, four tab-separated columns: kind, context, key, text. All seven languages are complete,
  so a new string needs its row in every language of its table. Rows go beside related entries,
  never at the end of the file. A key is in a language's catalogues once (a `str` row by context
  and key, a `plural` row by key alone): check that the row is not there already, in the shell's
  table, the interfaces' or a notation's, as the coverage test refuses a second one.
- `every_row_finds_its_text_in_every_language` looks every row of every table up through the
  shell's `tr_ctx` and `trn`, in every language, so a row a table lacks in one language, or a
  table the shell does not register, fails it. It reads the texts from the catalogues
  themselves, so a translation fix or a new row needs no change to it.
- Translate the whole sentence with its punctuation; placeholders such as `{n}` may move. A row a
  model translated says so, as the catalogue header records its provenance.
- The catalogue screen test refuses hidden characters, addresses and instructions in a row.
- A new language is a catalogue in every table plus a row in `LANGUAGES`
  (`crates/shell-api/src/i18n/mod.rs`), in the interfaces' `CATALOGUES` (the same file), in the
  shell's `CATALOGUES` (`crates/ui-egui/src/i18n/mod.rs`) and in each notation's (`crates/er-ui/src/er_ui.rs`,
  `crates/uml-ui/src/uml_ui.rs`); each crate's `every_catalogue_in_the_folder_is_registered` fails without the rows. Japanese and Chinese draw with an installed family
  (`crates/ui-egui/src/fonts.rs`).
- Wording is plain and UK English: colour, licence, catalogue.

## Tests
A screen gets a harness test in `crates/ui-egui/src/tests.rs` that draws it in both appearances
and checks its ids: `skills/archforge-tests/SKILL.md`.
