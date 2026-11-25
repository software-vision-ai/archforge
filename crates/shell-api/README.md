# archforge-shell-api

**Purpose.** The shell's interfaces: what a notation's screens implement, and what they may use
from the shell, so a notation's screens build on this crate and never on the shell crate
(`archforge-ui-egui`) itself (`forge-ux/adr/0002`, "Notations are plugins", and `0003`, "Two
registries"). It holds the automation-id registry; story AR-780 moves the design tokens and shared
icons, the shared controls and canvas geometry, the interfaces `NotationUi` and `DiagramKind` with
the data types they name, and the translations machinery here, unchanged.

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

**Non-goals.** Installing fonts, which stays in the shell; anything that touches the file system or
the operating system, which the host does through `Services`.
