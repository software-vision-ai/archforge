//! The MVP's end-to-end test: the real shell over the real project host, driven through
//! its controls as a user would. A new project from Home; a class diagram; classes placed with the
//! Class tool and named in place; an attribute and an operation added in Properties; one line of
//! each relationship kind drawn with the Relationship tool; undo and redo from the keyboard; Save;
//! the project closed and opened again from Home's recent list, its model and layout as they were.
//! While the host works on its own threads the test draws only the frames the window asks for, as
//! the real window does, so a window that would wait for the pointer to move fails it.

use std::time::{Duration, Instant};

use archforge_ui_egui::{Folder, Services, Shell};
use egui_kittest::Harness;

type H = Harness<'static, Shell>;

fn rect_of(h: &H, id: &str) -> egui::Rect {
    let w = h.state().widgets().iter().find(|w| w.id == id).unwrap_or_else(|| panic!("no control {id}"));
    egui::Rect::from_min_max(egui::pos2(w.rect[0], w.rect[1]), egui::pos2(w.rect[2], w.rect[3]))
}

fn click(h: &mut H, at: egui::Pos2) {
    h.event(egui::Event::PointerMoved(at));
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE });
    }
    h.run();
}

fn click_on(h: &mut H, id: &str) {
    let at = rect_of(h, id).center();
    click(h, at);
}

fn drag(h: &mut H, from: egui::Pos2, to: egui::Pos2) {
    h.event(egui::Event::PointerMoved(from));
    h.event(egui::Event::PointerButton { pos: from, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE });
    h.run();
    h.event(egui::Event::PointerMoved(to));
    h.run();
    h.event(egui::Event::PointerButton { pos: to, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE });
    h.run();
}

fn key(h: &mut H, key: egui::Key, modifiers: egui::Modifiers) {
    h.key_down_modifiers(modifiers, key);
    h.key_up_modifiers(modifiers, key);
    h.run();
}

fn type_text(h: &mut H, text: &str) {
    h.event(egui::Event::Text(text.into()));
    h.run();
}

/// Waits until `done` holds, the host answering on its own threads, drawing only the frames the
/// window asks for, as the real window does: the next one once the delay it asked for has passed,
/// never one it did not ask for. A window that asks for no frame while `done` does not hold yet
/// would wait for the pointer to move, so the test fails there, naming the step. Past a minute it
/// fails too.
fn wait(h: &mut H, what: &str, done: impl Fn(&Shell) -> bool) {
    let started = Instant::now();
    while !done(h.state()) {
        assert!(started.elapsed() < Duration::from_secs(60), "{what} did not happen in time");
        let delay = h.output().viewport_output.get(&egui::ViewportId::ROOT).map_or(Duration::MAX, |v| v.repaint_delay);
        assert!(delay != Duration::MAX, "{what}: the window asked for no frame and would wait for the pointer to move");
        std::thread::sleep(delay);
        h.run();
    }
}

/// The open model's rows, as saving writes them: every element and every diagram's layout.
fn rows(h: &H) -> archforge_mof::rows::Rows {
    let (tables, junctions) = archforge_notations::storage();
    archforge_mof::rows::snapshot(h.state().session.as_ref().expect("a model is open").doc(), &tables, &junctions).unwrap()
}

/// `rows` with each table's and each list table's rows in one order, so two snapshots of the same
/// model compare equal whatever order their rows were read in.
fn sorted(mut rows: archforge_mof::rows::Rows) -> archforge_mof::rows::Rows {
    for t in &mut rows.tables {
        t.sort_by_key(|r| format!("{r:?}"));
    }
    for j in &mut rows.junctions {
        j.sort_by_key(|r| format!("{r:?}"));
    }
    rows
}

/// The node of the class named `name` on the open diagram, as the canvas registers it.
fn node_of(h: &H, name: &str) -> String {
    let s = h.state().session.as_ref().unwrap();
    let st = s.doc();
    let n = st
        .alive()
        .filter(|o| st.class_of(*o).ok() == Some(archforge_diagram::NODE))
        .find(|n| st.get(*n, archforge_diagram::node::ELEMENT).ok().and_then(|v| v.as_ref()).is_some_and(|e| archforge_core::name_of(st, e) == name))
        .unwrap_or_else(|| panic!("no node of {name}"));
    format!("workspace.canvas.node.{}", st.uuid(n).unwrap())
}

fn edges(h: &H) -> usize {
    let st = h.state().session.as_ref().unwrap().doc();
    st.alive().filter(|o| st.class_of(*o).ok() == Some(archforge_diagram::EDGE)).count()
}

#[test]
fn a_user_models_a_class_diagram_saves_it_closes_it_and_opens_it_again_as_it_was() {
    archforge_ui_egui::i18n::set_language("en");
    let base = std::env::temp_dir().join(format!("af-acceptance-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    let host = std::sync::Arc::new(crate::projects::Projects::new(&base.join("settings"), "Archforge acceptance".into()));
    let folder = base.join("Documents/Archforge");
    std::fs::create_dir_all(&folder).unwrap();
    let services = Services {
        projects: Some(host.clone()),
        ids: Some(std::sync::Arc::new(crate::services::fresh_id)),
        projects_folder: Some(Folder { path: folder.clone(), label: "Documents / Archforge".into() }),
        ..Services::default()
    };
    let mut h = Harness::builder().with_size(egui::vec2(1600.0, 1000.0)).build_ui_state(
        |ui, shell: &mut Shell| {
            shell.logic(ui.ctx());
            shell.ui(ui);
        },
        Shell::new(services),
    );
    h.run();

    // 1. A new project from Home.
    click_on(&mut h, "home.start.new-project");
    h.state_mut().new_project.as_mut().expect("the dialog opens").name = "Shop".into();
    h.run();
    click_on(&mut h, "new-project.create");
    wait(&mut h, "the project opening", |s| s.session.is_some());
    assert_eq!(h.state().title(), "Shop — Archforge");

    // 2. A class diagram, its package made with it.
    click_on(&mut h, "workspace.tabs.add");
    click_on(&mut h, "workspace.new-diagram.kind");
    click_on(&mut h, "workspace.new-diagram.kind.1");
    h.state_mut().diagrams.new.as_mut().expect("New diagram opens").name = "Domain".into();
    h.run();
    click_on(&mut h, "workspace.new-diagram.create");
    assert!(h.state().diagrams.new.is_none());

    // 3. Five classes placed with the Class tool and named in place.
    let canvas = rect_of(&h, "workspace.canvas");
    let c = canvas.center();
    for (name, dx, dy) in
        [("Order", -280.0, -170.0), ("Customer", 280.0, -170.0), ("Line", -280.0, 150.0), ("Payment", 280.0, 150.0), ("Card", 0.0, -40.0)]
    {
        click_on(&mut h, "workspace.tools.class");
        click(&mut h, c + egui::vec2(dx, dy));
        h.state_mut().canvas.rename.as_mut().unwrap_or_else(|| panic!("{name} is named in place")).text = name.into();
        key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
    }

    // 4. An attribute and an operation on Order, in Properties.
    let order = node_of(&h, "Order");
    click_on(&mut h, &order);
    h.state_mut().state.show_panel(archforge_ui_egui::state::Panel::Properties);
    h.run();
    click_on(&mut h, "workspace.properties.attributes.add");
    type_text(&mut h, "total");
    click_on(&mut h, "workspace.properties.member.type");
    let real = archforge_ui_egui_type_index(&h, "Real");
    click_on(&mut h, &format!("workspace.properties.member.type.{real}"));
    click_on(&mut h, "workspace.properties.member.apply");
    click_on(&mut h, "workspace.properties.operations.add");
    type_text(&mut h, "place");
    click_on(&mut h, "workspace.properties.member.apply");
    h.run();

    // 5. One line of each relationship kind.
    for (kind, from, to) in [
        ("association", "Customer", "Order"),
        ("directed", "Order", "Payment"),
        ("aggregation", "Customer", "Payment"),
        ("composition", "Order", "Line"),
        ("generalization", "Card", "Payment"),
        ("dependency", "Line", "Payment"),
    ] {
        click_on(&mut h, "workspace.tools.relationship-kinds");
        click_on(&mut h, &format!("workspace.relate.{kind}"));
        click_on(&mut h, "workspace.tools.relationship");
        let (a, b) = (rect_of(&h, &node_of(&h, from)).center(), rect_of(&h, &node_of(&h, to)).center());
        let before = edges(&h);
        drag(&mut h, a, b);
        assert_eq!(edges(&h), before + 1, "{kind} from {from} to {to}: {:?}", h.state().canvas.notice);
    }
    assert_eq!(edges(&h), 6);

    // 6. Undo and redo from the keyboard: the last two lines go and come back.
    click(&mut h, canvas.left_top() + egui::vec2(40.0, 40.0));
    let cmd = egui::Modifiers::COMMAND;
    key(&mut h, egui::Key::Z, cmd);
    key(&mut h, egui::Key::Z, cmd);
    assert_eq!(edges(&h), 4);
    key(&mut h, egui::Key::Z, cmd | egui::Modifiers::SHIFT);
    key(&mut h, egui::Key::Z, cmd | egui::Modifiers::SHIFT);
    assert_eq!(edges(&h), 6);

    // 7. Save, then close: nothing asks, since nothing is unsaved.
    key(&mut h, egui::Key::S, cmd);
    wait(&mut h, "the save", |s| s.session.as_ref().is_some_and(|x| !x.is_dirty()) && !s.is_saving());
    let saved = rows(&h);
    click_on(&mut h, "workspace.header.project.0.close");
    assert!(h.state().ask.is_none(), "a saved project closes without asking");
    wait(&mut h, "the closing", |s| s.tabs.is_empty());

    // 8. Opened again from Home's recent list: the model and its layout as they were, saved.
    click_on(&mut h, "home.recent.0");
    wait(&mut h, "the opening's answer", |s| !s.is_opening());
    assert!(h.state().session_error.is_none(), "{:?}", h.state().session_error);
    wait(&mut h, "the opening again", |s| s.session.is_some());
    // The project hands rows back in id order, not the order they were made in: each table is
    // compared as the set of its rows.
    assert_eq!(sorted(rows(&h)), sorted(saved), "the model and its diagram come back exactly");
    assert!(!h.state().session.as_ref().unwrap().is_dirty());
    assert_eq!(h.state().title(), "Shop — Archforge");

    host.shutdown();
    let _ = std::fs::remove_dir_all(&base);
}

/// The place of a built-in type in the type menu of a project with no types of its own: None
/// first, then the built-in types in their order.
fn archforge_ui_egui_type_index(_h: &H, label: &str) -> usize {
    ["None", "Integer", "String", "Boolean", "Real", "UnlimitedNatural"].iter().position(|l| *l == label).expect("a built-in type")
}
