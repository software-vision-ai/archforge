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

use egui::{Rect, Sense, Vec2, pos2, vec2};

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

/// The explorers of the activity rail, top to bottom: automation id, the explorer, its icon and its
/// title. Diagrams comes first, so the diagrams are found first (owner, 2026-11-04).
const EXPLORERS: [(&str, Explorer, &ux::Icon, &str); 3] = [
    ("diagrams", Explorer::Diagrams, &icons::DIAGRAM, "Diagrams"),
    ("models", Explorer::Models, &icons::CUBE, "Models"),
    ("source-control", Explorer::SourceControl, &icons::BRANCH, "Versions"),
];

/// The panels of the right rail: automation id, the panel, its icon and its title.
const PANELS: [(&str, Panel, &ux::Icon, &str); 4] = [
    // Chat's panel keeps its id, the agent's.
    ("agent", Panel::Agent, &icons::CHAT_BUBBLE, "Chat"),
    ("appearance", Panel::Appearance, &icons::PALETTE, "Appearance"),
    ("properties", Panel::Properties, &icons::PROPERTIES, "Properties"),
    ("skills", Panel::Skills, &icons::SKILLS, "Skills"),
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

/// The columns for the explorer's and the panel's widths; none for one the user folded away.
fn columns(body: Rect, explorer: Option<f32>, panel: Option<f32>) -> Columns {
    // Left to right: the activity rail, the explorer, the centre, the right rail, the panel. The
    // right rail sits between the centre and the panel, as the design has it; without the panel
    // it closes the window's right edge.
    let mut left = body.left() + ACTIVITY;
    let mut right = body.right();
    // A width that is not a number (never from the preferences file, which cannot hold one) falls
    // back to the design's.
    let finite = |w: f32, default: f32| if w.is_finite() { w } else { default };
    let explorer_w = explorer.map(|w| finite(w, EXPLORER_DEFAULT).clamp(EXPLORER_RANGE.0, EXPLORER_RANGE.1));
    let panel_w = panel.map(|w| finite(w, PANEL_DEFAULT).clamp(PANEL_RANGE.0, PANEL_RANGE.1));
    let mut cols = Columns { explorer: None, centre: Rect::NOTHING, panel: None, rail: Rect::NOTHING };
    // What fits beside a centre of the least width; the panel gives way first, then the explorer.
    let fits = |w: f32| right - left - RAIL - w >= CENTRE_MIN;
    let (show_explorer, show_panel) = match (explorer_w, panel_w) {
        (Some(e), Some(p)) if fits(e + p) => (Some(e), Some(p)),
        (Some(e), _) if fits(e) => (Some(e), None),
        (None, Some(p)) if fits(p) => (None, Some(p)),
        _ => (None, None),
    };
    if let Some(w) = show_explorer {
        cols.explorer = Some(Rect::from_min_max(pos2(left, body.top()), pos2(left + w, body.bottom())));
        left += w;
    }
    if let Some(w) = show_panel {
        cols.panel = Some(Rect::from_min_max(pos2(right - w, body.top()), pos2(right, body.bottom())));
        right -= w;
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
        let explorer_w = (!shell.state.explorer_collapsed).then_some(shell.state.explorer_width);
        let panel_w = (!shell.state.panel_collapsed).then_some(shell.state.panel_width);
        let cols = columns(body, explorer_w, panel_w);
        // The command key with K opens the search, over the project's diagrams and elements.
        crate::home_search::shortcut(shell, ui.ctx());
        // The bar and the strip first: a click on a tab or Home there changes what shows, and the
        // frame ends.
        if !bar_row(shell, ui, &p, bar) || !title_strip(shell, ui, &p, strip, false) {
            return;
        }
        activity_rail(shell, ui, &p, Rect::from_min_size(body.min, vec2(ACTIVITY, body.height())));
        undo_redo(shell, ui);
        if let Some(r) = cols.explorer {
            explorer(shell, ui, &p, r);
            resize(shell, ui, &p, r.right(), body, "workspace.explorer.resize", true);
        }
        centre(shell, ui, &p, cols.centre);
        // A deleted conversation's toast with Undo, at the centre's foot (AR-217).
        crate::chat_delete::toast(shell, ui, &p, cols.centre);
        if let Some(r) = cols.panel {
            panel(shell, ui, &p, r, &name);
            resize(shell, ui, &p, r.left(), body, "workspace.panel.resize", false);
        }
        right_rail(shell, ui, &p, cols.rail);
        status_bar(shell, ui, &p, status);
        // Over everything: the explorer's dialog and the New diagram dialog, when they ask.
        crate::explorer::delete_dialog(shell, ui, &p);
        crate::chat_delete::dialog(shell, ui, &p);
        crate::save_version::dialog(shell, ui, &p);
        crate::diagrams::new_dialog(shell, ui, &p);
    });
}

/// The bar: the brand and the tabs (Home and the open projects) at the left; the search field,
/// help and the avatar at the right. False when a tab was clicked (the frame ends).
fn bar_row(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect) -> bool {
    let paint = ui.painter();
    paint.rect_filled(r, 0.0, p.surface);
    paint.rect_filled(Rect::from_min_max(pos2(r.left(), r.bottom() - 1.0), r.max), 0.0, p.border);
    let wordmark = crate::home::brand(paint, p, r);
    let (end, changed) = project_tabs(shell, ui, p, r, wordmark.right());
    if changed {
        return false;
    }
    crate::home::bar_tail(shell, ui, p, r, end + 24.0, "workspace.header", &tr("Find an element, view, or action"), "button");
    true
}

/// What a click on the bar's tabs asks for.
enum TabClick {
    Home,
    Show(usize),
    Close(usize),
}

/// The bar's tabs after the brand: Home, then one chip per open project with its folder, its name
/// and its close button; the one shown is tinted. They keep left of help: names shrink first, and
/// when even short names do not fit, Home and the tabs behind become icons (their names in the
/// tooltip) while the shown tab keeps a short name and its close button. Gives where they end,
/// and whether a click changed what the window shows (the frame then ends).
pub(crate) fn project_tabs(shell: &mut Shell, ui: &egui::Ui, p: &Palette, r: Rect, after: f32) -> (f32, bool) {
    const GAP: f32 = 6.0;
    /// A chip's width beside its name: the folder, the paddings and the close button.
    const CHROME: f32 = 32.0 + 8.0 + 24.0;
    /// An icon tab, and the narrowest name a full tab shows.
    const ICON: f32 = 32.0;
    const NAME_MIN: f32 = 24.0;
    let paint = ui.painter().clone();
    let mid = crate::home::bar_mid(r);
    paint.rect_filled(Rect::from_center_size(pos2(after + 16.5, mid), vec2(1.0, 20.0)), 0.0, p.border);
    let mut click = None;
    let tabs: Vec<(String, bool, bool)> = shell
        .tabs
        .iter()
        .enumerate()
        // A project with unsaved changes shows a dot before its name, as the title does.
        .map(|(i, t)| (if shell.tab_unsaved(i) { format!("• {}", t.name) } else { t.name.clone() }, shell.active == Some(i), t.closing()))
        .collect();
    let widths: Vec<f32> = tabs.iter().map(|(name, ..)| ux::galley(&paint, name, types::UI_STRONG, p.text, None).size().x).collect();
    let home_on = shell.active.is_none();
    let home_label = ux::galley(&paint, &tr("Home"), types::UI, if home_on { p.accent } else { p.text }, None);
    let home_full = 8.0 + 16.0 + 6.0 + home_label.size().x + 10.0;
    // Help's left edge, less a margin: the tabs end before it.
    let limit = r.right() - 96.0;
    let start = after + 29.0;
    let n = tabs.len() as f32;
    let full_room = |home_w: f32| -> Option<f32> {
        let room = ((limit - start - home_w - GAP - n * GAP) / n.max(1.0) - CHROME).min(220.0);
        (tabs.is_empty() || room >= NAME_MIN).then_some(room)
    };
    // Compact: the shown tab takes what the icons leave, at most a short name; with less than the
    // narrowest name it shows its folder and its close button alone.
    let (home_w, room, compact) = match full_room(home_full) {
        Some(room) => (home_full, room, false),
        None => (ICON, (limit - start - ICON - GAP - (n - 1.0) * (ICON + GAP) - CHROME - GAP).min(60.0), true),
    };
    let named = room >= NAME_MIN;
    // Home, a tab like the projects': tinted while it is shown.
    let home = Rect::from_min_size(pos2(start, mid - 14.0), vec2(home_w, 28.0));
    let home_resp = control(shell, ui, "workspace.header.home", "tab", home, true).on_hover_text(tr("Home"));
    if home_on {
        paint.rect_filled(home, 7.0, p.accent_tint);
    } else if home_resp.hovered() {
        paint.rect_filled(home, 7.0, p.page);
    }
    let home_ink = if home_on { p.accent } else { p.text_secondary };
    if compact {
        ux::icon_mid(&paint, home.center(), &icons::HOME, home_ink);
    } else {
        ux::icon_mid(&paint, pos2(home.left() + 16.0, mid), &icons::HOME, home_ink);
        paint.galley(pos2(home.left() + 30.0, mid - home_label.size().y / 2.0), home_label, if home_on { p.accent } else { p.text });
    }
    if home_resp.clicked() && !home_on {
        click = Some(TabClick::Home);
    }
    let mut x = home.right() + GAP;
    for (i, (name, on, closing)) in tabs.iter().enumerate() {
        // A tab whose project is closing stays, muted, until it has closed.
        let ink = if *closing {
            p.text_muted
        } else if *on {
            p.accent
        } else {
            p.text
        };
        let icon_ink = if *on && !*closing { p.accent } else { p.text_secondary };
        let as_icon = compact && !*on;
        let chip_w = if as_icon {
            ICON
        } else if named {
            CHROME + widths[i].min(room)
        } else {
            CHROME - 8.0
        };
        let chip = Rect::from_min_size(pos2(x, mid - 14.0), vec2(chip_w, 28.0));
        let resp = control(shell, ui, &format!("workspace.header.project.{i}"), "tab", chip, true).on_hover_text(name.clone());
        if as_icon {
            if *on {
                paint.rect_filled(chip, 7.0, p.accent_tint);
            } else if resp.hovered() {
                paint.rect_filled(chip, 7.0, p.page);
            }
            ux::icon_mid(&paint, chip.center(), &icons::FOLDER, icon_ink);
            if resp.clicked() && !*on {
                click = Some(TabClick::Show(i));
            }
            x = chip.right() + GAP;
            continue;
        }
        let close = Rect::from_center_size(pos2(chip.right() - 16.0, mid), Vec2::splat(20.0));
        let close_resp = control(shell, ui, &format!("workspace.header.project.{i}.close"), "button", close, true).on_hover_text(tr("Close project"));
        if *on {
            paint.rect_filled(chip, 7.0, p.accent_tint);
        } else if resp.hovered() || close_resp.hovered() {
            paint.rect_filled(chip, 7.0, p.page);
        }
        if close_resp.hovered() {
            paint.rect_filled(close, 5.0, if *on { p.surface } else { p.border });
        }
        ux::icon_mid(&paint, pos2(chip.left() + 17.0, mid), &icons::FOLDER, icon_ink);
        if named {
            let g = ux::galley(&paint, name, if *on { types::UI_STRONG } else { types::UI }, ink, Some(room));
            paint.galley(pos2(chip.left() + 32.0, mid - g.size().y / 2.0), g, ink);
        }
        ux::icon_mid(&paint, close.center(), &icons::CLOSE, icon_ink);
        if close_resp.clicked() {
            click = Some(TabClick::Close(i));
        } else if resp.clicked() && !*on {
            click = Some(TabClick::Show(i));
        }
        x = chip.right() + GAP;
    }
    let changed = click.is_some();
    match click {
        Some(TabClick::Home) => shell.go_home(),
        Some(TabClick::Show(i)) => shell.activate(i),
        Some(TabClick::Close(i)) => shell.close_tab(i),
        None => {}
    }
    (x, changed)
}

/// Undo and redo from the keyboard, through the engine, while no field has the keyboard.
fn undo_redo(shell: &mut Shell, ui: &egui::Ui) {
    if ui.memory(|m| m.focused().is_some())
        || shell.chat_panel.row_menu.is_some()
        || shell.chat_panel.ask.is_some()
        || shell.explorer.delete.is_some()
        || shell.diagrams.new.is_some()
        || shell.generate.is_some()
        || shell.about.is_some()
    {
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
    // Before the rail says the count, so its words and its badge are of one frame.
    crate::versions::refresh(shell, ui.ctx());
    let active = shell.state.explorer;
    for (i, (id, which, icon, title)) in EXPLORERS.iter().enumerate() {
        let button = Rect::from_min_size(pos2(r.left() + 10.0, r.top() + 8.0 + i as f32 * RAIL_PITCH), RAIL_BUTTON);
        // Marked while its explorer is open; a folded explorer marks none.
        let on = *which == active && !shell.state.explorer_collapsed;
        if on {
            paint.rect_filled(button, 6.0, p.accent_tint);
            paint.rect_filled(Rect::from_min_size(pos2(r.left(), button.top() + 8.0), vec2(2.0, 20.0)), 1.0, p.accent);
        }
        ux::icon_in(paint, Rect::from_center_size(button.center(), Vec2::splat(18.0)), icon, if on { p.accent } else { p.text_secondary });
        // Versions says how many working changes there are, in its tooltip and its badge.
        let words = if *which == Explorer::SourceControl { crate::versions::rail_words(shell.versions.working) } else { tr(title) };
        let resp = control(shell, ui, &format!("workspace.explorers.{id}"), "tab", button, true).on_hover_text(&words);
        let _ = resp.clone().accessible_name(&words);
        if resp.clicked() {
            shell.state.rail_explorer(*which);
        }
        if *which == Explorer::SourceControl {
            crate::versions::rail_badge(shell, ui, p, button);
        }
    }
}

/// The explorer column: Models is the tree over the session, Diagrams the open project's diagrams
/// and Versions its history.
fn explorer(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect) {
    let paint = ui.painter().clone();
    paint.rect_filled(r, 0.0, p.surface);
    paint.rect_filled(Rect::from_min_max(pos2(r.right() - 1.0, r.top()), r.max), 0.0, p.border);
    match shell.state.explorer {
        Explorer::Models => crate::explorer::draw(shell, ui, p, r),
        Explorer::Diagrams => crate::diagrams::draw(shell, ui, p, r),
        Explorer::SourceControl => crate::versions::draw(shell, ui, p, r),
    }
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
    let r = if shell.looking_only() { banner(shell, ui, p, r) } else { r };
    crate::canvas::centre(shell, ui, p, r);
}

/// The banner over the centre while a version shows, with the way back; gives the room left below.
fn banner(shell: &mut Shell, ui: &egui::Ui, p: &Palette, r: Rect) -> Rect {
    let band = Rect::from_min_size(r.min, vec2(r.width(), 44.0));
    let paint = ui.painter();
    paint.rect_filled(band, 0.0, p.accent_tint);
    ux::icon_mid(paint, pos2(band.left() + 20.0, band.center().y), &icons::HISTORY, p.accent);
    let words = crate::version_chip::read_only_words(shell);
    let back = tr("Back to current work");
    let g = ux::galley(paint, &back, types::UI, p.on_accent, None);
    let button = Rect::from_min_size(pos2(band.right() - 12.0 - g.size().x - 20.0, band.top() + 8.0), vec2(g.size().x + 20.0, 28.0));
    ux::text_mid(paint, band.left() + 36.0, band.center().y, &words, types::UI, p.accent, room(button.left() - band.left() - 48.0));
    paint.rect_filled(button, 6.0, p.accent_fill);
    paint.galley(button.center() - g.size() / 2.0, g, p.on_accent);
    if control(shell, ui, "workspace.banner.back", "button", button, true).accessible_name(&back).clicked() {
        shell.back_to_current_work();
    }
    Rect::from_min_max(pos2(r.left(), band.bottom()), r.max)
}

/// The right rail: the four panels, the active one marked.
fn right_rail(shell: &mut Shell, ui: &egui::Ui, p: &Palette, r: Rect) {
    let paint = ui.painter();
    paint.rect_filled(r, 0.0, p.page);
    paint.rect_filled(Rect::from_min_max(pos2(r.right() - 1.0, r.top()), r.max), 0.0, p.border);
    let active = shell.state.panel;
    for (i, (id, which, icon, title)) in PANELS.iter().enumerate() {
        let button = Rect::from_min_size(pos2(r.left() + 4.0, r.top() + 8.0 + i as f32 * RAIL_PITCH), RAIL_BUTTON);
        let on = *which == active && !shell.state.panel_collapsed;
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
            shell.state.rail_panel(*which);
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
    if control(shell, ui, "workspace.panel.collapse", "button", collapse, true).on_hover_text(tr("Collapse panel")).clicked() {
        shell.state.panel_collapsed = true;
    }
    paint.rect_filled(Rect::from_min_size(pos2(r.left(), r.top() + 43.0), vec2(r.width(), 1.0)), 0.0, p.border);
    let which = shell.state.panel;
    if which == Panel::Agent {
        crate::chat_view::header(shell, ui, p, r);
    }
    if shell.looking_only() && which != Panel::Agent {
        read_only_badge(ui, p, r);
    }
    let editable = which == Panel::Agent || !shell.looking_only();
    ui.add_enabled_ui(editable, |ui| crate::panels::draw(shell, ui, p, r, 58.0, which, name));
}

/// The badge at the head of Properties while a version shows.
fn read_only_badge(ui: &egui::Ui, p: &Palette, r: Rect) {
    let paint = ui.painter();
    let g = ux::galley(paint, &tr("Read only"), types::CAPTION, p.accent, None);
    let pill = Rect::from_min_size(pos2(r.right() - 48.0 - g.size().x - 34.0, r.top() + 10.0), vec2(g.size().x + 34.0, 24.0));
    paint.rect_filled(pill, 12.0, p.accent_tint);
    ux::icon_in(paint, Rect::from_center_size(pos2(pill.left() + 16.0, pill.center().y), Vec2::splat(12.0)), &icons::LOCK, p.accent);
    paint.galley(pos2(pill.left() + 26.0, pill.center().y - g.size().y / 2.0), g, p.accent);
}

/// Chat's agent access in the status bar (the design's "Agent" item), its right edge at `right`:
/// an accent dot and "Chat is using the model" while a conversation holds it, a purple dot and
/// "Agent access off" while none does. A control like any other (pointer, Tab, Enter, its id);
/// either opens Settings on AI Agents, where the agents are set up. Gives its left edge.
fn agent_status(shell: &mut Shell, ui: &egui::Ui, p: &Palette, bar: Rect, right: f32) -> f32 {
    let held = shell.chats.holds_access();
    let (text, dot) = if held { (tr("Chat is using the model"), p.accent) } else { (tr("Agent access off"), p.purple) };
    let paint = ui.painter();
    let g = ux::galley(paint, &text, types::CAPTION, p.text_secondary, None);
    // The design's frame: 8 before the dot, the 6 dot, 9 to the text, 24 after it; 32 tall.
    let r = Rect::from_min_max(pos2(right - (8.0 + 6.0 + 9.0 + g.size().x + 24.0), bar.center().y - 16.0), pos2(right, bar.center().y + 16.0));
    let resp = control(shell, ui, "workspace.status.agent", "button", r, true).on_hover_text(tr("AI Agents"));
    let _ = resp.clone().accessible_name(&text);
    if resp.hovered() {
        paint.rect_filled(r, 6.0, p.page);
    }
    crate::settings::focus_ring(paint, p, r, 6.0, &resp);
    paint.circle_filled(pos2(r.left() + 11.0, r.center().y), 3.0, dot);
    paint.galley(pos2(r.left() + 23.0, r.center().y - g.size().y / 2.0), g, p.text_secondary);
    if resp.clicked() {
        crate::settings::open_at(shell, crate::settings::Section::Agents);
    }
    r.left()
}

/// The status bar: what is true about the project at the left, where it lives at the right.
fn status_bar(shell: &mut Shell, ui: &egui::Ui, p: &Palette, r: Rect) {
    let paint = ui.painter();
    paint.rect_filled(r, 0.0, p.surface);
    paint.rect_filled(Rect::from_min_size(r.min, vec2(r.width(), 1.0)), 0.0, p.border);
    // Saving: every change is written to the project a moment after it is made;
    // Save commits it. The status says which, and why a write or a save failed.
    let writing = match (&shell.services.projects, &shell.project) {
        (Some(h), Some(project)) => h.writing(&project.path),
        _ => Default::default(),
    };
    let chord = if ui.ctx().os() == egui::os::OperatingSystem::Mac { "⌘S" } else { "Ctrl S" };
    let mut detail: Option<String> = None;
    let (dot, text) = match &shell.session {
        None => (p.text_muted, tr("The model could not be opened")),
        Some(_) if shell.looking_only() => (p.accent, crate::version_chip::read_only_words(shell)),
        Some(_) if shell.is_saving() => (p.accent, tr("Saving…")),
        // Open works from the workspace too (Cmd+O, File ▸ Open…): its wait and its failure show
        // here, and the project stays open.
        Some(_) if shell.is_opening() => (p.accent, tr("Opening…")),

        Some(_) if shell.open_error.is_some() => {
            let why = shell.open_error.clone().unwrap_or_default();
            (p.danger, crate::i18n::fmt(&tr("The project could not be opened: {why}"), &[("why", &why)]))
        }
        Some(_) if shell.picker_error.is_some() => (p.danger, shell.picker_error.clone().unwrap_or_default()),
        Some(_) if shell.save_error.is_some() => {
            detail = shell.save_error.clone();
            (p.danger, tr("Not saved"))
        }
        Some(_) if writing.error.is_some() => {
            detail = writing.error.clone();
            (p.danger, tr("Changes could not be written to the project"))
        }
        Some(_) if shell.close_error().is_some() => {
            let why = shell.close_error().unwrap_or_default().to_string();
            (p.danger, crate::i18n::fmt(&tr("The project could not be closed: {why}"), &[("why", &why)]))
        }
        // A description typed in Properties and not yet written, or refused, is unsaved work too:
        // never Saved while it waits.
        Some(s) if s.is_dirty() || crate::panels::description_pending(&shell.panels) => {
            (p.amber, crate::i18n::fmt(&tr("Unsaved changes · {chord} saves"), &[("chord", chord)]))
        }
        Some(_) => (p.green, tr("Saved")),
    };
    // Beside the status, not in its place: it stays true however the status changes.
    let detail = detail.or_else(|| shell.project.as_ref().and_then(|o| o.notice.as_ref()).map(crate::BranchNotice::words));
    paint.circle_filled(pos2(r.left() + 24.0, r.center().y), 3.0, dot);
    let y = r.center().y - 8.0;
    // The location first, right-aligned, then Chat's agent access beside it; the status and its
    // reason take what is left.
    let mut right = r.right() - 24.0;
    if let Some(location) = shell.location() {
        let g = ux::galley(paint, &location, types::CAPTION, p.text_secondary, room(r.width() / 2.0 - 60.0));
        right -= g.size().x;
        paint.galley(pos2(right, y), g, p.text_secondary);
        right -= 8.0;
    }
    if shell.services.agents.is_some() {
        right = agent_status(shell, ui, p, r, right) - 8.0;
    } else if shell.location().is_some() {
        right -= 16.0;
    }
    let start = r.left() + 36.0;
    let status = ux::text(paint, pos2(start, y), &text, types::CAPTION, p.text_secondary, room(right - start));
    if let Some(why) = detail {
        // The host's own words, small and untranslated.
        ux::text(paint, pos2(status.right() + 12.0, y), &why, types::CAPTION, p.text_muted, room(right - status.right() - 12.0));
    }
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
        let wide = columns(body(1600.0), Some(EXPLORER_DEFAULT), Some(PANEL_DEFAULT));
        assert!(wide.explorer.is_some() && wide.panel.is_some());
        assert_eq!(wide.centre.width(), 1600.0 - ACTIVITY - EXPLORER_DEFAULT - PANEL_DEFAULT - RAIL);
        // The rail stands between the centre and the panel, which closes the window's edge.
        assert_eq!(wide.rail.left(), wide.centre.right());
        assert_eq!(wide.panel.map(|r| r.left()), Some(wide.rail.right()));
        assert_eq!(wide.panel.map(|r| r.right()), Some(1600.0));
        let medium = columns(body(900.0), Some(EXPLORER_DEFAULT), Some(PANEL_DEFAULT));
        assert!(medium.explorer.is_some() && medium.panel.is_none());
        let narrow = columns(body(500.0), Some(EXPLORER_DEFAULT), Some(PANEL_DEFAULT));
        assert!(narrow.explorer.is_none() && narrow.panel.is_none());
        assert_eq!(narrow.centre.width(), 500.0 - ACTIVITY - RAIL);
        assert_eq!(narrow.rail.right(), 500.0);
        let clamped = columns(body(1600.0), Some(10_000.0), Some(-5.0));
        assert_eq!(clamped.explorer.map(|r| r.width()), Some(EXPLORER_RANGE.1));
        assert_eq!(clamped.panel.map(|r| r.width()), Some(PANEL_RANGE.0));
        // A folded column gives its width to the centre; the panel alone fits where both did not.
        let folded = columns(body(1600.0), None, Some(PANEL_DEFAULT));
        assert!(folded.explorer.is_none());
        assert_eq!(folded.centre.width(), 1600.0 - ACTIVITY - PANEL_DEFAULT - RAIL);
        let panel_alone = columns(body(900.0), None, Some(PANEL_DEFAULT));
        assert!(panel_alone.panel.is_some());
        let none = columns(body(1600.0), None, None);
        assert_eq!(none.centre.width(), 1600.0 - ACTIVITY - RAIL);
    }
}
