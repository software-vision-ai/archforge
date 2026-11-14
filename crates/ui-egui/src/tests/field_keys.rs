//! A text field's keys on Windows and Linux (AR-451): the control key with H, K, U or W deletes
//! nothing there, a Mac's control key keeps its text keys, the plain deletions stay, and the
//! chords still run their actions. Windows and Linux are tested by the shape of their key events
//! only: the control key sets `ctrl` and `command` together.

use egui::{Key, Modifiers};

use super::*;

/// The control key as Windows and Linux report it.
const CONTROL: Modifiers = Modifiers::CTRL.plus(Modifiers::COMMAND);

/// A new column's editor in `theme` with the keyboard in its name field, which holds `column_1`.
fn name_field(theme: ThemeKind) -> Harness<'static, Shell> {
    let (mut h, _) = table_harness();
    h.state_mut().state.theme = theme;
    click_id(&mut h, "workspace.properties.columns.add");
    click_id(&mut h, "workspace.properties.column.name");
    h
}

/// The name in the column's editor.
fn name(h: &Harness<'static, Shell>) -> String {
    h.state().panels.column.as_ref().map(|c| c.name.clone()).unwrap_or_default()
}

/// The name after `chord` is pressed with the cursor at the text's start or end.
fn after(theme: ThemeKind, at: Key, chord: Key, modifiers: Modifiers) -> String {
    let mut h = name_field(theme);
    key(&mut h, at, Modifiers::NONE);
    assert_eq!(name(&h), "column_1", "{theme:?}: the field holds the name before {chord:?}");
    key(&mut h, chord, modifiers);
    name(&h)
}

/// Each chord with the cursor where its deletion would show, and what a Mac's control key leaves.
const CHORDS: [(Key, Key, &str); 4] = [(Key::Home, Key::K, ""), (Key::End, Key::H, "column_"), (Key::End, Key::U, ""), (Key::End, Key::W, "")];

/// On Windows and Linux no chord of the control key and H, K, U or W changes a field's text, with
/// Shift or without; a Mac's control key deletes as its system's text keys do, with its command
/// key or without.
#[test]
fn the_control_chords_delete_nothing_in_a_field_on_windows_and_linux() {
    in_both_appearances(|theme| {
        for (at, chord, mac) in CHORDS {
            for modifiers in [CONTROL, CONTROL.plus(Modifiers::SHIFT)] {
                assert_eq!(after(theme, at, chord, modifiers), "column_1", "{theme:?}: {chord:?} with {modifiers:?}");
            }
            for modifiers in [Modifiers::CTRL, Modifiers::CTRL.plus(Modifiers::MAC_CMD).plus(Modifiers::COMMAND)] {
                assert_eq!(after(theme, at, chord, modifiers), mac, "{theme:?}: a Mac's control key and {chord:?} with {modifiers:?}");
            }
        }
    });
}

/// Backspace, Delete and the control key's word deletion still delete, and typing still types.
#[test]
fn the_plain_deletions_stay() {
    in_both_appearances(|theme| {
        assert_eq!(after(theme, Key::End, Key::Backspace, Modifiers::NONE), "column_", "{theme:?}");
        assert_eq!(after(theme, Key::Home, Key::Delete, Modifiers::NONE), "olumn_1", "{theme:?}");
        assert_eq!(after(theme, Key::End, Key::Backspace, CONTROL), "", "{theme:?}: a word back");
        assert_eq!(after(theme, Key::Home, Key::Delete, CONTROL), "", "{theme:?}: a word on");
        let mut h = name_field(theme);
        key(&mut h, Key::End, Modifiers::NONE);
        key(&mut h, Key::K, CONTROL);
        h.event(egui::Event::Text("abc".into()));
        h.run();
        assert_eq!(name(&h), "column_1abc", "{theme:?}: the field still has the keyboard after the chord");
    });
}

/// The chords still do their actions' work: Ctrl+K opens the search, in whose field it then does
/// nothing, and Ctrl+Shift+H asks for the versions.
#[test]
fn the_chords_still_run_their_actions() {
    in_both_appearances(|theme| {
        let (mut h, host) = saving_harness(Some(commerce()), None);
        h.state_mut().state.theme = theme;
        host.version_reads.lock().unwrap().clear();
        key(&mut h, Key::K, CONTROL);
        assert!(h.state().home_search.open, "{theme:?}: Ctrl+K is Find…'s");
        h.event(egui::Event::Text("order".into()));
        h.run();
        key(&mut h, Key::Home, Modifiers::NONE);
        key(&mut h, Key::K, CONTROL);
        assert_eq!(h.state().home_search.query, "order", "{theme:?}: Find… is off in its own field, so nothing happens");
        key(&mut h, Key::K, Modifiers::CTRL);
        assert_eq!(h.state().home_search.query, "", "{theme:?}: the field had the keyboard, as a Mac's control key shows");
        key(&mut h, Key::Escape, Modifiers::NONE);
        h.run();
        key(&mut h, Key::H, CONTROL.plus(Modifiers::SHIFT));
        assert_eq!(host.version_reads.lock().unwrap().len(), 1, "{theme:?}: Ctrl+Shift+H opens History");
    });
}

/// A Mac's Control with an action's own chord is nobody's chord: with the keyboard in a field it
/// runs no action and the field deletes nothing on it, though Control and H alone delete there.
#[test]
fn a_macs_control_with_an_actions_chord_does_nothing_in_a_field() {
    in_both_appearances(|theme| {
        let history = Modifiers::MAC_CMD.plus(Modifiers::COMMAND).plus(Modifiers::SHIFT);
        let (mut h, host) = saving_harness(Some(commerce()), None);
        h.state_mut().state.theme = theme;
        key(&mut h, Key::K, Modifiers::MAC_CMD.plus(Modifiers::COMMAND));
        h.event(egui::Event::Text("order".into()));
        h.run();
        key(&mut h, Key::H, Modifiers::CTRL);
        assert_eq!(h.state().home_search.query, "orde", "{theme:?}: the field has the keyboard, and Control with H deletes");
        host.version_reads.lock().unwrap().clear();
        assert!(crate::actions::available(h.state(), crate::actions::VERSIONS), "{theme:?}: History can open");
        key(&mut h, Key::H, history.plus(Modifiers::CTRL));
        assert_eq!(h.state().home_search.query, "orde", "{theme:?}: nothing is deleted");
        assert_eq!(host.version_reads.lock().unwrap().len(), 0, "{theme:?}: and History stays shut");
        key(&mut h, Key::H, history);
        assert_eq!(host.version_reads.lock().unwrap().len(), 1, "{theme:?}: the chord itself opens History from there");
        assert_eq!(h.state().home_search.query, "orde", "{theme:?}");
    });
}
