//! The Edit menu's rows reach their own target: the focused text field, the canvas's or the
//! panels' model, selected label text, or nothing, in a held session, while only viewing and behind
//! a dialog as well.

use egui::{Event, Key, Modifiers, OutputCommand, ViewportCommand, ViewportId};

use super::*;
use crate::actions::{self, COPY, CUT, PASTE, REDO, SELECT_ALL, UNDO, run};
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

/// The rows of the clipboard and Select all.
const CLIP: [&str; 4] = [CUT, COPY, PASTE, SELECT_ALL];

/// The selection's ids, sorted.
fn selection(h: &Harness<'static, Shell>) -> Vec<String> {
    let s = h.state().session.as_ref().unwrap();
    let mut ids: Vec<String> = s.view().selection.iter().map(|o| s.doc().uuid(*o).unwrap().to_string()).collect();
    ids.sort();
    ids
}

/// The harness's project with Customer and a note drawn on an open diagram, Order left off it,
/// Customer selected and the keyboard on nothing; and the ids of what the diagram draws.
fn drawn(theme: ThemeKind) -> (Harness<'static, Shell>, Vec<String>) {
    let (mut h, customer) = table_harness();
    h.state_mut().state.theme = theme;
    let s = h.state_mut().session.as_mut().unwrap();
    let schema = s.doc().alive().find(|o| s.doc().class_of(*o).ok() == Some(archforge_er::SCHEMA)).unwrap();
    let schema = s.doc().uuid(schema).unwrap().to_string();
    let id = |v: serde_json::Value| v["id"].as_str().unwrap().to_string();
    s.execute("er.entity.add", &serde_json::json!({ "schema": schema, "name": "Order" })).unwrap();
    let d = id(s.execute("diagram.add", &serde_json::json!({ "kind": "er", "element": schema, "name": "Shop" })).unwrap().value);
    s.execute("diagram.place", &serde_json::json!({ "diagram": d, "element": customer, "x": 0, "y": 0, "width": 240, "height": 72 })).unwrap();
    let note = id(s.execute("diagram.note", &serde_json::json!({ "diagram": d, "text": "Why", "x": 900, "y": 40 })).unwrap().value);
    s.execute("view.open-diagram", &serde_json::json!({ "diagram": d })).unwrap();
    s.execute("view.select", &serde_json::json!({ "ids": [customer] })).unwrap();
    h.state_mut().edit.keyboard = Keyboard::Nobody;
    h.run_steps(2);
    let mut all = vec![customer, note];
    all.sort();
    (h, all)
}

/// Every state the clipboard rows and Select all can be asked in: which are on, and that each one
/// on hands its step to its target while each one off changes nothing.
#[test]
fn the_clipboard_rows_and_select_all_go_to_their_target_in_every_state() {
    type Setup = fn(&mut Shell);
    /// Gives a text field the keyboard.
    fn field(s: &mut Shell) {
        s.edit.keyboard = Keyboard::Field;
    }
    /// Gives the canvas the keyboard.
    fn canvas(s: &mut Shell) {
        s.canvas.focused = true;
    }
    /// Shows a version of the model as it is now, its canvas with the keyboard.
    fn viewing(s: &mut Shell) {
        let (tables, junctions) = archforge_notations::storage();
        let rows = archforge_mof::rows::snapshot(s.session.as_ref().unwrap().doc(), &tables, &junctions).unwrap();
        let preview = s.build(&rows).unwrap();
        s.show_version(preview, "v".into());
        canvas(s);
    }
    let states: [(&str, Setup, &[&str]); 13] = [
        ("a field", field, &CLIP),
        ("the model", |_| {}, &[]),
        ("the canvas", canvas, &[SELECT_ALL]),
        ("the explorer after the canvas", |s| (canvas(s), s.explorer.focused = true).1, &[]),
        ("a control that is no field", |s| (canvas(s), s.edit.keyboard = Keyboard::Other).1, &[]),
        ("text selected on a label", |s| s.edit.text_selected = true, &[COPY]),
        ("held", |s| (canvas(s), s.begin_hold().unwrap()).1, &[]),
        ("a field while held", |s| (field(s), s.begin_hold().unwrap()).1, &CLIP),
        ("viewing a version", viewing, &[SELECT_ALL]),
        ("a dialog over the window", |s| (canvas(s), crate::support::open(s)).1, &[]),
        ("a field in a dialog", |s| (field(s), crate::support::open(s)).1, &CLIP),
        ("a gesture", |s| (canvas(s), s.canvas.gesture = Some(crate::editing::Gesture::Marquee { from: egui::Pos2::ZERO })).1, &[]),
        ("no project shown", |s| (canvas(s), s.active = None).1, &[]),
    ];
    let select_all = Event::Key { key: Key::A, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::COMMAND };
    for ((name, setup, on), theme) in states.into_iter().zip([ThemeKind::Light, ThemeKind::Dark].into_iter().cycle()) {
        for id in CLIP {
            let (mut h, all) = drawn(theme);
            setup(h.state_mut());
            assert_eq!(actions::available(h.state(), id), on.contains(&id), "{name}, {id}");
            let (before, chosen) = (revision(&h), selection(&h));
            let on_field = h.state().edit.keyboard == Keyboard::Field;
            let (steps, paste, picked) = match id {
                _ if !on.contains(&id) => (vec![], false, chosen),
                CUT => (vec![Event::Cut], false, chosen),
                COPY => (vec![Event::Copy], false, chosen),
                PASTE => (vec![], true, chosen),
                _ if on_field => (vec![select_all.clone()], false, chosen),
                _ => (vec![], false, all),
            };
            run(h.state_mut(), id);
            assert_eq!((&h.state().edit.steps, h.state().edit.paste, selection(&h)), (&steps, paste, picked), "{name}, {id}");
            assert_eq!(revision(&h), before, "{name}, {id}: the model never moves");
        }
    }
}

/// The rows and ⌘A act on a real field, each step asserted: Select all takes the whole text, Copy
/// copies it, Cut copies and removes it, Paste asks the window for the clipboard.
#[test]
fn on_a_field_the_rows_select_copy_cut_and_paste_its_text() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, _) = drawn(theme);
        crate::new_project::open(h.state_mut());
        h.run_steps(3);
        assert!(h.ctx.text_edit_focused());
        let name = |h: &Harness<'static, Shell>| h.state().new_project.as_ref().unwrap().name.clone();
        let copied = |h: &Harness<'static, Shell>| h.output().platform_output.commands.contains(&OutputCommand::CopyText("Shop".into()));
        let before = revision(&h);
        h.event(Event::Text("Old".into()));
        h.run();
        assert_eq!(name(&h), "Old");
        run(h.state_mut(), SELECT_ALL);
        h.run();
        h.event(Event::Text("Shop".into()));
        h.run();
        assert_eq!(name(&h), "Shop", "Select all took the whole text");
        h.event(Event::Text("s".into()));
        key(&mut h, Key::A, Modifiers::COMMAND);
        h.event(Event::Text("Shop".into()));
        h.run();
        assert_eq!(name(&h), "Shop", "the field took its own ⌘A");
        run(h.state_mut(), SELECT_ALL);
        h.step();
        run(h.state_mut(), COPY);
        h.step();
        assert!(copied(&h), "Copy copied the selection");
        assert_eq!(name(&h), "Shop", "and left the text");
        run(h.state_mut(), CUT);
        h.step();
        assert!(copied(&h), "Cut copied the selection");
        assert_eq!(name(&h), "", "and took it");
        run(h.state_mut(), PASTE);
        h.step();
        assert!(h.output().viewport_output[&ViewportId::ROOT].commands.contains(&ViewportCommand::RequestPaste), "Paste asks for the clipboard");
        assert!(!h.state().edit.paste, "once");
        assert_eq!(revision(&h), before, "the model never moved");
    }
}

/// ⌘A on the canvas takes what the open diagram draws and no more, as the row does.
#[test]
fn command_a_on_the_canvas_selects_what_the_diagram_draws() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, all) = drawn(theme);
        h.state_mut().canvas.focused = true;
        key(&mut h, Key::A, Modifiers::COMMAND);
        assert_eq!(selection(&h), all, "Customer and the note, never Order");
        let s = h.state_mut().session.as_mut().unwrap();
        let schema = s.doc().alive().find(|o| s.doc().class_of(*o).ok() == Some(archforge_er::SCHEMA)).unwrap();
        let schema = s.doc().uuid(schema).unwrap().to_string();
        let empty = s.execute("diagram.add", &serde_json::json!({ "kind": "er", "element": schema, "name": "Empty" })).unwrap().value["id"]
            .as_str()
            .unwrap()
            .to_string();
        s.execute("view.open-diagram", &serde_json::json!({ "diagram": empty })).unwrap();
        key(&mut h, Key::A, Modifiers::COMMAND);
        assert_eq!(selection(&h), [empty], "a diagram that draws nothing selects itself");
    }
}

/// Text selected on a label turns Copy on, and Copy copies it; a press elsewhere turns it off.
#[test]
fn copy_takes_text_selected_on_a_label() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        crate::i18n::set_language("en");
        let mut shell = Shell::new(Services::default());
        shell.state.theme = theme;
        let words = "Selected words";
        let mut h = Harness::builder().with_size(egui::vec2(1600.0, 1000.0)).build_ui_state(
            move |ui, shell: &mut Shell| {
                shell.logic(ui.ctx());
                shell.ui(ui);
                egui::Area::new(crate::widgets::egui_id("test.label"))
                    .order(egui::Order::Foreground)
                    .fixed_pos(egui::pos2(600.0, 500.0))
                    .show(ui.ctx(), |ui| ui.label(words));
            },
            shell,
        );
        h.run();
        assert!(!actions::available(h.state(), COPY));
        // A drag across the label's words.
        for (x, pressed) in [(601.0, true), (760.0, false)] {
            h.event(Event::PointerMoved(egui::pos2(x, 507.0)));
            h.run();
            h.event(Event::PointerButton { pos: egui::pos2(x, 507.0), button: egui::PointerButton::Primary, pressed, modifiers: Modifiers::NONE });
            h.run();
        }
        assert!(actions::available(h.state(), COPY), "the selection turns Copy on");
        assert!(!actions::available(h.state(), CUT), "but no field's row");
        run(h.state_mut(), COPY);
        h.step();
        assert!(h.output().platform_output.commands.contains(&OutputCommand::CopyText(words.into())), "{:?}", h.output().platform_output.commands);
        click(&mut h, egui::pos2(300.0, 900.0));
        assert!(!actions::available(h.state(), COPY), "a press elsewhere ends it");
    }
}
