//! The MVP's end-to-end tests: the real shell over the real project host, driven through
//! its controls as a user would, for each notation. UML: a new project from Home; a class diagram; classes placed with the
//! Class tool and named in place; an attribute and an operation added in Properties; one line of
//! each relationship kind drawn with the Relationship tool; undo and redo from the keyboard; Save;
//! the project closed and opened again from Home's recent list, its model and layout as they were.
//! ER: a new project; an ER diagram with its schema; two tables placed with the Entity tool; their
//! columns and primary keys in Properties; a one-to-many relationship chosen in the tool's popover
//! and drawn, its foreign key added; undo and redo; saved, closed, reopened and compared.
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

/// The real shell over the real project host, in a folder of its own named after `journey`, on
/// Home: the harness, the host and the folder to remove after.
fn start(journey: &str) -> (H, std::sync::Arc<crate::projects::Projects>, std::path::PathBuf) {
    archforge_ui_egui::i18n::set_language("en");
    let base = std::env::temp_dir().join(format!("af-acceptance-{journey}-{}", std::process::id()));
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
    (h, host, base)
}

/// A new project named `name`, made from Home's start card, open and its model ready.
fn new_project(h: &mut H, name: &str) {
    click_on(h, "home.start.new-project");
    h.state_mut().new_project.as_mut().expect("the dialog opens").name = name.into();
    h.run();
    click_on(h, "new-project.create");
    wait(h, "the project opening", |s| s.session.is_some());
}

/// Save, close (nothing asks, since nothing is unsaved), and open the project again from Home's
/// recent list; gives the rows saved, to compare with what comes back.
fn save_close_reopen(h: &mut H) -> archforge_mof::rows::Rows {
    key(h, egui::Key::S, egui::Modifiers::COMMAND);
    wait(h, "the save", |s| s.session.as_ref().is_some_and(|x| !x.is_dirty()) && !s.is_saving());
    let saved = rows(h);
    click_on(h, "workspace.header.project.0.close");
    assert!(h.state().ask.is_none(), "a saved project closes without asking");
    wait(h, "the closing", |s| s.tabs.is_empty());
    click_on(h, "home.recent.0");
    wait(h, "the opening's answer", |s| !s.is_opening());
    assert!(h.state().session_error.is_none(), "{:?}", h.state().session_error);
    wait(h, "the opening again", |s| s.session.is_some());
    saved
}

#[test]
fn a_user_models_a_class_diagram_saves_it_closes_it_and_opens_it_again_as_it_was() {
    let (mut h, host, base) = start("uml");

    // 1. A new project from Home.
    new_project(&mut h, "Shop");
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

    // 7 and 8. Saved, closed and opened again from Home's recent list: the model and its layout
    // as they were, saved.
    let saved = save_close_reopen(&mut h);
    // The project hands rows back in id order, not the order they were made in: each table is
    // compared as the set of its rows.
    assert_eq!(sorted(rows(&h)), sorted(saved), "the model and its diagram come back exactly");
    assert!(!h.state().session.as_ref().unwrap().is_dirty());
    assert_eq!(h.state().title(), "Shop — Archforge");

    host.shutdown();
    let _ = std::fs::remove_dir_all(&base);
}

/// The objects of class `class` in the open model.
fn count(h: &H, class: archforge_mof::ClassId) -> usize {
    let st = h.state().session.as_ref().unwrap().doc();
    st.alive().filter(|o| st.class_of(*o).ok() == Some(class)).count()
}

/// A column added to the selected table in Properties: its name, its type and whether it is the
/// table's primary key, then Apply.
fn add_column(h: &mut H, name: &str, ty: &str, primary_key: bool) {
    click_on(h, "workspace.properties.columns.add");
    let edit = h.state_mut().panels.column.as_mut().unwrap_or_else(|| panic!("{name}: the column editor opens"));
    edit.name = name.into();
    edit.ty = ty.into();
    edit.primary_key = primary_key;
    edit.not_null = primary_key;
    h.run();
    click_on(h, "workspace.properties.column.apply");
    assert!(h.state().panels.column.is_none(), "{name}: Apply writes the column and closes the editor: {:?}", h.state().panels.notice);
}

#[test]
fn a_user_models_an_er_diagram_saves_it_closes_it_and_opens_it_again_as_it_was() {
    let (mut h, host, base) = start("er");

    // 1. A new project from Home.
    new_project(&mut h, "Library");

    // 2. An ER diagram, its schema made with it.
    click_on(&mut h, "workspace.tabs.add");
    h.state_mut().diagrams.new.as_mut().expect("New diagram opens").name = "Library schema".into();
    h.run();
    click_on(&mut h, "workspace.new-diagram.create");
    assert!(h.state().diagrams.new.is_none());
    assert_eq!(count(&h, archforge_er::SCHEMA), 1, "the diagram's schema is made with it");

    // 3. Two tables placed with the Entity tool and named in place.
    let canvas = rect_of(&h, "workspace.canvas");
    let c = canvas.center();
    for (name, dx) in [("Author", -260.0), ("Book", 260.0)] {
        click_on(&mut h, "workspace.tools.entity");
        click(&mut h, c + egui::vec2(dx, -60.0));
        h.state_mut().canvas.rename.as_mut().unwrap_or_else(|| panic!("{name} is named in place")).text = name.into();
        key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
    }
    assert_eq!(count(&h, archforge_er::ENTITY), 2);

    // 4. Columns in Properties, each table's first one its primary key.
    h.state_mut().state.show_panel(archforge_ui_egui::state::Panel::Properties);
    for (table, columns) in
        [("Author", [("author_id", "BIGINT", true), ("name", "TEXT", false)]), ("Book", [("book_id", "BIGINT", true), ("title", "TEXT", false)])]
    {
        let node = node_of(&h, table);
        click_on(&mut h, &node);
        for (name, ty, primary_key) in columns {
            add_column(&mut h, name, ty, primary_key);
        }
    }
    assert_eq!(count(&h, archforge_er::ATTRIBUTE), 4);
    assert_eq!(count(&h, archforge_er::PRIMARY_KEY), 2, "each table has its primary key");

    // 5. One to many, chosen in the Relationship tool's popover, drawn from Author to Book: the
    // line, and the foreign key with its column that it adds to Book.
    click(&mut h, canvas.left_top() + egui::vec2(40.0, 40.0));
    click_on(&mut h, "workspace.tools.relationship-types");
    click_on(&mut h, "workspace.cardinality.preset.0");
    key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
    click_on(&mut h, "workspace.tools.relationship");
    let (author, book) = (rect_of(&h, &node_of(&h, "Author")).center(), rect_of(&h, &node_of(&h, "Book")).center());
    drag(&mut h, author, book);
    assert_eq!(edges(&h), 1, "the relationship's line: {:?}", h.state().canvas.notice);
    assert_eq!(count(&h, archforge_er::RELATIONSHIP), 1);
    assert_eq!(count(&h, archforge_er::FOREIGN_KEY), 1, "drawing it adds Book's foreign key");
    assert_eq!(count(&h, archforge_er::ATTRIBUTE), 5, "and the column it refers to Author with");

    // 6. Undo and redo from the keyboard: the relationship, its key and column go and come back.
    click(&mut h, canvas.left_top() + egui::vec2(40.0, 40.0));
    let cmd = egui::Modifiers::COMMAND;
    key(&mut h, egui::Key::Z, cmd);
    assert_eq!((edges(&h), count(&h, archforge_er::FOREIGN_KEY), count(&h, archforge_er::ATTRIBUTE)), (0, 0, 4));
    key(&mut h, egui::Key::Z, cmd | egui::Modifiers::SHIFT);
    assert_eq!((edges(&h), count(&h, archforge_er::FOREIGN_KEY), count(&h, archforge_er::ATTRIBUTE)), (1, 1, 5));

    // 7 and 8. Saved, closed and opened again from Home's recent list: the model and its layout
    // as they were, saved.
    let saved = save_close_reopen(&mut h);
    assert_eq!(sorted(rows(&h)), sorted(saved), "the model and its diagram come back exactly");
    assert!(!h.state().session.as_ref().unwrap().is_dirty());
    assert_eq!(h.state().title(), "Library — Archforge");

    host.shutdown();
    let _ = std::fs::remove_dir_all(&base);
}

/// The place of a built-in type in the type menu of a project with no types of its own: None
/// first, then the built-in types in their order.
fn archforge_ui_egui_type_index(_h: &H, label: &str) -> usize {
    ["None", "Integer", "String", "Boolean", "Real", "UnlimitedNatural"].iter().position(|l| *l == label).expect("a built-in type")
}
