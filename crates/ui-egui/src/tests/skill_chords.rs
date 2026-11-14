//! The skill text editor's chords: ⌘B, ⌘I and ⌘K act only with exactly their modifiers (AR-450).

use egui::{Key, Modifiers};

use super::*;

/// A skill open in its tab in `theme`, its text field holding the keyboard with "names" selected.
fn skill_harness(theme: ThemeKind) -> Harness<'static, Shell> {
    let (mut h, customer) = table_harness();
    h.state_mut().state.theme = theme;
    let skill = {
        let s = h.state_mut().session.as_mut().unwrap();
        let out = s.execute("skill.add", &serde_json::json!({ "subject": customer, "name": "Customer naming", "body": "Use names." })).unwrap();
        s.execute("view.select", &serde_json::json!({ "ids": [customer] })).unwrap();
        out.value["id"].as_str().unwrap().to_string()
    };
    let at = rect_of(&h, "workspace.panels.skills").center();
    click(&mut h, at);
    let at = rect_of(&h, &format!("workspace.skills.row.{skill}")).left_center() + egui::vec2(40.0, 0.0);
    click(&mut h, at);
    let editor = crate::widgets::egui_id("workspace.skill.text");
    let mut state = egui::text_edit::TextEditState::load(&h.ctx, editor).unwrap_or_default();
    state.cursor.set_char_range(Some(egui::text::CCursorRange::two(egui::text::CCursor::new(4), egui::text::CCursor::new(9))));
    state.store(&h.ctx, editor);
    h.ctx.memory_mut(|m| m.request_focus(editor));
    h.run_steps(2);
    assert!(h.ctx.memory(|m| m.has_focus(editor)), "{theme:?}: the text has the keyboard");
    assert_eq!(h.state().canvas.skills[0].selection, Some((4, 9)), "{theme:?}");
    h
}

/// With Shift or Alt held as well, and when its key is let go, a chord leaves the text alone;
/// pressed alone, it formats the selection.
#[test]
fn a_chord_with_shift_or_alt_held_leaves_the_text_alone() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        for (k, formatted) in [(Key::B, "Use **names**."), (Key::I, "Use *names*."), (Key::K, "Use [names](url).")] {
            let mut h = skill_harness(theme);
            for extra in [Modifiers::SHIFT, Modifiers::ALT, Modifiers::SHIFT | Modifiers::ALT] {
                key(&mut h, k, Modifiers::COMMAND | extra);
                assert_eq!(h.state().canvas.skills[0].text, "Use names.", "{theme:?} {k:?} with {extra:?}");
            }
            h.key_up_modifiers(Modifiers::COMMAND, k);
            h.run();
            assert_eq!(h.state().canvas.skills[0].text, "Use names.", "{theme:?} {k:?} let go");
            key(&mut h, k, Modifiers::COMMAND);
            assert_eq!(h.state().canvas.skills[0].text, formatted, "{theme:?} {k:?} alone");
        }
    }
}
