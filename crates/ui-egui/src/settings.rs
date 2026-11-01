//! The Settings dialog: for now, who the user is. The name entered here is kept
//! in the preferences, and the bar's avatar shows its initials; with no name it shows JD, for John
//! Doe. Archforge does not read the name from the operating system's account.
//!
//! It opens from the avatar and with the command key and comma, over Home or a project; Done,
//! the close button and Escape close it. A name takes effect as it is typed.

use egui::{Color32, Rect, Sense, Stroke, StrokeKind, Vec2, pos2, vec2};

use crate::i18n::tr;
use crate::ux::{self, icons, types};
use crate::{Shell, widgets};

const WIDTH: f32 = 520.0;
const HEIGHT: f32 = 330.0;
const PAD: f32 = 32.0;
/// The least distance from the window's edges when the window is smaller than the dialog.
const EDGE: f32 = 16.0;
const FIELD: f32 = 44.0;
/// The longest name kept.
pub const MAX_NAME: usize = 80;
/// The name the avatar stands for while the user has entered none.
const NOBODY: &str = "John Doe";

/// The dialog while it is open: whether its field has taken the keyboard yet.
#[derive(Debug, Default)]
pub struct Dialog {
    focused: bool,
}

/// The initials the bar's avatar shows: the name's, or JD while there is none.
pub fn avatar_initials(shell: &Shell) -> String {
    let name = shell.state.user_name.trim();
    crate::home::initials(if name.is_empty() { NOBODY } else { name }).unwrap_or_else(|| "JD".into())
}

pub fn open(shell: &mut Shell) {
    if shell.settings.is_none() {
        shell.settings = Some(Dialog::default());
    }
}

/// Opens Settings with the command key and comma, unless a text is being typed.
pub(crate) fn shortcut(shell: &mut Shell, ctx: &egui::Context) {
    if !ctx.egui_wants_keyboard_input()
        && ctx.input_mut(|i| i.consume_shortcut(&egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::Comma)))
    {
        open(shell);
    }
}

fn control(shell: &mut Shell, ui: &egui::Ui, id: &str, kind: &'static str, rect: Rect) -> egui::Response {
    widgets::record_visible(ui, &mut shell.registry, id, kind, rect);
    ui.interact(rect, widgets::egui_id(id), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand)
}

pub fn show(shell: &mut Shell, ui: &mut egui::Ui) {
    if shell.settings.is_none() || !ux::ready(ui.ctx()) {
        return;
    }
    let Some(mut dialog) = shell.settings.take() else { return };
    let p = *ux::palette(shell.state.theme.is_dark());
    let window = ui.max_rect();
    let mut close = ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
    egui::Area::new(egui::Id::unique("settings")).order(egui::Order::Foreground).fixed_pos(window.min).show(ui.ctx(), |ui| {
        ui.set_min_size(window.size());
        // The veil takes every click behind the dialog.
        ui.painter().rect_filled(window, 0.0, p.scrim);
        let _ = ui.interact(window, ui.scope_id().with("settings.veil"), Sense::click_and_drag());
        let size = vec2(WIDTH.min(window.width() - 2.0 * EDGE).max(0.0), HEIGHT.min(window.height() - 2.0 * EDGE).max(0.0));
        let frame = Rect::from_center_size(window.center(), size);
        let paint = ui.painter().clone();
        paint.add(egui::epaint::Shadow { offset: [0, 16], blur: 48, spread: 0, color: p.shadow }.as_shape(frame, 16.0));
        paint.rect_filled(frame, 16.0, p.surface);
        paint.rect_stroke(frame, 16.0, Stroke::new(1.0, p.dialog_edge), StrokeKind::Inside);
        let _ = ui.interact(frame, ui.scope_id().with("settings.frame"), Sense::click());
        let at = |x: f32, y: f32| frame.min + vec2(x, y);
        let inner = (frame.width() - 2.0 * PAD).max(0.0);
        // The header: its icon, the title and its line, and the close button.
        let tile = Rect::from_min_size(at(PAD, PAD), Vec2::splat(40.0));
        paint.rect_filled(tile, 10.0, p.accent_tint);
        ux::icon_in_tile(&paint, tile, &icons::EDIT, p.accent);
        let room = (frame.width() - 88.0 - 72.0).max(0.0);
        ux::text(&paint, at(88.0, 30.0), &tr("Settings"), types::SECTION, p.text, Some(room));
        ux::text(&paint, at(88.0, 61.0), &tr("How Archforge knows you."), types::BODY, p.text_secondary, Some(room));
        let x = Rect::from_min_size(at(frame.width() - 60.0, 28.0), Vec2::splat(32.0));
        let resp = control(shell, ui, "settings.close", "button", x).on_hover_text(tr("Close"));
        if resp.hovered() {
            paint.rect_filled(x, 8.0, p.border);
        }
        ux::icon(&paint, x.min + vec2(9.0, 9.0), &icons::CLOSE, 14.0, p.text_secondary);
        close |= resp.clicked();
        // Your name, beside the avatar it makes.
        ux::text(&paint, at(PAD, 116.0), &tr("Your name"), types::UI_STRONG, p.text, Some(inner));
        let avatar = Rect::from_min_size(at(PAD, 142.0 + (FIELD - 36.0) / 2.0), Vec2::splat(36.0));
        paint.rect_filled(avatar, 18.0, p.accent_tint);
        let g = ux::galley(&paint, &avatar_initials(shell), types::UI_STRONG, p.accent, None);
        paint.galley(avatar.center() - g.size() / 2.0, g, p.accent);
        let field = Rect::from_min_size(at(PAD + 48.0, 142.0), vec2((inner - 48.0).max(0.0), FIELD));
        let font = types::LEAD.font();
        let row = ui.fonts_mut(|f| f.row_height(&font));
        let text_rect = Rect::from_min_size(pos2(field.left() + 14.0, field.center().y - row / 2.0), vec2((field.width() - 28.0).max(0.0), row));
        paint.rect_filled(field, 8.0, p.surface);
        let hint = egui::RichText::new(NOBODY).font(font.clone()).color(p.text_muted);
        let edit = egui::TextEdit::singleline(&mut shell.state.user_name)
            .id(widgets::egui_id("settings.name"))
            .font(font)
            .text_color(p.text)
            .hint_text(hint)
            .frame(egui::Frame::NONE)
            .margin(egui::Margin::ZERO)
            .char_limit(MAX_NAME)
            .desired_width(text_rect.width());
        let resp = ui.put(text_rect, edit);
        let _ = resp.clone().accessible_name(tr("Your name"));
        widgets::record_visible(ui, &mut shell.registry, "settings.name", "field", field);
        if !dialog.focused {
            resp.request_focus();
            dialog.focused = true;
        }
        // Enter in the field is Done.
        close |= resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        let ring = if resp.has_focus() { Stroke::new(2.0, p.focus) } else { Stroke::new(1.0, p.border) };
        paint.rect_stroke(field, 8.0, ring, StrokeKind::Inside);
        ux::text(
            &paint,
            at(PAD, 142.0 + FIELD + 10.0),
            &tr("The bar shows your initials. Without a name they are JD, for John Doe."),
            types::SMALL,
            p.text_muted,
            Some(inner),
        );
        // Done, at the footer's right.
        paint.rect_filled(Rect::from_min_size(pos2(frame.left(), frame.bottom() - 84.0), vec2(frame.width(), 1.0)), 0.0, p.border);
        let done = Rect::from_min_size(pos2(frame.right() - PAD - 104.0, frame.bottom() - 62.0), vec2(104.0, 40.0));
        let resp = control(shell, ui, "settings.done", "button", done);
        paint.rect_filled(done, 8.0, if resp.hovered() { p.accent_fill.gamma_multiply(0.9) } else { p.accent_fill });
        centred(&paint, done, &tr("Done"), p.on_accent);
        close |= resp.clicked();
    });
    if close {
        // What is kept: one line, no leading or trailing spaces.
        shell.state.user_name = clean(&shell.state.user_name);
    } else {
        shell.settings = Some(dialog);
    }
}

/// A name as it is kept: one line without its outer spaces, at most [`MAX_NAME`] characters.
pub fn clean(name: &str) -> String {
    let one: String = name.chars().filter(|c| !c.is_control()).take(MAX_NAME).collect();
    one.trim().to_string()
}

fn centred(paint: &egui::Painter, r: Rect, label: &str, color: Color32) {
    let g = ux::galley(paint, label, types::UI_STRONG, color, Some(r.width() - 8.0));
    paint.galley(r.center() - g.size() / 2.0, g, color);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_is_kept_as_one_clean_line() {
        assert_eq!(clean("  Ada Lovelace \n"), "Ada Lovelace");
        assert_eq!(clean(&"x".repeat(200)).chars().count(), MAX_NAME);
        assert_eq!(clean("\u{7}"), "");
    }
}
