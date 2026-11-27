//! The key presses the shell's actions and the shared controls take: a shortcut (`Keys`), taken
//! from the frame's input matched exactly (`take`, `take_alone`), and whether exactly the wanted
//! modifiers are held (`held_exactly`); a key pressed plainly, with no command key, Control or Alt
//! (`plain_press`, `take_plain`), and a chord read without taking it (`chord_press`). The actions
//! themselves are the shell's.

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

/// Whether `key` was pressed in `input` with no command key, Control or Alt held, with Shift or
/// without: the press an arrow's own reader acts on. With one of them held it is a chord's, as
/// ⌥⌘← is Older version's, and a chord whose action is off stays in the input: no reader behind it
/// takes it for a plain arrow.
pub fn plain_press(input: &egui::InputState, key: Key) -> bool {
    input.events.iter().any(|e| matches!(e, egui::Event::Key { key: k, pressed: true, modifiers, .. } if *k == key && plain(modifiers)))
}

/// Whether `held` has no command key, Control or Alt in it: Shift alone, or nothing.
fn plain(held: &Modifiers) -> bool {
    !(held.command || held.ctrl || held.mac_cmd || held.alt)
}

/// Whether `key` was pressed in `input` plainly, as `plain_press` reads it, and takes those
/// presses. A press with a command key, Control or Alt stays in the input for its own chord.
pub fn take_plain(input: &mut egui::InputState, key: Key) -> bool {
    let before = input.events.len();
    input.events.retain(|e| !matches!(e, egui::Event::Key { key: k, pressed: true, modifiers, .. } if *k == key && plain(modifiers)));
    input.events.len() != before
}

/// Whether `key` was pressed in `input` with exactly the modifiers `wanted` held (`held_exactly`):
/// not with Shift or Alt more or less, and not with a Mac's Control. Takes nothing.
pub fn chord_press(input: &egui::InputState, key: Key, wanted: Modifiers) -> bool {
    input
        .events
        .iter()
        .any(|e| matches!(e, egui::Event::Key { key: k, pressed: true, modifiers, .. } if *k == key && held_exactly(*modifiers, wanted)))
}
