//! The View menu: Find…, the canvas's zooms, Whole diagram and Minimap, as the minimap's own
//! controls do them, then a row for each explorer and panel. Minimap and those rows are ticked
//! while theirs shows, and choosing one shows it or hides it. Rows answer as the table of actions does.

use egui::{Rect, pos2};

#[cfg(any())] // AR-796 parked
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

/// A row for the explorer or panel `$which`: it shows it, or folds it away while it shows, as a
/// click on its rail icon `$rail` does, while a project shows.
macro_rules! show_row {
    ($id:expr, $word:expr, $rail:ident, $which:expr) => {
        Action {
            id: $id,
            label: |_| $word,
            available: open_here,
            run: |s| {
                if open_here(s) {
                    s.state.$rail($which)
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
        label: |_| tr("Minimap"),
        available: minimap_fits,
        run: |s| s.canvas.minimap_open ^= minimap_fits(s),
        keys: None,
        search: None,
    },
    show_row!(DIAGRAMS, tr("Diagrams"), rail_explorer, Explorer::Diagrams),
    show_row!(MODELS, tr("Models"), rail_explorer, Explorer::Models),
    show_row!(VERSIONS, tr("Versions"), rail_explorer, Explorer::SourceControl),
    show_row!(CHAT, tr("Chat"), rail_panel, Panel::Agent),
    show_row!(APPEARANCE, tr("Appearance"), rail_panel, Panel::Appearance),
    show_row!(PROPERTIES, tr("Properties"), rail_panel, Panel::Properties),
    show_row!(SKILLS, tr("Skills"), rail_panel, Panel::Skills),
];

/// Whether a project's workspace shows, a dialog over the window or not.
fn workspace_shows(shell: &Shell) -> bool {
    shell.project.is_some() && shell.active.is_some()
}

/// Whether a project's workspace shows, with nothing over the window.
fn open_here(shell: &Shell) -> bool {
    free(shell) && workspace_shows(shell)
}

/// Whether a diagram shows on the canvas, with no version's details or changed element over it,
/// a dialog over the window or not.
fn diagram_shows(shell: &Shell) -> bool {
    workspace_shows(shell) && active_kind(shell).is_some() && !crate::model_menu::covered(shell)
}

/// Whether a diagram shows on the canvas and nothing is over the window, so the canvas's rows can act.
fn drawn(shell: &Shell) -> bool {
    free(shell) && diagram_shows(shell)
}

/// Whether a diagram shows on a canvas, as last drawn, large enough for the minimap, with nothing
/// over the window.
fn minimap_fits(shell: &Shell) -> bool {
    drawn(shell) && has_room(shell)
}

/// Whether the row `id` is ticked: its minimap, explorer or panel shows now. A dialog over the
/// window turns the rows off but leaves the ticks, since what they tick still shows behind it.
/// `None` for a row with no tick, which only acts.
pub fn ticked(shell: &Shell, id: &str) -> Option<bool> {
    let (state, here) = (&shell.state, workspace_shows(shell));
    let explorer = |which| here && state.explorer == which && !state.explorer_collapsed;
    let panel = |which| here && state.panel == which && !state.panel_collapsed;
    Some(match id {
        MINIMAP => shell.canvas.minimap_open && diagram_shows(shell) && has_room(shell),
        DIAGRAMS => explorer(Explorer::Diagrams),
        MODELS => explorer(Explorer::Models),
        VERSIONS => explorer(Explorer::SourceControl),
        CHAT => panel(Panel::Agent),
        APPEARANCE => panel(Panel::Appearance),
        PROPERTIES => panel(Panel::Properties),
        SKILLS => panel(Panel::Skills),
        _ => return None,
    })
}

/// Whether the canvas, as last drawn, has room for the minimap: wide and tall enough.
fn has_room(shell: &Shell) -> bool {
    canvas::minimap_room(shell, Rect::from_min_size(pos2(0.0, 0.0), shell.canvas.size))
}

/// Zooms by `factor` about the canvas's centre, as the minimap's − and + do.
fn zoom(shell: &mut Shell, factor: f32) {
    let centre = shell.canvas.size / 2.0;
    canvas::zoom_by(&mut shell.canvas, factor, centre);
}

/// Why the row `id` is off, in words: the first thing that keeps it off. With a diagram showing
/// only Minimap is ever off, for want of room.
pub(crate) fn why_off(shell: &Shell, id: &str) -> String {
    let on_canvas = [ZOOM_IN, ZOOM_OUT, WHOLE, MINIMAP].contains(&id);
    if id == FIND && shell.home_search.cramped {
        tr("Widen the window to search")
    } else if id == FIND {
        tr("Leave the text field first")
    } else if !open_here(shell) {
        tr("Open or go back to a project first")
    } else if on_canvas && !drawn(shell) {
        tr("Show a diagram first")
    } else {
        tr("Make the canvas larger to show the minimap")
    }
}
