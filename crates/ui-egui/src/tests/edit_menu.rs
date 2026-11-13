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

/// The harness's project with Customer, a note, a note nested in Customer's box, Address and the
/// relationship's line between them drawn on an open diagram, Order left off it,
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
    let table = id(s
        .execute("diagram.place", &serde_json::json!({ "diagram": d, "element": customer, "x": 0, "y": 0, "width": 240, "height": 72 }))
        .unwrap()
        .value);
    let note = id(s.execute("diagram.note", &serde_json::json!({ "diagram": d, "text": "Why", "x": 900, "y": 40 })).unwrap().value);
    let inner = id(s.execute("diagram.note", &serde_json::json!({ "diagram": d, "text": "How", "parent": table, "x": 8, "y": 8 })).unwrap().value);
    let address = id(s.execute("er.entity.add", &serde_json::json!({ "schema": schema, "name": "Address" })).unwrap().value);
    let at = id(s.execute("diagram.place", &serde_json::json!({ "diagram": d, "element": address, "x": 400, "y": 0 })).unwrap().value);
    let lives = id(s
        .execute("er.relationship.add", &serde_json::json!({ "from": customer, "to": address, "name": "lives at", "create_foreign_key": false }))
        .unwrap()
        .value);
    s.execute("diagram.connect", &serde_json::json!({ "diagram": d, "element": lives, "source": table, "target": at })).unwrap();
    crate::canvas::open(h.state_mut(), archforge_mof::Uuid::parse(&d).unwrap());
    let s = h.state_mut().session.as_mut().unwrap();
    s.execute("view.select", &serde_json::json!({ "ids": [customer] })).unwrap();
    h.state_mut().edit.keyboard = Keyboard::Nobody;
    h.run_steps(2);
    let mut all = vec![customer, note, inner, address, lives];
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
    /// Opens a version's details over the canvas, which keeps the keyboard.
    fn details(s: &mut Shell) {
        canvas(s);
        s.state.explorer = crate::state::Explorer::SourceControl;
        s.services.projects = Some(std::sync::Arc::new(crate::actions::tests::CountingHost::default()));
        crate::version_details::open(s, version_row("c1", None, (None, false), "2026-11-06T15:05:00Z"));
    }
    let states: [(&str, Setup, &[&str]); 17] = [
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
        ("a version's details over the canvas", details, &[]),
        ("a gesture", |s| (canvas(s), s.canvas.gesture = Some(crate::editing::Gesture::Marquee { from: egui::Pos2::ZERO })).1, &[]),
        ("no project shown", |s| (canvas(s), s.active = None).1, &[]),
        ("no diagram on the canvas, the model's view still on one", |s| (canvas(s), s.canvas.active = None).1, &[]),
        ("a line's menu open on the canvas", |s| (canvas(s), s.canvas.line_menu = Some(egui::Pos2::ZERO)).1, &[]),
        ("the classifier's menu of kinds open", |s| (canvas(s), s.canvas.popover = Some(crate::uml_classifier::KINDS)).1, &[]),
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
        assert_eq!(selection(&h), all, "every box and the line, never Order");
        let s = h.state_mut().session.as_mut().unwrap();
        let schema = s.doc().alive().find(|o| s.doc().class_of(*o).ok() == Some(archforge_er::SCHEMA)).unwrap();
        let schema = s.doc().uuid(schema).unwrap().to_string();
        let empty = s.execute("diagram.add", &serde_json::json!({ "kind": "er", "element": schema, "name": "Empty" })).unwrap().value["id"]
            .as_str()
            .unwrap()
            .to_string();
        crate::canvas::open(h.state_mut(), archforge_mof::Uuid::parse(&empty).unwrap());
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
        (shell.state.theme, shell.in_window_menu) = (theme, true);
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
        // From the keys: F10, over to Edit, whose first live row is Copy, and Enter.
        for k in [Key::F10, Key::ArrowRight] {
            key(&mut h, k, Modifiers::NONE);
        }
        assert_eq!(h.state().menu_bar.row, Some(COPY));
        h.key_press(Key::Enter);
        assert!((0..4).any(|_| (h.step(), copied(&h, words)).1), "the row copied the words");
        let at = rect_of(&h, "strip.menu.edit").center();
        click(&mut h, at);
        assert!(!actions::available(h.state(), COPY), "the press that opens the menu ends the selection");
    }
}

/// Whether the frame just drawn copied `text`.
fn copied(h: &Harness<'static, Shell>, text: &str) -> bool {
    h.output().platform_output.commands.contains(&OutputCommand::CopyText(text.into()))
}

/// Rows chosen by the pointer act on the text the field had selected when the menu opened, though
/// the click took the keyboard, and with it the selection, from the field. Each step asserted.
#[test]
fn rows_chosen_by_the_pointer_act_on_the_fields_selection() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let mut h = project(theme);
        h.state_mut().in_window_menu = true;
        h.run();
        for id in ["workspace.properties.columns.add", "workspace.properties.column.name"] {
            let at = rect_of(&h, id).center();
            click(&mut h, at);
        }
        let name = |h: &Harness<'static, Shell>| h.state().panels.column.as_ref().unwrap().name.clone();
        // Selects all of the field's text by its keys, then chooses the Edit row `row` by the pointer;
        // whether the frames after it copied "Shop", and asked for the clipboard.
        let choose = |h: &mut Harness<'static, Shell>, row: &str| {
            key(h, Key::A, Modifiers::COMMAND);
            let at = rect_of(h, "strip.menu.edit").center();
            click(h, at);
            let at = rect_of(h, &format!("strip.menu.row.edit.{row}")).center();
            h.event(Event::PointerMoved(at));
            h.run();
            for pressed in [true, false] {
                h.event(Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Modifiers::NONE });
            }
            let paste = |h: &Harness<'static, Shell>| h.output().viewport_output[&ViewportId::ROOT].commands.contains(&ViewportCommand::RequestPaste);
            (0..4).map(|_| (h.step(), (copied(h, "Shop"), paste(h))).1).fold((false, false), |a, b| (a.0 || b.0, a.1 || b.1))
        };
        key(&mut h, Key::A, Modifiers::COMMAND);
        h.event(Event::Text("Shop".into()));
        h.run();
        assert_eq!(name(&h), "Shop");
        assert_eq!((choose(&mut h, "copy"), name(&h)), ((true, false), "Shop".into()), "Copy copied the selection");
        assert_eq!((choose(&mut h, "cut"), name(&h)), ((true, false), String::new()), "Cut took it");
        h.event(Event::Text("Old".into()));
        h.run();
        assert_eq!(choose(&mut h, "paste"), (false, true), "Paste asked for the clipboard");
        h.event(Event::Paste("New".into()));
        h.run();
        assert_eq!(name(&h), "New", "the pasted text took the selection's place");
    }
}

/// The name typed in place, in Models and on the canvas, stays open through the in-window Edit menu:
/// Copy, Cut and Paste act on its selection, the menu closed by its title leaves it open with its
/// text selected, and a press elsewhere then commits it, as it does with no menu.
#[test]
fn the_inline_name_stays_open_through_the_in_window_edit_menu() {
    for (theme, on_canvas) in [(ThemeKind::Light, false), (ThemeKind::Dark, false), (ThemeKind::Light, true), (ThemeKind::Dark, true)] {
        let (mut h, _) = drawn(theme);
        (h.state_mut().in_window_menu, h.state_mut().explorer.focused) = (true, !on_canvas);
        h.run();
        if on_canvas {
            // The header's right end, clear of the note inside the box.
            let node = node_of(&h, archforge_mof::Uuid::parse(&selection(&h)[0]).unwrap());
            let at = rect_of(&h, &format!("workspace.canvas.node.{node}")).right_top() + egui::vec2(-20.0, 12.0);
            (0..2).for_each(|_| click(&mut h, at));
        } else {
            key(&mut h, Key::F2, Modifiers::NONE);
        }
        let name = |h: &Harness<'static, Shell>| {
            h.state().canvas.rename.as_ref().map(|r| r.text.clone()).or(h.state().explorer.rename.as_ref().map(|r| r.text.clone()))
        };
        // Selects the name's text, opens Edit, clicks `at`; whether the frames after copied "Customer".
        let menu = |h: &mut Harness<'static, Shell>, at: &str| {
            key(h, Key::A, Modifiers::COMMAND);
            through_edit(h, at, "Customer")
        };
        assert!(menu(&mut h, "strip.menu.row.edit.copy") && name(&h).as_deref() == Some("Customer"), "{on_canvas}: Copy copied the name");
        assert!(menu(&mut h, "strip.menu.row.edit.cut") && name(&h).as_deref() == Some(""), "{on_canvas}: Cut took it");
        menu(&mut h, "strip.menu.row.edit.paste");
        h.event(Event::Paste("Order".into()));
        h.run();
        menu(&mut h, "strip.menu.edit");
        h.event(Event::Text("Item".into()));
        h.run();
        assert_eq!(name(&h).as_deref(), Some("Item"), "{on_canvas}: pasted, and the menu closed by its title left the name selected");
        menu(&mut h, "workspace.properties.kind");
        let st = h.state().session.as_ref().unwrap().doc();
        assert!(name(&h).is_none() && st.alive().any(|o| archforge_core::name_of(st, o) == "Item"), "{on_canvas}: the press elsewhere committed it");
        h.state_mut().menu_bar.held = Some(crate::widgets::egui_id("gone"));
        h.step();
        assert_eq!(h.state().menu_bar.held, None, "{on_canvas}: a name gone while held is let go once");
    }
}

/// Opens the in-window Edit menu and clicks `at` by the pointer; whether the frames after copied `text`.
fn through_edit(h: &mut Harness<'static, Shell>, at: &str, text: &str) -> bool {
    for id in ["strip.menu.edit", at] {
        h.run();
        let at = rect_of(h, id).center();
        h.event(Event::PointerMoved(at));
        h.run();
        (0..2).for_each(|n| {
            h.event(Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: n == 0, modifiers: Modifiers::NONE })
        });
    }
    (0..4).any(|_| (h.step(), copied(h, text)).1)
}

/// An editor on the canvas, its text `typed` "Draft", stays open through the in-window Edit menu:
/// Select all, Copy, Cut and Paste act on its text, `pasted` replacing it. File, opened from Edit
/// with the field held, finds it committed.
fn stays_open(h: &mut Harness<'static, Shell>, typed: fn(&Shell) -> Option<String>, pasted: &str, theme: ThemeKind) {
    let row = |r: &str| format!("strip.menu.row.edit.{r}");
    through_edit(h, &row("select-all"), "");
    assert!(through_edit(h, &row("copy"), "Draft") && typed(h.state()).as_deref() == Some("Draft"), "{theme:?}: Copy copied all of it");
    assert!(through_edit(h, &row("cut"), "Draft") && typed(h.state()).as_deref() == Some(""), "{theme:?}: Cut took it");
    through_edit(h, &row("paste"), "");
    h.event(Event::Paste(pasted.into()));
    h.run();
    assert_eq!(typed(h.state()).as_deref(), Some(pasted), "{theme:?}: pasted");
    for id in ["strip.menu.edit", "strip.menu.file"] {
        let at = rect_of(h, id).center();
        click(h, at);
    }
    assert_eq!((h.state().menu_bar.open, typed(h.state())), (Some("file"), None), "{theme:?}: File, from Edit, found it committed");
    key(h, Key::Escape, Modifiers::NONE);
}

/// A note's text typed on the canvas, and a class's member typed in place, in its name field and
/// in its type field, stay open through the in-window Edit menu, whose press is a click away from
/// the member's row; another menu commits them.
#[test]
fn the_note_and_member_editors_stay_open_through_the_in_window_edit_menu() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, _) = drawn(theme);
        h.state_mut().in_window_menu = true;
        h.run();
        let text = |st: &archforge_mof::Store, o| st.get(o, archforge_diagram::node::TEXT).ok().and_then(|v| v.as_str().map(str::to_string));
        let st = h.state().session.as_ref().unwrap().doc();
        let note = st.uuid(st.alive().find(|o| text(st, *o).as_deref() == Some("Why")).unwrap()).unwrap();
        let at = rect_of(&h, &format!("workspace.canvas.node.{note}")).center();
        (0..2).for_each(|_| click(&mut h, at));
        key(&mut h, Key::A, Modifiers::COMMAND);
        h.event(Event::Text("Draft".into()));
        key(&mut h, Key::End, Modifiers::NONE);
        stays_open(&mut h, |s| s.canvas.note_edit.as_ref().map(|n| n.text.clone()), "Later", theme);
        let st = h.state().session.as_ref().unwrap().doc();
        assert_eq!(text(st, st.by_uuid(note).unwrap()).as_deref(), Some("Later"), "{theme:?}: the note's text");
        // From the keys as well: Edit ▸ Copy acts on the note, and File ▸ Settings… finds it committed.
        let walk = |h: &mut Harness<'static, Shell>, edit: bool, to: &str| {
            key(h, Key::F10, Modifiers::NONE);
            if edit {
                key(h, Key::ArrowRight, Modifiers::NONE);
            }
            while h.state().menu_bar.row != Some(to) {
                key(h, Key::ArrowDown, Modifiers::NONE);
            }
            let enter = |pressed| Event::Key { key: Key::Enter, physical_key: None, pressed, repeat: false, modifiers: Modifiers::NONE };
            h.event(enter(true));
            let copied = (0..4).any(|_| (h.step(), copied(h, "Later")).1);
            h.event(enter(false));
            copied
        };
        (0..2).for_each(|_| click(&mut h, at));
        key(&mut h, Key::A, Modifiers::COMMAND);
        assert!(walk(&mut h, true, COPY) && h.state().canvas.note_edit.is_some(), "{theme:?}: Copy from the keys copied the note");
        key(&mut h, Key::End, Modifiers::NONE);
        h.event(Event::Text("!".into()));
        walk(&mut h, false, actions::SETTINGS);
        let st = h.state().session.as_ref().unwrap().doc();
        assert_eq!(
            (h.state().canvas.note_edit.is_none(), text(st, st.by_uuid(note).unwrap())),
            (true, Some("Later!".into())),
            "{theme:?}: committed"
        );

        let mut h = workspace_harness(theme, egui::vec2(1600.0, 1000.0));
        crate::uml_ui::open_sample(h.state_mut()).unwrap();
        h.state_mut().in_window_menu = true;
        let st = h.state().session.as_ref().unwrap().doc();
        let catalog = st
            .uuid(st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_uml::CLASS) && archforge_core::name_of(st, *o) == "Catalog").unwrap())
            .unwrap();
        for ty in [false, true] {
            h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [catalog.to_string()] })).unwrap();
            h.run();
            let at = rect_of(&h, "workspace.canvas.class.add-attribute").center();
            click(&mut h, at);
            h.event(Event::Text(if ty { "rating" } else { "Draft" }.into()));
            h.run();
            if ty {
                key(&mut h, Key::Tab, Modifiers::NONE);
                h.event(Event::Text("Draft".into()));
                h.run();
                stays_open(&mut h, |s| s.canvas.inline.as_ref().map(|i| i.ty.clone()), "Real", theme);
            } else {
                stays_open(&mut h, |s| s.canvas.inline.as_ref().map(|i| i.edit.name.clone()), "Later", theme);
            }
        }
        let rows = member_rows(&h, catalog, false);
        assert!(rows.iter().any(|r| r == "+ Later") && rows.iter().any(|r| r == "+ rating : Real"), "{theme:?}: {rows:?}");
    }
}

/// A saved project with Customer's name being typed as "Item" in Models, or on the canvas when
/// `on_canvas`, the in-window menu shown.
fn renaming_a_saved_project(theme: ThemeKind, on_canvas: bool) -> Harness<'static, Shell> {
    let (mut h, _) = drawn(theme);
    (h.state_mut().in_window_menu, h.state_mut().explorer.focused) = (true, !on_canvas);
    h.state_mut().session.as_mut().unwrap().mark_saved().unwrap();
    h.run();
    assert!(!h.state().tab_unsaved(h.state().active.unwrap()), "{theme:?}: nothing unsaved yet");
    if on_canvas {
        // The header's right end, clear of the note inside the box.
        let node = node_of(&h, archforge_mof::Uuid::parse(&selection(&h)[0]).unwrap());
        let at = rect_of(&h, &format!("workspace.canvas.node.{node}")).right_top() + egui::vec2(-20.0, 12.0);
        (0..2).for_each(|_| click(&mut h, at));
    } else {
        key(&mut h, Key::F2, Modifiers::NONE);
    }
    key(&mut h, Key::A, Modifiers::COMMAND);
    h.event(Event::Text("Item".into()));
    h.run();
    let typed = if on_canvas { h.state().canvas.rename.as_ref().map(|r| &r.text) } else { h.state().explorer.rename.as_ref().map(|r| &r.text) };
    assert_eq!(typed.map(String::as_str), Some("Item"), "{theme:?}: typed in place");
    h
}

/// File ▸ Close, chosen this frame, finds the name in the model, and once it runs asks about the
/// unsaved change and keeps the project open.
fn close_asks(mut h: Harness<'static, Shell>, theme: ThemeKind) {
    h.step();
    let st = h.state().session.as_ref().unwrap().doc();
    assert_eq!(h.state().menu_bar.chosen, Some(actions::CLOSE), "{theme:?}: Close waits for the next frame");
    assert!(st.alive().any(|o| archforge_core::name_of(st, o) == "Item"), "{theme:?}: the name is in the model before it runs");
    let tabs = h.state().tabs.len();
    h.run();
    assert!(matches!(h.state().ask, Some(crate::close_ask::Ask::Unsaved { .. })), "{theme:?}: Close asks about the unsaved name");
    assert_eq!((h.state().tabs.len(), h.state().session.is_some()), (tabs, true), "{theme:?}: the project is still open");
}

/// A name typed in place, in Models and on the canvas, is committed before a row of another menu
/// runs, so File ▸ Close chosen by the pointer on a saved project asks about it, as it would with no
/// menu: whether File is clicked straight away, or after Edit held the name, or after a press on
/// Edit let go on File opened nothing. Each press and release is a frame of its own.
#[test]
fn another_menus_row_finds_the_name_typed_in_place_committed() {
    let file = ["strip.menu.file", "strip.menu.file"];
    let ways: [&[&str]; 3] = [&file, &["strip.menu.edit", "strip.menu.edit", file[0], file[1]], &["strip.menu.edit", file[0], file[0], file[1]]];
    for (theme, on_canvas) in [(ThemeKind::Light, false), (ThemeKind::Dark, false), (ThemeKind::Light, true), (ThemeKind::Dark, true)] {
        for way in ways {
            let mut h = renaming_a_saved_project(theme, on_canvas);
            let presses: Vec<&str> = way.iter().copied().chain(["strip.menu.row.file.close"; 2]).collect();
            for (n, id) in presses.iter().enumerate() {
                let at = rect_of(&h, id).center();
                h.event(Event::PointerMoved(at));
                h.run();
                h.event(Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: n % 2 == 0, modifiers: Modifiers::NONE });
                // The release on Close is left for `close_asks` to draw.
                if n + 1 < presses.len() {
                    h.run();
                }
            }
            close_asks(h, theme);
        }
    }
}

/// From the keys as well: F10 and the arrows reach Edit with the name still open, and File ▸ Close
/// chosen with Enter finds it in the model and asks about it.
#[test]
fn a_row_chosen_from_the_keys_finds_the_name_typed_in_place_committed() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let mut h = renaming_a_saved_project(theme, false);
        key(&mut h, Key::F10, Modifiers::NONE);
        key(&mut h, Key::ArrowRight, Modifiers::NONE);
        assert_eq!((h.state().menu_bar.open, h.state().explorer.rename.as_ref().map(|r| r.text.as_str())), (Some("edit"), Some("Item")), "{theme:?}");
        key(&mut h, Key::ArrowLeft, Modifiers::NONE);
        while h.state().menu_bar.row != Some(actions::CLOSE) {
            key(&mut h, Key::ArrowDown, Modifiers::NONE);
        }
        h.event(Event::Key { key: Key::Enter, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE });
        close_asks(h, theme);
    }
}

/// Delete after Select all takes the boxes off the diagram and deletes nothing from the model,
/// though the selection holds a relationship or a generalization set beside the boxes.
#[test]
fn delete_after_select_all_deletes_no_line_or_set_from_the_model() {
    let alive = |h: &Harness<'static, Shell>, id: &str| {
        let st = h.state().session.as_ref().unwrap().doc();
        st.by_uuid(archforge_mof::Uuid::parse(id).unwrap()).is_some_and(|o| st.is_alive(o))
    };
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, all) = drawn(theme);
        h.state_mut().canvas.focused = true;
        key(&mut h, Key::A, Modifiers::COMMAND);
        assert_eq!(selection(&h), all);
        key(&mut h, Key::Delete, Modifiers::NONE);
        let s = h.state().session.as_ref().unwrap();
        let count = |class| s.doc().alive().filter(|o| s.doc().class_of(*o).ok() == Some(class)).count();
        assert_eq!((count(archforge_er::ENTITY), count(archforge_er::RELATIONSHIP)), (3, 1), "nothing left the model");
        let d = s.view().open_diagram.unwrap();
        assert!(s.doc().list(d, archforge_diagram::diagram::NODE).unwrap().is_empty(), "the boxes are off the diagram");
        let (mut h, _) = related_sample_in(theme, egui::vec2(1600.0, 1000.0));
        let (gens, payment) = crate::uml_ui::put_payment_kinds(h.state_mut()).unwrap();
        let s = h.state_mut().session.as_mut().unwrap();
        let set = s.execute("uml.generalization-set.add", &serde_json::json!({ "generalizations": gens, "name": "method" })).unwrap().value["id"]
            .as_str()
            .unwrap()
            .to_string();
        s.execute("view.select", &serde_json::json!({ "ids": [payment, set] })).unwrap();
        h.state_mut().canvas.focused = true;
        h.run();
        key(&mut h, Key::Delete, Modifiers::NONE);
        assert!(alive(&h, &set), "the set stays in the model");
    }
}
