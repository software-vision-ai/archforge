//! Properties' body and the keyboard over UML's editors (AR-470): Tab goes on past the fold and the
//! body follows the control it lands on, an editor brought into view goes no further, and the page
//! keys scroll the body while one of its controls has the keyboard.

use egui::{Key, Modifiers};
use egui_kittest::kittest::{Queryable, by};

use super::*;

const ADD: &str = "workspace.properties.constraints.add";

const APPLY: &str = "workspace.properties.constraint.apply";

const NAME: &str = "workspace.properties.name";

const ABSTRACT: &str = "workspace.properties.class.abstract";

/// The sample's Order with twenty more attributes in Properties, at the ordinary window height:
/// its Constraints section starts below the fold, more than a page down.
fn long_class(theme: ThemeKind) -> Harness<'static, Shell> {
    let (mut h, _) = related_sample_in(theme, egui::vec2(1600.0, 1000.0));
    let order = named(&h, archforge_uml::CLASS, "Order").to_string();
    archforge_ui_egui::snapshot::run(h.state_mut(), "view.select", serde_json::json!({ "ids": [order] })).unwrap();
    for n in 0..20 {
        archforge_ui_egui::snapshot::run(h.state_mut(), "uml.attribute.add", serde_json::json!({ "class": order, "name": format!("extra{n}") }))
            .unwrap();
    }
    h.state_mut().state.show_panel(archforge_shell_api::state::Panel::Properties);
    h.run();
    assert!(!has_id(&h, ADD) && h.state().panels.scroll == 0.0, "{theme:?}: Constraints starts out of view");
    h
}

/// The control that has the keyboard, if the body laid it out: whether it is wholly in view.
fn held_in_view(h: &Harness<'static, Shell>) -> Option<bool> {
    let at = archforge_ui_egui::testing::registry(h.state()).laid_out(h.ctx.memory(|m| m.focused())?)?;
    Some(h.state().panels.body.unwrap().contains_rect(at))
}

/// Presses Tab, with `modifiers`, until `id` has the keyboard; every control of the body it lands
/// on by the way is in view.
fn tab_to(h: &mut Harness<'static, Shell>, id: &str, modifiers: Modifiers) {
    for _ in 0..200 {
        if h.ctx.memory(|m| m.has_focus(archforge_ui_egui::widgets::egui_id(id))) {
            return;
        }
        key(h, Key::Tab, modifiers);
        assert_ne!(held_in_view(h), Some(false), "the keyboard is on a control out of view, on the way to {id}");
    }
    panic!("Tab never reached {id}");
}

/// How far the body is scrolled.
fn scroll(h: &Harness<'static, Shell>) -> f32 {
    h.state().panels.scroll
}

/// Tab reaches the Constraints `+` below the fold and the body brings it into view, listed and
/// focused for a screen reader, and Tab on past the body leaves it there; Enter opens its editor,
/// brought into view; Tab reaches the editor's Apply, Space presses it, and Shift+Tab goes back up to the Name field: by the keyboard
/// alone.
#[test]
fn tab_goes_on_past_the_fold_and_the_body_follows() {
    in_both_appearances(|theme| {
        let mut h = long_class(theme);
        tab_to(&mut h, ADD, Modifiers::NONE);
        assert!(scroll(&h) > 0.0 && has_id(&h, ADD), "{theme:?}: the body followed to the + ({})", scroll(&h));
        let node = archforge_ui_egui::widgets::egui_id(ADD).accesskit_id();
        assert_eq!(h.query_all(by().predicate(|n| n.is_focused() && n.locate().0 == node)).count(), 1, "{theme:?}: a screen reader is on it");
        let at = scroll(&h);
        tab_to(&mut h, "workspace.panel.resize", Modifiers::NONE);
        assert_eq!(scroll(&h), at, "{theme:?}: a control after the body moves nothing");
        tab_to(&mut h, ADD, Modifiers::SHIFT);
        key(&mut h, Key::Enter, Modifiers::NONE);
        assert!(uml_of(h.state()).constraint.is_some(), "{theme:?}: Enter on the + opens the editor");
        let (foot, body) = (rect_of(&h, APPLY), h.state().panels.body.unwrap());
        assert!(body.contains_rect(foot), "{theme:?}: the editor is brought into view, to its foot");
        tab_to(&mut h, "workspace.properties.constraint.expression", Modifiers::NONE);
        h.event(egui::Event::Text("total > 0".into()));
        h.run();
        assert_eq!(uml_of(h.state()).constraint.as_ref().unwrap().expression, "total > 0", "{theme:?}: typed in its expression");
        tab_to(&mut h, APPLY, Modifiers::NONE);
        assert!(has_id(&h, APPLY), "{theme:?}: Apply is listed where the keyboard is");
        key(&mut h, Key::Space, Modifiers::NONE);
        assert_eq!(constraint_rows(&h, "Order").len(), 1, "{theme:?}: Space on Apply wrote the constraint");
        tab_to(&mut h, ADD, Modifiers::NONE);
        let low = scroll(&h);
        tab_to(&mut h, NAME, Modifiers::SHIFT);
        assert!(scroll(&h) < low && has_id(&h, NAME), "{theme:?}: Shift+Tab brought the body back up ({low} to {})", scroll(&h));
    });
}

/// The last attribute's pencil in full view, by Tab, and its editor opened by Enter: as the key
/// goes down only, or with Tab pressed as it goes up, the frame that brings the editor into view.
fn attribute_editor(theme: ThemeKind, tab: bool) -> Harness<'static, Shell> {
    let mut h = long_class(theme);
    let body = h.state().panels.body.unwrap();
    let pencil = ids(&h).into_iter().filter(|i| i.starts_with("workspace.properties.attribute.") && i.ends_with(".edit"));
    let last = pencil.filter(|i| body.contains_rect(rect_of(&h, i))).last().unwrap();
    tab_to(&mut h, &last, Modifiers::NONE);
    h.key_down(Key::Enter);
    h.step();
    assert!(uml_of(h.state()).member.is_some(), "{theme:?}: Enter on the pencil opens the attribute's editor");
    h.key_up(Key::Enter);
    if tab {
        h.key_down(Key::Tab);
        h.step();
        h.key_up(Key::Tab);
    }
    h.run();
    h
}

/// The attribute's editor takes the keyboard into its name field in the frame that brings it into
/// view, and Tab pressed then moves on from there: each time to a control laid out before that
/// scroll. The body takes the scroll into account and goes no further than the editor took it,
/// which puts the card's foot 8 above the body's, 12 under Apply.
#[test]
fn tab_as_an_editor_comes_into_view_goes_no_further() {
    in_both_appearances(|theme| {
        let alone = attribute_editor(theme, false);
        assert!(scroll(&alone) > 0.0, "{theme:?}: the editor was brought into view");
        let body = alone.state().panels.body.unwrap();
        assert_eq!(rect_of(&alone, "workspace.properties.member.apply").bottom(), body.bottom() - 20.0, "{theme:?}: Apply where the reveal put it");
        assert_eq!(held_in_view(&alone), Some(true), "{theme:?}: the editor's name field has the keyboard, in view");
        let h = attribute_editor(theme, true);
        assert_eq!(held_in_view(&h), Some(true), "{theme:?}: the control Tab moved to is in view");
        assert_eq!(scroll(&h), scroll(&alone), "{theme:?}: the body went no further than the editor brought it");
    });
}

/// A control the pointer gives the keyboard to stays where it is, and the frame that follows the
/// keyboard asks for the next one, which draws the body where it went.
#[test]
fn the_body_follows_the_keyboard_only_and_asks_for_its_frame() {
    in_both_appearances(|theme| {
        let mut h = long_class(theme);
        let body = h.state().panels.body.unwrap();
        let cut = rect_of(&h, NAME).center().y - body.top();
        h.state_mut().panels.scroll = cut;
        h.run();
        let field = rect_of(&h, NAME);
        assert!(field.top() < body.top() && field.bottom() > body.top() + 4.0, "{theme:?}: the Name field is cut by the body's top");
        click(&mut h, egui::pos2(field.center().x, field.bottom() - 2.0));
        assert_eq!((held_in_view(&h), scroll(&h)), (Some(false), cut), "{theme:?}: a click gives it the keyboard and moves nothing");
        // The keyboard moved by the program, as by a screen reader, with no key or pointer event.
        h.ctx.memory_mut(|m| m.request_focus(archforge_ui_egui::widgets::egui_id(ADD)));
        h.step();
        assert!(scroll(&h) > cut && h.ctx.has_requested_repaint(), "{theme:?}: the body moved and asks to be drawn again");
    });
}

/// Page Down and Page Up move the body by a page and End and Home to its ends, while a control of
/// the body has the keyboard: not from a text field, which keeps its keys, nor with the keyboard
/// elsewhere or nowhere, with a menu open, the in-window menu bar's too, also in the frame a click
/// of the pointer or a screen reader opens it in, or with the control key held.
#[test]
fn the_page_keys_scroll_the_body_while_it_has_the_keyboard() {
    in_both_appearances(|theme| {
        let mut h = long_class(theme);
        let room = h.state().panels.content - h.state().panels.body.unwrap().height();
        let page = (h.state().panels.body.unwrap().height() - archforge_shell_api::panels::ROW).min(room);
        for key_ in [Key::PageDown, Key::End] {
            key(&mut h, key_, Modifiers::NONE);
            assert_eq!(scroll(&h), 0.0, "{theme:?}: {key_:?} with the keyboard nowhere");
        }
        tab_to(&mut h, "workspace.tools.select", Modifiers::NONE);
        key(&mut h, Key::PageDown, Modifiers::NONE);
        assert_eq!(scroll(&h), 0.0, "{theme:?}: with the keyboard on the canvas tools");
        // The Name field keeps End and Home: what is typed after them lands at its ends.
        tab_to(&mut h, NAME, Modifiers::NONE);
        for (key_, typed, reads) in [(Key::End, "y", "Ordery"), (Key::Home, "x", "xOrdery"), (Key::PageDown, "", "xOrdery")] {
            key(&mut h, key_, Modifiers::NONE);
            h.event(egui::Event::Text(typed.into()));
            h.run();
            let text = h.state().panels.name.as_ref().map(|n| n.text.clone());
            assert_eq!((text.as_deref(), scroll(&h)), (Some(reads), 0.0), "{theme:?}: {key_:?} in the Name field");
        }
        key(&mut h, Key::Escape, Modifiers::NONE);
        tab_to(&mut h, "workspace.properties.class.owner", Modifiers::NONE);
        key(&mut h, Key::Enter, Modifiers::NONE);
        assert!(h.state().panels.menu.is_some(), "{theme:?}: Enter opens the owner's menu");
        key(&mut h, Key::PageDown, Modifiers::NONE);
        assert_eq!(scroll(&h), 0.0, "{theme:?}: with a menu open");
        key(&mut h, Key::Escape, Modifiers::NONE);
        tab_to(&mut h, ABSTRACT, Modifiers::NONE);
        key(&mut h, Key::PageDown, Modifiers::CTRL.plus(Modifiers::COMMAND));
        assert_eq!(scroll(&h), 0.0, "{theme:?}: with the control key as Windows and Linux send it");
        // The in-window menu bar opened by F10 leaves the keyboard on the check box; its keys are its own.
        h.state_mut().in_window_menu = true;
        key(&mut h, Key::F10, Modifiers::NONE);
        assert!(archforge_ui_egui::testing::open_menu(h.state()).is_some(), "{theme:?}: F10 opens the bar's first menu");
        key(&mut h, Key::End, Modifiers::NONE);
        assert_eq!(scroll(&h), 0.0, "{theme:?}: with the menu bar open");
        key(&mut h, Key::Escape, Modifiers::NONE);
        assert!(
            archforge_ui_egui::testing::open_menu(h.state()).is_none()
                && h.ctx.memory(|m| m.has_focus(archforge_ui_egui::widgets::egui_id(ABSTRACT))),
            "{theme:?}: back"
        );
        for (key_, reaches) in [(Key::PageDown, page), (Key::PageUp, 0.0), (Key::End, room), (Key::Home, 0.0)] {
            key(&mut h, key_, Modifiers::NONE);
            assert_eq!(scroll(&h), reaches, "{theme:?}: {key_:?} with the keyboard on a check box of the body");
        }
        // Nor in the frame of the click that opens the bar, by the pointer or a screen reader (AR-518).
        for pointer in [true, false] {
            open_file_with(&mut h, Key::End, Modifiers::NONE, pointer);
            assert_eq!(scroll(&h), 0.0, "{theme:?}: with the click that opens the menu bar, by the pointer: {pointer}");
            key(&mut h, Key::Escape, Modifiers::NONE);
            assert!(h.ctx.memory(|m| m.has_focus(archforge_ui_egui::widgets::egui_id(ABSTRACT))), "{theme:?}: back on the check box");
        }
    });
}
