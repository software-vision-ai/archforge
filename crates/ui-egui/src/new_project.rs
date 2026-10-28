//! The New project dialog, over Home: the design's Proposal 03. The project's name, the folder its
//! file goes in, a line on diagrams, and what every project holds; Cancel and Create project.
//!
//! The geometry is the design's: a dialog 760 by 640 px, 32 px padding, centred over a veil that
//! covers the window. A smaller window shrinks it to 16 px from its edges; the body between the
//! header and the footer then scrolls, and below 600 px of inner width the strip's three parts
//! stack.
//!
//! Shell first: Change… and Create project are drawn as designed and not available yet, as
//! creating the project's database comes with creating projects. Cancel, the close button and
//! Escape close the dialog; the window behind it takes no clicks while it is open.

use egui::{Color32, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2, pos2, vec2};

use crate::i18n::{fmt, tr};
use crate::ux::{self, Palette, icons, types};
use crate::{Shell, widgets};

const WIDTH: f32 = 760.0;
const HEIGHT: f32 = 640.0;
const PAD: f32 = 32.0;
/// The least distance from the window's edges when the window is smaller than the dialog.
const EDGE: f32 = 16.0;
/// Where the body starts and how much the footer takes from the bottom.
const BODY_TOP: f32 = 108.0;
const FOOTER: f32 = 84.0;
/// The body's own layout, from its top: the name, the location, the diagrams line, the strip.
const NAME_FIELD: f32 = 26.0;
const NAME_HINT: f32 = 76.0;
const LOCATION: f32 = 112.0;
const LOCATION_FIELD: f32 = 138.0;
const LOCATION_HINT: f32 = 188.0;
const DIAGRAMS: f32 = 232.0;
const DIAGRAMS_LINE: f32 = 254.0;
const PARTS: f32 = 298.0;
const PARTS_TOP: f32 = 324.0;
const FIELD: f32 = 44.0;
const GAP: f32 = 12.0;
/// Below this inner width the strip's parts stack.
const THREE_UP: f32 = 600.0;
/// The longest project name, in characters, and the most a name field takes.
pub const MAX_NAME: usize = 100;
const NAME_LIMIT: usize = 200;
/// The project file's extension.
pub const EXTENSION: &str = "archforge";
/// The characters a file name cannot hold on some platform.
const FORBIDDEN: &[char] = &['/', '\\', ':', '*', '?', '"', '<', '>', '|'];
/// The names Windows keeps for devices, with any extension.
const RESERVED: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5",
    "LPT6", "LPT7", "LPT8", "LPT9",
];

/// The dialog while it is open.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Dialog {
    /// The name as typed.
    pub name: String,
    /// Whether the name field has been given the keyboard yet, which it is once, on opening.
    focused: bool,
}

/// Opens the dialog with an empty name.
pub fn open(shell: &mut Shell) {
    shell.new_project = Some(Dialog::default());
}

/// Why `name` cannot name a project's file, on any platform; `None` when it can. An empty name has
/// no problem to report: it only keeps Create project off.
pub fn name_problem(name: &str) -> Option<String> {
    if name.is_empty() {
        return None;
    }
    if name.chars().any(char::is_control) {
        return Some(tr("A project name cannot contain control characters."));
    }
    if let Some(c) = name.chars().find(|c| FORBIDDEN.contains(c)) {
        return Some(fmt(&tr("A project name cannot contain {char}."), &[("char", &c.to_string())]));
    }
    if name.chars().count() > MAX_NAME {
        return Some(fmt(&tr("A project name can be at most {n} characters."), &[("n", &MAX_NAME.to_string())]));
    }
    if name.starts_with(char::is_whitespace) || name.ends_with(char::is_whitespace) {
        return Some(tr("A project name cannot start or end with a space."));
    }
    if name.starts_with('.') {
        return Some(tr("A project name cannot start with a dot."));
    }
    if name.ends_with('.') {
        return Some(tr("A project name cannot end with a dot."));
    }
    let stem = name.split('.').next().unwrap_or(name).trim_end();
    if RESERVED.iter().any(|r| r.eq_ignore_ascii_case(stem)) {
        return Some(tr("Windows keeps this name for itself; choose another."));
    }
    None
}

/// The project file's name for a project called `name`.
pub fn file_name(name: &str) -> String {
    format!("{name}.{EXTENSION}")
}

/// Whether `name` can be created.
fn creatable(name: &str) -> bool {
    !name.is_empty() && name_problem(name).is_none()
}

/// Draws the dialog over everything when it is open, and closes it on Cancel, the close button or
/// Escape.
pub fn show(shell: &mut Shell, ui: &mut egui::Ui) {
    // As Home, the dialog waits for the design's faces, which arrive at the end of the first frame.
    if shell.new_project.is_none() || !ux::ready(ui.ctx()) {
        return;
    }
    let Some(mut dialog) = shell.new_project.take() else { return };
    let p = *ux::palette(shell.state.theme.is_dark());
    let window = ui.max_rect();
    let mut close = ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
    egui::Area::new(egui::Id::new("new-project")).order(egui::Order::Foreground).fixed_pos(window.min).show(ui.ctx(), |ui| {
        ui.set_min_size(window.size());
        // The veil takes every click behind the dialog; on macOS its title strip still moves the window.
        ui.painter().rect_filled(window, 0.0, p.scrim);
        let veil = ui.interact(window, ui.id().with("new-project.veil"), Sense::click_and_drag());
        let strip = Rect::from_min_size(window.min, vec2(window.width(), 28.0));
        if shell.top_inset > 0.0 && veil.drag_started() && veil.interact_pointer_pos().is_some_and(|at| strip.contains(at)) {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }
        let size = vec2(WIDTH.min(window.width() - 2.0 * EDGE).max(0.0), HEIGHT.min(window.height() - 2.0 * EDGE).max(0.0));
        let frame = Rect::from_center_size(window.center(), size);
        let paint = ui.painter();
        paint.add(egui::epaint::Shadow { offset: [0, 16], blur: 48, spread: 0, color: p.shadow }.as_shape(frame, 16.0));
        paint.rect_filled(frame, 16.0, p.surface);
        paint.rect_stroke(frame, 16.0, Stroke::new(1.0, p.dialog_edge), StrokeKind::Inside);
        // The dialog's own area takes its clicks, so none reaches the veil.
        let _ = ui.interact(frame, ui.id().with("new-project.frame"), Sense::click());
        if header(shell, ui, &p, frame) {
            close = true;
        }
        // A dialog shorter than designed gives its body the room the gap above the footer had.
        let gap = if size.y < HEIGHT { GAP } else { PAD };
        let body = Rect::from_min_max(pos2(frame.left(), frame.top() + BODY_TOP), pos2(frame.right(), frame.bottom() - FOOTER - gap));
        body_scroll(shell, ui, &p, body, &mut dialog);
        if footer(shell, ui, &p, frame, &dialog) {
            close = true;
        }
    });
    if !close {
        shell.new_project = Some(dialog);
    }
}

/// A control's hit area, recorded for automation; one that is not built yet says so on hover.
fn control(shell: &mut Shell, ui: &egui::Ui, id: &str, kind: &'static str, rect: Rect, live: bool) -> egui::Response {
    widgets::record_visible(ui, &mut shell.registry, id, kind, rect);
    let resp = ui.interact(rect, widgets::egui_id(id), Sense::click());
    if live { resp.on_hover_cursor(egui::CursorIcon::PointingHand) } else { resp.on_hover_text(tr("Not available yet")) }
}

/// The icon, the title and its line, and the close button; gives whether it was clicked.
fn header(shell: &mut Shell, ui: &egui::Ui, p: &Palette, frame: Rect) -> bool {
    let paint = ui.painter();
    let at = |x: f32, y: f32| frame.min + vec2(x, y);
    let tile = Rect::from_min_size(at(PAD, PAD), Vec2::splat(40.0));
    paint.rect_filled(tile, 10.0, p.accent_tint);
    ux::icon(paint, tile.min + vec2(11.0, 11.0), &icons::FOLDER, 18.0, p.accent);
    let room = (frame.width() - 88.0 - 72.0).max(0.0);
    ux::text(paint, at(88.0, 30.0), &tr("New project"), types::SECTION, p.text, Some(room));
    ux::text(
        paint,
        at(88.0, 61.0),
        &tr("Your models, every view of them, and the issues and conversations about them."),
        types::BODY,
        p.text_secondary,
        Some(room),
    );
    let close = Rect::from_min_size(at(frame.width() - 60.0, 28.0), Vec2::splat(32.0));
    let resp = control(shell, ui, "new-project.close", "button", close, true).on_hover_text(tr("Close"));
    if resp.hovered() {
        ui.painter().rect_filled(close, 8.0, p.border);
    }
    ux::icon(ui.painter(), close.min + vec2(9.0, 9.0), &icons::CLOSE, 14.0, p.text_secondary);
    resp.clicked()
}

/// The body between the header and the footer, scrolling when the window is too small for it.
fn body_scroll(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, body: Rect, dialog: &mut Dialog) {
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(body).layout(egui::Layout::top_down(egui::Align::Min)));
    child.set_clip_rect(body.intersect(ui.clip_rect()));
    child.spacing_mut().scroll = egui::style::ScrollStyle::floating();
    egui::ScrollArea::vertical().id_salt("new-project.body").auto_shrink([false, false]).show(&mut child, |ui| {
        let origin = ui.max_rect().min + vec2(PAD, 0.0);
        let inner = (body.width() - 2.0 * PAD).max(0.0);
        let height = content(shell, ui, p, origin, inner, dialog);
        ui.allocate_rect(Rect::from_min_size(ui.max_rect().min, vec2(body.width(), height)), Sense::hover());
    });
}

/// The name, the location, the diagrams line and the strip; gives their height.
fn content(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, origin: Pos2, inner: f32, dialog: &mut Dialog) -> f32 {
    let at = |y: f32| pos2(origin.x, origin.y + y);
    let paint = ui.painter().clone();
    // The name.
    ux::text(&paint, at(0.0), &tr("Project name"), types::UI_STRONG, p.text, Some(inner));
    let field = Rect::from_min_size(at(NAME_FIELD), vec2(inner, FIELD));
    let font = types::LEAD.font();
    let row = ui.fonts_mut(|f| f.row_height(&font));
    let text_rect = Rect::from_min_size(pos2(field.left() + 14.0, field.center().y - row / 2.0), vec2((field.width() - 28.0).max(0.0), row));
    paint.rect_filled(field, 8.0, p.surface);
    let hint = egui::RichText::new(tr("Name your project")).font(font.clone()).color(p.text_muted);
    let edit = egui::TextEdit::singleline(&mut dialog.name)
        .id(widgets::egui_id("new-project.name"))
        .font(font)
        .text_color(p.text)
        .hint_text(hint)
        .frame(egui::Frame::NONE)
        .margin(egui::Margin::ZERO)
        .char_limit(NAME_LIMIT)
        .desired_width(text_rect.width());
    let resp = ui.put(text_rect, edit);
    widgets::record_visible(ui, &mut shell.registry, "new-project.name", "field", field);
    if !dialog.focused {
        resp.request_focus();
        dialog.focused = true;
    }
    let ring = if resp.has_focus() {
        Stroke::new(2.0, p.focus)
    } else if resp.hovered() {
        Stroke::new(1.0, p.border_hover)
    } else {
        Stroke::new(1.0, p.border)
    };
    paint.rect_stroke(field, 8.0, ring, StrokeKind::Inside);
    let problem = name_problem(&dialog.name);
    match &problem {
        Some(why) => ux::text(&paint, at(NAME_HINT), why, types::SMALL, p.amber, Some(inner)),
        None => ux::text(&paint, at(NAME_HINT), &tr("You can rename it later."), types::SMALL, p.text_muted, Some(inner)),
    };
    // The location.
    ux::text(&paint, at(LOCATION), &tr("Location"), types::UI_STRONG, p.text, Some(inner));
    let field = Rect::from_min_size(at(LOCATION_FIELD), vec2(inner, FIELD));
    paint.rect_filled(field, 8.0, p.surface);
    paint.rect_stroke(field, 8.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
    ux::icon(&paint, field.min + vec2(14.0, 13.0), &icons::FOLDER, 18.0, p.text_secondary);
    let change = Rect::from_min_size(pos2(field.right() - 98.0, field.top() + 6.0), vec2(92.0, 32.0));
    let room = Some((change.left() - field.left() - 44.0 - 12.0).max(0.0));
    let folder = shell.services.projects_folder.as_ref().map(|f| f.label.clone());
    match &folder {
        Some(label) => ux::text(&paint, field.min + vec2(44.0, 12.0), label, types::BODY, p.text, room),
        None => ux::text(&paint, field.min + vec2(44.0, 12.0), &tr("No folder chosen yet"), types::BODY, p.text_muted, room),
    };
    paint.rect_filled(change, 6.0, p.page);
    let g = ux::galley(&paint, &tr("Change…"), types::UI_STRONG, p.text, Some(change.width() - 8.0));
    paint.galley(change.center() - g.size() / 2.0, g, p.text);
    control(shell, ui, "new-project.location.change", "button", change, false);
    let file_line = if creatable(&dialog.name) && folder.is_some() {
        fmt(&tr("Creates {file} here: one versioned project file that keeps every change."), &[("file", &file_name(&dialog.name))])
    } else {
        tr("The project file is made here: one versioned file that keeps every change.")
    };
    ux::text(&paint, at(LOCATION_HINT), &file_line, types::SMALL, p.text_muted, Some(inner));
    // Diagrams come as the user works.
    ux::text(&paint, at(DIAGRAMS), &tr("Diagrams"), types::UI_STRONG, p.text, Some(inner));
    let line = ux::galley_rows(
        &paint,
        &tr("Add diagrams of any kind as you go: UML 2, C4 and many more. Every diagram shows the same model."),
        types::SMALL,
        p.text_secondary,
        Some(inner),
        2,
    );
    paint.galley(at(DIAGRAMS_LINE), line, p.text_secondary);
    // What every project holds.
    ux::text(&paint, at(PARTS), &tr("In every project"), types::UI_STRONG, p.text, Some(inner));
    let parts = [
        (tr("Issues"), tr("Work tracked on the elements it concerns."), &icons::ISSUE, p.amber, p.amber_tint),
        (tr("Conversations"), tr("Chats and prompts, linked to what they touched."), &icons::BUBBLE, p.purple, p.purple_tint),
        (tr("History"), tr("Every change kept, to compare or restore."), &icons::HISTORY, p.green, p.green_tint),
    ];
    let three = inner >= THREE_UP;
    let (w, h) = if three { ((inner - 2.0 * GAP) / 3.0, 92.0) } else { (inner, 64.0) };
    for (i, (title, body, icon, ink, tint)) in parts.iter().enumerate() {
        let (x, y) = if three { (i as f32 * (w + GAP), PARTS_TOP) } else { (0.0, PARTS_TOP + i as f32 * (h + GAP)) };
        let r = Rect::from_min_size(at(y) + vec2(x, 0.0), vec2(w, h));
        paint.rect_filled(r, 10.0, p.page);
        let tile = Rect::from_min_size(r.min + vec2(14.0, 14.0), Vec2::splat(32.0));
        paint.rect_filled(tile, 8.0, *tint);
        ux::icon(&paint, tile.min + vec2(8.0, 8.0), icon, 16.0, *ink);
        ux::text(&paint, r.min + vec2(56.0, 14.0), title, types::UI_STRONG, p.text, Some((w - 70.0).max(0.0)));
        let rows = if three { 3 } else { 1 };
        let g = ux::galley_rows(&paint, body, types::SMALL, p.text_secondary, Some((w - 70.0).max(0.0)), rows);
        paint.galley(r.min + vec2(56.0, 36.0), g, p.text_secondary);
    }
    if three { PARTS_TOP + h } else { PARTS_TOP + 3.0 * h + 2.0 * GAP }
}

/// The divider, Cancel and Create project; gives whether Cancel was clicked.
fn footer(shell: &mut Shell, ui: &egui::Ui, p: &Palette, frame: Rect, dialog: &Dialog) -> bool {
    let paint = ui.painter();
    paint.rect_filled(Rect::from_min_size(pos2(frame.left(), frame.bottom() - FOOTER), vec2(frame.width(), 1.0)), 0.0, p.border);
    let top = frame.bottom() - 62.0;
    let create = Rect::from_min_size(pos2(frame.right() - PAD - 144.0, top), vec2(144.0, 40.0));
    let cancel = Rect::from_min_size(pos2(create.left() - GAP - 104.0, top), vec2(104.0, 40.0));
    let cancelled = control(shell, ui, "new-project.cancel", "button", cancel, true);
    let paint = ui.painter();
    paint.rect_filled(cancel, 8.0, if cancelled.hovered() { p.page } else { p.surface });
    paint.rect_stroke(cancel, 8.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
    centred(paint, cancel, &tr("Cancel"), p.text);
    control(shell, ui, "new-project.create", "button", create, false);
    // A name that cannot be created keeps the button faded.
    let fill = if creatable(&dialog.name) { p.accent_fill } else { p.accent_fill.gamma_multiply(0.45) };
    let paint = ui.painter();
    paint.rect_filled(create, 8.0, fill);
    centred(paint, create, &tr("Create project"), p.on_accent);
    cancelled.clicked()
}

fn centred(paint: &egui::Painter, r: Rect, label: &str, color: Color32) {
    let g = ux::galley(paint, label, types::UI_STRONG, color, Some(r.width() - 8.0));
    paint.galley(r.center() - g.size() / 2.0, g, color);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_that_cannot_name_a_file_on_any_platform_is_refused_with_its_reason() {
        crate::i18n::set_language("en");
        for good in ["Commerce platform", "Ünïcødé ✓", "v2.1 model", "a", "CONSOLE", "Com10"] {
            assert_eq!(name_problem(good), None, "{good}");
        }
        assert_eq!(name_problem(""), None, "an empty name only keeps Create project off");
        assert!(!creatable(""));
        let says = |n: &str| name_problem(n).unwrap();
        assert!(says("a/b").contains('/'));
        assert!(says("a:b").contains(':'));
        assert!(says("a\tb").contains("control"));
        assert!(says(" lead").contains("space") && says("trail ").contains("space"));
        assert!(says(".hidden").contains("start with a dot"));
        assert!(says("ends.").contains("end with a dot"));
        assert!(says("con").contains("Windows") && says("LPT1.notes").contains("Windows") && says("nul").contains("Windows"));
        assert!(says(&"x".repeat(MAX_NAME + 1)).contains("100"));
        assert_eq!(name_problem(&"é".repeat(MAX_NAME)), None, "the limit counts characters, not bytes");
        assert_eq!(file_name("Commerce platform"), "Commerce platform.archforge");
    }
}
