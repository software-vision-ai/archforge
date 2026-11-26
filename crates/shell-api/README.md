# archforge-shell-api

**Purpose.** The shell's interfaces: what a notation's screens implement, and what they may use
from the shell, so a notation's screens build on this crate and never on the shell crate
(`archforge-ui-egui`) itself (`forge-ux/adr/0002`, "Notations are plugins", and `0003`, "Two
registries"). It holds the automation-id registry and the design (`ux`); story AR-780 moves the
shared controls and canvas geometry, the interfaces `NotationUi` and `DiagramKind` with the data
types they name, and the translations machinery here, unchanged.

**Layer.** 9, the shell interfaces' layer: above `archforge-notations` (8), below the notations'
screens (10) and the shell (11). Builds for the portable target; holds no process or network code
(the gate's `layers` step scans it as it scans the shell).

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

**Non-goals.** Installing fonts and the app icon, which stay in the shell, as do the icons only one
notation draws until its screens crate takes them; anything that touches the file system or
the operating system, which the host does through `Services`.
