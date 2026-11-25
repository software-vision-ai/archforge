# archforge-shell-api

**Purpose.** The shell's interfaces: what a notation's screens implement, and what they may use
from the shell, so a notation's screens build on this crate and never on the shell crate
(`archforge-ui-egui`) itself (`forge-ux/adr/0002`, "Notations are plugins", and `0003`, "Two
registries"). Empty for now: story AR-780 moves the automation-id registry, the design tokens and
shared icons, the shared controls and canvas geometry, the interfaces `NotationUi` and
`DiagramKind` with the data types they name, and the translations machinery here, unchanged.

**Layer.** 9, the shell interfaces' layer: above `archforge-notations` (8), below the notations'
screens (10) and the shell (11). Builds for the portable target; holds no process or network code
(the gate's `layers` step scans it as it scans the shell).

**Public API.** None yet.

**Non-goals.** Installing fonts, which stays in the shell; anything that touches the file system or
the operating system, which the host does through `Services`.
