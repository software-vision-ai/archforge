//! The menu bar in the title strip on Windows and Linux: the groups of the table below, each a popup
//! of rows from the table of actions. macOS keeps its native menu, so `Shell::in_window_menu` is false
//! there and nothing here draws. A row chosen waits for the next frame's start, as a native item does.

use egui::{Rect, Sense, Stroke, StrokeKind, pos2, vec2};

use crate::Shell;
use crate::actions::{self, ABOUT, CLOSE, GENERATE, Keys, NEW, OPEN, QUIT, SAVE, SETTINGS};
use crate::home::{control, off_words};
use crate::i18n::tr;
use crate::ux::{self, Palette, types};
use crate::widgets;

/// One menu of the bar.
struct Group {
    /// Part of its automation id, never shown.
    id: &'static str,
    title: fn() -> String,
    /// Runs of actions, set apart by a divider.
    sections: &'static [&'static [&'static str]],
}

/// The bar's menus, left to right. On these systems Quit and About live here, as the design has it.
static GROUPS: &[Group] = &[
    Group { id: "file", title: || tr("File"), sections: &[&[NEW, OPEN, SAVE, GENERATE], &[SETTINGS, CLOSE], &[QUIT]] },
    Group { id: "help", title: || tr("Help"), sections: &[&[ABOUT]] },
];

const GROUP_HEIGHT: f32 = 24.0;
const GROUP_PAD: f32 = 8.0;
const GROUP_GAP: f32 = 4.0;
const MENU_WIDTH: f32 = 300.0;
const MENU_PAD: f32 = 4.0;
const ROW: f32 = 30.0;
const DIVIDER: f32 = 9.0;

/// The bar between frames: the group open, by id, and the action of a row chosen, waiting to run.
#[derive(Default)]
pub(crate) struct Bar {
    pub(crate) open: Option<&'static str>,
    chosen: Option<&'static str>,
}

/// Once a frame: runs the row chosen, then closes a menu a dialog now covers or the bar no longer draws.
pub(crate) fn keep_valid(shell: &mut Shell) {
    if let Some(id) = shell.menu_bar.chosen.take() {
        actions::run(shell, id);
    }
    if shell.menu_bar.open.is_some() && !(shell.in_window_menu && actions::free(shell)) {
        shell.menu_bar.open = None;
    }
}

/// Escape closes an open menu before anything behind it can read the key.
pub(crate) fn dismiss_key(shell: &mut Shell, ctx: &egui::Context) {
    if shell.menu_bar.open.is_some() && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
        shell.menu_bar.open = None;
    }
}

/// The keys of `keys` written for Windows and Linux, such as "Ctrl+Shift+G", in the catalogue's words.
pub(crate) fn chord(keys: &Keys) -> String {
    let mut parts = vec![tr("Ctrl")];
    if keys.modifiers.shift {
        parts.push(tr("Shift"));
    }
    parts.push(keys.key.symbol_or_name().to_string());
    parts.join("+")
}

/// Draws the bar in `strip` from `start` and the open group's menu; a press elsewhere closes it.
pub(crate) fn show(shell: &mut Shell, ui: &egui::Ui, p: &Palette, strip: Rect, start: f32) {
    if !shell.in_window_menu {
        return;
    }
    let paint = ui.painter().clone();
    let (mid, mut x, mut bar, mut shown) = (strip.center().y, start, Rect::NOTHING, None);
    for group in GROUPS {
        let was_open = shell.menu_bar.open == Some(group.id);
        let title = (group.title)();
        let text = ux::galley(&paint, &title, types::SMALL, p.text, None);
        let size = vec2(text.size().x + 2.0 * GROUP_PAD, GROUP_HEIGHT);
        let button = Rect::from_center_size(pos2(x + size.x / 2.0, mid), size);
        let resp = control(shell, ui, &format!("strip.menu.{}", group.id), "menu", button, true).accessible_name(title.as_str());
        if was_open {
            paint.rect_filled(button, 5.0, p.surface);
        } else if resp.hovered() {
            paint.rect_filled(button, 5.0, p.border);
        }
        paint.galley(pos2(button.left() + GROUP_PAD, mid - text.size().y / 2.0), text, p.text);
        if resp.clicked() {
            shell.menu_bar.open = if was_open { None } else { Some(group.id) };
        }
        if shell.menu_bar.open == Some(group.id) {
            shown = Some((group, button));
        }
        bar = bar.union(button);
        x = button.right() + GROUP_GAP;
    }
    let Some((group, button)) = shown else { return };
    let (menu, picked) = popup(shell, ui, p, group, pos2(button.left(), strip.bottom()));
    let outside = ui.input(|i| i.pointer.any_pressed() && !i.pointer.interact_pos().is_some_and(|at| menu.contains(at) || bar.contains(at)));
    if let Some(id) = picked {
        shell.menu_bar.chosen = Some(id);
        shell.menu_bar.open = None;
        ui.ctx().request_repaint();
    } else if outside {
        shell.menu_bar.open = None;
    }
}

/// The menu of `group` at `at`, kept inside the window: its rectangle and the row picked this frame.
fn popup(shell: &mut Shell, ui: &egui::Ui, p: &Palette, group: &Group, at: egui::Pos2) -> (Rect, Option<&'static str>) {
    let height = 2.0 * MENU_PAD + group.sections.iter().map(|s| s.len() as f32 * ROW + DIVIDER).sum::<f32>() - DIVIDER;
    let window = ui.ctx().content_rect();
    let x = at.x.min(window.right() - MENU_WIDTH - 8.0).max(window.left() + 8.0);
    let m = Rect::from_min_size(pos2(x, at.y), vec2(MENU_WIDTH, height));
    let picked = ux::overlay(ui, widgets::egui_id("strip.menu.layer"), m, |ui| {
        let paint = ui.painter().clone();
        ux::shadow(&paint, m, 8.0, p.shadow);
        paint.rect_filled(m, 8.0, p.surface);
        paint.rect_stroke(m, 8.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
        let mut picked = None;
        let mut y = m.top() + MENU_PAD;
        for (n, section) in group.sections.iter().enumerate() {
            if n > 0 {
                let line = pos2(m.left() + MENU_PAD + 8.0, y + 4.0);
                paint.rect_filled(Rect::from_min_size(line, vec2(MENU_WIDTH - 2.0 * (MENU_PAD + 8.0), 1.0)), 0.0, p.border);
                y += DIVIDER;
            }
            for id in *section {
                let row = Rect::from_min_size(pos2(m.left() + MENU_PAD, y), vec2(MENU_WIDTH - 2.0 * MENU_PAD, ROW));
                y += ROW;
                if draw_row(shell, ui, p, id, row) {
                    picked = Some(*id);
                }
            }
        }
        picked
    });
    (m, picked)
}

/// One row, its name and keys; off, it says why on hover and takes no click. True when clicked.
fn draw_row(shell: &mut Shell, ui: &egui::Ui, p: &Palette, id: &str, row: Rect) -> bool {
    let paint = ui.painter();
    let label = actions::label(shell, id);
    let auto = format!("strip.menu.row.{id}");
    let live = actions::available(shell, id);
    let resp = if live {
        control(shell, ui, &auto, "menu-item", row, true).accessible_name(label.as_str())
    } else {
        widgets::record_visible(ui, &mut shell.registry, &auto, "menu-item", row);
        let why = why_off(shell, id);
        let resp = ui.interact(row, widgets::egui_id(&auto), Sense::hover());
        ui.ctx().accesskit_node_builder(widgets::egui_id(&auto), |node| {
            node.set_role(egui::accesskit::Role::MenuItem);
            node.set_disabled();
        });
        resp.on_hover_text(why).accessible_name(label.as_str())
    };
    if live && resp.hovered() {
        paint.rect_filled(row, 5.0, p.accent_tint);
    }
    let keys = actions::keys(id).map(chord).unwrap_or_default();
    let detail = ux::galley(paint, &keys, types::CAPTION, p.text_secondary, None);
    let width = detail.size().x;
    paint.galley(pos2(row.right() - 10.0 - width, row.center().y - detail.size().y / 2.0), detail, p.text_secondary);
    let ink = if live { p.text } else { p.text_muted };
    ux::text_mid(paint, row.left() + 10.0, row.center().y, &label, types::SMALL, ink, Some((row.width() - 28.0 - width).max(0.0)));
    live && resp.clicked()
}

/// Why the row of the action `id` is off, in words: the first thing that keeps it off.
fn why_off(shell: &Shell, id: &str) -> String {
    let no_project = shell.session.is_none() || shell.active.is_none();
    match id {
        NEW | OPEN if shell.tabs.len() >= crate::MAX_TABS => tr("Close a project to open another"),
        OPEN if shell.opening.is_some() => tr("Opening…"),
        SAVE | GENERATE | CLOSE if no_project => tr("Open or go back to a project first"),
        SAVE if shell.saving.is_some() => tr("Saving…"),
        SAVE if shell.is_held() => tr("The project is busy"),
        CLOSE if shell.active.and_then(|i| shell.tabs.get(i)).is_some_and(|t| t.closing.is_some()) => tr("The project is busy"),
        _ => off_words(shell),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actions::tests::{shell, tab};
    use std::sync::mpsc::channel;

    /// The keys are written as the design writes them.
    #[test]
    fn keys_are_written_as_the_design_has_them() {
        crate::i18n::set_language("en");
        let text = |id: &str| actions::keys(id).map(chord);
        assert_eq!(text(GENERATE).as_deref(), Some("Ctrl+Shift+G"));
        assert_eq!(text(SETTINGS).as_deref(), Some("Ctrl+,"));
        assert_eq!(text(NEW), None);
    }

    /// Each row off names the first thing that keeps it off, falling back to the general words.
    #[test]
    fn a_row_off_says_why() {
        crate::i18n::set_language("en");
        let (mut s, ..) = shell();
        s.opening = Some(channel().1);
        assert_eq!(why_off(&s, OPEN), "Opening…");
        s.saving = Some((channel().1, 0, 0));
        assert_eq!(why_off(&s, SAVE), "Saving…");
        s.saving = None;
        s.begin_hold().unwrap();
        assert_eq!(why_off(&s, SAVE), "The project is busy");
        let (mut s, ..) = shell();
        s.tabs[0].closing = Some(channel().1);
        assert_eq!(why_off(&s, CLOSE), "The project is busy");
        s.active = None;
        assert_eq!(why_off(&s, SAVE), "Open or go back to a project first");
        s.tabs = (0..crate::MAX_TABS).map(|_| tab()).collect();
        assert_eq!(why_off(&s, NEW), "Close a project to open another");
    }
}
