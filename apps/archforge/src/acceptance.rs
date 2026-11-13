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

use crate::test_host::Sandbox;

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

/// The longest run of frames the window may ask for at once, each right after the last, before
/// the test calls it a window that never stops drawing. One answer from the host asks for one or
/// two such frames.
const AT_ONCE: usize = 30;

/// The delay the window asked for before its next frame; `Duration::MAX` when it asked for none.
fn asked(h: &H) -> Duration {
    h.output().viewport_output.get(&egui::ViewportId::ROOT).map_or(Duration::MAX, |v| v.repaint_delay)
}

/// Waits until `done` holds, the host answering on its own threads, drawing only the frames the
/// window asks for, as the real window does: the next one once the delay it asked for has passed,
/// never one it did not ask for. A window that asks for no frame while `done` does not hold yet
/// would wait for the pointer to move, so the test fails there, naming the step. It draws one frame
/// at a time, not a harness `run`, whose few steps a chain of the host's answers can outlast on a
/// loaded machine; a window that asks for more than `AT_ONCE` frames in a row at once never stops
/// drawing and fails the test. Past a minute it fails too.
fn wait(h: &mut H, what: &str, done: impl Fn(&H) -> bool) {
    let started = Instant::now();
    let mut at_once = 0;
    while !done(h) {
        assert!(started.elapsed() < Duration::from_secs(60), "{what} did not happen in time");
        let delay = asked(h);
        assert!(delay != Duration::MAX, "{what}: the window asked for no frame and would wait for the pointer to move");
        at_once = if delay.is_zero() { at_once + 1 } else { 0 };
        assert!(at_once <= AT_ONCE, "{what}: the window asks for a frame at once on every frame and never stops drawing");
        std::thread::sleep(delay);
        h.step();
    }
}

/// Draws the frames the window asks for until it asks for none, so the reads it starts on its own
/// after an opening or a save (the versions, then the branches) have answered before the next
/// click. An answer that lands among a click's frames asks for frames of its own, and on a loaded
/// machine took a click past the harness's step limit.
fn settle(h: &mut H, what: &str) {
    wait(h, what, |h| asked(h) == Duration::MAX);
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
/// Home: the harness, and the host's guard, which stops the host and removes the folder when the
/// test ends, failed or not.
fn start(journey: &str) -> (H, Sandbox) {
    archforge_ui_egui::i18n::set_language("en");
    let sandbox = Sandbox::named(&format!("acceptance-{journey}"), None);
    let folder = sandbox.base.join("Documents/Archforge");
    std::fs::create_dir_all(&folder).unwrap();
    let services = Services {
        projects: Some(sandbox.host.clone()),
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
    (h, sandbox)
}

/// A new project named `name`, made from Home's start card, open and its model ready.
fn new_project(h: &mut H, name: &str) {
    click_on(h, "home.start.new-project");
    h.state_mut().new_project.as_mut().expect("the dialog opens").name = name.into();
    h.run();
    click_on(h, "new-project.create");
    wait(h, "the project opening", |h| h.state().session.is_some());
    settle(h, "the opened project's reads");
}

/// Save, close (nothing asks, since nothing is unsaved), and open the project again from Home's
/// recent list; gives the rows saved, to compare with what comes back.
fn save_close_reopen(h: &mut H) -> archforge_mof::rows::Rows {
    key(h, egui::Key::S, egui::Modifiers::COMMAND);
    wait(h, "the save", |h| h.state().session.as_ref().is_some_and(|x| !x.is_dirty()) && !h.state().is_saving());
    settle(h, "the saved project's reads");
    let saved = rows(h);
    click_on(h, "workspace.header.project.0.close");
    assert!(h.state().ask.is_none(), "a saved project closes without asking");
    wait(h, "the closing", |h| h.state().tabs.is_empty());
    click_on(h, "home.recent.0");
    wait(h, "the opening's answer", |h| !h.state().is_opening());
    assert!(h.state().session_error.is_none(), "{:?}", h.state().session_error);
    wait(h, "the opening again", |h| h.state().session.is_some());
    saved
}

/// A user models a class diagram through the real shell, saves, closes and reopens it unchanged.
#[test]
fn a_user_models_a_class_diagram_saves_it_closes_it_and_opens_it_again_as_it_was() {
    let (mut h, _sandbox) = start("uml");

    // 1. A new project from Home.
    new_project(&mut h, "Shop");
    assert_eq!(h.state().title(), "Shop — Archforge");

    // 2. A class diagram, its package made with it.
    click_on(&mut h, "workspace.tabs.add");
    click_on(&mut h, "workspace.new-diagram.type.uml.class");
    click_on(&mut h, "workspace.new-diagram.continue");
    h.state_mut().diagrams.new.as_mut().expect("New diagram opens").name = "Domain".into();
    h.run();
    click_on(&mut h, "workspace.new-diagram.create");
    assert!(h.state().diagrams.new.is_none());

    // 3. Five classes placed with the classifier tool, at its first kind, Class, and named in place.
    let canvas = rect_of(&h, "workspace.canvas");
    let c = canvas.center();
    for (name, dx, dy) in
        [("Order", -280.0, -170.0), ("Customer", 280.0, -170.0), ("Line", -280.0, 150.0), ("Payment", 280.0, 150.0), ("Card", 0.0, -40.0)]
    {
        click_on(&mut h, "workspace.tools.classifier");
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

/// A user models an ER diagram through the real shell, saves, closes and reopens it unchanged.
#[test]
fn a_user_models_an_er_diagram_saves_it_closes_it_and_opens_it_again_as_it_was() {
    let (mut h, _sandbox) = start("er");

    // 1. A new project from Home.
    new_project(&mut h, "Library");

    // 2. An ER diagram, its schema made with it.
    click_on(&mut h, "workspace.tabs.add");
    click_on(&mut h, "workspace.new-diagram.continue");
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
}

/// A model the writer can't turn into rows is a write that failed, never lost silently: an agent's
/// save says why, and closing with Save asks before closing without writing. No command makes
/// such a model, so the test sets one in place.
#[test]
fn a_model_that_cannot_be_turned_into_rows_is_neither_saved_nor_closed_away() {
    use archforge_ui_egui::{ProjectHost, close_ask::Ask, control::answer};
    let (mut h, sandbox) = start("unbuildable");
    let host = &sandbox.host;
    new_project(&mut h, "Shop");
    let path = h.state().tabs[0].path.clone();
    let too_large = |st: &mut archforge_mof::Store| {
        let u = st.universe().clone();
        let class = u.classes().find(|c| u.class_name(*c) == "LiteralUnlimitedNatural").unwrap();
        let value = u.info(class).unwrap().stored.iter().copied().find(|p| u.prop(*p).is_some_and(|x| &*x.name == "value")).unwrap();
        let o = st.create(class).unwrap();
        st.set(o, value, archforge_mof::Value::Unlimited(Some(u64::MAX))).unwrap();
    };
    let mut session = archforge_notations::session_from_rows_repaired(&rows(&h), Box::new(crate::services::fresh_id), too_large).unwrap();
    session.execute("er.schema.add", &serde_json::json!({ "name": "shop" })).unwrap();
    h.state_mut().session = Some(session);
    h.step();
    wait(&mut h, "the write that fails", |_| host.writing(&path).error.is_some());
    let ticket = answer(h.state_mut(), "app.save", &serde_json::json!({})).unwrap()["ticket"].clone();
    h.step();
    wait(&mut h, "the save", |h| !h.state().is_saving());
    let failed = answer(h.state_mut(), "app.saved", &serde_json::json!({ "ticket": ticket })).unwrap_err();
    assert!(failed.contains("too large to store"), "{failed}");
    settle(&mut h, "the failed save's reads");

    click_on(&mut h, "workspace.header.project.0.close");
    assert!(matches!(h.state().ask, Some(Ask::Unsaved { .. })), "{:?}", h.state().ask);
    // Pressed without a harness run: the save's answer and the reads after it would outlast one.
    let at = rect_of(&h, "close-ask.save").center();
    h.event(egui::Event::PointerMoved(at));
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE });
    }
    h.step();
    wait(&mut h, "the question after the save", |h| matches!(&h.state().ask, Some(Ask::CannotWrite { why, .. }) if why.contains("too large")));
    assert_eq!(h.state().tabs.len(), 1, "still open");
}

/// The rows of History, top first.
fn version_rows(s: &Shell) -> Vec<String> {
    let mut rows: Vec<_> = s.widgets().iter().filter(|w| w.id.starts_with("workspace.versions.row.") && w.id.matches('.').count() == 3).collect();
    rows.sort_by(|a, b| a.rect[1].total_cmp(&b.rect[1]));
    rows.into_iter().map(|w| w.id.clone()).collect()
}

/// Opens the details of the version `row` and Compare… there, and waits for what it finds: the
/// cards of the differences.
fn compared(h: &mut H, row: &str) -> usize {
    wait(h, "the version listed", |h| h.state().widgets().iter().any(|w| w.id == row));
    click_on(h, row);
    wait(h, "the details", |h| h.state().widgets().iter().any(|w| w.id == "workspace.version-details.compare"));
    click_on(h, "workspace.version-details.compare");
    wait(h, "the differences counted", |h| h.state().widgets().iter().any(|w| w.id == "workspace.version-compare.count"));
    h.state().widgets().iter().filter(|w| w.id.starts_with("workspace.version-compare.change.")).count()
}

/// A user saves two versions, then compares the older with current work, an unsaved change on top,
/// first from current work and then while the newer version is shown: the work set aside is what
/// it is compared with, not the version in view. Then the older with the newer, both read at their
/// revisions.
#[test]
fn a_user_compares_a_version_with_current_work_also_while_another_is_shown() {
    let (mut h, _sandbox) = start("compare");
    new_project(&mut h, "Shop");
    for schema in ["sales", "billing", "returns"] {
        h.state_mut().session.as_mut().unwrap().execute("er.schema.add", &serde_json::json!({ "name": schema })).unwrap();
        h.run();
        if schema != "returns" {
            key(&mut h, egui::Key::S, egui::Modifiers::COMMAND);
            wait(&mut h, "the version saved", |h| !h.state().is_saving() && h.state().session.as_ref().is_some_and(|x| !x.is_dirty()));
        }
    }
    click_on(&mut h, "workspace.explorers.source-control");
    wait(&mut h, "both versions listed", |h| version_rows(h.state()).len() == 2);
    let [newer, older] = <[String; 2]>::try_from(version_rows(h.state())).unwrap();
    assert_eq!(compared(&mut h, &older), 2, "billing and returns");
    click_on(&mut h, "workspace.version-compare.close");
    click_on(&mut h, &newer);
    click_on(&mut h, "workspace.version-details.view");
    wait(&mut h, "the newer version shown", |h| h.state().viewing().is_some());
    assert_eq!(compared(&mut h, &older), 2, "against the version in view, only billing");
    click_on(&mut h, "workspace.version-compare.after");
    click_on(&mut h, "workspace.version-compare.list.row.1");
    let changes = |h: &Shell| h.widgets().iter().filter(|w| w.id.starts_with("workspace.version-compare.change.")).count();
    wait(&mut h, "the newer version compared", |h| changes(h.state()) == 1);
}

/// The place of a built-in type in the type menu of a project with no types of its own: None
/// first, then the built-in types in their order.
fn archforge_ui_egui_type_index(_h: &H, label: &str) -> usize {
    ["None", "Integer", "String", "Boolean", "Real", "UnlimitedNatural"].iter().position(|l| *l == label).expect("a built-in type")
}

/// A secure store the test can look into after the host has used it, counting its reads.
struct Shared(std::sync::Arc<archforge_secrets::MemoryStore>, std::sync::Arc<std::sync::atomic::AtomicUsize>);

impl archforge_secrets::SecretStore for Shared {
    fn get(&self, name: &archforge_secrets::SecretName) -> Result<archforge_secrets::Secret, archforge_secrets::SecretError> {
        self.1.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.0.get(name)
    }
    fn set(&self, name: &archforge_secrets::SecretName, secret: &archforge_secrets::Secret) -> Result<(), archforge_secrets::SecretError> {
        self.0.set(name, secret)
    }
    fn delete(&self, name: &archforge_secrets::SecretName) -> Result<archforge_secrets::Deleted, archforge_secrets::SecretError> {
        self.0.delete(name)
    }
}

/// Draws frames until `done` holds, as the window does while its host works; fails past a budget.
fn until(h: &mut H, what: &str, done: impl Fn(&Shell) -> bool) {
    let start = Instant::now();
    while !done(h.state()) {
        assert!(start.elapsed() < Duration::from_secs(20), "waited too long for {what}");
        std::thread::sleep(Duration::from_millis(10));
        h.run();
    }
}

/// Settings → AI Agents over the real agents host, its secure store an in-memory one and no program
/// on its PATH: a key typed into the page reaches the store whole, the page shows its last four,
/// it is replaced and removed there, and the preferences file never holds it.
#[test]
fn an_api_key_goes_to_the_secure_store_and_never_into_the_preferences() {
    use archforge_secrets::SecretStore;
    const CANARY: &str = "sk-ant-api03-CANARY-7a1e9f-do-not-store-4F2a";
    archforge_ui_egui::i18n::set_language("en");
    let base = std::env::temp_dir().join(format!("archforge-agents-acceptance-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    let store = std::sync::Arc::new(archforge_secrets::MemoryStore::new());
    let reads = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let host = crate::agents::Agents::with(&base, Box::new(Shared(store.clone(), reads.clone())), archforge_agents::HostEnv::default());
    let services = Services { agents: Some(std::sync::Arc::new(host)), ..Services::default() };
    let mut h = Harness::builder().with_size(egui::vec2(1600.0, 1400.0)).build_ui_state(
        |ui, shell: &mut Shell| {
            shell.logic(ui.ctx());
            shell.ui(ui);
        },
        Shell::new(services),
    );
    h.run();
    until(&mut h, "the look at start", |s| !s.agents.looking() && s.agents.look.claude != archforge_ui_egui::ai_agents::Program::NotLooked);
    assert_eq!(h.state().agents.look.claude, archforge_ui_egui::ai_agents::Program::Missing, "no program on an empty PATH");
    // Launching and Settings' Profile read no key: the store is first read by AI Agents.
    click_on(&mut h, "home.header.account");
    assert_eq!(reads.load(std::sync::atomic::Ordering::Relaxed), 0, "no key is read at launch");
    click_on(&mut h, "settings.nav.agents");
    until(&mut h, "the keys read", |s| s.agents.keys_known());
    assert_eq!(reads.load(std::sync::atomic::Ordering::Relaxed), 2, "both keys read once, when the page needed them");
    // Claude's program is missing here, so its card offers the install steps; Codex on an API key
    // offers the key field.
    h.state_mut().state.agents.codex_sign_in = archforge_ui_egui::ai_agents::CodexSignIn::ApiKey;
    h.state_mut().agents.look.codex = archforge_ui_egui::ai_agents::Program::Found { version: "0.166.0".into(), tested: true };
    h.run();
    click_on(&mut h, "settings.agents.codex.key");
    h.event(egui::Event::Text(CANARY.into()));
    h.run();
    click_on(&mut h, "settings.agents.codex.save");
    until(&mut h, "the key saved", |s| !s.agents.saving(archforge_ui_egui::ai_agents::Provider::Codex));
    assert_eq!(store.get(&archforge_secrets::SecretName::OPENAI_API_KEY).unwrap().expose(), CANARY, "the store holds the key whole");
    assert_eq!(h.state().agents.look.codex_key, archforge_ui_egui::ai_agents::Key::Saved { last4: "4F2a".into() });
    // The preferences, captured and written as the app writes them, hold no part of the key.
    crate::prefs::save(&base, &crate::prefs::capture(h.state())).unwrap();
    let written = std::fs::read_to_string(base.join(crate::prefs::FILE)).unwrap();
    assert!(written.contains("codex_sign_in"), "the agents' choices are in the file");
    for part in [CANARY, "CANARY", "7a1e9f", "4F2a"] {
        assert!(!written.contains(part), "the preferences hold {part:?}");
    }
    // Replace, then Remove.
    click_on(&mut h, "settings.agents.codex.replace");
    h.event(egui::Event::Text("sk-proj-second-key-0000-9c1E".into()));
    h.run();
    click_on(&mut h, "settings.agents.codex.save");
    until(&mut h, "the key replaced", |s| !s.agents.saving(archforge_ui_egui::ai_agents::Provider::Codex));
    assert_eq!(store.get(&archforge_secrets::SecretName::OPENAI_API_KEY).unwrap().expose(), "sk-proj-second-key-0000-9c1E");
    click_on(&mut h, "settings.agents.codex.remove");
    until(&mut h, "the key removed", |s| !s.agents.saving(archforge_ui_egui::ai_agents::Provider::Codex));
    assert!(matches!(store.get(&archforge_secrets::SecretName::OPENAI_API_KEY), Err(archforge_secrets::SecretError::NotFound { .. })));
    assert_eq!(h.state().agents.look.codex_key, archforge_ui_egui::ai_agents::Key::Absent);
    let _ = std::fs::remove_dir_all(&base);
}

/// On a system without a secure store the host says so, and a key handed to it anyway is refused,
/// never kept elsewhere.
#[test]
fn without_a_secure_store_the_host_keeps_no_key() {
    use archforge_ui_egui::ai_agents::{AgentHost, KeyText, Provider};
    let base = std::env::temp_dir().join(format!("archforge-agents-nostore-{}", std::process::id()));
    let host = crate::agents::Agents::with(&base, Box::new(archforge_secrets::MemoryStore::unavailable()), archforge_agents::HostEnv::default());
    let keys = host.read_keys().recv().unwrap();
    assert!(keys.store.missing(), "{keys:?}");
    let refused = host.save_key(Provider::Claude, KeyText::new("sk-ant-api03-canary-0000-4F2a".into()).unwrap()).recv().unwrap();
    assert!(refused.unwrap_err().contains("no secure credential store"));
    let _ = std::fs::remove_dir_all(&base);
}
