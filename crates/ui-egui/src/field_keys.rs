//! Text fields on Windows and Linux never delete on Ctrl+H, Ctrl+K, Ctrl+U or Ctrl+W (AR-451).
//! The toolkit's text field reads the control key with one of those letters as a deletion on
//! every system: a character back, to the line's end, to its start, a word back. On macOS those
//! are the system's own text keys and stay. Elsewhere the control key is the command key, so the
//! same chord is an action's (Find…, History) or nobody's, and a field that deleted on it would
//! lose text by surprise. One hook on the frame's input covers every field, since each of them
//! reads its keys from that input.

use egui::{Event, Key, RawInput};

/// Takes the control key off the four letters' key events as a frame's input arrives.
struct NoLineChords;

impl egui::Plugin for NoLineChords {
    /// The hook's name, for the toolkit's profiling.
    fn debug_name(&self) -> &'static str {
        "archforge.field-keys"
    }

    /// Runs before anything reads the frame's keys. Only the Windows and Linux shape of the chord
    /// is changed: there the control key sets `ctrl` and `command` together and never `mac_cmd`.
    /// The event keeps `command`, which is all the actions' keys and a skill's link key match on,
    /// and loses `ctrl`, which is all the field's deletions match on. A Mac's control key sets
    /// `ctrl` alone, and with its command key sets `mac_cmd`, so neither is touched.
    fn input_hook(&mut self, _ctx: &egui::Context, input: &mut RawInput) {
        for event in &mut input.events {
            if let Event::Key { key: Key::H | Key::K | Key::U | Key::W, modifiers, .. } = event
                && modifiers.command
                && !modifiers.mac_cmd
            {
                modifiers.ctrl = false;
            }
        }
    }
}

/// Registers the hook; later calls are ignored. It acts from the next frame on, and a field takes
/// the keyboard a frame before it can read a key.
pub(crate) fn install(ctx: &egui::Context) {
    ctx.add_plugin(NoLineChords);
}
