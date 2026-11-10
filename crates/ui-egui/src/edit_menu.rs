//! Where Undo and Redo go. A text field with the keyboard keeps its own history, so the action
//! reaches it as its keys would; with no field they step the model's history, from the canvas and
//! from anywhere else. With nothing to act on they are off. On macOS the native menu takes the
//! keys before the window sees them, so this is a field's only route there.

use archforge_notations::Session;
use egui::text_edit::TextEditState;
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
    /// What was asked of the field, waiting for the frame that can reach it.
    pub(crate) steps: Vec<Event>,
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
}

/// Hands the waiting steps to the field, as the input of this frame.
pub(crate) fn deliver(shell: &mut Shell, ctx: &egui::Context) {
    ctx.input_mut(|i| i.events.append(&mut shell.edit.steps));
}

/// Whether a text field has the keyboard.
pub(crate) fn on_field(shell: &Shell) -> bool {
    shell.edit.keyboard == Keyboard::Field
}

/// Whether Undo or Redo can act: on a field, or on a shown model that is not read-only and has the
/// step `has`, while no control has the keyboard and nothing is up over the window, a history row's
/// menu included. A hold makes the model read-only, so it needs no rule of its own.
fn can_step(shell: &Shell, has: fn(&Session) -> bool) -> bool {
    let window = shell.edit.keyboard == Keyboard::Nobody && free(shell) && shell.chat_panel.row_menu.is_none() && shell.active.is_some();
    on_field(shell) || (window && shell.session.as_ref().is_some_and(|s| !s.read_only() && has(s)))
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
