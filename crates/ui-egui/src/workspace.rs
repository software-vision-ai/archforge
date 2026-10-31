//! The workspace, the window once a project is open (slice W1 of the workspace chrome, from the
//! design's frame "Workspace · Project just created"): the title strip, the project bar, the
//! activity rail at the left with the explorer column beside it, the centre with the diagram tabs
//! over the canvas (`canvas`), the right rail with its four panels, and the status bar. Every
//! control the design shows is drawn; those whose slices are not built yet take no click and say
//! so on hover. The explorer and the panel keep their widths in the UI state and resize by their
//! dividers.
//!
//! The geometry is the design's at 1600 px: the activity rail 52 wide, the explorer 368, the tab
//! strip 36 tall, the right rail 40, the panel 280, the status bar 40. In a narrower window the
//! panel, then the explorer, give way so the centre keeps at least `CENTRE_MIN`.

use egui::{Rect, Sense, Stroke, StrokeKind, Vec2, pos2, vec2};

use crate::home::{BAR, STRIP, control, title_strip};
use crate::i18n::tr;
use crate::state::{Explorer, Panel};
use crate::ux::{self, Palette, icons, types};
use crate::{Shell, widgets};

/// The activity rail's width, the right rail's, the tab strip's and the status bar's heights.
const ACTIVITY: f32 = 52.0;
const RAIL: f32 = 40.0;
const STATUS: f32 = 40.0;
/// The explorer's and the panel's widths: the design's, and the range a drag keeps them in.
pub(crate) const EXPLORER_DEFAULT: f32 = 368.0;
pub(crate) const PANEL_DEFAULT: f32 = 280.0;
const EXPLORER_RANGE: (f32, f32) = (240.0, 560.0);
const PANEL_RANGE: (f32, f32) = (240.0, 480.0);
/// The least width the centre keeps before the panel, then the explorer, are hidden.
const CENTRE_MIN: f32 = 320.0;
/// A divider's hit area, centred on the column edge.
const DIVIDER: f32 = 12.0;
/// A rail button and the pitch between two.
const RAIL_BUTTON: Vec2 = vec2(32.0, 36.0);
const RAIL_PITCH: f32 = 46.0;

/// The explorers of the activity rail: automation id, the explorer, its icon and its title.
const EXPLORERS: [(&str, Explorer, &ux::Icon, &str); 3] = [
    ("models", Explorer::Models, &icons::CUBE, "Models"),
    ("diagrams", Explorer::Diagrams, &icons::DIAGRAM, "Diagrams"),
    ("source-control", Explorer::SourceControl, &icons::BRANCH, "Source Control"),
];

/// The panels of the right rail: automation id, the panel, its icon and its title.
const PANELS: [(&str, Panel, &ux::Icon, &str); 4] = [
    ("agent", Panel::Agent, &icons::CHAT, "Agent"),
    ("appearance", Panel::Appearance, &icons::BRUSH, "Appearance"),
    ("properties", Panel::Properties, &icons::TABLE, "Properties"),
    ("skills", Panel::Skills, &icons::SKILL, "Skills"),
];

/// Every text of the tables to translate: they reach `tr` through the tables, where the catalogue
/// tests cannot find them as literals.
#[cfg(test)]
pub(crate) fn texts() -> impl Iterator<Item = &'static str> {
    EXPLORERS.iter().map(|(_, _, _, t)| *t).chain(PANELS.iter().map(|(_, _, _, t)| *t))
}

/// The columns of the window below the bar, from the state's widths and the window's width.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Columns {
    explorer: Option<Rect>,
    centre: Rect,
    panel: Option<Rect>,
    rail: Rect,
}

fn columns(body: Rect, explorer: f32, panel: f32) -> Columns {
    // Left to right: the activity rail, the explorer, the centre, the right rail, the panel. The
    // right rail sits between the centre and the panel, as the design has it; without the panel
    // it closes the window's right edge.
    let mut left = body.left() + ACTIVITY;
    let mut right = body.right();
    // A width that is not a number (never from the preferences file, which cannot hold one) falls
    // back to the design's.
    let finite = |w: f32, default: f32| if w.is_finite() { w } else { default };
    let explorer_w = finite(explorer, EXPLORER_DEFAULT).clamp(EXPLORER_RANGE.0, EXPLORER_RANGE.1);
    let panel_w = finite(panel, PANEL_DEFAULT).clamp(PANEL_RANGE.0, PANEL_RANGE.1);
    let mut cols = Columns { explorer: None, centre: Rect::NOTHING, panel: None, rail: Rect::NOTHING };
    if right - left - RAIL - panel_w >= CENTRE_MIN + explorer_w {
        cols.explorer = Some(Rect::from_min_max(pos2(left, body.top()), pos2(left + explorer_w, body.bottom())));
        left += explorer_w;
        cols.panel = Some(Rect::from_min_max(pos2(right - panel_w, body.top()), pos2(right, body.bottom())));
        right -= panel_w;
    } else if right - left - RAIL - explorer_w >= CENTRE_MIN {
        cols.explorer = Some(Rect::from_min_max(pos2(left, body.top()), pos2(left + explorer_w, body.bottom())));
        left += explorer_w;
    }
    cols.rail = Rect::from_min_max(pos2(right - RAIL, body.top()), pos2(right, body.bottom()));
    right -= RAIL;
    cols.centre = Rect::from_min_max(pos2(left, body.top()), pos2(right, body.bottom()));
    cols
}

/// A wrap width that is never negative.
fn room(w: f32) -> Option<f32> {
    Some(w.max(0.0))
}

pub fn show(shell: &mut Shell, ui: &mut egui::Ui) {
    let p = *ux::palette(shell.state.theme.is_dark());
    egui::CentralPanel::default().frame(egui::Frame::NONE.fill(p.page)).show(ui, |ui| {
        let full = ui.max_rect();
        if !ux::ready(ui.ctx()) {
            ui.ctx().request_repaint();
            return;
        }
        let Some(name) = shell.project.as_ref().map(|o| o.name.clone()) else { return };
        let strip = Rect::from_min_size(full.min, vec2(full.width(), STRIP));
        let bar = Rect::from_min_size(pos2(full.left(), strip.bottom()), vec2(full.width(), BAR));
        let status = Rect::from_min_max(pos2(full.left(), full.bottom() - STATUS), full.max);
        let body = Rect::from_min_max(pos2(full.left(), bar.bottom()), pos2(full.right(), status.top()));
        let cols = columns(body, shell.state.explorer_width, shell.state.panel_width);
        // The bar and the strip first: a click on Home there closes the project, and the frame ends.
        if !bar_row(shell, ui, &p, bar, &name) || !title_strip(shell, ui, &p, strip, false) {
            return;
        }
        activity_rail(shell, ui, &p, Rect::from_min_size(body.min, vec2(ACTIVITY, body.height())));
        undo_redo(shell, ui);
        if let Some(r) = cols.explorer {
            explorer(shell, ui, &p, r);
            resize(shell, ui, &p, r.right(), body, "workspace.explorer.resize", true);
        }
        centre(shell, ui, &p, cols.centre);
        if let Some(r) = cols.panel {
            panel(shell, ui, &p, r, &name);
            resize(shell, ui, &p, r.left(), body, "workspace.panel.resize", false);
        }
        right_rail(shell, ui, &p, cols.rail);
        status_bar(shell, ui, &p, status);
        // Over everything: the explorer's dialog and the New diagram dialog, when they ask.
        crate::explorer::delete_dialog(shell, ui, &p);
        crate::diagrams::new_dialog(shell, ui, &p);
    });
}

/// The bar: the brand and Home at the left, then the project; the search field, help and the
/// avatar at the right. False when Home closed the project.
fn bar_row(shell: &mut Shell, ui: &egui::Ui, p: &Palette, r: Rect, name: &str) -> bool {
    let paint = ui.painter();
    paint.rect_filled(r, 0.0, p.surface);
    paint.rect_filled(Rect::from_min_max(pos2(r.left(), r.bottom() - 1.0), r.max), 0.0, p.border);
    let mid = crate::home::bar_mid(r);
    let wordmark = crate::home::brand(paint, p, r);
    paint.rect_filled(Rect::from_center_size(pos2(wordmark.right() + 16.5, mid), vec2(1.0, 20.0)), 0.0, p.border);
    // Home, a ghost button: back to the start screen.
    let home_x = wordmark.right() + 29.0;
    let home_label = ux::galley(paint, &tr("Home"), types::UI, p.text, None);
    let home = Rect::from_min_max(pos2(home_x, mid - 14.0), pos2(home_x + 8.0 + 16.0 + 6.0 + home_label.size().x + 10.0, mid + 14.0));
    let home_resp = control(shell, ui, "workspace.header.home", "button", home, true);
    if home_resp.hovered() {
        paint.rect_filled(home, 6.0, p.page);
    }
    ux::icon_mid(paint, pos2(home.left() + 16.0, mid), &icons::HOME, p.text_secondary);
    paint.galley(pos2(home.left() + 30.0, mid - home_label.size().y / 2.0), home_label, p.text);
    if home_resp.clicked() {
        shell.close_project();
        return false;
    }
    // The project, a chip with the folder, its name and a chevron; the project switcher comes
    // later. The chip never runs under the controls at the right.
    let chip_x = home.right() + 10.0;
    let chip_room = (r.right() - 96.0 - chip_x - 74.0).max(40.0);
    let g = ux::galley(paint, name, types::UI_STRONG, p.accent, Some(chip_room.min(260.0)));
    let chip = Rect::from_min_max(pos2(chip_x, mid - 14.0), pos2(chip_x + 32.0 + g.size().x + 10.0 + 8.0 + 12.0, mid + 14.0));
    paint.rect_filled(chip, 7.0, p.accent_tint);
    ux::icon_mid(paint, pos2(chip.left() + 17.0, mid), &icons::FOLDER, p.accent);
    paint.galley(pos2(chip.left() + 32.0, mid - g.size().y / 2.0), g, p.accent);
    ux::icon_mid(paint, pos2(chip.right() - 16.0, mid + 0.5), &icons::CHEVRON_DOWN, p.accent);
    control(shell, ui, "workspace.header.project", "button", chip, false);
    crate::home::bar_tail(shell, ui, p, r, chip.right() + 24.0, "workspace.header", &tr("Find an element, view, or action"), "button");
    true
}

/// Undo and redo from the keyboard, through the engine, while no field has the keyboard.
fn undo_redo(shell: &mut Shell, ui: &egui::Ui) {
    if ui.memory(|m| m.focused().is_some()) || shell.explorer.delete.is_some() || shell.diagrams.new.is_some() {
        return;
    }
    let (undo, redo) = ui.input(|i| {
        let z = i.key_pressed(egui::Key::Z) && i.modifiers.command;
        (z && !i.modifiers.shift, (z && i.modifiers.shift) || (i.key_pressed(egui::Key::Y) && i.modifiers.command))
    });
    if let Some(session) = shell.session.as_mut() {
        if undo && session.can_undo() {
            let _ = session.execute("edit.undo", &serde_json::Value::Null);
        } else if redo && session.can_redo() {
            let _ = session.execute("edit.redo", &serde_json::Value::Null);
        }
    }
}

/// The activity rail: the explorers, the active one marked with the tint and a bar at the left.
fn activity_rail(shell: &mut Shell, ui: &egui::Ui, p: &Palette, r: Rect) {
    let paint = ui.painter();
    paint.rect_filled(r, 0.0, p.surface);
    paint.rect_filled(Rect::from_min_max(pos2(r.right() - 1.0, r.top()), r.max), 0.0, p.border);
    let active = shell.state.explorer;
    for (i, (id, which, icon, title)) in EXPLORERS.iter().enumerate() {
        let button = Rect::from_min_size(pos2(r.left() + 10.0, r.top() + 8.0 + i as f32 * RAIL_PITCH), RAIL_BUTTON);
        let on = *which == active;
        if on {
            paint.rect_filled(button, 6.0, p.accent_tint);
            paint.rect_filled(Rect::from_min_size(pos2(r.left(), button.top() + 8.0), vec2(2.0, 20.0)), 1.0, p.accent);
        }
        ux::icon_in(paint, Rect::from_center_size(button.center(), Vec2::splat(18.0)), icon, if on { p.accent } else { p.text_secondary });
        if control(shell, ui, &format!("workspace.explorers.{id}"), "tab", button, true).on_hover_text(tr(title)).clicked() {
            shell.state.explorer = *which;
        }
    }
}

/// The explorer column: Models is the tree over the session; Diagrams and Source Control show
/// their titles, filters and what is true, which is nothing yet.
fn explorer(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect) {
    let paint = ui.painter().clone();
    paint.rect_filled(r, 0.0, p.surface);
    paint.rect_filled(Rect::from_min_max(pos2(r.right() - 1.0, r.top()), r.max), 0.0, p.border);
    let at = |x: f32, y: f32| r.min + vec2(x, y);
    let (title, filter, empty) = match shell.state.explorer {
        Explorer::Models => {
            crate::explorer::draw(shell, ui, p, r);
            return;
        }
        Explorer::Diagrams => {
            crate::diagrams::draw(shell, ui, p, r);
            return;
        }
        Explorer::SourceControl => (tr("Source Control"), tr("Filter changes…"), tr("No changes yet")),
    };
    ux::text(&paint, at(20.0, 25.0), &title.to_uppercase(), types::EYEBROW, p.text, room(r.width() - 80.0));
    let add = Rect::from_min_size(pos2(r.right() - 40.0, r.top() + 20.0), Vec2::splat(24.0));
    ux::icon_in(&paint, Rect::from_center_size(add.center(), Vec2::splat(16.0)), &icons::PLUS, p.text_secondary);
    control(shell, ui, "workspace.explorer.add", "button", add, false);
    let field = Rect::from_min_max(at(16.0, 65.0), pos2(r.right() - 16.0, r.top() + 97.0));
    paint.rect_filled(field, 8.0, p.page);
    paint.rect_stroke(field, 8.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
    ux::icon(&paint, field.min + vec2(10.0, 9.0), &icons::SEARCH, 13.0, p.text_secondary);
    ux::text(&paint, field.min + vec2(33.0, 7.0), &filter, types::SMALL, p.text_secondary, room(field.width() - 44.0));
    control(shell, ui, "workspace.explorer.filter", "button", field, false);
    ux::text(&paint, at(36.0, 119.0), &empty, types::SMALL, p.text_muted, room(r.width() - 52.0));
}

/// A column divider: dragging it changes the column's width within its range.
fn resize(shell: &mut Shell, ui: &egui::Ui, p: &Palette, x: f32, body: Rect, id: &str, explorer: bool) {
    let hit = Rect::from_min_max(pos2(x - DIVIDER / 2.0, body.top()), pos2(x + DIVIDER / 2.0, body.bottom()));
    widgets::record_visible(ui, &mut shell.registry, id, "divider", hit);
    let resp = ui.interact(hit, widgets::egui_id(id), Sense::drag()).on_hover_cursor(egui::CursorIcon::ResizeHorizontal);
    if resp.dragged() {
        let dx = resp.drag_delta().x;
        if explorer {
            shell.state.explorer_width = (shell.state.explorer_width + dx).clamp(EXPLORER_RANGE.0, EXPLORER_RANGE.1);
        } else {
            shell.state.panel_width = (shell.state.panel_width - dx).clamp(PANEL_RANGE.0, PANEL_RANGE.1);
        }
    }
    if resp.hovered() || resp.dragged() {
        ui.painter().rect_filled(Rect::from_min_max(pos2(x - 1.0, body.top()), pos2(x + 1.0, body.bottom())), 0.0, p.focus);
    }
}

/// The centre: the diagram tabs over the canvas, drawn by the canvas module.
fn centre(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect) {
    crate::canvas::centre(shell, ui, p, r);
}

/// The right rail: the four panels, the active one marked.
fn right_rail(shell: &mut Shell, ui: &egui::Ui, p: &Palette, r: Rect) {
    let paint = ui.painter();
    paint.rect_filled(r, 0.0, p.page);
    paint.rect_filled(Rect::from_min_max(pos2(r.right() - 1.0, r.top()), r.max), 0.0, p.border);
    let active = shell.state.panel;
    for (i, (id, which, icon, title)) in PANELS.iter().enumerate() {
        let button = Rect::from_min_size(pos2(r.left() + 4.0, r.top() + 8.0 + i as f32 * RAIL_PITCH), RAIL_BUTTON);
        let on = *which == active;
        if on {
            paint.rect_filled(button, 6.0, p.accent_tint);
            paint.rect_filled(Rect::from_min_size(pos2(r.left(), button.top() + 8.0), vec2(2.0, 20.0)), 1.0, p.accent);
        }
        // Every rail icon is grey until it is the open panel; the agent's then wears its purple.
        let ink = match (on, *which == Panel::Agent) {
            (true, true) => p.purple,
            (true, false) => p.accent,
            (false, _) => p.text_secondary,
        };
        ux::icon_in(paint, Rect::from_center_size(button.center(), Vec2::splat(18.0)), icon, ink);
        if control(shell, ui, &format!("workspace.panels.{id}"), "tab", button, true).on_hover_text(tr(title)).clicked() {
            shell.state.panel = *which;
        }
    }
}

/// The right panel: its header with the collapse control, then its body from `panels`.
fn panel(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect, name: &str) {
    let paint = ui.painter().clone();
    paint.rect_filled(r, 0.0, p.surface);
    let title = PANELS.iter().find(|(_, which, _, _)| *which == shell.state.panel).map(|(_, _, _, t)| tr(t)).unwrap_or_default();
    ux::text(&paint, r.min + vec2(16.0, 12.0), &title, types::UI_STRONG, p.text, room(r.width() - 72.0));
    let collapse = Rect::from_min_size(pos2(r.right() - 36.0, r.top() + 8.0), vec2(28.0, 28.0));
    ux::icon_in(&paint, Rect::from_center_size(collapse.center(), Vec2::splat(16.0)), &icons::COLLAPSE, p.text_secondary);
    control(shell, ui, "workspace.panel.collapse", "button", collapse, false).on_hover_text(tr("Collapse panel"));
    paint.rect_filled(Rect::from_min_size(pos2(r.left(), r.top() + 43.0), vec2(r.width(), 1.0)), 0.0, p.border);
    let which = shell.state.panel;
    crate::panels::draw(shell, ui, p, r, 58.0, which, name);
}

/// The status bar: what is true about the project at the left, where it lives at the right.
fn status_bar(shell: &mut Shell, ui: &egui::Ui, p: &Palette, r: Rect) {
    let paint = ui.painter();
    paint.rect_filled(r, 0.0, p.surface);
    paint.rect_filled(Rect::from_min_size(r.min, vec2(r.width(), 1.0)), 0.0, p.border);
    let (dot, text) = match &shell.session {
        Some(s) if s.is_dirty() => (p.amber, tr("Not saved to the project yet")),
        Some(_) => (p.green, tr("Project created")),
        None => (p.text_muted, tr("The model could not be opened")),
    };
    paint.circle_filled(pos2(r.left() + 24.0, r.center().y), 3.0, dot);
    let y = r.center().y - 8.0;
    // The location first, right-aligned; the status and its reason take what is left.
    let mut right = r.right() - 24.0;
    if let Some(location) = shell.location() {
        let g = ux::galley(paint, &location, types::CAPTION, p.text_secondary, room(r.width() / 2.0 - 60.0));
        right -= g.size().x;
        paint.galley(pos2(right, y), g, p.text_secondary);
        right -= 24.0;
    }
    let start = r.left() + 36.0;
    let status = ux::text(paint, pos2(start, y), &text, types::CAPTION, p.text_secondary, room(right - start));
    if shell.session.is_none()
        && let Some(why) = &shell.session_error
    {
        let (why, ink) = match why {
            crate::SessionError::NoIdSource => (tr("The host gives no ids for the model's elements"), p.text_secondary),
            // The engine's own words, small and untranslated, as the New project dialog shows a detail.
            crate::SessionError::Engine(e) => (e.clone(), p.text_muted),
        };
        ux::text(paint, pos2(status.right() + 12.0, y), &why, types::CAPTION, ink, room(right - status.right() - 12.0));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_columns_give_way_from_the_right_as_the_window_narrows() {
        let body = |w: f32| Rect::from_min_size(egui::Pos2::ZERO, vec2(w, 1000.0));
        let wide = columns(body(1600.0), EXPLORER_DEFAULT, PANEL_DEFAULT);
        assert!(wide.explorer.is_some() && wide.panel.is_some());
        assert_eq!(wide.centre.width(), 1600.0 - ACTIVITY - EXPLORER_DEFAULT - PANEL_DEFAULT - RAIL);
        // The rail stands between the centre and the panel, which closes the window's edge.
        assert_eq!(wide.rail.left(), wide.centre.right());
        assert_eq!(wide.panel.map(|r| r.left()), Some(wide.rail.right()));
        assert_eq!(wide.panel.map(|r| r.right()), Some(1600.0));
        let medium = columns(body(900.0), EXPLORER_DEFAULT, PANEL_DEFAULT);
        assert!(medium.explorer.is_some() && medium.panel.is_none());
        let narrow = columns(body(500.0), EXPLORER_DEFAULT, PANEL_DEFAULT);
        assert!(narrow.explorer.is_none() && narrow.panel.is_none());
        assert_eq!(narrow.centre.width(), 500.0 - ACTIVITY - RAIL);
        assert_eq!(narrow.rail.right(), 500.0);
        let clamped = columns(body(1600.0), 10_000.0, -5.0);
        assert_eq!(clamped.explorer.map(|r| r.width()), Some(EXPLORER_RANGE.1));
        assert_eq!(clamped.panel.map(|r| r.width()), Some(PANEL_RANGE.0));
    }
}
