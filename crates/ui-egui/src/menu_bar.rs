//! The menu bar in the title strip on Windows and Linux: the groups of the table below, each a popup
//! of rows from the table of actions. macOS keeps its native menu, so `Shell::in_window_menu` is false
//! there and nothing here draws. A row chosen waits for the next frame's start, as a native item does.
//! F10 reaches the bar from the keys: the arrows walk groups and live rows, wrapping, Enter or Space runs the row.

use egui::{Rect, Sense, Stroke, StrokeKind, pos2, vec2};

use crate::Shell;
use crate::actions::{
    self, ABOUT, BACK, BRANCHES, CLOSE, COMPARE, GENERATE, Keys, NEW, NEWER, OPEN, QUIT, REDO, SAVE, SAVE_VERSION, SETTINGS, UNDO, VERSIONS,
};
use crate::home::{control, off_words};
use crate::i18n::tr;
use crate::ux::{self, Palette, types};
use crate::widgets;

/// One menu of the bar.
struct Group {
    /// Part of its automation id, never shown.
    id: &'static str,
    title: fn() -> String,
    /// Runs of actions now, set apart by a divider.
    sections: fn(&Shell) -> Vec<Vec<&'static str>>,
}

/// The bar's menus, left to right. On these systems Quit and About live here, as the design has it.
static GROUPS: &[Group] = &[
    Group { id: "file", title: || tr("File"), sections: |_| vec![vec![NEW, OPEN, SAVE, GENERATE], vec![SETTINGS, CLOSE], vec![QUIT]] },
    Group { id: "edit", title: || tr("Edit"), sections: |_| vec![vec![UNDO, REDO]] },
    Group { id: "model", title: || tr("Model"), sections: crate::model_menu::sections },
    Group { id: "versions", title: || tr("Versions"), sections: |_| actions::VERSIONS_MENU.iter().map(|s| s.to_vec()).collect() },
    Group { id: "help", title: || tr("Help"), sections: |_| vec![vec![ABOUT]] },
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
    pub(crate) chosen: Option<&'static str>,
    /// The row the keys are on; none leaves the highlight to the pointer.
    pub(crate) row: Option<&'static str>,
    /// What had the keyboard focus before the menu opened: egui drops the focus on Esc, so it comes back here.
    pub(crate) back: Option<egui::Id>,
}

/// Once a frame: runs the row chosen, then closes a menu a dialog now covers or the bar no longer draws.
pub(crate) fn keep_valid(shell: &mut Shell, ctx: &egui::Context) {
    if let Some(id) = shell.menu_bar.chosen.take() {
        // A pointer's click on the row took the keyboard from the field the menu was opened on.
        if id.starts_with("edit.")
            && let Some(back) = shell.menu_bar.back
        {
            ctx.memory_mut(|m| m.request_focus(back));
            crate::edit_menu::note(shell, ctx);
        }
        actions::run(shell, id);
    }
    if shell.menu_bar.open.is_some() && !(shell.in_window_menu && actions::free(shell)) {
        shell.menu_bar.open = None;
    }
}

/// Takes a press of `key` with no modifier at all; a key held down counts again only when `repeats`.
/// Not `consume_key`, which lets Shift through: Shift+F10 opens the row menus of the explorer and Chat.
pub(crate) fn press(ctx: &egui::Context, key: egui::Key, repeats: bool) -> bool {
    ctx.input_mut(|i| {
        let found = i.events.iter().position(|e| {
            matches!(e, egui::Event::Key { key: k, pressed: true, repeat, modifiers, .. }
                if *k == key && (repeats || !repeat) && modifiers.matches_exact(egui::Modifiers::NONE))
        });
        found.map(|at| i.events.remove(at)).is_some()
    })
}

/// The next live row of `group` after `from`, going down or up and wrapping; from no row, the first
/// live one that way. None when every row is off.
fn step(shell: &Shell, group: &Group, from: Option<&str>, down: bool) -> Option<&'static str> {
    let mut rows: Vec<&'static str> = (group.sections)(shell).concat();
    if !down {
        rows.reverse();
    }
    let start = from.and_then(|id| rows.iter().position(|r| *r == id)).map_or(0, |at| at + 1);
    (0..rows.len()).map(|n| rows[(start + n) % rows.len()]).find(|id| actions::available(shell, id))
}

/// The keys of the bar, before anything behind it can read them: F10 opens the first menu when the
/// bar can open at all, and with a menu open F10 or Esc closes it, handing the focus back, the
/// arrows move, and Enter or Space runs the row if it can still act.
pub(crate) fn keys(shell: &mut Shell, ctx: &egui::Context) {
    use egui::Key::{ArrowDown, ArrowLeft, ArrowRight, ArrowUp, Enter, Escape, F10, Space};
    let Some(at) = GROUPS.iter().position(|g| shell.menu_bar.open == Some(g.id)) else {
        shell.menu_bar.back = ctx.memory(egui::Memory::focused);
        if shell.in_window_menu && actions::free(shell) && press(ctx, F10, false) {
            shell.menu_bar.open = Some(GROUPS[0].id);
            shell.menu_bar.row = step(shell, &GROUPS[0], None, true);
        }
        return;
    };
    // Tab and the arrows were read for the focus before this frame began; it stays where it was.
    ctx.memory_mut(|m| m.move_focus(egui::FocusDirection::None));
    let from = shell.menu_bar.row;
    let across = if press(ctx, ArrowRight, true) {
        1
    } else if press(ctx, ArrowLeft, true) {
        GROUPS.len() - 1
    } else {
        0
    };
    if press(ctx, Escape, false) || press(ctx, F10, false) {
        shell.menu_bar.open = None;
        if let Some(id) = shell.menu_bar.back {
            ctx.memory_mut(|m| m.request_focus(id));
        }
    } else if across > 0 {
        let group = &GROUPS[(at + across) % GROUPS.len()];
        shell.menu_bar.open = Some(group.id);
        shell.menu_bar.row = step(shell, group, None, true);
    } else if press(ctx, ArrowDown, true) {
        shell.menu_bar.row = step(shell, &GROUPS[at], from, true);
    } else if press(ctx, ArrowUp, true) {
        shell.menu_bar.row = step(shell, &GROUPS[at], from, false);
    } else if (press(ctx, Enter, false) || press(ctx, Space, false))
        && let Some(id) = from.filter(|id| actions::available(shell, id))
    {
        shell.menu_bar.chosen = Some(id);
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
            shell.menu_bar.row = None;
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
    let sections = (group.sections)(shell);
    let window = ui.ctx().content_rect();
    let (count, dividers) = (sections.iter().map(Vec::len).sum::<usize>() as f32, sections.len().saturating_sub(1) as f32 * DIVIDER);
    let pitch = ((window.bottom() - at.y - 8.0 - 2.0 * MENU_PAD - dividers) / count).clamp(types::SMALL.line, ROW);
    let height = 2.0 * MENU_PAD + count * pitch + dividers;
    let x = at.x.min(window.right() - MENU_WIDTH - 8.0).max(window.left() + 8.0);
    let m = Rect::from_min_size(pos2(x, at.y), vec2(MENU_WIDTH, height));
    // A pointer moving over the menu takes the highlight back from the keys.
    if ui.input(|i| i.pointer.delta() != egui::Vec2::ZERO && i.pointer.hover_pos().is_some_and(|at| m.contains(at))) {
        shell.menu_bar.row = None;
    }
    let picked = ux::overlay(ui, widgets::egui_id("strip.menu.layer"), m, |ui| {
        let paint = ui.painter().clone();
        ux::shadow(&paint, m, 8.0, p.shadow);
        paint.rect_filled(m, 8.0, p.surface);
        paint.rect_stroke(m, 8.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
        let mut picked = None;
        let mut y = m.top() + MENU_PAD;
        for (n, section) in sections.iter().enumerate() {
            if n > 0 {
                let line = pos2(m.left() + MENU_PAD + 8.0, y + 4.0);
                paint.rect_filled(Rect::from_min_size(line, vec2(MENU_WIDTH - 2.0 * (MENU_PAD + 8.0), 1.0)), 0.0, p.border);
                y += DIVIDER;
            }
            for id in section {
                let row = Rect::from_min_size(pos2(m.left() + MENU_PAD, y), vec2(MENU_WIDTH - 2.0 * MENU_PAD, pitch));
                y += pitch;
                if draw_row(shell, ui, p, id, row) {
                    picked = Some(*id);
                }
            }
        }
        picked
    });
    (m, picked)
}

/// One row, its name and keys, highlighted under the keys or else the pointer; off, it says why on
/// hover and takes no click. True when clicked.
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
    let keyed = live && shell.menu_bar.row == Some(id);
    if keyed {
        ui.ctx().accesskit_node_builder(widgets::egui_id(&auto), |node| node.set_selected(true));
    }
    if keyed || (live && shell.menu_bar.row.is_none() && resp.hovered()) {
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
        _ if id.starts_with("model.") => crate::model_menu::why_off(shell),
        _ if id.starts_with("versions.") => versions_off(shell, id),
        _ if id.starts_with("edit.") && shell.is_held() => tr("The project is busy"),
        _ if id.starts_with("edit.") && !shell.looking_only() => tr("Nothing to act on here"),
        _ => off_words(shell),
    }
}

/// Why the Versions menu's row `id` is off: no project, then the row's own reason, then a hold.
fn versions_off(shell: &Shell, id: &str) -> String {
    if shell.session.is_none() || shell.project.is_none() || shell.active.is_none() {
        return tr("Open or go back to a project first");
    }
    if id == SAVE_VERSION
        && let Some(why) = crate::save_version::off_because(shell)
    {
        return why;
    }
    let listed = shell.versions.page.as_ref().is_some_and(|p| !p.versions.is_empty());
    match id {
        BACK if shell.version_in_view().is_none() => tr("Current work is showing"),
        _ if shell.is_held() => tr("The project is busy"),
        VERSIONS if shell.version_in_view().is_some() => tr("Back to current work first"),
        COMPARE if !listed => tr("No versions yet."),
        COMPARE => tr("Loading"),
        NEWER if shell.stepping_from().is_none() => tr("No newer version"),
        BRANCHES => tr("This project has one branch"),
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
        let (mut s, ..) = shell();
        assert_eq!(why_off(&s, UNDO), "Nothing to act on here");
        assert_eq!(why_off(&s, "model.note"), "Show a diagram first");
        s.begin_hold().unwrap();
        assert_eq!(why_off(&s, UNDO), "The project is busy");
    }

    /// A Versions row off says first that there is no project, then its own reason, then the hold.
    #[test]
    fn a_versions_row_off_says_why() {
        crate::i18n::set_language("en");
        let (mut s, ..) = shell();
        let words = |s: &Shell| [SAVE_VERSION, COMPARE, BRANCHES, BACK, NEWER].map(|id| why_off(s, id));
        assert_eq!(words(&s), ["Loading", "No versions yet.", "This project has one branch", "Current work is showing", "No newer version"]);
        let entry = crate::VersionEntry {
            hash: "c1".into(),
            description: None,
            author: None,
            by_agent: false,
            date: String::new(),
            previous: None,
            shared: false,
        };
        (s.versions.page, s.versions.stale) = (Some(crate::VersionsPage { versions: vec![entry], more: false }), true);
        assert_eq!(why_off(&s, COMPARE), "Loading", "listed, but perhaps not the newest");
        s.begin_hold().unwrap();
        assert_eq!(why_off(&s, VERSIONS), "The project is busy");
        assert_eq!(why_off(&s, BACK), "Current work is showing", "its own reason comes before the hold");
        let (mut s, ..) = shell();
        s.show_version(actions::tests::session(), "c1".to_string());
        assert_eq!(why_off(&s, VERSIONS), "Back to current work first");
        assert_eq!(why_off(&s, SAVE_VERSION), "Back to current work to save a version");
        s.active = None;
        assert_eq!(why_off(&s, BACK), "Open or go back to a project first");
    }
}
