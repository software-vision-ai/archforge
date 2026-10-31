//! Home, the screen the application starts on: the design's launch screen for a returning user,
//! in Light and Dark. It fills the window on its own: the title strip on top (on macOS the
//! window's title bar, the traffic lights at its left), the bar under it, and the page below them
//! scrolling.
//!
//! The geometry is the design's, in its pixels: the strip is 28 px tall, the bar 48, the page keeps
//! 72 px margins and every position on it is measured from the bar's bottom edge. Where the window
//! is not the design's 1600 px wide, these rules apply: the start cards share the
//! width (four, two or one to a row) and what the design anchors right stays right.
//!
//! The colours are the palette of the shell's theme (Light or Dark); the strip's theme switch
//! changes it. The New project card opens its dialog. Controls whose screens are not built yet
//! are drawn as designed, do nothing when clicked and say so on hover.

use egui::{Color32, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2, pos2, vec2};

use crate::i18n::tr;
use crate::state::{RecentSort, RecentView};
use crate::theme::ThemeKind;
use crate::ux::{self, Palette, icons, types};
use crate::{Shell, widgets};

/// The title strip's height: on macOS the strip is the title bar, and at 32 points the traffic
/// lights sit on its middle line.
pub(crate) const STRIP: f32 = 32.0;
/// Where the strip's controls start: clear of the traffic lights on macOS, and
/// elsewhere.
const STRIP_LEFT_MAC: f32 = 78.0;
const STRIP_LEFT: f32 = 10.0;
/// The bar's height, its bottom divider included.
pub(crate) const BAR: f32 = 48.0;
/// The page's side margins.
const MARGIN: f32 = 72.0;
/// The gap between start cards.
const GAP: f32 = 24.0;
const CARD: Vec2 = vec2(346.0, 170.0);
/// The content widths below which the start cards go two, then one, to a row.
const TWO_UP: f32 = 1100.0;
const ONE_UP: f32 = 560.0;
/// A recent project's row and the distance from one row's top to the next.
const ROW: f32 = 52.0;
const ROW_PITCH: f32 = 56.0;
/// A gallery card's height.
const CARD_HEIGHT: f32 = 200.0;
/// A button's height, padding and the gap between its icon and its label.
const BUTTON: f32 = 40.0;
const BUTTON_PAD: f32 = 16.0;
const BUTTON_GAP: f32 = 8.0;

/// How long ago `then` was at `now` (seconds since 1970), as a recent project's row says it.
pub(crate) fn ago(then: u64, now: u64) -> String {
    let s = now.saturating_sub(then);
    match s {
        0..60 => tr("Just now"),
        60..3_600 => crate::i18n::trn("{n} minute ago", "{n} minutes ago", s / 60),
        3_600..86_400 => crate::i18n::trn("{n} hour ago", "{n} hours ago", s / 3_600),
        86_400..172_800 => tr("Yesterday"),
        _ => crate::i18n::trn("{n} day ago", "{n} days ago", s / 86_400),
    }
}

/// The colours of a start card or a model thumbnail: its ink and its tint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Hue {
    Blue,
    Green,
    Purple,
    Amber,
}

impl Hue {
    fn ink(self, p: &Palette) -> Color32 {
        match self {
            Hue::Blue => p.accent,
            Hue::Green => p.green,
            Hue::Purple => p.purple,
            Hue::Amber => p.amber,
        }
    }
    fn tint(self, p: &Palette) -> Color32 {
        match self {
            Hue::Blue => p.accent_tint,
            Hue::Green => p.green_tint,
            Hue::Purple => p.purple_tint,
            Hue::Amber => p.amber_tint,
        }
    }
}

/// Whether a start card leads anywhere yet: New project opens its dialog; the others open screens
/// not built so far, so they take no click and show no hover border.
fn start_live(shell: &Shell, id: &str) -> bool {
    (id == "new-project" && shell.tabs.len() < crate::MAX_TABS) || (id == "open-project" && can_open(shell))
}

/// Whether projects can be opened: the host picks and opens them, and a tab is free.
pub(crate) fn can_open(shell: &Shell) -> bool {
    shell.services.pick_project.is_some() && shell.services.projects.is_some() && shell.tabs.len() < crate::MAX_TABS
}

/// The four start cards: automation id, title, line, link, icon and colour.
const STARTS: [(&str, &str, &str, &str, &ux::Icon, Hue); 4] = [
    ("new-project", "New project", "A clean start for your next idea.", "Create project", &icons::PLUS, Hue::Blue),
    ("open-project", "Open a project", "Continue from a project on this device.", "Browse files", &icons::FOLDER, Hue::Green),
    ("example", "Explore an example", "Learn by navigating a connected model.", "Explore example", &icons::MAP, Hue::Purple),
    ("import", "Import a model", "Bring existing elements into a project.", "Choose a file", &icons::IMPORT, Hue::Amber),
];

/// The start cards left off Home while what they lead to cannot be done at all: importing a
/// model. Their texts stay in the catalogues for when they come back.
const HIDDEN_STARTS: [&str; 1] = ["import"];

/// The start cards Home shows.
fn shown_starts() -> Vec<&'static (&'static str, &'static str, &'static str, &'static str, &'static ux::Icon, Hue)> {
    STARTS.iter().filter(|s| !HIDDEN_STARTS.contains(&s.0)).collect()
}

/// Every text of the start cards to translate: they reach `tr` through the table, where the
/// catalogue tests cannot find them as literals.
#[cfg(test)]
pub(crate) fn start_texts() -> impl Iterator<Item = &'static str> {
    STARTS.iter().flat_map(|(_, title, line, link, _, _)| [*title, *line, *link])
}

pub fn show(shell: &mut Shell, ui: &mut egui::Ui) {
    let p = *ux::palette(shell.state.theme.is_dark());
    egui::CentralPanel::default().frame(egui::Frame::NONE.fill(p.page)).show(ui, |ui| {
        let full = ui.max_rect();
        // The design's faces are installed at the end of the first frame; until then the page is
        // drawn bare, as text in a family egui does not have yet would stop the program.
        if !ux::ready(ui.ctx()) {
            ui.ctx().request_repaint();
            return;
        }
        let strip = Rect::from_min_size(full.min, vec2(full.width(), STRIP));
        let bar = Rect::from_min_size(pos2(full.left(), strip.bottom()), vec2(full.width(), BAR));
        crate::home_search::shortcut(shell, ui.ctx());
        // The bar first, so the strip's drag area does not cover it; a tab clicked ends the frame.
        if !bar_row(shell, ui, &p, bar) {
            return;
        }
        title_strip(shell, ui, &p, strip, true);
        let body = Rect::from_min_max(pos2(full.left(), bar.bottom()), full.max);
        let mut page = ui.new_child(egui::UiBuilder::new().max_rect(body).layout(egui::Layout::top_down(egui::Align::Min)));
        page.set_clip_rect(body);
        // The design draws no scroll bar; a thin floating one shows while the page scrolls or the
        // pointer is near it.
        page.spacing_mut().scroll = egui::style::ScrollStyle::floating();
        egui::ScrollArea::vertical().id_salt("home.page").auto_shrink([false, false]).show(&mut page, |ui| {
            let origin = ui.max_rect().min;
            let height = content(shell, ui, &p, origin, body.width(), body.height());
            ui.allocate_rect(Rect::from_min_size(origin, vec2(body.width(), height)), Sense::hover());
        });
    });
}

/// A control's hit area, recorded for automation. A control whose screen is not built yet says so
/// on hover and its click is never taken.
pub(crate) fn control(shell: &mut Shell, ui: &egui::Ui, id: &str, kind: &'static str, rect: Rect, live: bool) -> egui::Response {
    widgets::record_visible(ui, &mut shell.registry, id, kind, rect);
    let resp = ui.interact(rect, widgets::egui_id(id), Sense::click());
    if live { resp.on_hover_cursor(egui::CursorIcon::PointingHand) } else { resp.on_hover_text(tr("Not available yet")) }
}

/// The title strip: the active Home button and Open project at the left, beside the traffic
/// lights on macOS, and the theme switch at the right. On macOS the strip is the window's title
/// bar, so dragging it moves the window and a double click zooms it.
/// False when the Home button closed the project.
pub(crate) fn title_strip(shell: &mut Shell, ui: &egui::Ui, p: &Palette, r: Rect, home_active: bool) -> bool {
    let paint = ui.painter();
    paint.rect_filled(r, 0.0, p.title_strip);
    if shell.top_inset > 0.0 {
        let drag = ui.interact(r, ui.scope_id().with("home.title-drag"), Sense::click_and_drag());
        if drag.drag_started() {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }
        if drag.double_clicked() {
            let zoomed = ui.ctx().input(|i| i.viewport().maximized.unwrap_or(false));
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Maximized(!zoomed));
        }
    }
    let left = r.left() + if shell.top_inset > 0.0 { STRIP_LEFT_MAC } else { STRIP_LEFT };
    // Every control is centred on the strip's middle line, 24-point hit boxes.
    let mid = r.center().y;
    // Home: the active place on the start screen, in the accent colour; in the workspace the way
    // back to it.
    let home = Rect::from_center_size(pos2(left + 12.0, mid), Vec2::splat(24.0));
    let home_resp = control(shell, ui, "strip.home", "tab", home, true).on_hover_text(tr("Home"));
    if home_resp.hovered() && !home_active {
        paint.rect_filled(home, 6.0, p.border);
    }
    ux::icon_mid(paint, home.center(), &icons::HOME, if home_active { p.accent } else { p.text_secondary });
    if home_resp.clicked() && !home_active {
        // Home shows; the open projects stay open in their tabs.
        shell.go_home();
        return false;
    }
    // Open project, a ghost button beside it: the plus and its label on the same line.
    let ghost = home.right() + 6.0;
    ux::icon_mid(paint, pos2(ghost + 13.0, mid), &icons::PLUS, p.text_secondary);
    let label = ux::text_mid(paint, ghost + 24.0, mid, &tr("Open project"), types::UI, p.text, None);
    let ghost_rect = Rect::from_min_max(pos2(ghost, mid - 12.0), pos2(label.right() + 8.0, mid + 12.0));
    let live = can_open(shell);
    if control(shell, ui, "strip.open-project", "button", ghost_rect, live).clicked() && live {
        shell.open_from_picker();
    }
    // The theme switch: the moon in Light, the sun in Dark; a click changes the appearance.
    let dark = shell.state.theme.is_dark();
    let switch = Rect::from_center_size(pos2(r.right() - 24.0, mid), Vec2::splat(24.0));
    let hint = if dark { tr("Switch to the light appearance") } else { tr("Switch to the dark appearance") };
    let resp = control(shell, ui, "strip.theme", "button", switch, true).on_hover_text(hint);
    if resp.hovered() {
        paint.rect_filled(switch, 6.0, p.border);
    }
    ux::icon_mid(paint, switch.center(), if dark { &icons::SUN } else { &icons::MOON }, p.text_secondary);
    if resp.clicked() {
        shell.state.theme = if dark { ThemeKind::Light } else { ThemeKind::Dark };
    }
    true
}

/// The bar: the brand at the left, then the tabs while projects are open; the search field, help
/// and the avatar at the right. False when a tab was clicked (the frame ends).
fn bar_row(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect) -> bool {
    let paint = ui.painter();
    paint.rect_filled(r, 0.0, p.surface);
    paint.rect_filled(Rect::from_min_max(pos2(r.left(), r.bottom() - 1.0), r.max), 0.0, p.border);
    let wordmark = brand(paint, p, r);
    let mut end = wordmark.right();
    if !shell.tabs.is_empty() {
        let (tabs_end, changed) = crate::workspace::project_tabs(shell, ui, p, r, wordmark.right());
        if changed {
            return false;
        }
        end = tabs_end;
    }
    bar_tail(shell, ui, p, r, end + 24.0, "home.header", &tr("Search projects and actions"), "field");
    true
}

/// The bar's middle line: everything on the bar is centred on it (its bottom divider excluded).
pub(crate) fn bar_mid(r: Rect) -> f32 {
    r.top() + (r.height() - 1.0) / 2.0
}

/// The brand at the bar's left: the mark and the wordmark on the bar's middle line; gives the
/// wordmark's rectangle.
pub(crate) fn brand(paint: &egui::Painter, p: &Palette, r: Rect) -> Rect {
    let mid = bar_mid(r);
    let mark = Rect::from_center_size(pos2(r.left() + 28.0, mid), Vec2::splat(24.0));
    paint.rect_filled(mark, 6.0, p.accent_fill);
    ux::icon_exact(paint, mark.center() - vec2(11.33, 10.67) / 2.0, &icons::MARK_A, 11.33, 1.6, p.on_accent);
    ux::text_mid(paint, mark.right() + 10.0, mid, "Archforge", types::LEAD_STRONG, p.text, None)
}

/// The bar's right: the search field from `start` at the earliest, help and the avatar, all on
/// the bar's middle line. The controls' ids start with `prefix`.
#[allow(clippy::too_many_arguments, reason = "the bar's two screens differ in their ids, placeholder and search kind")]
pub(crate) fn bar_tail(
    shell: &mut Shell,
    ui: &mut egui::Ui,
    p: &Palette,
    r: Rect,
    start: f32,
    prefix: &str,
    placeholder: &str,
    search_kind: &'static str,
) {
    let paint = ui.painter().clone();
    let paint = &paint;
    let mid = bar_mid(r);
    let right = r.right();
    if let Some(initials) = shell.services.user_name.as_deref().and_then(initials) {
        let avatar = Rect::from_center_size(pos2(right - 30.0, mid), Vec2::splat(28.0));
        paint.rect_filled(avatar, 14.0, p.accent_tint);
        let g = ux::galley(paint, &initials, types::CAPTION_STRONG, p.accent, None);
        paint.galley(avatar.center() - g.size() / 2.0, g, p.accent);
        control(shell, ui, &format!("{prefix}.account"), "button", avatar, false);
    }
    let help = Rect::from_center_size(pos2(right - 68.0, mid), Vec2::splat(28.0));
    ux::icon_mid(paint, help.center(), &icons::HELP, p.text_secondary);
    control(shell, ui, &format!("{prefix}.help"), "button", help, false);
    // The search field keeps its distance from the left part when the window is narrow.
    let end = help.left() - 12.0;
    let start = (end - 300.0).max(start);
    if end - start >= 160.0 {
        let field = Rect::from_min_max(pos2(start, mid - 15.0), pos2(end, mid + 15.0));
        // Home's field searches; the workspace's waits for its own search.
        if prefix == "home.header" {
            crate::home_search::field(shell, ui, p, field, placeholder);
            return;
        }
        paint.rect_filled(field, 15.0, p.page);
        paint.rect_stroke(field, 15.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
        ux::icon_mid(paint, pos2(field.left() + 18.0, mid), &icons::SEARCH, p.text_secondary);
        let chord = if ui.ctx().os() == egui::os::OperatingSystem::Mac { "⌘K" } else { "Ctrl K" };
        let key = ux::galley(paint, chord, types::CAPTION, p.text_muted, None);
        let key_at = pos2(field.right() - 14.0 - key.size().x, mid - key.size().y / 2.0);
        let room = (key_at.x - 8.0 - (field.left() + 32.0)).max(0.0);
        ux::text_mid(paint, field.left() + 32.0, mid, placeholder, types::UI, p.text_secondary, Some(room));
        paint.galley(key_at, key, p.text_muted);
        control(shell, ui, &format!("{prefix}.search"), search_kind, field, false);
    }
}

/// Two letters for the avatar: the first of the first and the last name, or the first two of a
/// single name; none for a name with no letters.
pub(crate) fn initials(name: &str) -> Option<String> {
    let words: Vec<&str> = name.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).collect();
    let first = |w: &str| w.chars().next();
    let out: String = match words.as_slice() {
        [] => return None,
        [one] => one.chars().take(2).collect(),
        [a, .., z] => first(a).into_iter().chain(first(z)).collect(),
    };
    Some(out.to_uppercase())
}

/// A button as the design draws it, `BUTTON` tall at `at`: an optional icon,
/// then the label, `BUTTON_PAD` either side, hugging its content. Gives its rect.
fn button_rect(paint: &egui::Painter, at: Pos2, label: &str, icon: bool) -> (Rect, std::sync::Arc<egui::Galley>) {
    let g = ux::galley(paint, label, types::UI_STRONG, Color32::PLACEHOLDER, None);
    let icon_w = if icon { 18.0 + BUTTON_GAP } else { 0.0 };
    (Rect::from_min_size(at, vec2(BUTTON_PAD * 2.0 + icon_w + g.size().x, BUTTON)), g)
}

/// The page below the bar; gives its height.
fn content(shell: &mut Shell, ui: &egui::Ui, p: &Palette, origin: Pos2, width: f32, view_height: f32) -> f32 {
    let paint = ui.painter().clone();
    let left = origin.x + MARGIN;
    let right = origin.x + width - MARGIN;
    let inner = (right - left).max(0.0);
    let at = |x: f32, y: f32| pos2(left + x, origin.y + y);
    // The welcome. Opening a project is the start card's and the title strip's.
    ux::text(&paint, at(0.0, 39.0), &tr("Welcome home").to_uppercase(), types::EYEBROW, p.accent, None);
    ux::text(&paint, at(0.0, 67.0), &tr("Welcome to Archforge"), types::DISPLAY, p.text, Some(inner));
    ux::text(
        &paint,
        at(0.0, 124.0),
        &tr("Start with the big picture. Or with one detail. Bring it all together."),
        types::LEAD,
        p.text_secondary,
        Some(inner),
    );
    // Why the last opening failed: the host's words, at the right, level with the title.
    if let Some(why) = shell.open_error.clone() {
        let line = crate::i18n::fmt(&tr("The project could not be opened: {why}"), &[("why", &why)]);
        let g = ux::galley_rows(&paint, &line, types::CAPTION, p.danger, Some(inner.clamp(0.0, 480.0)), 2);
        paint.galley(pos2(right - g.size().x, origin.y + 81.0), g, p.danger);
    }
    let mut y = 175.0;
    if !shell.state.home_banner_closed {
        banner(shell, ui, p, &paint, Rect::from_min_size(at(0.0, y), vec2(inner, 92.0)));
        y += 128.0;
    }
    // Start something.
    ux::text(&paint, at(0.0, y), &tr("Start something"), types::SECTION, p.text, Some(inner));
    ux::text(
        &paint,
        at(0.0, y + 33.0),
        &tr("Choose a starting point. You can change direction at any time."),
        types::UI,
        p.text_secondary,
        Some(inner),
    );
    let starts = shown_starts();
    // As many a row as the width takes, and no more columns than cards.
    let per_row: usize = if inner >= TWO_UP {
        4
    } else if inner >= ONE_UP {
        2
    } else {
        1
    }
    .min(starts.len().max(1));
    let card_w = (inner - GAP * (per_row as f32 - 1.0)) / per_row as f32;
    let cards_top = y + 72.0;
    for (i, start) in starts.iter().enumerate() {
        let (row, col) = (i / per_row, i % per_row);
        let r = Rect::from_min_size(at(col as f32 * (card_w + GAP), cards_top + row as f32 * (CARD.y + GAP)), vec2(card_w, CARD.y));
        start_card(shell, ui, p, &paint, r, start);
    }
    let rows = starts.len().div_ceil(per_row);
    y = cards_top + rows as f32 * CARD.y + (rows as f32 - 1.0) * GAP + 38.0;
    // Pinned folders.
    ux::text(&paint, at(0.0, y), &tr("Pinned folders"), types::SUBSECTION, p.text, Some(inner));
    ux::text(&paint, at(0.0, y + 30.0), &tr("Keep your project locations close."), types::UI, p.text_secondary, Some(inner));
    let mut x = 0.0;
    let mut tiles_top = y + 70.0;
    for (i, folder) in shell.state.pinned.clone().iter().enumerate() {
        if x > 0.0 && x + 328.0 > inner {
            x = 0.0;
            tiles_top += 86.0;
        }
        pinned_tile(shell, ui, p, &paint, Rect::from_min_size(at(x, tiles_top), vec2(328.0f32.min(inner), 66.0)), i, folder);
        x += 348.0;
    }
    if x > 0.0 && x + 234.0 > inner {
        x = 0.0;
        tiles_top += 86.0;
    }
    if shell.state.pinned.len() < crate::state::MAX_PINNED {
        let pin = Rect::from_min_size(at(x, tiles_top), vec2(234.0f32.min(inner), 66.0));
        let live = shell.services.pick_folder.is_some();
        let resp = control(shell, ui, "home.pin-folder", "button", pin, live);
        paint.rect_stroke(pin, 10.0, Stroke::new(1.0, if live && resp.hovered() { p.border_hover } else { p.border }), StrokeKind::Inside);
        ux::icon(&paint, pin.min + vec2(20.0, 24.0), &icons::PLUS, 18.0, p.text_secondary);
        ux::text(&paint, pin.min + vec2(52.0, 23.0), &tr("Pin a folder"), types::BODY, p.text_secondary, Some(pin.width() - 62.0));
        if live && resp.clicked() {
            shell.pin_from_picker();
        }
    }
    y = tiles_top + 106.0;
    // Recent projects: the heading at the left, sorting and the view switch on its line.
    ux::text(&paint, at(0.0, y), &tr("Recent projects"), types::SECTION, p.text, Some(inner - 280.0));
    ux::text(&paint, at(0.0, y + 32.0), &tr("Pick up in the exact view you left."), types::UI, p.text_secondary, Some(inner - 280.0));
    let line = origin.y + y + 14.5;
    let switch = Rect::from_min_size(pos2(right - 118.0, line - 19.0), vec2(118.0, 38.0));
    let chevron = Rect::from_min_size(pos2(switch.left() - 43.5, line - 8.0), Vec2::splat(16.0));
    let sort_word = match shell.state.recent_sort {
        RecentSort::LastOpened => tr("Last opened"),
        RecentSort::Name => tr("Name"),
    };
    let sort_label = ux::galley(&paint, &sort_word, types::SMALL, p.text_secondary, None);
    let sort_at = pos2(chevron.left() - 20.5 - sort_label.size().x, line - 8.5);
    let sort = Rect::from_min_max(pos2(sort_at.x - 8.0, switch.top()), pos2(chevron.right() + 8.0, switch.bottom()));
    let sort_open = shell.panels.menu == Some(crate::panels::Menu::RecentSort);
    let sort_resp = control(shell, ui, "home.recent.sort", "button", sort, true).on_hover_text(tr("Sort"));
    if sort_resp.hovered() || sort_open {
        paint.rect_filled(sort, 8.0, p.surface);
    }
    paint.galley(sort_at, sort_label, p.text_secondary);
    ux::icon(&paint, chevron.min + vec2(4.0, 6.0), &icons::CHEVRON_DOWN, 8.0, p.text_secondary);
    if sort_resp.clicked() {
        shell.panels.menu = if sort_open { None } else { Some(crate::panels::Menu::RecentSort) };
    }
    paint.rect_filled(switch, 8.0, p.surface);
    paint.rect_stroke(switch, 8.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
    let list = Rect::from_min_size(switch.min + vec2(4.0, 4.0), vec2(51.0, 30.0));
    let grid = Rect::from_min_size(switch.min + vec2(63.0, 4.0), vec2(51.0, 30.0));
    for (b, view, icon, key, hint) in
        [(list, RecentView::List, &icons::LIST, "list", tr("List")), (grid, RecentView::Gallery, &icons::GRID, "grid", tr("Gallery"))]
    {
        let on = shell.state.recent_view == view;
        if on {
            paint.rect_filled(b, 5.0, p.accent_tint);
        }
        ux::icon_in(&paint, b, icon, if on { p.accent } else { p.text_secondary });
        if control(shell, ui, &format!("home.recent.{key}"), "toggle", b, true).on_hover_text(hint).clicked() {
            shell.state.recent_view = view;
        }
    }
    let mut row_top = y + 75.0;
    if shell.state.recent.is_empty() {
        ux::text(&paint, at(0.0, row_top), &tr("No recent projects yet"), types::UI, p.text_secondary, Some(inner));
        ux::text(&paint, at(0.0, row_top + 28.0), &tr("Projects you create or open will appear here."), types::SMALL, p.text_secondary, Some(inner));
        row_top += 45.0 + 11.0;
    } else {
        let now = now();
        let projects = sorted_recent(&shell.state.recent, shell.state.recent_sort);
        match shell.state.recent_view {
            RecentView::List => {
                for (i, project) in projects.iter().enumerate() {
                    let row = Rect::from_min_size(at(0.0, row_top), vec2(inner, ROW));
                    if recent_row(shell, ui, p, &paint, row, i, project, now) {
                        shell.open_path(&project.path);
                    }
                    row_top += ROW_PITCH;
                }
                row_top += ROW - ROW_PITCH + 11.0;
            }
            RecentView::Gallery => {
                // The cards stand in the start cards' columns, so the page lines up.
                let columns = per_row as f32;
                let width = ((inner - (columns - 1.0) * GAP) / columns).max(0.0);
                for (i, project) in projects.iter().enumerate() {
                    let (row, column) = ((i as f32 / columns).floor(), i as f32 % columns);
                    let card = Rect::from_min_size(at(column * (width + GAP), row_top + row * (CARD_HEIGHT + GAP)), vec2(width, CARD_HEIGHT));
                    if recent_card(shell, ui, p, &paint, card, i, project, now) {
                        shell.open_path(&project.path);
                    }
                }
                let rows = (projects.len() as f32 / columns).ceil();
                row_top += rows * (CARD_HEIGHT + GAP) - GAP + 11.0;
            }
        }
    }
    if shell.panels.menu == Some(crate::panels::Menu::RecentSort) {
        let anchor = Rect::from_min_max(pos2((sort.right() - 160.0).min(sort.left()), sort.top()), sort.max);
        let items = [tr("Last opened"), tr("Name")];
        match crate::panels::menu(shell, ui, p, anchor, "home.recent.sort", &items) {
            Some(0) => shell.state.recent_sort = RecentSort::LastOpened,
            Some(_) => shell.state.recent_sort = RecentSort::Name,
            None => {}
        }
    }
    // The footer: a rule and two lines, after the page or at the window's bottom, whichever is
    // lower.
    let rule = row_top.max(view_height - 31.0);
    paint.rect_filled(Rect::from_min_size(at(0.0, rule), vec2(inner, 1.0)), 0.0, p.border);
    ux::text(&paint, at(0.0, rule + 10.0), &tr("Your models, your workspace."), types::TINY, p.text_secondary, Some(inner / 2.0));
    let help = ux::galley(&paint, &tr("Help & shortcuts"), types::TINY, p.text_secondary, None);
    let help_at = pos2(right - help.size().x, origin.y + rule + 10.0);
    let help_rect = Rect::from_min_size(help_at, help.size());
    paint.galley(help_at, help, p.text_secondary);
    control(shell, ui, "home.footer.help", "link", help_rect.expand(4.0), false);
    rule + 31.0
}

/// The dismissible community banner; its close button works and is remembered.
fn banner(shell: &mut Shell, ui: &egui::Ui, p: &Palette, paint: &egui::Painter, r: Rect) {
    paint.rect_filled(r, 12.0, p.surface);
    paint.rect_stroke(r, 12.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
    let tile = Rect::from_min_size(r.min + vec2(20.0, 22.0), Vec2::splat(48.0));
    paint.rect_filled(tile, 12.0, p.accent_tint);
    ux::icon_in_tile(paint, tile, &icons::CHAT, p.accent);
    // The two buttons hug their labels; the second ends 68 px before the banner's right edge.
    let (whats_new, new_label) = button_rect(paint, Pos2::ZERO, &tr("What’s new"), false);
    let whats_new = whats_new.translate(vec2(r.right() - 68.0 - whats_new.width(), r.top() + 26.0));
    let (community, community_label) = button_rect(paint, Pos2::ZERO, &tr("Community"), true);
    let community = community.translate(vec2(whats_new.left() - 12.0 - community.width(), r.top() + 26.0));
    let room = Some((community.left() - r.left() - 86.0 - 24.0).max(0.0));
    ux::text(paint, r.min + vec2(86.0, 21.5), &tr("Build better, together."), types::LEAD_STRONG, p.text, room);
    ux::text(
        paint,
        r.min + vec2(86.0, 51.5),
        &tr("Get help, share ideas, and help shape the future of Archforge."),
        types::UI,
        p.text_secondary,
        room,
    );
    for (id, b, label, chat) in [("community", community, community_label, true), ("whats-new", whats_new, new_label, false)] {
        paint.rect_filled(b, 8.0, p.surface);
        paint.rect_stroke(b, 8.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
        let mut x = b.left() + BUTTON_PAD;
        if chat {
            ux::icon(paint, pos2(x, b.top() + 11.0), &icons::CHAT, 18.0, p.text);
            x += 18.0 + BUTTON_GAP;
        }
        paint.galley(pos2(x, b.top() + 10.5), label, p.text);
        control(shell, ui, &format!("home.banner.{id}"), "button", b, false);
    }
    let close = Rect::from_min_size(pos2(r.right() - 34.0, r.top() + 20.0), Vec2::splat(14.0));
    let resp = control(shell, ui, "home.banner.close", "button", close.expand(8.0), true).on_hover_text(tr("Close"));
    ux::icon(paint, close.min, &icons::CLOSE, 14.0, if resp.hovered() { p.text } else { p.text_muted });
    if resp.clicked() {
        shell.state.home_banner_closed = true;
    }
}

fn start_card(
    shell: &mut Shell,
    ui: &egui::Ui,
    p: &Palette,
    paint: &egui::Painter,
    r: Rect,
    (id, title, line, link, icon, hue): &(&str, &str, &str, &str, &ux::Icon, Hue),
) {
    let live = start_live(shell, id);
    let resp = control(shell, ui, &format!("home.start.{id}"), "card", r, live);
    let border = if live && resp.hovered() { p.border_hover } else { p.border };
    if live && resp.clicked() {
        if *id == "open-project" {
            shell.open_from_picker();
        } else {
            crate::new_project::open(shell);
        }
    }
    paint.rect_filled(r, 12.0, p.surface);
    paint.rect_stroke(r, 12.0, Stroke::new(1.0, border), StrokeKind::Inside);
    let tile = Rect::from_min_size(r.min + vec2(20.0, 20.0), Vec2::splat(36.0));
    paint.rect_filled(tile, 9.0, hue.tint(p));
    ux::icon_in_tile(paint, tile, icon, hue.ink(p));
    ux::text(paint, r.min + vec2(70.0, 25.5), &tr(title), types::CARD_TITLE, p.text, Some(r.width() - 90.0));
    let g = ux::galley_rows(paint, &tr(line), types::BODY, p.text_secondary, Some(r.width() - 40.0), 2);
    paint.galley(r.min + vec2(20.0, 76.0), g, p.text_secondary);
    ux::text(paint, r.min + vec2(20.0, 131.0), &tr(link), types::UI_STRONG, hue.ink(p), Some(r.width() - 76.0));
    ux::icon(paint, pos2(r.right() - 36.0, r.top() + 132.5), &icons::ARROW, 16.0, hue.ink(p));
}

/// A pinned folder's tile: its name and the folder it is in. A click opens the project picker in
/// it; its menu opens a project there too, or unpins it.
fn pinned_tile(shell: &mut Shell, ui: &egui::Ui, p: &Palette, paint: &egui::Painter, r: Rect, i: usize, folder: &std::path::Path) {
    let live = can_open(shell);
    let resp = control(shell, ui, &format!("home.pinned.{i}"), "card", r, live).on_hover_text(folder.display().to_string());
    paint.rect_filled(r, 10.0, p.surface);
    paint.rect_stroke(r, 10.0, Stroke::new(1.0, if live && resp.hovered() { p.border_hover } else { p.border }), StrokeKind::Inside);
    ux::icon(paint, r.min + vec2(20.0, 21.5), &icons::FOLDER, 23.0, p.text_secondary);
    let name = folder.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let place = match folder.parent().and_then(|p| p.file_name()) {
        Some(parent) => format!("{} / {name}", parent.to_string_lossy()),
        None => folder.display().to_string(),
    };
    ux::text(paint, r.min + vec2(57.0, 13.0), &name, types::BODY_STRONG, p.text, Some(r.width() - 106.0));
    ux::text(paint, r.min + vec2(57.0, 37.0), &place, types::CAPTION, p.text_secondary, Some(r.width() - 106.0));
    let more = Rect::from_center_size(pos2(r.right() - 24.5, r.center().y), Vec2::splat(28.0));
    let menu = crate::panels::Menu::Pinned(i);
    let open = shell.panels.menu == Some(menu);
    let more_resp = control(shell, ui, &format!("home.pinned.{i}.more"), "button", more, true).on_hover_text(tr("More"));
    if more_resp.hovered() || open {
        paint.rect_filled(more, 6.0, p.page);
    }
    ux::icon_mid(paint, more.center(), &icons::MORE, if more_resp.hovered() || open { p.text_secondary } else { p.text_muted });
    if more_resp.clicked() {
        shell.panels.menu = if open { None } else { Some(menu) };
    } else if live && resp.clicked() {
        shell.open_from_picker_in(Some(folder));
    }
    if shell.panels.menu == Some(menu) {
        let anchor = Rect::from_min_max(pos2(more.right() - 180.0, more.top()), more.max);
        // Opening is offered only while a project can open; unpinning always.
        let items: Vec<String> = if live { vec![tr("Open a project here"), tr("Unpin")] } else { vec![tr("Unpin")] };
        if let Some(k) = crate::panels::menu(shell, ui, p, anchor, &format!("home.pinned.{i}.menu"), &items) {
            if live && k == 0 {
                shell.open_from_picker_in(Some(folder));
            } else {
                shell.state.unpin(folder);
            }
        }
    }
}

/// The time now, in seconds since 1970, as the recent list records it.
pub(crate) fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// The recent projects in the order the user chose: the last opened first, or by name, A to Z
/// whatever the case, the last opened first among equal names.
fn sorted_recent(recent: &[crate::state::RecentProject], sort: RecentSort) -> Vec<crate::state::RecentProject> {
    let mut out = recent.to_vec();
    out.sort_by_key(|r| std::cmp::Reverse(r.opened));
    if sort == RecentSort::Name {
        out.sort_by_cached_key(|r| r.name.to_lowercase());
    }
    out
}

/// A recent project's model picture in `r`: the design's 82 by 48 thumbnail, scaled to fit `r`
/// and centred in it, four tables joined by links on the hue's tint.
fn model_thumb(p: &Palette, paint: &egui::Painter, r: Rect, hue: Hue) {
    let k = (r.width() / 82.0).min(r.height() / 48.0);
    let art = Rect::from_center_size(r.center(), vec2(82.0, 48.0) * k);
    paint.rect_filled(r, 6.0, hue.tint(p));
    ux::icon(paint, art.min + vec2(23.0, 14.0) * k, &icons::MODEL_LINKS, 36.0 * k, hue.ink(p));
    for (x, y) in [(11.0, 7.0), (48.0, 7.0), (11.0, 28.0), (48.0, 28.0)] {
        let e = Rect::from_min_size(art.min + vec2(x, y) * k, vec2(23.0, 13.0) * k);
        paint.rect_filled(e, 2.0, p.surface);
        paint.rect_stroke(e, 2.0, Stroke::new(1.0, hue.ink(p)), StrokeKind::Inside);
    }
}

/// A recent project's card in the gallery: its model picture, its name, where it lives and when
/// it was last opened; true when clicked.
#[allow(clippy::too_many_arguments, reason = "a card takes its place in the gallery and the time it is read at")]
fn recent_card(
    shell: &mut Shell,
    ui: &egui::Ui,
    p: &Palette,
    paint: &egui::Painter,
    r: Rect,
    i: usize,
    project: &crate::state::RecentProject,
    now: u64,
) -> bool {
    let live = shell.services.projects.is_some();
    let mut resp = control(shell, ui, &format!("home.recent.{i}"), "card", r, live);
    if live {
        resp = resp.on_hover_text(project.path.display().to_string());
    }
    paint.rect_filled(r, 12.0, p.surface);
    paint.rect_stroke(r, 12.0, Stroke::new(1.0, if resp.hovered() { p.border_hover } else { p.border }), StrokeKind::Inside);
    let thumb = Rect::from_min_size(r.min + vec2(8.0, 8.0), vec2((r.width() - 16.0).max(0.0), 112.0));
    model_thumb(p, paint, thumb, [Hue::Blue, Hue::Green, Hue::Purple][i % 3]);
    let w = (r.width() - 32.0).max(0.0);
    ux::text(paint, r.min + vec2(16.0, 132.0), &project.name, types::BODY_STRONG, p.text, Some(w));
    let place = shell.location_of(&project.path).unwrap_or_default();
    ux::text(paint, r.min + vec2(16.0, 156.0), &place, types::CAPTION, p.text_secondary, Some(w));
    ux::text(paint, r.min + vec2(16.0, 176.0), &ago(project.opened, now), types::SMALL, p.text_secondary, Some(w));
    live && resp.clicked()
}

/// A recent project's row: its name and where it lives, when it was last opened; true when clicked.
#[allow(clippy::too_many_arguments, reason = "a row takes its place in the list and the time it is read at")]
fn recent_row(
    shell: &mut Shell,
    ui: &egui::Ui,
    p: &Palette,
    paint: &egui::Painter,
    r: Rect,
    i: usize,
    project: &crate::state::RecentProject,
    now: u64,
) -> bool {
    let live = shell.services.projects.is_some();
    let mut resp = control(shell, ui, &format!("home.recent.{i}"), "row", r, live);
    if live {
        resp = resp.on_hover_text(project.path.display().to_string());
    }
    // The design draws the row under the pointer on the surface, with a border.
    if resp.hovered() {
        paint.rect_filled(r, 8.0, p.surface);
        paint.rect_stroke(r, 8.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
    }
    model_thumb(p, paint, Rect::from_min_size(r.min + vec2(12.0, 2.0), vec2(82.0, 48.0)), [Hue::Blue, Hue::Green, Hue::Purple][i % 3]);
    let w = r.width();
    ux::text(paint, r.min + vec2(112.0, 6.0), &project.name, types::BODY_STRONG, p.text, Some(w * 0.674 - 136.0));
    let place = shell.location_of(&project.path).unwrap_or_default();
    ux::text(paint, r.min + vec2(112.0, 30.0), &place, types::CAPTION, p.text_secondary, Some(w * 0.674 - 136.0));
    ux::text(paint, r.min + vec2(w * 0.801, 17.5), &ago(project.opened, now), types::SMALL, p.text_secondary, Some(w * 0.12));
    ux::icon(paint, pos2(r.right() - 32.0, r.top() + 18.0), &icons::ARROW, 16.0, p.accent);
    live && resp.clicked()
}

#[cfg(test)]
mod tests {
    use super::initials;

    #[test]
    fn initials_take_the_first_and_last_names_first_letters() {
        assert_eq!(initials("Robin Meyer").as_deref(), Some("RM"));
        assert_eq!(initials("Ana María de la Cruz").as_deref(), Some("AC"));
        assert_eq!(initials("robin").as_deref(), Some("RO"));
        assert_eq!(initials("  ").as_deref(), None);
        assert_eq!(initials("José").as_deref(), Some("JO"));
    }
}
