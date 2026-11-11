//! Where the Edit menu's actions go. A text field with the keyboard keeps its own history,
//! clipboard and selection, so the action reaches it as its keys would; with no field Undo and Redo
//! step the model's history, Copy takes text selected on a label and Select all takes the canvas.
//! Cut and Paste act on fields alone, as the model has no clipboard. With nothing to act on an
//! action is off. On macOS the native menu takes the keys before the window sees them, so this is a
//! field's only route there.

use archforge_notations::Session;
use egui::text_edit::TextEditState;
use egui::text_selection::LabelSelectionState;
use egui::{Event, Key, Modifiers};

use crate::Shell;
use crate::actions::free;

/// Who has the keyboard.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Keyboard {
    #[default]
    Nobody,
    /// A text field, which reads its own keys.
    Field,
    /// A control that is no field: neither a field's actions nor the model's are for it.
    Other,
}

/// The state of the routing between frames.
#[derive(Default)]
pub(crate) struct Edit {
    pub(crate) keyboard: Keyboard,
    /// Whether text on a label, such as a skill's preview, is selected: only Copy can take it.
    pub(crate) text_selected: bool,
    /// What was asked of the field, waiting for the frame that can reach it.
    pub(crate) steps: Vec<Event>,
    /// Whether a paste was asked of the field, waiting for the window to read the system's clipboard.
    pub(crate) paste: bool,
}

/// Records who has the keyboard, the field a menu was opened on counting while it is open. Called
/// as a frame's drawing ends, so the next frame's menus and keys ask about the focus as it stands.
pub(crate) fn note(shell: &mut Shell, ctx: &egui::Context) {
    let focused = if shell.menu_bar.open.is_some() { shell.menu_bar.back } else { ctx.memory(egui::Memory::focused) };
    shell.edit.keyboard = match focused {
        Some(id) if TextEditState::load(ctx, id).is_some() => Keyboard::Field,
        Some(_) => Keyboard::Other,
        None => Keyboard::Nobody,
    };
    shell.edit.text_selected = ctx.plugin_opt::<LabelSelectionState>().is_some_and(|p| p.lock().has_selection());
}

/// Hands the waiting steps to the field, as the input of this frame. A paste is asked of the window
/// instead, the one part of the program that reads the system's clipboard: its text arrives as the
/// next frame's input, which is asked for now.
pub(crate) fn deliver(shell: &mut Shell, ctx: &egui::Context) {
    ctx.input_mut(|i| i.events.append(&mut shell.edit.steps));
    if std::mem::take(&mut shell.edit.paste) {
        ctx.send_viewport_cmd(egui::ViewportCommand::RequestPaste);
        ctx.request_repaint();
    }
}

/// Whether a text field has the keyboard.
pub(crate) fn on_field(shell: &Shell) -> bool {
    shell.edit.keyboard == Keyboard::Field
}

/// Whether the shown project is free for the menu to reach: no control has the keyboard and nothing
/// is up over the window, a history row's menu included.
fn window_free(shell: &Shell) -> bool {
    shell.edit.keyboard == Keyboard::Nobody && free(shell) && shell.chat_panel.row_menu.is_none() && shell.active.is_some()
}

/// Whether Undo or Redo can act: on a field, or on a shown model that is not read-only and has the
/// step `has`, while the window is free. A hold makes the model read-only, so it needs no rule of
/// its own.
fn can_step(shell: &Shell, has: fn(&Session) -> bool) -> bool {
    on_field(shell) || (window_free(shell) && shell.session.as_ref().is_some_and(|s| !s.read_only() && has(s)))
}

/// Takes the step on the field, as `modifiers` with Z would, or else on the model through the engine.
fn step(shell: &mut Shell, modifiers: Modifiers, command: &str, has: fn(&Session) -> bool) {
    if on_field(shell) {
        shell.edit.steps.push(Event::Key { key: Key::Z, physical_key: None, pressed: true, repeat: false, modifiers });
    } else if can_step(shell, has)
        && let Some(session) = shell.session.as_mut()
        && let Err(e) = session.execute(command, &serde_json::Value::Null)
    {
        shell.canvas.notice = Some(e.to_string());
    }
}

/// Whether Undo can act now.
pub(crate) fn can_undo(shell: &Shell) -> bool {
    can_step(shell, Session::can_undo)
}

/// Undoes in the field or the model.
pub(crate) fn undo(shell: &mut Shell) {
    step(shell, Modifiers::COMMAND, "edit.undo", Session::can_undo);
}

/// Whether Redo can act now.
pub(crate) fn can_redo(shell: &Shell) -> bool {
    can_step(shell, Session::can_redo)
}

/// Redoes in the field or the model.
pub(crate) fn redo(shell: &mut Shell) {
    step(shell, Modifiers::COMMAND | Modifiers::SHIFT, "edit.redo", Session::can_redo);
}

/// Cuts the field's selection, as its keys would; there is no clipboard of model elements.
pub(crate) fn cut(shell: &mut Shell) {
    if on_field(shell) {
        shell.edit.steps.push(Event::Cut);
    }
}

/// Whether Copy can act now: on a field, or on text selected on a label.
pub(crate) fn can_copy(shell: &Shell) -> bool {
    on_field(shell) || shell.edit.text_selected
}

/// Copies from the field or the selected label, which both read the same event.
pub(crate) fn copy(shell: &mut Shell) {
    if can_copy(shell) {
        shell.edit.steps.push(Event::Copy);
    }
}

/// Pastes the system's clipboard into the field; there is no clipboard of model elements.
pub(crate) fn paste(shell: &mut Shell) {
    shell.edit.paste |= on_field(shell);
}

/// The diagram Select all takes: the open one, while the canvas has the keyboard, the window is
/// free, no gesture runs and the project is not held. Viewing a version may select, as a click there
/// does.
fn canvas_diagram(shell: &Shell) -> Option<archforge_mof::Obj> {
    let ready = window_free(shell) && shell.canvas.focused && !shell.explorer.focused && shell.canvas.gesture.is_none() && !shell.is_held();
    shell.session.as_ref().filter(|_| ready).and_then(|s| s.view().open_diagram)
}

/// Whether Select all can act now.
pub(crate) fn can_select_all(shell: &Shell) -> bool {
    on_field(shell) || canvas_diagram(shell).is_some()
}

/// Selects all of the field's text, as its keys would, or everything on the open diagram.
pub(crate) fn select_all(shell: &mut Shell) {
    if on_field(shell) {
        shell.edit.steps.push(Event::Key { key: Key::A, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::COMMAND });
    } else if let Some(diagram) = canvas_diagram(shell) {
        crate::editing::select_all(shell, diagram);
    }
}
