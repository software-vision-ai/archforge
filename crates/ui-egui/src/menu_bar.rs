//! The menu bar in the title strip on Windows and Linux: the groups of the table below, each a popup
//! of rows from the table of actions. macOS keeps its native menu, so `Shell::in_window_menu` is false
//! there and nothing here draws. A row chosen waits for the next frame's start, as a native item does.
//! F10 reaches the bar from the keys: the arrows walk groups and rows, off or not, wrapping, Enter or Space runs a row that can act.
//! Alt with a title's letter opens its menu, and with a menu open a row's letter, alone or with Alt, runs the row if it can act.
//! To a screen reader the bar is a menu bar, the open menu its title's menu, and the row under the keys has its focus.
//! A press away from the bar and its open menu only closes the menu, and so does the window losing the focus.
//! Each title and row has an access letter, its catalogue row in the `access` context keyed by the title's
//! `menu.<group>` or the row's action id: underlined while Alt alone is held or the keys walk a menu, as
//! Windows shows them, and the AccessKit access key of its node, which the Windows and Linux adapters do
//! not report to a screen reader yet.

use std::sync::Arc;

use archforge_shell_api::menu_bar::Hold;
use egui::text_edit::TextEditState;
use egui::{Rect, Sense, Stroke, StrokeKind, pos2, vec2};
use ux::Colour;

use crate::Shell;
use crate::actions::{
    self, ABOUT, BACK, BRANCHES, CLOSE, COMPARE, COPY, CUT, GENERATE, Keys, NEW, NEW_BRANCH, NEWER, OPEN, PASTE, REDO, SAVE, SAVE_VERSION,
    SELECT_ALL, UNDO, VERSIONS,
};
use crate::cx::Ctx;
use crate::home::{control, off_words};
use crate::i18n::{fmt, tr, tr_ctx};
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
    Group { id: "file", title: || tr("File"), sections: actions::file_menu },
    Group { id: "edit", title: || tr("Edit"), sections: |_| EDIT_MENU.iter().map(|s| s.to_vec()).collect() },
    Group { id: "view", title: || tr_ctx("menu", "View"), sections: |_| crate::view_menu::VIEW_MENU.iter().map(|s| s.to_vec()).collect() },
    Group { id: "model", title: || tr("Model"), sections: crate::model_menu::sections },
    Group { id: "versions", title: || tr("Versions"), sections: |_| actions::VERSIONS_MENU.iter().map(|s| s.to_vec()).collect() },
    Group { id: "help", title: || tr("Help"), sections: |_| vec![vec![ABOUT]] },
];

/// The Edit menu's rows by section.
static EDIT_MENU: [&[&str]; 3] = [&[UNDO, REDO], &[CUT, COPY, PASTE], &[SELECT_ALL]];

/// The catalogues' context of the access letters.
const ACCESS: &str = "access";

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
    /// The text `back` had selected, when it is a field: a field that loses the focus drops it.
    pub(crate) selected: Option<egui::text::CCursorRange>,
    /// Where the bar was drawn last.
    rect: Option<Rect>,
    /// Whether a press of this frame landed on the bar or on its open menu.
    pub(crate) pressed: bool,
    /// The inline editor the bar keeps open after taking its keyboard, until the menu closes.
    pub(crate) held: Option<egui::Id>,
    /// Whether a click on the open menu's own title closed it: the keyboard goes back, as after Esc,
    /// once that click is over, since a field gives up a focus handed to it during a click elsewhere.
    closed_by_title: bool,
}

impl Bar {
    /// The bar's hold, for the narrow context, over this bar.
    pub(crate) fn hold(&mut self) -> Hold<'_> {
        Hold { open: self.open, chosen: self.chosen, back: self.back, rect: self.rect, held: &mut self.held }
    }
}

/// Once a frame: runs the row chosen, then closes a menu the window lost the focus under, as Esc
/// closes it, and one a dialog now covers or the bar no longer draws.
pub(crate) fn keep_valid(shell: &mut Shell, ctx: &egui::Context) {
    shell.menu_bar.pressed = false;
    let unfocused = ctx.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::WindowFocused(false))));
    if unfocused && shell.menu_bar.open.take().is_some() {
        give_back(shell, ctx);
    }
    if let Some(id) = shell.menu_bar.chosen.take() {
        // A pointer's click on the row took the keyboard, and its selection, from the field the
        // menu was opened on.
        if id.starts_with("edit.") && shell.menu_bar.back.is_some() {
            give_back(shell, ctx);
            crate::edit_menu::note(shell, ctx);
        }
        actions::run(shell, id);
    }
    // A menu closed by its own title, with no row chosen, gives the keyboard back and leaves the name
    // typed in place as it was; once, so a name that went meanwhile takes no keyboard later.
    if shell.menu_bar.open.is_none() && !on_bar(shell, ctx) {
        let by_title = std::mem::take(&mut shell.menu_bar.closed_by_title);
        if shell.menu_bar.held.take().is_some() || by_title {
            give_back(shell, ctx);
        }
    }
    if shell.menu_bar.open.is_some() && !(shell.in_window_menu && actions::free(shell)) {
        shell.menu_bar.open = None;
    }
}

/// Gives the field the menu was opened on the keyboard back, with the text it had selected.
fn give_back(shell: &Shell, ctx: &egui::Context) {
    let Some(back) = shell.menu_bar.back else { return };
    if let (Some(range), Some(mut field)) = (shell.menu_bar.selected, TextEditState::load(ctx, back)) {
        field.cursor.set_char_range(Some(range));
        field.store(ctx, back);
    }
    ctx.memory_mut(|m| m.request_focus(back));
}

/// Whether the pointer is down on the bar, or let go on it this frame.
fn on_bar(shell: &Shell, ctx: &egui::Context) -> bool {
    on(shell.menu_bar.rect, ctx)
}

/// Whether the pointer is down on the bar drawn at `rect`, or let go on it this frame.
fn on(rect: Option<Rect>, ctx: &egui::Context) -> bool {
    ctx.input(|i| (i.pointer.any_down() || i.pointer.any_released()) && rect.zip(i.pointer.interact_pos()).is_some_and(|(bar, at)| bar.contains(at)))
}

/// [`ends_cx`] over the whole shell, for code not on the narrow context yet.
pub(crate) fn ends(shell: &mut Shell, ctx: &egui::Context, id: egui::Id, lost: bool, focused: bool) -> bool {
    shell.cx(|cx| ends_cx(cx, ctx, id, lost, focused))
}

/// Whether the inline editor `id`, which commits when it loses the keyboard, commits now: on `lost`,
/// unless the bar holds it, having taken the keyboard for the Edit menu, for an Edit row chosen, or
/// for a press on the bar before its menu opens. A held editor commits in the first frame another
/// menu is open, or a row of another menu is chosen, so that row finds it committed when it runs
/// the frame after, as with no menu; an Edit row or a press that opens nothing gives it the
/// keyboard back instead.
pub(crate) fn ends_cx(cx: &mut Ctx, ctx: &egui::Context, id: egui::Id, lost: bool, focused: bool) -> bool {
    let bar = &mut cx.menu_bar;
    let edit = bar.open == Some("edit") || bar.chosen.is_some_and(|r| r.starts_with("edit.")) || (bar.open.is_none() && on(bar.rect, ctx));
    if bar.back == Some(id) && edit {
        *bar.held = Some(id);
        return false;
    }
    let held = bar.held.take_if(|h| *h == id).is_some();
    lost || (held && !focused)
}

/// Whether a title of the bar is clicked in this frame, by the pointer or by a screen reader, so that
/// its menu opens, or closes, as the strip is drawn. Known as the frame starts, so that the shortcuts
/// and `Shell::ui` hold the frame's keys before anything behind the bar reads them (AR-518). A title
/// that has the keyboard opens on its own Space or Enter, which a hold would take from it; there
/// nothing behind the bar has the keyboard. Only a bar drawn has titles to click.
pub(crate) fn clicked(ctx: &egui::Context) -> bool {
    let pointer = ctx.interaction_snapshot(|s| s.clicked);
    let title = |g: &Group| widgets::egui_id(&format!("strip.menu.{}", g.id));
    GROUPS.iter().map(title).any(|t| pointer == Some(t) || ctx.input(|i| i.has_accesskit_action_request(t, egui::accesskit::Action::Click)))
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

/// The next row of `group` after `from`, off or not, going down or up and wrapping; from no row,
/// the first that way. A row off is reached so that its reason is, and takes no Enter.
fn step(shell: &Shell, group: &Group, from: Option<&str>, down: bool) -> Option<&'static str> {
    let mut rows: Vec<&'static str> = (group.sections)(shell).concat();
    if !down {
        rows.reverse();
    }
    let start = from.and_then(|id| rows.iter().position(|r| *r == id)).map_or(0, |at| at + 1);
    rows.get(start % rows.len().max(1)).copied()
}

/// The row a menu opens on from the keys: the first that can act, or the first when none can, so
/// that a menu with every row off still says a row.
fn first(shell: &Shell, group: &Group) -> Option<&'static str> {
    let rows = (group.sections)(shell).concat();
    rows.iter().find(|id| actions::available(shell, id)).or(rows.first()).copied()
}

/// Puts a screen reader's focus on the row under the keys, as the active descendant of what has
/// the keyboard: the field the menu was opened on, or else the window. The keyboard itself never
/// moves, so the field keeps its text, its selection and the name typed in place. Called once the
/// frame is drawn, when the field and the row are both known to the reader: a row is one only
/// while its menu is open.
pub(crate) fn say_row(shell: &Shell, ctx: &egui::Context) {
    let row = shell.menu_bar.row.map(|id| widgets::egui_id(&format!("strip.menu.row.{id}")));
    let Some(row) = row.filter(|row| ctx.has_accesskit_node(*row)) else { return };
    let holder = ctx.memory(egui::Memory::focused).filter(|id| ctx.has_accesskit_node(*id)).unwrap_or_else(egui::accesskit_root_id);
    ctx.accesskit_node_builder(holder, |node| node.set_active_descendant(row.accesskit_id()));
}

/// The keys of the bar, before anything behind it can read them: F10 opens the first menu and Alt
/// with a title's letter that title's, when the bar can open at all, and with a menu open F10 or Esc
/// closes it, handing the focus back, the arrows move, and Enter or Space, or a row's letter, runs
/// the row if it can act.
pub(crate) fn keys(shell: &mut Shell, ctx: &egui::Context) {
    use egui::Key::{ArrowDown, ArrowLeft, ArrowRight, ArrowUp, Enter, Escape, F10, Space};
    let Some(at) = GROUPS.iter().position(|g| shell.menu_bar.open == Some(g.id)) else {
        shell.menu_bar.back = ctx.memory(egui::Memory::focused);
        shell.menu_bar.selected = shell.menu_bar.back.and_then(|id| TextEditState::load(ctx, id)?.cursor.char_range());
        if !(shell.in_window_menu && actions::free(shell)) {
            return;
        }
        // An Alt letter no title has opens nothing and types nothing, as on Windows.
        let opens = if press(ctx, F10, false) {
            GROUPS.first()
        } else {
            ctx.input_mut(|i| take_letter(i, false)).and_then(|c| GROUPS.iter().find(|g| letter(&format!("menu.{}", g.id)) == Some(c)))
        };
        if let Some(group) = opens {
            shell.menu_bar.open = Some(group.id);
            shell.menu_bar.row = first(shell, group);
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
        give_back(shell, ctx);
    } else if across > 0 {
        let group = &GROUPS[(at + across) % GROUPS.len()];
        shell.menu_bar.open = Some(group.id);
        shell.menu_bar.row = first(shell, group);
    } else if press(ctx, ArrowDown, true) {
        shell.menu_bar.row = step(shell, &GROUPS[at], from, true);
    } else if press(ctx, ArrowUp, true) {
        shell.menu_bar.row = step(shell, &GROUPS[at], from, false);
    } else if (press(ctx, Enter, false) || press(ctx, Space, false))
        && let Some(id) = from.filter(|id| actions::available(shell, id))
    {
        choose(shell, ctx, id);
    } else if let Some(c) = ctx.input_mut(|i| take_letter(i, true))
        && let Some(id) = (GROUPS[at].sections)(shell).concat().into_iter().find(|id| letter(id) == Some(c))
        && actions::available(shell, id)
    {
        // A letter no row has, or a row's that is off, does nothing.
        choose(shell, ctx, id);
    }
}

/// Chooses the row `id` from the keys: the menu closes at once and the row runs on the next frame.
fn choose(shell: &mut Shell, ctx: &egui::Context, id: &'static str) {
    shell.menu_bar.chosen = Some(id);
    shell.menu_bar.open = None;
    // Another menu's row finds a field committed, a name typed in place too, as after a click.
    if !id.starts_with("edit.")
        && let Some(back) = shell.menu_bar.back
    {
        ctx.memory_mut(|m| m.surrender_focus(back));
    }
}

/// Takes the first press of a letter key A to Z in `input` with Alt alone held, or with nothing held
/// either when `bare`, and the text it typed, which follows it, so no field drawn later types it;
/// its letter, unless it is a held key's repeat, which counts for nothing. Ctrl+Alt, as AltGr comes
/// on Windows, Shift, and the command key make another key's press, which stays in the input.
fn take_letter(input: &mut egui::InputState, bare: bool) -> Option<char> {
    let ours =
        |held: &egui::Modifiers| actions::held_exactly(*held, egui::Modifiers::ALT) || (bare && actions::held_exactly(*held, egui::Modifiers::NONE));
    let at = input
        .events
        .iter()
        .position(|e| matches!(e, egui::Event::Key { key, pressed: true, modifiers, .. } if one_letter(key.name()).is_some() && ours(modifiers)))?;
    let egui::Event::Key { key, repeat, .. } = input.events.remove(at) else { return None };
    if matches!(input.events.get(at), Some(egui::Event::Text(_))) {
        input.events.remove(at);
    }
    one_letter(key.name()).filter(|_| !repeat)
}

/// The keys of `keys` written for Windows and Linux, such as "Ctrl+Shift+G" or "Ctrl+Alt+←", in the
/// catalogue's words; an arrow key is drawn as its arrow.
pub(crate) fn chord(keys: &Keys) -> String {
    let mut parts = vec![tr("Ctrl")];
    if keys.modifiers.alt {
        parts.push(tr_ctx("key", "Alt"));
    }
    if keys.modifiers.shift {
        parts.push(tr("Shift"));
    }
    let key = match keys.key {
        egui::Key::ArrowLeft => "←",
        egui::Key::ArrowRight => "→",
        key => key.symbol_or_name(),
    };
    parts.push(key.to_string());
    parts.join("+")
}

/// Draws the bar in `strip` from `start` and the open group's menu; a press elsewhere closes it and
/// reaches nothing else, and a click on the open menu's own title closes it too, giving the keyboard back.
/// The titles are the items of a menu bar to a screen reader.
pub(crate) fn show(shell: &mut Shell, ui: &egui::Ui, p: &Palette, strip: Rect, start: f32) {
    if !shell.in_window_menu {
        return;
    }
    let paint = ui.painter().clone();
    let role = egui::UiBuilder::new().accessibility_role(egui::accesskit::Role::MenuBar).accessibility_parent(ui.unique_id());
    let titles = egui::Ui::new(ui.ctx().clone(), widgets::egui_id("strip.menu.bar"), role.layer_id(ui.layer_id()).max_rect(strip));
    let (mid, mut x, mut bar, mut shown) = (strip.center().y, start, Rect::NOTHING, None);
    for group in GROUPS {
        let was_open = shell.menu_bar.open == Some(group.id);
        let (title, key) = ((group.title)(), format!("menu.{}", group.id));
        let text = lettered(&paint, &title, &key, p.text, None);
        let size = vec2(text.0.size().x + 2.0 * GROUP_PAD, GROUP_HEIGHT);
        let button = Rect::from_center_size(pos2(x + size.x / 2.0, mid), size);
        let resp = control(shell, &titles, &format!("strip.menu.{}", group.id), "menu", button, true).accessible_name(title.as_str());
        ui.ctx().accesskit_node_builder(resp.id, |node| {
            node.set_role(egui::accesskit::Role::MenuItem);
            node.set_expanded(was_open);
            letter(&key).inspect(|c| node.set_access_key(c.to_string()));
        });
        if was_open {
            paint.rect_filled(button, 5.0, p.surface);
        } else if resp.hovered() {
            paint.rect_filled(button, 5.0, p.border);
        }
        paint_lettered(&paint, pos2(button.left() + GROUP_PAD, mid - text.0.size().y / 2.0), text, p.text, cued(shell, ui.ctx()));
        if resp.clicked() {
            shell.menu_bar.open = if was_open { None } else { Some(group.id) };
            shell.menu_bar.row = None;
            shell.menu_bar.closed_by_title = was_open;
        }
        if shell.menu_bar.open == Some(group.id) {
            shown = Some((group, button, resp.id));
        }
        bar = bar.union(button);
        x = button.right() + GROUP_GAP;
    }
    shell.menu_bar.rect = Some(bar);
    let press = ui.input(|i| i.pointer.any_pressed().then(|| i.pointer.interact_pos()));
    shell.menu_bar.pressed = press.flatten().is_some_and(|at| bar.contains(at));
    let Some((group, button, title)) = shown else { return };
    let (menu, picked) = popup(shell, ui, p, group, title, pos2(button.left(), strip.bottom()));
    shell.menu_bar.pressed |= press.flatten().is_some_and(|at| menu.contains(at));
    let outside = press.is_some() && !shell.menu_bar.pressed;
    if let Some(id) = picked {
        shell.menu_bar.chosen = Some(id);
        shell.menu_bar.open = None;
        ui.ctx().request_repaint();
    } else if outside {
        shell.menu_bar.open = None;
    }
}

/// The four parts of `window` around `bar`, which together leave out the bar alone.
fn around(window: Rect, bar: Rect) -> [Rect; 4] {
    let (above, below) =
        (Rect::from_min_max(window.min, pos2(window.right(), bar.top())), Rect::from_min_max(pos2(window.left(), bar.bottom()), window.max));
    let (left, right) = (
        Rect::from_min_max(pos2(window.left(), bar.top()), bar.left_bottom()),
        Rect::from_min_max(bar.right_top(), pos2(window.right(), bar.bottom())),
    );
    [above, below, left, right]
}

/// The menu of `group` at `at`, kept inside the window: its rectangle and the row picked this frame.
/// To a screen reader its rows are a menu under `title`, the group's item in the bar.
/// Its layer covers the window around the bar, under the rows, so a press away from both lands on
/// the layer: nothing behind the menu is pressed, dragged or pointed at, and the release that
/// follows finds the layer gone and reaches nothing either.
fn popup(shell: &mut Shell, ui: &egui::Ui, p: &Palette, group: &Group, title: egui::Id, at: egui::Pos2) -> (Rect, Option<&'static str>) {
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
        // Sensing both, so it wins over what lies behind at its parts' edges too; never the keyboard,
        // which also keeps the parts from a screen reader.
        for (n, part) in around(window, shell.menu_bar.rect.unwrap_or(Rect::NOTHING)).into_iter().enumerate() {
            ui.interact(part, widgets::egui_id(&format!("strip.menu.away.{n}")), Sense::CLICK | Sense::DRAG);
        }
        let ui = &ui.new_child(egui::UiBuilder::new().accessibility_role(egui::accesskit::Role::Menu).accessibility_parent(title));
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

/// One row, a menu item with its name and keys, highlighted under the keys or else the pointer; off,
/// it says why on hover and under the keys, and to a screen reader always, and takes no click. True
/// when clicked.
fn draw_row(shell: &mut Shell, ui: &egui::Ui, p: &Palette, id: &str, row: Rect) -> bool {
    let paint = ui.painter();
    let label = actions::label(shell, id);
    let auto = format!("strip.menu.row.{id}");
    let live = actions::available(shell, id);
    let keyed = shell.menu_bar.row == Some(id);
    let resp = if live {
        control(shell, ui, &auto, "menu-item", row, true).accessible_name(label.as_str())
    } else {
        widgets::record_visible(ui, &mut shell.registry, &auto, "menu-item", row);
        let why = why_off(shell, id);
        let resp = ui.interact(row, widgets::egui_id(&auto), Sense::hover());
        ui.ctx().accesskit_node_builder(widgets::egui_id(&auto), |node| {
            node.set_disabled();
            node.set_description(why.as_str());
        });
        // The keys bring no pointer, so under them the reason shows by itself, beside the menu:
        // the rows the arrows go to next stay in sight.
        if keyed {
            let beside = egui::Popup::from_response(&resp).kind(egui::PopupKind::Tooltip).align(egui::RectAlign::RIGHT_START);
            beside.show(|ui| ui.label(why.as_str()));
        }
        let resp = if keyed { resp } else { resp.on_hover_text(why) };
        resp.accessible_name(label.as_str())
    };
    // A menu item to a screen reader whether it can act or not, as the row menus' rows are.
    ui.ctx().accesskit_node_builder(widgets::egui_id(&auto), |node| {
        node.set_role(egui::accesskit::Role::MenuItem);
        if keyed {
            node.set_selected(true);
        }
        letter(id).inspect(|c| node.set_access_key(c.to_string()));
    });
    if keyed || (live && shell.menu_bar.row.is_none() && resp.hovered()) {
        paint.rect_filled(row, 5.0, p.accent_tint);
    }
    let keys = actions::keys(id).map(chord).unwrap_or_default();
    let detail = ux::galley(paint, &keys, types::CAPTION, p.text_secondary, None);
    let width = detail.size().x;
    paint.galley(pos2(row.right() - 10.0 - width, row.center().y - detail.size().y / 2.0), detail, p.text_secondary.into());
    let ink = if live { p.text } else { p.text_muted };
    let text = lettered(paint, &label, id, ink, Some((row.width() - 28.0 - width).max(0.0)));
    paint_lettered(paint, pos2(row.left() + 10.0, row.center().y - text.0.size().y / 2.0), text, ink, cued(shell, ui.ctx()));
    tick(shell, ui, p, id, row);
    live && resp.clicked()
}

/// The access letter of `key`, a title's `menu.<group>` or a row's action id, in the language shown:
/// its catalogue row, one letter A to Z, as only those have a key of their own. None for anything
/// else, such as a missing row, which reads as its key.
fn letter(key: &str) -> Option<char> {
    one_letter(&tr_ctx(ACCESS, key))
}

/// The letter `text` is when it is one letter A to Z and nothing else.
fn one_letter(text: &str) -> Option<char> {
    let mut chars = text.chars();
    chars.next().filter(|c| c.is_ascii_uppercase() && chars.next().is_none())
}

/// What a title or row shows for `label`, and where in it the letter of `key` is: the first character
/// that is the letter in either case, or else the letter in brackets after the label, in the
/// catalogue's words, as a label with no such letter, Japanese or Chinese say, shows it.
fn marked(label: &str, key: &str) -> (String, Option<usize>) {
    let Some(letter) = letter(key) else { return (label.to_string(), None) };
    match label.chars().position(|c| c.to_ascii_uppercase() == letter) {
        Some(at) => (label.to_string(), Some(at)),
        None => {
            let text = fmt(&tr("{label}({letter})"), &[("label", label), ("letter", &letter.to_string())]);
            let at = text.chars().position(|c| c == letter);
            (text, at)
        }
    }
}

/// `label` laid out in the bar's text, as `marked` gives it, with where its letter is.
fn lettered(paint: &egui::Painter, label: &str, key: &str, ink: Colour, width: Option<f32>) -> (Arc<egui::Galley>, Option<usize>) {
    let (text, at) = marked(label, key);
    (ux::galley(paint, &text, types::SMALL, ink, width), at)
}

/// Draws a text of `lettered` from `top_left`, its letter underlined while `cued`.
fn paint_lettered(paint: &egui::Painter, top_left: egui::Pos2, (text, at): (Arc<egui::Galley>, Option<usize>), ink: Colour, cued: bool) {
    if let Some(at) = at.filter(|_| cued) {
        ux::underline_char(paint, top_left, &text, at, types::SMALL, ink);
    }
    paint.galley(top_left, text, ink.into());
}

/// Whether the letters show: while Alt alone is held, or the keys walk an open menu, as on Windows.
/// Every way a menu closes leaves `row` as it was, so the menu being open is asked too.
fn cued(shell: &Shell, ctx: &egui::Context) -> bool {
    (shell.menu_bar.open.is_some() && shell.menu_bar.row.is_some()) || ctx.input(|i| i.modifiers.matches_exact(egui::Modifiers::ALT))
}

/// Every set of rows a menu of the bar can show at once, by its group's id: the Model menu once per
/// notation, File with Settings… and Exit, as in the window.
#[cfg(test)]
pub(crate) fn every_menu() -> Vec<(&'static str, Vec<&'static str>)> {
    let fixed = [("file", actions::FILE_MENU.concat()), ("edit", EDIT_MENU.concat()), ("view", crate::view_menu::VIEW_MENU.concat())];
    let rest = [("versions", actions::VERSIONS_MENU.concat()), ("help", vec![ABOUT])];
    fixed.into_iter().chain(crate::model_menu::every_menu().into_iter().map(|rows| ("model", rows))).chain(rest).collect()
}

/// The keys of every access letter: each title's and each row's, once.
#[cfg(test)]
pub(crate) fn access_keys() -> Vec<String> {
    let rows = every_menu().into_iter().flat_map(|(_, rows)| rows).map(String::from);
    let mut keys: Vec<String> = GROUPS.iter().map(|g| format!("menu.{}", g.id)).chain(rows).collect();
    keys.sort();
    keys.dedup();
    keys
}

/// A row that shows and hides something, as View's panel rows do: a check box to a screen reader,
/// checked while its thing shows, and then with a tick at its right. Any other row stays as it is.
fn tick(shell: &Shell, ui: &egui::Ui, p: &Palette, id: &str, row: Rect) {
    let Some(on) = actions::ticked(shell, id) else { return };
    ui.ctx().accesskit_node_builder(widgets::egui_id(&format!("strip.menu.row.{id}")), |node| {
        node.set_role(egui::accesskit::Role::MenuItemCheckBox);
        node.set_toggled(if on { egui::accesskit::Toggled::True } else { egui::accesskit::Toggled::False });
    });
    if on {
        ux::icon_mid(ui.painter(), pos2(row.right() - 16.0, row.center().y), &ux::icons::TICK, p.accent);
    }
}

/// Why the row of the action `id` is off, in words: the first thing that keeps it off.
pub(crate) fn why_off(shell: &Shell, id: &str) -> String {
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
        // Off only for want of a field or of selected text, held or viewing a version as well.
        CUT | COPY | PASTE => tr("Nothing to act on here"),
        _ if id.starts_with("view.") => crate::view_menu::why_off(shell, id),
        _ if id.starts_with("edit.") && shell.is_held() => tr("The project is busy"),
        _ if id.starts_with("edit.") && (id == SELECT_ALL || !shell.looking_only()) => tr("Nothing to act on here"),
        _ => off_words(shell),
    }
}

/// Why the Versions menu's row `id` is off: no project, then the row's own reason, then a hold.
/// Compare… follows what `version_details::can_compare` decides: with no host it is not available,
/// with its page open it is already showing, after a failed read it says so, and it says No versions
/// yet only of a list read empty and known to be current; otherwise Loading.
fn versions_off(shell: &Shell, id: &str) -> String {
    if shell.session.is_none() || shell.project.is_none() || shell.active.is_none() {
        return tr("Open or go back to a project first");
    }
    if id == SAVE_VERSION
        && let Some(why) = crate::save_version::off_because(shell)
    {
        return why;
    }
    // An empty list says there are no versions only while it is known to be current, as
    // `version_details::to_compare` trusts a list only then.
    let none_yet = shell.versions.page.as_ref().is_some_and(|p| p.versions.is_empty()) && crate::versions::last_known(&shell.versions);
    match id {
        BACK if shell.version_in_view().is_none() => tr("Current work is showing"),
        _ if shell.is_held() => tr("The project is busy"),
        NEW_BRANCH if shell.versions.making.is_some() => tr("Wait for the change under way to finish."),
        VERSIONS if shell.version_in_view().is_some() => tr("Back to current work first"),
        COMPARE if shell.services.projects.is_none() => off_words(shell),
        COMPARE if crate::version_details::compare_open(shell) => tr("Already showing"),
        COMPARE if shell.versions.failed.is_some() => tr("The versions couldn't be read."),
        COMPARE if none_yet => tr("No versions yet."),
        COMPARE => tr("Loading"),
        NEWER if shell.stepping_from().is_none() => tr("No newer version"),
        BRANCHES => tr("This project has one branch"),
        _ => off_words(shell),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actions::SETTINGS;
    use crate::actions::tests::{shell, tab};
    use std::sync::mpsc::channel;

    /// The keys are written as the design writes them.
    #[test]
    fn keys_are_written_as_the_design_has_them() {
        crate::i18n::set_language("en");
        let text = |id: &str| actions::keys(id).map(chord);
        assert_eq!(text(GENERATE).as_deref(), Some("Ctrl+Shift+G"));
        assert_eq!(text(SETTINGS).as_deref(), Some("Ctrl+,"));
        assert_eq!(text(CLOSE).as_deref(), Some("Ctrl+W"));
        assert_eq!(text(NEW), None);
    }

    /// In every language each title and row has a letter A to Z, the titles each their own across the
    /// bar and the rows each their own within every set a menu can show; a label with letters of its
    /// own holds it, so only Japanese and Chinese show it in brackets.
    #[test]
    fn every_title_and_row_has_its_own_letter_in_every_language() {
        let (mut s, ..) = shell();
        s.in_window_menu = true;
        let menus = every_menu();
        assert!(GROUPS.iter().all(|g| menus.iter().any(|(id, _)| *id == g.id)), "every menu is checked");
        for lang in crate::i18n::LANGUAGES {
            crate::i18n::set_language(lang.code);
            let titles = GROUPS.iter().map(|g| ((g.title)(), format!("menu.{}", g.id)));
            let rows = menus.iter().map(|(_, rows)| rows.iter().map(|id| (actions::label(&s, id), id.to_string())).collect::<Vec<_>>());
            for set in std::iter::once(titles.collect::<Vec<_>>()).chain(rows) {
                let mut seen = Vec::new();
                for (label, key) in set {
                    let letter = letter(&key).unwrap_or_else(|| panic!("{}: {key} has no letter", lang.code));
                    assert!(!seen.contains(&letter), "{}: {letter} of {key} twice among {seen:?}", lang.code);
                    seen.push(letter);
                    let own = label.chars().any(|c| c.is_ascii_alphabetic());
                    assert_eq!(marked(&label, &key).0 == label, own, "{}: {key}'s {letter} in {label:?}", lang.code);
                }
            }
        }
    }

    /// The letter is the first character that is it in either case, else it shows in brackets after
    /// the label; a key with no letter marks nothing.
    #[test]
    fn the_letter_is_marked_in_the_label_or_after_it() {
        crate::i18n::set_language("en");
        assert_eq!(marked("Exit", "app.quit"), ("Exit".to_string(), Some(1)), "Exit's X");
        assert_eq!(marked("Undo", "edit.undo"), ("Undo".to_string(), Some(0)));
        assert_eq!(marked("Datei", "menu.file"), ("Datei(F)".to_string(), Some(6)), "a label without F");
        assert_eq!(marked("File", "no.such"), ("File".to_string(), None));
        assert_eq!(["F", "FI", "f", "É", "", "file.save"].map(one_letter), [Some('F'), None, None, None, None, None], "one letter A to Z");
        crate::i18n::set_language("ja");
        assert_eq!(marked(&tr("File"), "menu.file"), ("ファイル(F)".to_string(), Some(5)));
        crate::i18n::set_language("de");
        assert_eq!(marked(&tr("Exit"), "app.quit"), ("Beenden".to_string(), Some(0)), "German's own letter");
    }

    /// A letter's press with Alt alone, or with nothing held when bare, is taken with the text that
    /// follows it, and a repeat is taken too but counts for nothing (AR-538). With Shift, AltGr
    /// (Ctrl+Alt), the command key of either shape, or nothing when not bare, it stays with its text,
    /// and so do a digit, a release and a text that does not follow it.
    #[test]
    fn a_letters_press_is_taken_with_its_text_only_with_alt_or_bare() {
        use egui::{Event, Key, Modifiers as M};
        let down = |key, modifiers, repeat| Event::Key { key, physical_key: None, pressed: true, repeat, modifiers };
        let text = |t: &str| Event::Text(t.to_string());
        let taken = |events: &[Event], bare| {
            let mut input = egui::InputState::default();
            input.events = events.to_vec();
            (take_letter(&mut input, bare), input.events)
        };
        let enter = down(Key::Enter, M::NONE, false);
        assert_eq!(taken(&[down(Key::F, M::ALT, false), text("f"), enter.clone()], false), (Some('F'), vec![enter.clone()]));
        assert_eq!(taken(&[down(Key::E, M::NONE, false), text("e")], true), (Some('E'), vec![]), "bare");
        assert_eq!(taken(&[down(Key::E, M::ALT, true), text("e")], true), (None, vec![]), "a repeat");
        let apart = [down(Key::F, M::ALT, false), enter.clone(), text("f")];
        assert_eq!(taken(&apart, false), (Some('F'), apart[1..].to_vec()), "only the text right after it");
        for held in [M::SHIFT, M::SHIFT | M::ALT, M::CTRL | M::COMMAND | M::ALT, M::CTRL | M::COMMAND, M::MAC_CMD | M::COMMAND | M::ALT] {
            let stays = [down(Key::E, held, false), text("€")];
            assert_eq!(taken(&stays, true), (None, stays.to_vec()), "{held:?}");
        }
        let release = Event::Key { key: Key::F, physical_key: None, pressed: false, repeat: false, modifiers: M::ALT };
        for stays in [vec![down(Key::F, M::NONE, false), text("f")], vec![down(Key::Num1, M::ALT, false), text("1")], vec![release]] {
            assert_eq!(taken(&stays, false), (None, stays.clone()));
        }
    }

    /// The parts around the bar cover every point of the window but the bar's own.
    #[test]
    fn the_parts_around_the_bar_leave_out_the_bar_alone() {
        let (window, bar) = (Rect::from_min_max(pos2(0.0, 0.0), pos2(800.0, 600.0)), Rect::from_min_max(pos2(200.0, 4.0), pos2(500.0, 28.0)));
        let covered = |x: f32, y: f32| around(window, bar).iter().filter(|part| part.contains(pos2(x, y))).count();
        let outside = [(350.0, 2.0), (350.0, 300.0), (100.0, 16.0), (650.0, 16.0), (1.0, 1.0), (799.0, 599.0), (100.0, 2.0), (650.0, 30.0)];
        assert_eq!(outside.map(|(x, y)| covered(x, y)), [1; 8], "each point beside the bar lies on one part");
        assert_eq!([(201.0, 5.0), (350.0, 16.0), (499.0, 27.0)].map(|(x, y)| covered(x, y)), [0; 3], "the bar's own points on none");
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
        let no = "Nothing to act on here".to_string();
        assert_eq!(why_off(&s, UNDO), no);
        assert_eq!(why_off(&s, "model.note"), "Show a diagram first");
        let (mut viewing, ..) = shell();
        viewing.show_version(crate::actions::tests::session(), "v".into());
        assert_eq!((why_off(&viewing, SELECT_ALL), why_off(&viewing, CUT)), (no.clone(), no.clone()), "viewing a version keeps neither off");
        assert_eq!(why_off(&viewing, UNDO), "Read only while viewing a version");
        s.begin_hold().unwrap();
        assert_eq!(why_off(&s, UNDO), "The project is busy");
        assert_eq!(
            (why_off(&s, SELECT_ALL), why_off(&s, PASTE)),
            ("The project is busy".to_string(), no),
            "Paste is off for want of a field, not for the hold"
        );
    }

    /// A Versions row off says first that there is no project, then its own reason, then the hold.
    #[test]
    fn a_versions_row_off_says_why() {
        crate::i18n::set_language("en");
        let (mut s, ..) = shell();
        let words = |s: &Shell| [SAVE_VERSION, COMPARE, BRANCHES, BACK, NEWER].map(|id| why_off(s, id));
        assert_eq!(words(&s), ["Loading", "Loading", "This project has one branch", "Current work is showing", "No newer version"]);
        s.versions.page = Some(crate::VersionsPage { versions: Vec::new(), more: false });
        assert_eq!(why_off(&s, COMPARE), "No versions yet.", "only of a list read and found empty");
        s.versions.stale = true;
        assert_eq!(why_off(&s, COMPARE), "Loading", "a save may have made the first version since");
        s.versions.stale = false;
        let host = s.services.projects.take();
        assert_eq!(why_off(&s, COMPARE), "Not available yet", "nothing can read a version");
        s.services.projects = host;
        s.versions.failed = Some("no server".into());
        assert_eq!(why_off(&s, COMPARE), "The versions couldn't be read.");
        s.versions.failed = None;
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
