//! Undo and Redo reach their own target: the focused text field, the canvas's or the panels' model,
//! or nothing, in a held session, while only viewing and behind a dialog as well.

use egui::{Key, Modifiers};

use super::*;
use crate::actions::{self, REDO, UNDO, run};
use crate::edit_menu::Keyboard;

/// The model's revision.
fn revision(h: &Harness<'static, Shell>) -> u64 {
    h.state().session.as_ref().unwrap().revision()
}

/// The harness's project with a table selected and a step undone, so both a step to undo and one
/// to redo wait, and the keyboard on nothing.
fn project(theme: ThemeKind) -> Harness<'static, Shell> {
    let (mut h, _) = table_harness();
    h.state_mut().state.theme = theme;
    h.state_mut().session.as_mut().unwrap().undo().unwrap();
    h.state_mut().edit.keyboard = Keyboard::Nobody;
    h.run_steps(2);
    h
}

/// Every state the rows can be asked in: which are on, and what running each takes it to.
#[test]
fn undo_and_redo_go_to_their_target_in_every_state() {
    type Setup = fn(&mut Shell);
    /// Gives a text field the keyboard.
    fn field(s: &mut Shell) {
        s.edit.keyboard = Keyboard::Field;
    }
    /// Makes the model read-only.
    fn read_only(s: &mut Shell) {
        s.session.as_mut().unwrap().set_read_only(true).unwrap();
    }
    /// Holds the project.
    fn held(s: &mut Shell) {
        s.begin_hold().unwrap();
    }
    /// Opens a Chat history row's menu.
    fn row_menu(s: &mut Shell) {
        s.chat_panel.row_menu = Some(crate::chat_delete::RowMenu { chat: archforge_mof::Uuid([1; 16]), row: 0, at: egui::Pos2::ZERO, reached: None });
    }
    #[derive(Clone, Copy, Debug, PartialEq)]
    enum Goes {
        Field,
        Model,
        Nowhere,
    }
    let states: [(&str, Setup, Goes); 13] = [
        ("a field", field, Goes::Field),
        ("the model", |_| {}, Goes::Model),
        ("the canvas", |s| s.canvas.focused = true, Goes::Model),
        ("a control that is no field", |s| s.edit.keyboard = Keyboard::Other, Goes::Nowhere),
        ("held", held, Goes::Nowhere),
        ("a field while held", |s| (field(s), held(s)).1, Goes::Field),
        ("read-only", read_only, Goes::Nowhere),
        ("a field while read-only", |s| (field(s), read_only(s)).1, Goes::Field),
        ("a dialog over the window", crate::support::open, Goes::Nowhere),
        ("a field in a dialog", |s| (field(s), crate::support::open(s)).1, Goes::Field),
        ("a history row's menu", row_menu, Goes::Nowhere),
        ("no project shown", |s| s.active = None, Goes::Nowhere),
        ("no model", |s| s.session = None, Goes::Nowhere),
    ];
    for ((name, setup, goes), theme) in states.into_iter().zip([ThemeKind::Light, ThemeKind::Dark].into_iter().cycle()) {
        for id in [UNDO, REDO] {
            let mut h = project(theme);
            setup(h.state_mut());
            assert_eq!(actions::available(h.state(), id), goes != Goes::Nowhere, "{name}, {id}");
            let before = h.state().session.as_ref().map(|s| s.revision());
            run(h.state_mut(), id);
            let moved = h.state().session.as_ref().map(|s| s.revision()) != before;
            assert_eq!((moved, h.state().edit.steps.len()), (goes == Goes::Model, usize::from(goes == Goes::Field)), "{name}, {id}");
        }
    }
}

/// The rows act on a real field and never on the model, and its own keys are not taken from it.
#[test]
fn on_a_field_the_rows_and_the_keys_change_the_field_and_not_the_model() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let mut h = project(theme);
        crate::new_project::open(h.state_mut());
        h.run_steps(3);
        assert!(h.ctx.text_edit_focused());
        let name = |h: &Harness<'static, Shell>| h.state().new_project.as_ref().unwrap().name.clone();
        let (first, before) = (name(&h), revision(&h));
        h.event(egui::Event::Text("x".into()));
        h.run();
        let typed = name(&h);
        assert_ne!(typed, first);
        run(h.state_mut(), UNDO);
        h.run();
        assert_eq!(name(&h), first, "the row undid the field");
        run(h.state_mut(), REDO);
        h.run();
        assert_eq!(name(&h), typed, "the row redid it");
        // Each key in a single frame: taken for the routing, it would wait for a frame none asks for.
        for (k, modifiers, wanted) in [
            (Key::Z, Modifiers::COMMAND, &first),
            (Key::Z, Modifiers::COMMAND | Modifiers::SHIFT, &typed),
            (Key::Z, Modifiers::COMMAND, &first),
            (Key::Y, Modifiers::COMMAND, &typed),
        ] {
            h.event(egui::Event::Key { key: k, physical_key: None, pressed: true, repeat: false, modifiers });
            h.step();
            assert_eq!(&name(&h), wanted, "the field took its own {k:?}");
        }
        assert_eq!(revision(&h), before, "the model never moved");
    }
}

/// A row chosen from the in-window menu by the pointer reaches the field the menu was opened on.
#[test]
fn the_pointer_on_the_in_window_menu_reaches_the_field_it_was_opened_on() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let mut h = project(theme);
        h.state_mut().in_window_menu = true;
        h.run();
        let at = rect_of(&h, "workspace.properties.columns.add").center();
        click(&mut h, at);
        let at = rect_of(&h, "workspace.properties.column.name").center();
        click(&mut h, at);
        assert!(h.ctx.text_edit_focused());
        let name = |h: &Harness<'static, Shell>| h.state().panels.column.as_ref().unwrap().name.clone();
        let first = name(&h);
        h.event(egui::Event::Text("x".into()));
        h.run();
        let (before, typed) = (revision(&h), name(&h));
        assert_ne!(typed, first);
        let at = rect_of(&h, "strip.menu.edit").center();
        click(&mut h, at);
        assert_eq!(h.state().edit.keyboard, Keyboard::Field, "the open menu still counts the field");
        let at = rect_of(&h, "strip.menu.row.edit.undo").center();
        click(&mut h, at);
        h.run();
        assert!(h.ctx.text_edit_focused(), "the field keeps the keyboard");
        assert_eq!((name(&h), revision(&h)), (first, before), "the row undid the field, not the model");
        // A row of another menu leaves the focus where the click put it.
        let field = crate::widgets::egui_id("workspace.properties.column.name");
        h.ctx.memory_mut(|m| m.surrender_focus(field));
        (h.state_mut().menu_bar.back, h.state_mut().menu_bar.chosen) = (Some(field), Some(actions::SAVE));
        h.step();
        assert!(!h.ctx.text_edit_focused(), "only an Edit row gives the field its keyboard back");
    }
}

/// With no field the rows and keys step the model; ⇧⌘Z is never Undo, and Ctrl Y is Redo's second key.
#[test]
fn with_no_field_the_keys_step_the_model() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, _) = table_harness();
        h.state_mut().state.theme = theme;
        let start = revision(&h);
        assert!(!actions::available(h.state(), REDO));
        run(h.state_mut(), REDO);
        assert!(h.state().canvas.notice.is_none(), "with no step to redo the row asks nothing of the engine");
        key(&mut h, Key::Z, Modifiers::COMMAND | Modifiers::SHIFT);
        assert_eq!(revision(&h), start, "with Redo off, Shift does not make the keys Undo's");
        h.ctx.memory_mut(|m| m.request_focus(crate::widgets::egui_id("strip.theme")));
        h.run();
        assert_eq!(h.state().edit.keyboard, Keyboard::Other, "a focused control is no field");
        key(&mut h, Key::Z, Modifiers::COMMAND);
        assert_eq!(revision(&h), start, "and the model's Undo is not for it");
        h.ctx.memory_mut(|m| m.surrender_focus(crate::widgets::egui_id("strip.theme")));
        h.run();
        key(&mut h, Key::Z, Modifiers::COMMAND);
        let undone = revision(&h);
        assert_ne!(undone, start);
        key(&mut h, Key::Y, Modifiers::COMMAND);
        assert_ne!(revision(&h), undone, "Ctrl Y redoes");
        assert!(!actions::available(h.state(), REDO));
    }
}
