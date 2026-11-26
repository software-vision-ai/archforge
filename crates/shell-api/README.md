# archforge-shell-api

**Purpose.** The shell's interfaces: what a notation's screens implement, and what they may use
from the shell, so a notation's screens build on this crate and never on the shell crate
(`archforge-ui-egui`) itself (`forge-ux/adr/0002`, "Notations are plugins", and `0003`, "Two
registries"). It holds the automation-id registry, the design (`ux`), the canvas geometry that
names no shell (`canvas`), the data types the interfaces `NotationUi` and `DiagramKind` hand
across that name no shell (`notation_ui`, `kinds`), how a search or a filter matches what is typed
(`search`) and the translations machinery (`i18n`), with which each crate registers its own
catalogues; story AR-780 moves the shared controls and the
interfaces themselves with the data types that still name the shell here, unchanged.

**Layer.** 9, the shell interfaces' layer: above `archforge-notations` (8), below the notations'
screens (10) and the shell (11). Builds for the portable target; holds no process or network code
(the gate's `layers` step scans it as it scans the shell). It takes egui, serde,
`archforge-mof` (4), whose elements, ids and store the interfaces' data types name,
`archforge-core` (5), whose skills a row of the Models tree lists under its element,
`archforge-diagram` (6), whose rectangles and line-end marks the canvas geometry reads and whose
notations a press on the canvas carries, craft-i18n, whose `Folder` folds what a search is typed
against, and `icu_properties`, which says where a letter ends so a search never matches inside one.

**Public API.**

| Item | Signature |
|---|---|
| `widgets::Registry` | the controls of the last complete frame, each with its automation id, kind and rectangle: `begin_frame`, `record`, `entries`, and how far a scrolling body's controls reach (`reach_from`, `reach_end`, `reach`, `laid_out`) |
| `widgets::Entry` | `pub struct Entry { pub id: String, pub kind: &'static str, pub rect: [f32; 4] }`, one recorded control, as `ui.inspect` lists it |
| `widgets::record_visible` | `pub fn record_visible(ui: &Ui, reg: &mut Registry, id: &str, kind: &'static str, rect: Rect)`: records a control only while it is inside its clip rectangle |
| `widgets::announce` | `pub fn announce(ui: &Ui, reg: &mut Registry, id: &str, area: Rect, words: &str)`: records a status and has a screen reader say it, as a polite live status |
| `widgets::egui_id` | `pub fn egui_id(id: &str) -> egui::Id`: the toolkit id of an automation id |
| `ux::{Colour, Palette, LIGHT, DARK, palette}` | the design's colours: a `Colour`'s value is private, so only `ux` makes one (a palette role, a named colour: `CLEAR`, `UNTINTED`, `PLACEHOLDER_INK`, `MARK`, `MARK_EDGE`; `data_colour` for a colour the document holds; `from_hsv`, `to_hsv` and `swatch_hue` for the colour picker); egui takes it as `Color32` |
| `ux::{Type, types, regular, semibold, ready}` | the design's text styles in Inter and their families; `ready` says whether the shell has installed them |
| `ux::{text, text_mid, text_slanted, text_marked, Marks, underline_char, galley, galley_rows, code_mid, cut_front, text_mid_front}` | text laid out and drawn as the design places it |
| `ux::{Icon, Seg, icons, icon, icon_weighted, icon_exact, icon_in, icon_mid, icon_in_tile, STROKE}` | the icons the shell and both notations draw, and their drawing: crisp, rasterised once per size into an alpha mask at the screen's own pixels, at a standard size by role with one stroke weight |
| `ux::{shadow, drop_rect, overlay}` | a floating surface's shadow; where a list opens from its anchor; a layer above the window's content for a menu or a list |
| `canvas::View` | `pub struct View { pub origin: Pos2, pub pan: Vec2, pub zoom: f32 }`, the view's mapping between diagram units and the screen: `to_screen`, `rect`, `to_canvas` |
| `canvas::{exit_point, self_loop, along, nesting_route}` | where a line from a card's centre leaves it; a self relationship's loop on its right side; the point `t` along a polyline; the route of the line that ties a nested card to its owner's and its ⊕ |
| `canvas::{mark_shape, mark_outline}` | where a line's end mark (`archforge_diagram::figure::EndMark`) sits and its corners as drawn |
| `canvas::{END_HANDLE, end_handles_at}` | a selected line's end handle's radius, and where its two handles sit clear of the marks and of each other |
| `canvas::{beside, beside_pair, qualifier_place, signature_place}` | where a line's end texts sit, one alone or both together; where a qualifier box sits against its card; where a template's signature box sits on its card |
| `notation_ui::{Offer, Added, Inside}` | what `+` in Models can add (key and word); what an addition gave back (the element and the rows opened to show it); what New inside ▸ offers on an element (its kinds, the rows refused with their reason, why none, where a line parts them) |
| `notation_ui::{Entry, Words}` | an element as the project search lists it (icon, order, label, detail); how Properties names an element (kind line, word, "This table", "On this table · {n}", heading's name) |
| `notation_ui::Removal` | `pub struct Removal { pub went: Vec<String>, pub after: Option<String> }`: what a notation works out about a removal of its own elements before it runs, for what is said once it went through |
| `notation_ui::{Layer, Overlaid}` | where among the canvas's own overlays a notation's go (`Drafts`, `Rows`, `Menus`, `Offers`); the canvas an overlay is drawn over (palette, rectangle, `View`, diagram, the nodes a gesture previews) |
| `notation_ui::Tree` | what a notation's rows of the Models tree are made with: `new(store, filter, expanded)` takes the Models filter as typed, folded and trimmed as one piece, the rows opened and the skills by the element they sit on; `filtering`, `matches`, `open`, `skills`, `row`, `skill_rows` |
| `notation_ui::Subject` | the selection Properties describes (`obj`, `id`, `class`, `name`, `kind`, `word`, `this`, `on_this`): `of` an element as its `Words` name it, or the model's `root` by the project's name |
| `notation_ui::{Row, Problem}` | a row of the Models tree as drawn (id, element, depth, kind and icon, muted, label, detail, note, whether it opens, is open and drags onto the canvas); a problem on the element it sits on (element, text) |
| `notation_ui::{Editor, Place, OpenEditor}` | a Properties editor's card (its notation's metamodel and the card's name); where an open editor edits (the item, or none for one being added, and its list); an open editor (where it edits, the refusal its card shows) |
| `notation_ui::{MenuId, Scope}` | a menu of the shell's one open-menu slot: whose it is (`Scope::Shell`, `Scope::Notation` by metamodel), its key and its index among a kind shown more than once; `shell`, `of`, `at`, `index_in` |
| `search::Typed` | what was typed into a search or a filter, folded once: `words` (each word apart), `phrase` (one piece), `trimmed` (one piece, its edges trimmed after folding); `is_empty`, `first`, `in_folded`, `in_any` |
| `search::{fold, joins, starts}` | `pub fn fold(text: &str) -> Folded`: a text folded as a search folds what is typed; `pub fn joins(prev: Option<char>, next: char) -> bool`: whether `next` goes on the letter before it; `pub fn starts<'a>(name: &'a Folded, word: &'a str) -> impl Iterator<Item = usize> + 'a`: where a folded word starts in a folded name, never inside a letter |
| `kinds::{ToolUse, KindTool}` | what a diagram kind's tool does (`Place`, `Connect`); a tool it brings to the toolbar (key, icon, hint, use, chevron, width) |
| `kinds::Press` | a press on the canvas as a diagram kind reads it: where, click or double click, the store and notations, the diagram and selection, and the node and line under a screen point |
| `kinds::{LineDrawn, ExtraLabel}` | a line as the canvas drew it this frame, for its diagram kind to draw what goes with it (line, element, points on screen from source to target, whether selected); a label a kind drew of no one node or line (the element, where it was on screen, the node that holds where it was moved, its hover text) |
| `i18n::{tr, tr_ctx, trn, fmt}` | a literal translated in this thread's language, one under a context, a plural with its count filled in, named placeholders filled in one pass; a key without a registered row is its own English text |
| `i18n::{Language, LANGUAGES, plural_forms, codes, set_language, language}` | the languages (code, native name, plural rule, complete or not), English first; how many plural forms a language's catalogue holds; the codes for a message; this thread's language |
| `i18n::{Rows, register, Unregistered}` | one language's rows in a crate's catalogue (code, file name, text); registering a crate's catalogues, once, a later table's row replacing an earlier one's under the same key; the refusal of a table with a catalogue for a code no language has, before anything is registered |
| `i18n::{Catalogue, parse}` | a catalogue's `str` and `plural` rows, as the lookups read them; malformed rows skipped |
| `i18n::check::malformed_row` | `pub fn malformed_row(text: &str) -> Option<(usize, usize)>`: the build-time row check, the first row without four tab-separated columns (line, columns found); a build script takes the file as a module of its own (`#[path]`) |

**Non-goals.** Installing fonts and the app icon, which stay in the shell, as do the icons only one
notation draws until its screens crate takes them; the catalogues, which each crate that brings
text keeps, embeds and registers (the shell's are in `crates/ui-egui/src/i18n/`, ER's and UML's in
its `er/` and `uml/` folders until their screens crates take them); anything that
touches the file system or the operating system, which the host does through `Services`.
