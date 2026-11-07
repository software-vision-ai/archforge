//! The Settings dialog: 880 points wide, its sections listed on the left (Profile, AI Agents) and
//! Done in the footer.
//!
//! Profile holds who the user is: the name entered here is kept in the preferences, and the bar's
//! avatar shows its initials; with no name it shows JD, for John Doe. Archforge does not read the
//! name from the operating system's account. AI Agents (`crate::settings_agents`) sets up the
//! agent programs Chat talks to.
//!
//! It opens from the avatar and with the command key and comma, over Home or a project; Done,
//! the close button and Escape close it. A choice takes effect as it is made. It is modal: Tab stays
//! in it, and the window behind takes no keys while it is open.

use egui::{Color32, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Vec2, pos2, vec2};

use crate::i18n::tr;
use crate::ux::{self, Palette, icons, types};
use crate::{Shell, widgets};

const WIDTH: f32 = 880.0;
/// The header's and the footer's height.
const HEADER: f32 = 72.0;
const FOOTER: f32 = 72.0;
/// The section list's width, and the content's left edge and right padding.
const NAV: f32 = 200.0;
pub(crate) const CONTENT_LEFT: f32 = NAV + 32.0;
pub(crate) const CONTENT_RIGHT: f32 = 32.0;
/// The body's least height, as Profile draws it.
const MIN_BODY: f32 = 296.0;
/// The least distance from the window's edges when the window is smaller than the dialog.
const EDGE: f32 = 16.0;
/// The longest name kept.
pub const MAX_NAME: usize = 80;
/// The name the avatar stands for while the user has entered none.
const NOBODY: &str = "John Doe";

/// A section of the dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Section {
    #[default]
    Profile,
    Agents,
}

/// The dialog while it is open: its section, whether its field has taken the keyboard yet, how
/// tall its body was last drawn, and the AI Agents page's own state.
#[derive(Debug, Default)]
pub struct Dialog {
    focused: bool,
    pub section: Section,
    body: f32,
    pub(crate) agents: crate::settings_agents::Page,
}

/// The initials the bar's avatar shows: the name's, or JD while there is none.
pub fn avatar_initials(shell: &Shell) -> String {
    let name = shell.state.user_name.trim();
    crate::home::initials(if name.is_empty() { NOBODY } else { name }).unwrap_or_else(|| "JD".into())
}

/// Opens Settings on Profile.
pub fn open(shell: &mut Shell) {
    open_at(shell, Section::Profile);
}

/// Opens Settings on `section`; an open dialog shows it, a key typed there and not saved dropped.
/// Opening runs nothing and sends nothing.
pub fn open_at(shell: &mut Shell, section: Section) {
    match shell.settings.as_mut() {
        Some(d) if d.section != section => {
            d.section = section;
            d.agents = crate::settings_agents::Page::default();
        }
        Some(_) => {}
        None => shell.settings = Some(Dialog { section, ..Dialog::default() }),
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

/// A control of the dialog, recorded under `id`; one that is not `live` only shows its tooltip.
pub(crate) fn control(shell: &mut Shell, ui: &egui::Ui, id: &str, kind: &'static str, rect: Rect, live: bool) -> Response {
    widgets::record_visible(ui, &mut shell.registry, id, kind, rect);
    let resp = ui.interact(rect, widgets::egui_id(id), if live { Sense::click() } else { Sense::hover() });
    if live { resp.on_hover_cursor(egui::CursorIcon::PointingHand) } else { resp }
}

/// How a compact button looks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Look {
    Primary,
    Secondary,
    Danger,
    /// Filled in the danger colour: the button that deletes, in a confirmation.
    DangerFill,
}

/// The design's compact button, 28 tall at `at`, hugging its label: gives its rect and response.
/// One that is not `live` is drawn faded and says `hover` on hover.
pub(crate) fn button(
    shell: &mut Shell,
    ui: &egui::Ui,
    p: &Palette,
    id: &str,
    at: Pos2,
    label: &str,
    look: Look,
    live: bool,
    hover: Option<&str>,
) -> (Rect, Response) {
    let paint = ui.painter().clone();
    let g = ux::galley(&paint, label, types::UI, p.text, None);
    let r = Rect::from_min_size(at, vec2((g.size().x + 20.0).ceil(), 28.0));
    let mut resp = control(shell, ui, id, "button", r, live);
    if let Some(h) = hover {
        resp = resp.on_hover_text(h);
    }
    let _ = resp.clone().accessible_name(label);
    let (fill, edge, ink) = match look {
        Look::Primary if live => (if resp.hovered() { p.accent_fill.gamma_multiply(0.9) } else { p.accent_fill }, None, p.on_accent),
        Look::Primary => (p.accent_fill.gamma_multiply(0.45), None, p.on_accent),
        Look::Secondary => (if live && resp.hovered() { p.page } else { p.surface }, Some(p.border), if live { p.text } else { p.text_muted }),
        Look::Danger => (if live && resp.hovered() { p.danger_tint } else { p.surface }, Some(p.border), if live { p.danger } else { p.text_muted }),
        Look::DangerFill if live => (if resp.hovered() { p.danger.gamma_multiply(0.9) } else { p.danger }, None, p.on_accent),
        Look::DangerFill => (p.danger.gamma_multiply(0.45), None, p.on_accent),
    };
    paint.rect_filled(r, 6.0, fill);
    if let Some(edge) = edge {
        paint.rect_stroke(r, 6.0, Stroke::new(1.0, edge), StrokeKind::Inside);
    }
    let g = ux::galley(&paint, label, types::UI, ink, None);
    paint.galley(pos2(r.left() + 10.0, r.center().y - g.size().y / 2.0), g, ink);
    focus_ring(&paint, p, r, 6.0, &resp);
    (r, resp)
}

/// An icon button `side` points square: an icon without words, its words in the tooltip and the
/// accessible name.
pub(crate) fn icon_button(shell: &mut Shell, ui: &egui::Ui, p: &Palette, id: &str, r: Rect, icon: &ux::Icon, words: &str) -> Response {
    let paint = ui.painter().clone();
    let resp = control(shell, ui, id, "button", r, true).on_hover_text(words);
    let _ = resp.clone().accessible_name(words);
    if resp.hovered() {
        paint.rect_filled(r, 6.0, p.border);
    }
    ux::icon_in(&paint, r, icon, p.text_secondary);
    focus_ring(&paint, p, r, 6.0, &resp);
    resp
}

/// Where the window keeps whether the keyboard was used last.
fn keyboard_id() -> egui::Id {
    egui::Id::unique("archforge.keyboard-last")
}

/// Once per frame: notes whether the keyboard or the pointer was used last, so focus rings show
/// only while the keyboard leads (AR-217). A key pressed sets it, a pointer button pressed clears it.
pub(crate) fn note_keyboard(ctx: &egui::Context) {
    let last = ctx.input(|i| {
        i.events.iter().rev().find_map(|e| match e {
            egui::Event::Key { pressed: true, .. } => Some(true),
            egui::Event::PointerButton { pressed: true, .. } => Some(false),
            _ => None,
        })
    });
    if let Some(keyboard) = last {
        ctx.data_mut(|d| d.insert_temp(keyboard_id(), keyboard));
    }
}

/// Whether the keyboard was used last, since the pointer last pressed.
pub(crate) fn keyboard_last(ctx: &egui::Context) -> bool {
    ctx.data(|d| d.get_temp::<bool>(keyboard_id())).unwrap_or(false)
}

/// The ring a control that has the keyboard shows, while the keyboard was used last.
pub(crate) fn focus_ring(paint: &egui::Painter, p: &Palette, r: Rect, radius: f32, resp: &Response) {
    if resp.has_focus() && keyboard_last(&resp.ctx) {
        paint.rect_stroke(r.expand(1.0), radius + 1.0, Stroke::new(2.0, p.focus), StrokeKind::Outside);
    }
}

/// A status badge's tone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Tone {
    Good,
    Warn,
    Bad,
    Quiet,
}

/// The design's status badge: a dot and a short text on its tone's tint, 20 tall, its left edge
/// at `left` or its right edge at `right`, cut to `room`.
pub(crate) fn badge(paint: &egui::Painter, p: &Palette, y: f32, left: Option<f32>, right: Option<f32>, text: &str, tone: Tone, room: f32) -> Rect {
    let (tint, ink) = match tone {
        Tone::Good => (p.green_tint, p.green),
        Tone::Warn => (p.amber_tint, p.amber),
        Tone::Bad => (p.danger_tint, p.danger),
        Tone::Quiet => (p.field, p.text_secondary),
    };
    let g = ux::galley(paint, text, types::CAPTION, ink, Some((room - 28.0).max(0.0)));
    let w = g.size().x + 30.0;
    let x = match (left, right) {
        (Some(l), _) => l,
        (None, Some(r)) => r - w,
        (None, None) => 0.0,
    };
    let r = Rect::from_min_size(pos2(x, y), vec2(w, 20.0));
    paint.rect_filled(r, 10.0, tint);
    paint.circle_filled(pos2(r.left() + 11.0, r.center().y), 3.0, ink);
    paint.galley(pos2(r.left() + 20.0, r.center().y - g.size().y / 2.0), g, ink);
    r
}

/// Text wrapped at `width` into at most `rows` lines at `at`: gives its height.
pub(crate) fn wrapped(paint: &egui::Painter, at: Pos2, width: f32, text: &str, ty: ux::Type, color: Color32, rows: usize) -> f32 {
    let g = ux::galley_rows(paint, text, ty, color, Some(width.max(0.0)), rows);
    let h = g.size().y;
    paint.galley(at, g, color);
    h
}

pub fn show(shell: &mut Shell, ui: &mut egui::Ui) {
    if shell.settings.is_none() || !ux::ready(ui.ctx()) {
        return;
    }
    let Some(mut dialog) = shell.settings.take() else { return };
    let p = *ux::palette(shell.state.theme.is_dark());
    let window = ui.max_rect();
    // Escape closes an open list first, then the dialog.
    let mut close = false;
    if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
        if dialog.agents.menu.is_some() {
            dialog.agents.menu = None;
        } else {
            close = true;
        }
    }
    egui::Area::new(egui::Id::unique("settings")).order(egui::Order::Foreground).fixed_pos(window.min).show(ui.ctx(), |ui| {
        ui.set_min_size(window.size());
        // A modal layer: Tab and clicks stay in the dialog, so nothing behind it moves.
        ui.ctx().memory_mut(|m| m.set_modal_layer(ui.layer_id()));
        // The veil takes every click behind the dialog.
        ui.painter().rect_filled(window, 0.0, p.scrim);
        let _ = ui.interact(window, ui.scope_id().with("settings.veil"), Sense::CLICK | Sense::DRAG);
        let body = dialog.body.max(MIN_BODY);
        let size = vec2(WIDTH.min(window.width() - 2.0 * EDGE).max(0.0), (HEADER + body + FOOTER).min(window.height() - 2.0 * EDGE).max(0.0));
        let frame = Rect::from_center_size(window.center(), size);
        let paint = ui.painter().clone();
        paint.add(egui::epaint::Shadow { offset: [0, 16], blur: 48, spread: 0, color: p.shadow }.as_shape(frame, 16.0));
        paint.rect_filled(frame, 16.0, p.surface);
        paint.rect_stroke(frame, 16.0, Stroke::new(1.0, p.dialog_edge), StrokeKind::Inside);
        let _ = ui.interact(frame, ui.scope_id().with("settings.frame"), Sense::CLICK);
        let at = |x: f32, y: f32| frame.min + vec2(x, y);
        // The header: the title and the close button.
        ux::text(&paint, at(32.0, 22.0), &tr("Settings"), types::SECTION, p.text, Some((frame.width() - 120.0).max(0.0)));
        let x = Rect::from_min_size(at(frame.width() - 56.0, 20.0), Vec2::splat(32.0));
        let resp = control(shell, ui, "settings.close", "button", x, true).on_hover_text(tr("Close"));
        let _ = resp.clone().accessible_name(tr("Close"));
        if resp.hovered() {
            paint.rect_filled(x, 8.0, p.border);
        }
        ux::icon(&paint, x.min + vec2(9.0, 9.0), &icons::CLOSE, 14.0, p.text_secondary);
        focus_ring(&paint, &p, x, 8.0, &resp);
        close |= resp.clicked();
        let top = frame.top() + HEADER;
        let bottom = (frame.bottom() - FOOTER).max(top);
        paint.hline(frame.x_range(), top, Stroke::new(1.0, p.border));
        paint.hline(frame.x_range(), bottom, Stroke::new(1.0, p.border));
        paint.vline(frame.left() + NAV, top..=bottom, Stroke::new(1.0, p.border));
        // The section list.
        let sections = [(Section::Profile, "profile", tr("Profile"), &icons::PERSON), (Section::Agents, "agents", tr("AI Agents"), &icons::AGENT)];
        for (i, (section, key, label, icon)) in sections.into_iter().enumerate() {
            let r = Rect::from_min_size(pos2(frame.left() + 16.0, top + 16.0 + 36.0 * i as f32), vec2(NAV - 32.0, 32.0));
            if r.bottom() > bottom {
                break;
            }
            let resp = control(shell, ui, &format!("settings.nav.{key}"), "tab", r, true);
            let _ = resp.clone().accessible_name(&label);
            let chosen = dialog.section == section;
            if chosen {
                paint.rect_filled(r, 6.0, p.accent_tint);
            } else if resp.hovered() {
                paint.rect_filled(r, 6.0, p.page);
            }
            let ink = if chosen { p.accent } else { p.text };
            ux::icon_in(
                &paint,
                Rect::from_center_size(pos2(r.left() + 18.0, r.center().y), Vec2::splat(16.0)),
                icon,
                if chosen { p.accent } else { p.text_secondary },
            );
            ux::text_mid(&paint, r.left() + 34.0, r.center().y, &label, types::UI, ink, Some(r.width() - 44.0));
            focus_ring(&paint, &p, r, 6.0, &resp);
            if resp.clicked() && !chosen {
                dialog.section = section;
                dialog.agents.menu = None;
                crate::settings_agents::shown_again(&mut dialog.agents);
                // A key typed and not saved does not wait behind another section.
                crate::settings_agents::forget_drafts(ui.ctx(), &mut dialog.agents);
            }
        }
        // The section's page, scrolled when the window is too short to show it all.
        let content = Rect::from_min_max(pos2(frame.left() + NAV + 1.0, top + 1.0), pos2(frame.right(), bottom - 1.0));
        let mut part = ui.new_child(egui::UiBuilder::new().max_rect(content));
        part.set_clip_rect(content);
        let reach = egui::ScrollArea::vertical()
            .id_salt(widgets::egui_id(match dialog.section {
                Section::Profile => "settings.body.profile",
                Section::Agents => "settings.body.agents",
            }))
            .auto_shrink([false, false])
            .max_height(content.height())
            .show(&mut part, |ui| {
                let (area, _) = ui.allocate_exact_size(vec2(content.width(), dialog.body.max(MIN_BODY) - 2.0), Sense::hover());
                let left = frame.left() + CONTENT_LEFT;
                let width = (frame.right() - CONTENT_RIGHT - left).max(0.0);
                match dialog.section {
                    Section::Profile => profile(shell, ui, &p, &mut dialog, &mut close, left, area.top(), width),
                    Section::Agents => crate::settings_agents::page(shell, ui, &p, &mut dialog.agents, left, area.top(), width),
                }
            })
            .inner;
        // The body hugs what the page drew; the next frame takes the new height.
        let wanted = (reach + 2.0).max(MIN_BODY);
        if (wanted - dialog.body).abs() > 0.5 {
            dialog.body = wanted;
            ui.ctx().request_repaint();
        }
        // Done, at the footer's right.
        let g = ux::galley(&paint, &tr("Done"), types::UI, p.on_accent, None);
        let w = (g.size().x + 24.0).max(52.0);
        let done = Rect::from_min_size(pos2(frame.right() - 50.0 - w, bottom + 20.0), vec2(w, 32.0));
        let resp = control(shell, ui, "settings.done", "button", done, true);
        let _ = resp.clone().accessible_name(tr("Done"));
        paint.rect_filled(done, 6.0, if resp.hovered() { p.accent_fill.gamma_multiply(0.9) } else { p.accent_fill });
        paint.galley(done.center() - g.size() / 2.0, g, p.on_accent);
        focus_ring(&paint, &p, done, 6.0, &resp);
        close |= resp.clicked();
    });
    if close {
        // What is kept: one line, no leading or trailing spaces; a key typed and not saved is
        // dropped, overwritten, and the fields' own memory of it cleared.
        shell.state.user_name = clean(&shell.state.user_name);
        crate::settings_agents::forget_drafts(ui.ctx(), &mut dialog.agents);
    } else {
        shell.settings = Some(dialog);
    }
}

/// Profile: Your name, beside the initials it makes. Gives how far the page reaches below `top`.
fn profile(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, dialog: &mut Dialog, close: &mut bool, left: f32, top: f32, width: f32) -> f32 {
    let paint = ui.painter().clone();
    ux::text(&paint, pos2(left, top + 26.0), &tr("Profile"), types::LEAD, p.text, Some(width));
    let h = wrapped(
        &paint,
        pos2(left, top + 52.0),
        width,
        &tr("Your name shows as initials in the bar's avatar. Archforge never reads it from the computer's account."),
        types::UI,
        p.text_secondary,
        3,
    );
    let y = top + 52.0 + h + 28.0;
    ux::text(&paint, pos2(left, y), &tr("Your name"), types::SMALL, p.text_secondary, Some(width));
    let field = Rect::from_min_size(pos2(left, y + 22.0), vec2(300.0_f32.min(width - 40.0).max(0.0), 28.0));
    paint.rect_filled(field, 6.0, p.field);
    let font = types::UI.font();
    let row = ui.fonts_mut(|f| f.row_height(&font));
    let text_rect = Rect::from_min_size(pos2(field.left() + 8.0, field.center().y - row / 2.0), vec2((field.width() - 16.0).max(0.0), row));
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
    *close |= resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
    if resp.has_focus() {
        paint.rect_stroke(field, 6.0, Stroke::new(2.0, p.focus), StrokeKind::Inside);
    }
    let avatar = Rect::from_center_size(pos2(field.right() + 24.0, field.center().y), Vec2::splat(28.0));
    paint.rect_filled(avatar, 14.0, p.accent_tint);
    let g = ux::galley(&paint, &avatar_initials(shell), types::CAPTION, p.accent, None);
    paint.galley(avatar.center() - g.size() / 2.0, g, p.accent);
    field.bottom() + 24.0 - top
}

/// A name as it is kept: one line without its outer spaces, at most [`MAX_NAME`] characters.
pub fn clean(name: &str) -> String {
    let one: String = name.chars().filter(|c| !c.is_control()).take(MAX_NAME).collect();
    one.trim().to_string()
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
