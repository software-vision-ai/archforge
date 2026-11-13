//! The View menu: Find…, the canvas's zooms, Whole diagram and Show minimap, as the minimap's own
//! controls do them, then a row for each explorer and panel, which opens it. Rows answer as the
//! table of actions does.

use egui::{Rect, pos2};

use crate::Shell;
use crate::actions::{Action, FIND, free};
use crate::canvas::{self, ZOOM_STEP};
use crate::i18n::tr;
use crate::kinds::active_kind;
use crate::state::{Explorer, Panel};

pub const ZOOM_IN: &str = "view.zoom-in";
pub const ZOOM_OUT: &str = "view.zoom-out";
pub const WHOLE: &str = "view.whole";
pub const MINIMAP: &str = "view.minimap";
pub const DIAGRAMS: &str = "view.diagrams";
pub const MODELS: &str = "view.models";
pub const VERSIONS: &str = "view.versions";
pub const CHAT: &str = "view.chat";
pub const APPEARANCE: &str = "view.appearance";
pub const PROPERTIES: &str = "view.properties";
pub const SKILLS: &str = "view.skills";

/// The View menu's rows by section, on every system (`docs/design/versioning.md`, section 11).
pub static VIEW_MENU: [&[&str]; 4] =
    [&[FIND], &[ZOOM_IN, ZOOM_OUT, WHOLE, MINIMAP], &[DIAGRAMS, MODELS, VERSIONS], &[CHAT, APPEARANCE, PROPERTIES, SKILLS]];

/// A row that acts on the canvas through `$run` while a diagram shows there.
macro_rules! canvas_row {
    ($id:expr, $word:expr, $run:expr) => {
        Action {
            id: $id,
            label: |_| $word,
            available: drawn,
            run: |s| {
                if drawn(s) {
                    $run(s)
                }
            },
            keys: None,
            search: None,
        }
    };
}

/// A row that opens the explorer or panel `$which` while a project shows and it is not open already.
macro_rules! show_row {
    ($id:expr, $word:expr, $field:ident, $folded:ident, $show:ident, $which:expr) => {
        Action {
            id: $id,
            label: |_| $word,
            available: |s| open_here(s) && !(s.state.$field == $which && !s.state.$folded),
            run: |s| {
                if open_here(s) {
                    s.state.$show($which)
                }
            },
            keys: None,
            search: None,
        }
    };
}

/// Every row but Find…, which the table of actions keeps for its keys.
pub(crate) static ROWS: &[Action] = &[
    canvas_row!(ZOOM_IN, tr("Zoom in"), |s: &mut Shell| zoom(s, ZOOM_STEP)),
    canvas_row!(ZOOM_OUT, tr("Zoom out"), |s: &mut Shell| zoom(s, 1.0 / ZOOM_STEP)),
    // The next drawing fits it, with the canvas's size and the diagram's bounds as the minimap's does.
    canvas_row!(WHOLE, tr("Whole diagram"), |s: &mut Shell| s.canvas.fit_pending = true),
    Action {
        id: MINIMAP,
        label: |_| tr("Show minimap"),
        available: |s| drawn(s) && !s.canvas.minimap_open,
        run: |s| s.canvas.minimap_open |= drawn(s),
        keys: None,
        search: None,
    },
    show_row!(DIAGRAMS, tr("Diagrams"), explorer, explorer_collapsed, show_explorer, Explorer::Diagrams),
    show_row!(MODELS, tr("Models"), explorer, explorer_collapsed, show_explorer, Explorer::Models),
    show_row!(VERSIONS, tr("Versions"), explorer, explorer_collapsed, show_explorer, Explorer::SourceControl),
    show_row!(CHAT, tr("Chat"), panel, panel_collapsed, show_panel, Panel::Agent),
    show_row!(APPEARANCE, tr("Appearance"), panel, panel_collapsed, show_panel, Panel::Appearance),
    show_row!(PROPERTIES, tr("Properties"), panel, panel_collapsed, show_panel, Panel::Properties),
    show_row!(SKILLS, tr("Skills"), panel, panel_collapsed, show_panel, Panel::Skills),
];

/// Whether a project's workspace shows, with nothing over the window.
fn open_here(shell: &Shell) -> bool {
    free(shell) && shell.project.is_some() && shell.active.is_some()
}

/// Whether a diagram shows on the canvas, with no version's details or changed element over it.
fn drawn(shell: &Shell) -> bool {
    open_here(shell) && active_kind(shell).is_some() && !crate::model_menu::covered(shell)
}

/// Zooms by `factor` about the canvas's centre, as the minimap's − and + do.
fn zoom(shell: &mut Shell, factor: f32) {
    let centre = shell.canvas.size / 2.0;
    canvas::zoom_by(&mut shell.canvas, factor, centre);
}

/// Why the row `id` is off, in words: the first thing that keeps it off.
pub(crate) fn why_off(shell: &Shell, id: &str) -> String {
    let on_canvas = [ZOOM_IN, ZOOM_OUT, WHOLE, MINIMAP].contains(&id);
    if id == FIND {
        tr("Leave the text field first")
    } else if !open_here(shell) {
        tr("Open or go back to a project first")
    } else if on_canvas && !drawn(shell) {
        tr("Show a diagram first")
    } else if id == MINIMAP && !canvas::minimap_shown(shell, Rect::from_min_size(pos2(0.0, 0.0), shell.canvas.size)) {
        tr("Widen the canvas to show the minimap")
    } else if id == MINIMAP {
        tr("The minimap is showing")
    } else {
        tr("Already showing")
    }
}
