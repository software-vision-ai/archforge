//! The key presses the shell's actions and the shared controls take: a shortcut (`Keys`), taken
//! from the frame's input matched exactly (`take`, `take_alone`), and whether exactly the wanted
//! modifiers are held (`held_exactly`). The actions themselves are the shell's.

use egui::{Key, Modifiers};

/// An action's keyboard shortcut, taken while the action is available.
pub struct Keys {
    pub modifiers: Modifiers,
    pub key: Key,
}

/// Whether exactly `wanted` is `held`, for a chord of the command key (⌘, Ctrl on other systems).
/// `matches_exact` is exact about Shift and Alt only: on macOS it lets Control with ⌘ through,
/// since it checks that `command` is set and never that `ctrl` is not. Windows and Linux set
/// `ctrl` and `command` together for their one key, and never `mac_cmd`.
pub fn held_exactly(held: Modifiers, wanted: Modifiers) -> bool {
    held.matches_exact(wanted) && !(held.ctrl && held.mac_cmd)
}

/// Whether `keys` were pressed in `input`, and takes that press. Matched exactly: an extra Shift
/// or Alt is another chord's press and stays, so ⇧⌘S never saves with Save version… off. macOS's
/// Control with the chord is nobody's: that press is taken too and counts for nothing, so a field
/// drawn later never reads it as one of Control's text keys. A chord without the command key, such
/// as ⇧F10, takes nothing with any modifier more or less, a Mac's Control included.
pub fn take(input: &mut egui::InputState, keys: &Keys) -> bool {
    let mut hit = false;
    input.events.retain(|e| match e {
        egui::Event::Key { key, pressed: true, modifiers, .. } if *key == keys.key && modifiers.matches_exact(keys.modifiers) => {
            hit |= held_exactly(*modifiers, keys.modifiers);
            false
        }
        _ => true,
    });
    hit
}

/// Whether `key` was pressed alone in `input`, with no modifier held, and takes those presses:
/// `take` with no modifiers. A press with Shift, Alt, Control or the command key stays in the input
/// for its own reader, a field's selection keys among them, as egui's own focus keys act only on a
/// press alone. `consume_key(Modifiers::NONE, …)` lets Shift and Alt through.
pub fn take_alone(input: &mut egui::InputState, key: Key) -> bool {
    take(input, &Keys { modifiers: Modifiers::NONE, key })
}
