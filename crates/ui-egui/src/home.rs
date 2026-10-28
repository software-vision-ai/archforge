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
//! changes it. Controls whose screens are not built yet are drawn as designed, do nothing when
//! clicked and say so on hover.

use egui::{Color32, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2, pos2, vec2};

use crate::i18n::tr;
use crate::theme::ThemeKind;
use crate::ux::{self, Palette, icons, types};
use crate::{Shell, widgets};

/// The title strip's height: the macOS title bar's, where the strip is that bar.
const STRIP: f32 = 28.0;
/// Where the strip's controls start beside the traffic lights on macOS, and elsewhere.
const STRIP_LEFT_MAC: f32 = 70.0;
const STRIP_LEFT: f32 = 10.0;
/// The bar's height, its bottom divider included.
const BAR: f32 = 48.0;
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
/// A button's height, padding and the gap between its icon and its label.
const BUTTON: f32 = 40.0;
const BUTTON_PAD: f32 = 16.0;
const BUTTON_GAP: f32 = 8.0;

/// A pinned folder: its name and where it is, as the page shows them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pinned {
    pub name: String,
    pub path: String,
}

/// A recent project as its row shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recent {
    pub name: String,
    /// The view it was left in, as the row names it.
    pub last_view: String,
    pub elements: u64,
    /// When it was last opened, as the page says it ("Today, 10:24").
    pub opened: String,
}

/// What Home lists. Both start empty: none can be made or opened yet.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Data {
    pub pinned: Vec<Pinned>,
    pub recent: Vec<Recent>,
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

/// Whether the start cards lead anywhere yet: each opens a screen not built so far, so none takes
/// a click or shows the hover border the design gives a card under the pointer.
const STARTS_LIVE: bool = false;

/// The four start cards: automation id, title, line, link, icon and colour.
const STARTS: [(&str, &str, &str, &str, &ux::Icon, Hue); 4] = [
    ("new-project", "New project", "A clean start for your next idea.", "Create project", &icons::PLUS, Hue::Blue),
    ("open-project", "Open a project", "Continue from a project on this device.", "Browse files", &icons::FOLDER, Hue::Green),
    ("example", "Explore an example", "Learn by navigating a connected model.", "Explore example", &icons::MAP, Hue::Purple),
    ("import", "Import a model", "Bring existing elements into a project.", "Choose a file", &icons::IMPORT, Hue::Amber),
];

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
        // The bar first, so the strip's drag area does not cover it.
        bar_row(shell, ui, &p, bar);
        title_strip(shell, ui, &p, strip);
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
fn control(shell: &mut Shell, ui: &egui::Ui, id: &str, kind: &'static str, rect: Rect, live: bool) -> egui::Response {
    widgets::record_visible(ui, &mut shell.registry, id, kind, rect);
    let resp = ui.interact(rect, widgets::egui_id(id), Sense::click());
    if live { resp.on_hover_cursor(egui::CursorIcon::PointingHand) } else { resp.on_hover_text(tr("Not available yet")) }
}

/// The title strip: the active Home button and Open project at the left, beside the traffic
/// lights on macOS, and the theme switch at the right. On macOS the strip is the window's title
/// bar, so dragging it moves the window and a double click zooms it.
fn title_strip(shell: &mut Shell, ui: &egui::Ui, p: &Palette, r: Rect) {
    let paint = ui.painter();
    paint.rect_filled(r, 0.0, p.title_strip);
    if shell.top_inset > 0.0 {
        let drag = ui.interact(r, ui.id().with("home.title-drag"), Sense::click_and_drag());
        if drag.drag_started() {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }
        if drag.double_clicked() {
            let zoomed = ui.ctx().input(|i| i.viewport().maximized.unwrap_or(false));
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Maximized(!zoomed));
        }
    }
    let left = r.left() + if shell.top_inset > 0.0 { STRIP_LEFT_MAC } else { STRIP_LEFT };
    // Home, the active place: its icon in the accent colour, straight on the strip.
    let home = Rect::from_min_size(pos2(left, r.top() + 2.0), Vec2::splat(24.0));
    ux::icon(paint, home.min + vec2(4.0, 4.0), &icons::HOME, 16.0, p.accent);
    control(shell, ui, "home.tab.home", "tab", home, true).on_hover_text(tr("Home"));
    // Open project, a ghost button beside it.
    let ghost = home.right() + 5.0;
    ux::icon_weighted(paint, pos2(ghost + 6.0, r.top() + 7.0), &icons::PLUS, 14.0, 1.5, p.text_secondary);
    let label = ux::text(paint, pos2(ghost + 27.0, r.top() + 4.5), &tr("Open project"), types::UI, p.text, None);
    control(
        shell,
        ui,
        "home.header.open-project",
        "button",
        Rect::from_min_max(pos2(ghost, r.top() + 2.0), pos2(label.right() + 8.0, r.top() + 26.0)),
        false,
    );
    // The theme switch: the moon in Light, the sun in Dark; a click changes the appearance.
    let dark = shell.state.theme.is_dark();
    let switch = Rect::from_min_size(pos2(r.right() - 42.0, r.top() + 2.0), Vec2::splat(24.0));
    let hint = if dark { tr("Switch to the light appearance") } else { tr("Switch to the dark appearance") };
    let resp = control(shell, ui, "home.theme", "button", switch, true).on_hover_text(hint);
    if resp.hovered() {
        paint.rect_filled(switch, 6.0, p.border);
    }
    ux::icon(paint, switch.min + vec2(5.0, 5.0), if dark { &icons::SUN } else { &icons::MOON }, 14.0, p.text_secondary);
    if resp.clicked() {
        shell.state.theme = if dark { ThemeKind::Light } else { ThemeKind::Dark };
    }
}

/// The bar: the brand at the left; the search field, help and the avatar at the right.
fn bar_row(shell: &mut Shell, ui: &egui::Ui, p: &Palette, r: Rect) {
    let paint = ui.painter();
    paint.rect_filled(r, 0.0, p.surface);
    paint.rect_filled(Rect::from_min_max(pos2(r.left(), r.bottom() - 1.0), r.max), 0.0, p.border);
    let at = |x: f32, y: f32| r.min + vec2(x, y);
    let mark = Rect::from_min_size(at(16.0, 12.0), Vec2::splat(24.0));
    paint.rect_filled(mark, 6.0, p.accent_fill);
    ux::icon(paint, mark.min + vec2(6.33, 6.67), &icons::MARK_A, 11.33, p.on_accent);
    let wordmark = ux::text(paint, at(50.0, 12.5), "Archforge", types::LEAD_STRONG, p.text, None);
    let right = r.right();
    if let Some(initials) = shell.services.user_name.as_deref().and_then(initials) {
        let avatar = Rect::from_min_size(pos2(right - 44.0, r.top() + 10.0), Vec2::splat(28.0));
        paint.rect_filled(avatar, 14.0, p.accent_tint);
        let g = ux::galley(paint, &initials, types::CAPTION_STRONG, p.accent, None);
        paint.galley(avatar.center() - g.size() / 2.0, g, p.accent);
        control(shell, ui, "home.header.account", "button", avatar, false);
    }
    let help = Rect::from_min_size(pos2(right - 80.0, r.top() + 14.0), Vec2::splat(20.0));
    ux::icon(paint, help.min, &icons::HELP, 20.0, p.text_secondary);
    control(shell, ui, "home.header.help", "button", help.expand(6.0), false);
    // The search field keeps 24 px from the wordmark when the window is narrow.
    let end = right - 96.0;
    let start = (end - 300.0).max(wordmark.right() + 24.0);
    if end - start >= 160.0 {
        let field = Rect::from_min_max(pos2(start, r.top() + 8.0), pos2(end, r.top() + 40.0));
        paint.rect_filled(field, 16.0, p.page);
        paint.rect_stroke(field, 16.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
        ux::icon(paint, field.min + vec2(10.0, 8.0), &icons::SEARCH, 16.0, p.text_secondary);
        let chord = if ui.ctx().os() == egui::os::OperatingSystem::Mac { "⌘K" } else { "Ctrl K" };
        let key = ux::galley(paint, chord, types::CAPTION, p.text_muted, None);
        let key_at = pos2(field.right() - 12.0 - key.size().x, field.top() + 8.0);
        let room = (key_at.x - 8.0 - (field.left() + 34.0)).max(0.0);
        ux::text(paint, field.min + vec2(34.0, 6.5), &tr("Search projects and actions"), types::UI, p.text_secondary, Some(room));
        paint.galley(key_at, key, p.text_muted);
        control(shell, ui, "home.header.search", "field", field, false);
    }
}

/// Two letters for the avatar: the first of the first and the last name, or the first two of a
/// single name; none for a name with no letters.
fn initials(name: &str) -> Option<String> {
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
    // The welcome, and Open project level with its title.
    ux::text(&paint, at(0.0, 39.0), &tr("Welcome home").to_uppercase(), types::EYEBROW, p.accent, None);
    let label = tr("Open project");
    let (probe, _) = button_rect(&paint, Pos2::ZERO, &label, true);
    let open = Rect::from_min_size(pos2(right - probe.width(), origin.y + 73.0), probe.size());
    ux::text(&paint, at(0.0, 67.0), &tr("Welcome to Archforge"), types::DISPLAY, p.text, Some((open.left() - left - 24.0).max(0.0)));
    ux::text(
        &paint,
        at(0.0, 124.0),
        &tr("Start with the big picture. Or with one detail. Bring it all together."),
        types::LEAD,
        p.text_secondary,
        Some(inner),
    );
    paint.rect_filled(open, 8.0, p.accent_fill);
    ux::icon(&paint, open.min + vec2(BUTTON_PAD, 11.0), &icons::FOLDER, 18.0, p.on_accent);
    ux::text(&paint, open.min + vec2(BUTTON_PAD + 18.0 + BUTTON_GAP, 10.5), &label, types::UI_STRONG, p.on_accent, None);
    control(shell, ui, "home.open-project", "button", open, false);
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
    let per_row: usize = if inner >= TWO_UP {
        4
    } else if inner >= ONE_UP {
        2
    } else {
        1
    };
    let card_w = (inner - GAP * (per_row as f32 - 1.0)) / per_row as f32;
    let cards_top = y + 72.0;
    for (i, start) in STARTS.iter().enumerate() {
        let (row, col) = (i / per_row, i % per_row);
        let r = Rect::from_min_size(at(col as f32 * (card_w + GAP), cards_top + row as f32 * (CARD.y + GAP)), vec2(card_w, CARD.y));
        start_card(shell, ui, p, &paint, r, start);
    }
    let rows = STARTS.len().div_ceil(per_row);
    y = cards_top + rows as f32 * CARD.y + (rows as f32 - 1.0) * GAP + 38.0;
    // Pinned folders.
    ux::text(&paint, at(0.0, y), &tr("Pinned folders"), types::SUBSECTION, p.text, Some(inner));
    ux::text(&paint, at(0.0, y + 30.0), &tr("Keep your project locations close."), types::UI, p.text_secondary, Some(inner));
    let mut x = 0.0;
    let mut tiles_top = y + 70.0;
    for (i, folder) in shell.home.pinned.clone().iter().enumerate() {
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
    let pin = Rect::from_min_size(at(x, tiles_top), vec2(234.0f32.min(inner), 66.0));
    paint.rect_stroke(pin, 10.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
    ux::icon(&paint, pin.min + vec2(20.0, 24.0), &icons::PLUS, 18.0, p.text_secondary);
    ux::text(&paint, pin.min + vec2(52.0, 23.0), &tr("Pin a folder"), types::BODY, p.text_secondary, Some(pin.width() - 62.0));
    control(shell, ui, "home.pin-folder", "button", pin, false);
    y = tiles_top + 106.0;
    // Recent projects: the heading at the left, sorting and the view switch on its line.
    ux::text(&paint, at(0.0, y), &tr("Recent projects"), types::SECTION, p.text, Some(inner - 280.0));
    ux::text(&paint, at(0.0, y + 32.0), &tr("Pick up in the exact view you left."), types::UI, p.text_secondary, Some(inner - 280.0));
    let line = origin.y + y + 14.5;
    let switch = Rect::from_min_size(pos2(right - 118.0, line - 19.0), vec2(118.0, 38.0));
    let chevron = Rect::from_min_size(pos2(switch.left() - 43.5, line - 8.0), Vec2::splat(16.0));
    let sort_label = ux::galley(&paint, &tr("Last opened"), types::SMALL, p.text_secondary, None);
    let sort_at = pos2(chevron.left() - 20.5 - sort_label.size().x, line - 8.5);
    let sort = Rect::from_min_max(pos2(sort_at.x - 8.0, switch.top()), pos2(chevron.right() + 8.0, switch.bottom()));
    paint.galley(sort_at, sort_label, p.text_secondary);
    ux::icon(&paint, chevron.min + vec2(4.0, 6.0), &icons::CHEVRON_DOWN, 8.0, p.text_secondary);
    control(shell, ui, "home.recent.sort", "button", sort, false);
    paint.rect_filled(switch, 8.0, p.surface);
    paint.rect_stroke(switch, 8.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
    let list = Rect::from_min_size(switch.min + vec2(4.0, 4.0), vec2(51.0, 30.0));
    let grid = Rect::from_min_size(switch.min + vec2(63.0, 4.0), vec2(51.0, 30.0));
    paint.rect_filled(list, 5.0, p.accent_tint);
    ux::icon_in(&paint, list, &icons::LIST, p.accent);
    ux::icon_in(&paint, grid, &icons::GRID, p.text_secondary);
    control(shell, ui, "home.recent.list", "toggle", list, true);
    control(shell, ui, "home.recent.grid", "toggle", grid, false);
    let mut row_top = y + 75.0;
    if shell.home.recent.is_empty() {
        ux::text(&paint, at(0.0, row_top), &tr("No recent projects yet"), types::UI, p.text_secondary, Some(inner));
        ux::text(&paint, at(0.0, row_top + 28.0), &tr("Projects you create or open will appear here."), types::SMALL, p.text_secondary, Some(inner));
        row_top += 45.0 + 11.0;
    } else {
        for (i, project) in shell.home.recent.clone().iter().enumerate() {
            recent_row(shell, ui, p, &paint, Rect::from_min_size(at(0.0, row_top), vec2(inner, ROW)), i, project);
            row_top += ROW_PITCH;
        }
        row_top += ROW - ROW_PITCH + 11.0;
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
    ux::icon_in(paint, tile, &icons::CHAT, p.accent);
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
    let resp = control(shell, ui, &format!("home.start.{id}"), "card", r, STARTS_LIVE);
    let border = if STARTS_LIVE && resp.hovered() { p.border_hover } else { p.border };
    paint.rect_filled(r, 12.0, p.surface);
    paint.rect_stroke(r, 12.0, Stroke::new(1.0, border), StrokeKind::Inside);
    let tile = Rect::from_min_size(r.min + vec2(20.0, 20.0), Vec2::splat(36.0));
    paint.rect_filled(tile, 9.0, hue.tint(p));
    ux::icon_in(paint, tile, icon, hue.ink(p));
    ux::text(paint, r.min + vec2(70.0, 25.5), &tr(title), types::CARD_TITLE, p.text, Some(r.width() - 90.0));
    let g = ux::galley_rows(paint, &tr(line), types::BODY, p.text_secondary, Some(r.width() - 40.0), 2);
    paint.galley(r.min + vec2(20.0, 76.0), g, p.text_secondary);
    ux::text(paint, r.min + vec2(20.0, 131.0), &tr(link), types::UI_STRONG, hue.ink(p), Some(r.width() - 76.0));
    ux::icon(paint, pos2(r.right() - 36.0, r.top() + 132.5), &icons::ARROW, 16.0, hue.ink(p));
}

fn pinned_tile(shell: &mut Shell, ui: &egui::Ui, p: &Palette, paint: &egui::Painter, r: Rect, i: usize, folder: &Pinned) {
    control(shell, ui, &format!("home.pinned.{i}"), "card", r, false);
    paint.rect_filled(r, 10.0, p.surface);
    paint.rect_stroke(r, 10.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
    ux::icon(paint, r.min + vec2(20.0, 21.5), &icons::FOLDER, 23.0, p.text_secondary);
    ux::text(paint, r.min + vec2(57.0, 13.0), &folder.name, types::BODY_STRONG, p.text, Some(r.width() - 106.0));
    ux::text(paint, r.min + vec2(57.0, 37.0), &folder.path, types::CAPTION, p.text_secondary, Some(r.width() - 106.0));
    ux::icon(paint, pos2(r.right() - 33.0, r.top() + 33.0), &icons::MORE, 17.0, p.text_muted);
}

fn recent_row(shell: &mut Shell, ui: &egui::Ui, p: &Palette, paint: &egui::Painter, r: Rect, i: usize, project: &Recent) {
    let resp = control(shell, ui, &format!("home.recent.{i}"), "row", r, false);
    // The design draws the row under the pointer on the surface, with a border.
    if resp.hovered() {
        paint.rect_filled(r, 8.0, p.surface);
        paint.rect_stroke(r, 8.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
    }
    let hue = [Hue::Blue, Hue::Green, Hue::Purple][i % 3];
    let thumb = Rect::from_min_size(r.min + vec2(12.0, 2.0), vec2(82.0, 48.0));
    paint.rect_filled(thumb, 6.0, hue.tint(p));
    ux::icon(paint, thumb.min + vec2(23.0, 14.0), &icons::MODEL_LINKS, 36.0, hue.ink(p));
    for (x, y) in [(11.0, 7.0), (48.0, 7.0), (11.0, 28.0), (48.0, 28.0)] {
        let e = Rect::from_min_size(thumb.min + vec2(x, y), vec2(23.0, 13.0));
        paint.rect_filled(e, 2.0, p.surface);
        paint.rect_stroke(e, 2.0, Stroke::new(1.0, hue.ink(p)), StrokeKind::Inside);
    }
    let w = r.width();
    ux::text(paint, r.min + vec2(112.0, 6.0), &project.name, types::BODY_STRONG, p.text, Some(w * 0.674 - 136.0));
    let view = crate::i18n::fmt(&tr("Last view: {view}"), &[("view", &project.last_view)]);
    ux::text(paint, r.min + vec2(112.0, 30.0), &view, types::CAPTION, p.text_secondary, Some(w * 0.674 - 136.0));
    let elements = crate::i18n::trn("{n} element", "{n} elements", project.elements);
    ux::text(paint, r.min + vec2(w * 0.674, 17.5), &elements, types::SMALL, p.text_secondary, Some(w * 0.126 - 12.0));
    ux::text(paint, r.min + vec2(w * 0.801, 17.5), &project.opened, types::SMALL, p.text_secondary, Some(w * 0.12));
    ux::icon(paint, pos2(r.right() - 32.0, r.top() + 18.0), &icons::ARROW, 16.0, p.accent);
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
