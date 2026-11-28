//! ER's window tests of the canvas: the Entity and Relationship tools, a line dropped on empty
//! canvas, the cardinality popover and the mapping table offer, a table's menu and its double clicks,
//! tables placed from Models and grown with their columns, the Diagrams explorer's ER diagrams, the
//! problem badges, and Generate.

use super::*;

/// A table box saved too short for its columns opens grown to show them, with no undo step.
#[test]
fn a_table_box_saved_too_short_for_its_columns_opens_grown_without_an_undo_step() {
    // A project written by an earlier version: Account holds two columns in a box that shows none.
    let rows = {
        let (mut h, host) = saving_harness(Some(commerce()), None);
        let s = h.state_mut().session.as_mut().unwrap();
        let id = |v: serde_json::Value| v["id"].as_str().unwrap().to_string();
        let schema = id(s.execute("er.schema.add", &serde_json::json!({ "name": "shop" })).unwrap().value);
        let account = id(s.execute("er.entity.add", &serde_json::json!({ "schema": schema, "name": "Account" })).unwrap().value);
        for name in ["employee_id", "employee_id_2"] {
            s.execute("er.attribute.add", &serde_json::json!({ "entity": account, "name": name, "type": "UUID" })).unwrap();
        }
        let d = id(s
            .execute("diagram.add", &serde_json::json!({ "kind": archforge_er::DIAGRAM_KIND, "element": schema, "name": "Shop" }))
            .unwrap()
            .value);
        s.execute("diagram.place", &serde_json::json!({ "diagram": d, "element": account, "x": 0, "y": 0, "width": 240, "height": 72 })).unwrap();
        h.run_steps(2);
        host.written.lock().unwrap().pop().unwrap().1
    };
    let opened = archforge_ui_egui::Opened { model: Some(std::sync::Arc::new(rows)), ..commerce() };
    let (h, host) = saving_harness(Some(opened), None);
    let session = h.state().session.as_ref().expect("the model read opens");
    let st = session.doc();
    let node = st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_diagram::NODE)).unwrap();
    assert_eq!(archforge_diagram::grid::bounds_of(st, node).height, er_place::content_height(2), "the box shows both columns");
    assert_eq!(session.undo_label(), None, "growing it is no step of the user's");
    assert!(!session.is_dirty(), "nor a change to save");
    assert!(host.written.lock().unwrap().is_empty());
}

/// A connection dropped on empty canvas makes a new table there, joined to the table it came from, in
/// one step.
#[test]
fn a_connection_dropped_on_empty_canvas_makes_a_new_table_joined_to_it_in_one_step() {
    let (mut h, _customer, _order) = two_tables_harness();
    let nodes = node_ids(&h);
    let customer_node = nodes.iter().min_by(|a, b| rect_of(&h, a).left().total_cmp(&rect_of(&h, b).left())).unwrap().clone();
    let tables = |h: &Harness<'static, Shell>| {
        let st = h.state().session.as_ref().unwrap().doc();
        st.alive().filter(|o| st.class_of(*o).ok() == Some(archforge_er::ENTITY)).count()
    };
    let before = (tables(&h), relationships(&h).len(), node_ids(&h).len());
    let at = rect_of(&h, &customer_node).center();
    click(&mut h, at);
    // From the connect handle to a spot well below the tables, where nothing is drawn.
    let from = rect_of(&h, "workspace.canvas.adorn.connect").center();
    let empty = rect_of(&h, &customer_node).center() + egui::vec2(0.0, 320.0);
    drag(&mut h, from, empty);
    h.run_steps(2);
    assert_eq!(
        (tables(&h), relationships(&h).len(), node_ids(&h).len()),
        (before.0 + 1, before.1 + 1, before.2 + 1),
        "{:?}",
        h.state().canvas.notice
    );
    let rename = h.state().canvas.rename.clone().expect("the new table's name is open for typing");
    assert_eq!(rename.text, "Entity");
    let st = h.state().session.as_ref().unwrap().doc();
    let rel = relationships(&h)[0];
    let to = st.get(rel, archforge_er::relationship::TO).unwrap().as_ref().unwrap();
    assert_eq!(st.uuid(to).unwrap(), rename.element, "the line joins the new table");
    assert!(!er_connect::has_foreign_key(st, rel), "the new table's relationship starts with its foreign key off");
    // One undo takes the table, its node and its line away together.
    let undo = h.state().session.as_ref().unwrap().undo_label();
    assert_eq!(undo, Some("Add table"));
    h.state_mut().canvas.rename = None;
    h.state_mut().session.as_mut().unwrap().undo().unwrap();
    h.run_steps(2);
    assert_eq!((tables(&h), relationships(&h).len(), node_ids(&h).len()), before);
}

/// A line drawn to a table without columns starts with its foreign key off.
#[test]
fn a_line_to_a_table_without_columns_starts_with_its_foreign_key_off() {
    let (mut h, customer, _order) = two_tables_harness();
    // A third table with no columns at all, beside the other two.
    let empty = {
        let s = h.state_mut().session.as_mut().unwrap();
        let st = s.doc();
        let schema = st.uuid(st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_er::SCHEMA)).unwrap()).unwrap().to_string();
        let d = st.uuid(s.view().open_diagram.unwrap()).unwrap().to_string();
        let e =
            s.execute("er.entity.add", &serde_json::json!({ "schema": schema, "name": "Note" })).unwrap().value["id"].as_str().unwrap().to_string();
        s.execute("diagram.place", &serde_json::json!({ "diagram": d, "element": e, "x": 100, "y": 420, "width": 240, "height": 72 })).unwrap();
        e
    };
    h.run_steps(2);
    let node_of = |h: &Harness<'static, Shell>, element: &str| {
        let st = h.state().session.as_ref().unwrap().doc();
        let e = st.by_uuid(archforge_mof::Uuid::parse(element).unwrap()).unwrap();
        let n = st
            .alive()
            .find(|n| {
                st.class_of(*n).ok() == Some(archforge_diagram::NODE) && st.get(*n, archforge_diagram::node::ELEMENT).unwrap().as_ref() == Some(e)
            })
            .unwrap();
        format!("workspace.canvas.node.{}", st.uuid(n).unwrap())
    };
    let (from, to) = (node_of(&h, &customer), node_of(&h, &empty));
    let columns_of = |h: &Harness<'static, Shell>, element: &str| {
        let st = h.state().session.as_ref().unwrap().doc();
        let e = st.by_uuid(archforge_mof::Uuid::parse(element).unwrap()).unwrap();
        st.list(e, archforge_er::entity::ATTRIBUTE).unwrap().len()
    };
    let before = (columns_of(&h, &customer), columns_of(&h, &empty));
    assert_eq!(before.1, 0);
    let at = rect_of(&h, "workspace.tools.relationship").center();
    click(&mut h, at);
    let (a, b) = (rect_of(&h, &from).center(), rect_of(&h, &to).center());
    drag(&mut h, a, b);
    let rels = relationships(&h);
    assert_eq!(rels.len(), 1, "{:?}", h.state().canvas.notice);
    let st = h.state().session.as_ref().unwrap().doc();
    assert!(!er_connect::has_foreign_key(st, rels[0]), "Customer has its key, but Note has no columns: the key starts off");
    assert_eq!((columns_of(&h, &customer), columns_of(&h, &empty)), before, "and no column is made on either table");
}

/// A new diagram whose new schema is refused says why in plain words, as Properties says a
/// refusal: no command id, no engine wording. In light and dark (AR-427).
#[test]
fn a_new_diagram_refused_by_its_schema_says_why_in_plain_words() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let mut h = workspace_harness(theme, egui::vec2(1600.0, 1000.0));
        h.state_mut().session.as_mut().unwrap().execute("er.schema.add", &serde_json::json!({ "name": "Shop" })).unwrap();
        h.state_mut().state.show_explorer(archforge_shell_api::state::Explorer::Diagrams);
        let step = archforge_ui_egui::diagrams::Step::Details;
        let new = archforge_ui_egui::diagrams::NewDiagram {
            step,
            name: "Shop".into(),
            kind: archforge_er::DIAGRAM_KIND,
            owner: None,
            menu: None,
            refused: None,
            opened: false,
        };
        h.state_mut().diagrams.new = Some(new);
        h.run();
        let at = rect_of(&h, "workspace.new-diagram.create").center();
        click(&mut h, at);
        let refused = h.state().diagrams.new.as_ref().and_then(|n| n.refused.clone()).expect("a second schema named Shop is refused");
        assert!(refused.starts_with("There is a schema named Shop") && !refused.contains("er."), "{theme:?}: {refused}");
        read_out(&h, &refused, "workspace.new-diagram.notice");
    }
}

/// The Diagrams explorer creates an ER diagram through the New diagram dialog and opens it in a tab.
#[test]
fn the_diagrams_explorer_creates_a_diagram_through_the_dialog_and_opens_it_in_a_tab() {
    let (mut h, _customer) = table_harness();
    let at = rect_of(&h, "workspace.explorers.diagrams").center();
    click(&mut h, at);
    assert!(!ids(&h).iter().any(|i| i.starts_with("workspace.diagrams.row.")), "no diagram yet");
    assert!(!ids(&h).iter().any(|i| i == "workspace.canvas"), "the canvas is plain without a diagram");
    // The + opens the dialog with the schema's name and the schema chosen.
    let at = rect_of(&h, "workspace.diagrams.add").center();
    click(&mut h, at);
    let edit = h.state().diagrams.new.clone().expect("the dialog opened");
    assert_eq!(edit.step, archforge_ui_egui::diagrams::Step::Kind, "the kind comes first");
    assert_eq!(edit.name, "Shop");
    assert!(edit.owner.is_some());
    let at = rect_of(&h, "workspace.new-diagram.continue").center();
    click(&mut h, at);
    assert_eq!(h.state().diagrams.new.as_ref().map(|n| n.step), Some(archforge_ui_egui::diagrams::Step::Details));
    let at = rect_of(&h, "workspace.new-diagram.create").center();
    click(&mut h, at);
    assert!(h.state().diagrams.new.is_none(), "the dialog closed");
    let diagram = {
        let st = h.state().session.as_ref().unwrap().doc();
        let d = st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_diagram::DIAGRAM)).expect("the diagram was added");
        st.uuid(d).unwrap()
    };
    assert_eq!(h.state().canvas.active, Some(diagram));
    assert_eq!(h.state().canvas.tabs, vec![diagram]);
    assert_eq!(selected_name(&h), "Shop", "the diagram is selected, so Properties shows it");
    let found = ids(&h);
    for id in [
        format!("workspace.tabs.{diagram}"),
        format!("workspace.diagrams.row.{diagram}"),
        "workspace.canvas".to_string(),
        "workspace.minimap.fit".to_string(),
    ] {
        assert!(found.contains(&id), "{id} in {found:?}");
    }
    // Closing the tab empties the centre; the row opens it again.
    let at = rect_of(&h, &format!("workspace.tabs.{diagram}.close")).center();
    click(&mut h, at);
    assert_eq!(h.state().canvas.active, None);
    assert!(h.state().canvas.tabs.is_empty());
    assert!(!ids(&h).iter().any(|i| i == "workspace.canvas"));
    let at = rect_of(&h, &format!("workspace.diagrams.row.{diagram}")).center();
    click(&mut h, at);
    assert_eq!(h.state().canvas.active, Some(diagram));
    // The filter narrows the rows.
    h.state_mut().diagrams.filter = "zzz".into();
    h.run();
    assert!(!ids(&h).iter().any(|i| i.starts_with("workspace.diagrams.row.")));
    h.state_mut().diagrams.filter = "sho".into();
    h.run();
    assert!(ids(&h).iter().any(|i| i == &format!("workspace.diagrams.row.{diagram}")));
    h.state_mut().diagrams.filter.clear();
    // A second diagram of a new schema: the schema is made with the diagram's name.
    let at = rect_of(&h, "workspace.tabs.add").center();
    click(&mut h, at);
    let at = rect_of(&h, "workspace.new-diagram.continue").center();
    click(&mut h, at);
    h.state_mut().diagrams.new.as_mut().unwrap().name = "Billing".into();
    let at = rect_of(&h, "workspace.new-diagram.schema").center();
    click(&mut h, at);
    let at = rect_of(&h, "workspace.new-diagram.schema.1").center();
    click(&mut h, at);
    assert_eq!(h.state().diagrams.new.as_ref().unwrap().owner, None, "New schema chosen");
    let at = rect_of(&h, "workspace.new-diagram.create").center();
    click(&mut h, at);
    assert!(h.state().diagrams.new.is_none(), "{:?}", h.state().diagrams.new);
    let st = h.state().session.as_ref().unwrap().doc();
    assert!(st.alive().any(|o| st.class_of(o).ok() == Some(archforge_er::SCHEMA) && archforge_core::name_of(st, o) == "Billing"));
    assert_eq!(h.state().canvas.tabs.len(), 2);
    assert_eq!(selected_name(&h), "Billing");
}

/// The canvas draws an ER diagram's tables and line, and the pointer selects, pans and zooms.
#[test]
fn the_canvas_draws_the_diagram_and_the_pointer_selects_pans_and_zooms() {
    let (mut h, customer, diagram) = diagram_harness();
    let found = ids(&h);
    let nodes: Vec<String> = found.iter().filter(|i| i.starts_with("workspace.canvas.node.")).cloned().collect();
    assert_eq!(nodes.len(), 2, "both tables are drawn: {found:?}");
    assert_eq!(found.iter().filter(|i| i.starts_with("workspace.canvas.edge.")).count(), 1, "the relationship is drawn");
    assert_eq!(selected_name(&h), "Shop schema", "opening selects the diagram");
    // The node at the left is Customer: a click selects the table behind it.
    let customer_node = nodes.iter().min_by(|a, b| rect_of(&h, a).left().total_cmp(&rect_of(&h, b).left())).unwrap().clone();
    let at = rect_of(&h, &customer_node).center();
    click(&mut h, at);
    assert_eq!(selected_name(&h), "Customer");
    let st = h.state().session.as_ref().unwrap().doc();
    let customer_obj = st.by_uuid(archforge_mof::Uuid::parse(&customer).unwrap()).unwrap();
    assert_eq!(h.state().session.as_ref().unwrap().view().selection, vec![customer_obj]);
    // A click on the empty canvas selects the diagram again.
    let canvas = rect_of(&h, "workspace.canvas");
    click(&mut h, canvas.left_top() + egui::vec2(20.0, 20.0));
    assert_eq!(selected_name(&h), "Shop schema");
    // The pan tool drags the view.
    let at = rect_of(&h, "workspace.tools.pan").center();
    click(&mut h, at);
    assert_eq!(h.state().canvas.tool, archforge_shell_api::canvas::Tool::Pan);
    let before = h.state().canvas.pan;
    let from = canvas.left_top() + egui::vec2(20.0, 20.0);
    drag(&mut h, from, from + egui::vec2(50.0, 20.0));
    let after = h.state().canvas.pan;
    assert!((after - before - egui::vec2(50.0, 20.0)).length() < 1.0, "{before:?} -> {after:?}");
    // The zoom buttons and Whole diagram.
    let fitted = h.state().canvas.zoom;
    let at = rect_of(&h, "workspace.zoom.in").center();
    click(&mut h, at);
    assert!(h.state().canvas.zoom > fitted);
    let at = rect_of(&h, "workspace.zoom.out").center();
    click(&mut h, at);
    click(&mut h, at);
    assert!(h.state().canvas.zoom < fitted);
    let at = rect_of(&h, "workspace.minimap.fit").center();
    click(&mut h, at);
    assert!((h.state().canvas.zoom - fitted).abs() < 0.001);
    // The toggles.
    let at = rect_of(&h, "workspace.tools.grid").center();
    click(&mut h, at);
    assert!(!h.state().canvas.grid);
    let at = rect_of(&h, "workspace.tools.key-status").center();
    click(&mut h, at);
    assert!(!h.state().canvas.key_status);
    let at = rect_of(&h, "workspace.minimap.toggle").center();
    click(&mut h, at);
    assert!(!h.state().canvas.minimap_open);
    assert!(!ids(&h).iter().any(|i| i == "workspace.minimap.fit"));
    let at = rect_of(&h, "workspace.minimap.toggle").center();
    click(&mut h, at);
    assert!(h.state().canvas.minimap_open);
    // Properties shows the diagram with its schema when the diagram is selected.
    click(&mut h, canvas.left_top() + egui::vec2(20.0, 20.0));
    assert!(ids(&h).iter().any(|i| i == "workspace.properties.schema"));
    let _ = diagram;
}

/// A table's menu removes it from the diagram and keeps it in the model; the delete dialog deletes it
/// from the model.
#[test]
fn the_entity_menu_removes_from_the_diagram_and_the_dialog_deletes_from_the_model() {
    let (mut h, customer, _diagram) = diagram_harness();
    let nodes = node_ids(&h);
    let customer_node = nodes.iter().min_by(|a, b| rect_of(&h, a).left().total_cmp(&rect_of(&h, b).left())).unwrap().clone();
    let order_node = nodes.iter().find(|n| **n != customer_node).unwrap().clone();
    // The add-column adorner opens the column editor for the selected table, and Properties shows
    // it in place of the panel that showed.
    let at = rect_of(&h, &customer_node).center();
    click(&mut h, at);
    h.state_mut().state.panel = archforge_shell_api::state::Panel::Skills;
    h.run();
    let at = rect_of(&h, "workspace.canvas.adorn.add-column").center();
    click(&mut h, at);
    assert!(er_of(h.state()).column.as_ref().is_some_and(|c| c.target.is_none() && c.entity.to_string() == customer));
    assert_eq!(h.state().state.panel, archforge_shell_api::state::Panel::Properties);
    // A right click opens the menu; Remove from this diagram keeps the table in the model.
    let at = rect_of(&h, &order_node).center();
    h.event(egui::Event::PointerMoved(at));
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Secondary, pressed, modifiers: egui::Modifiers::NONE });
    }
    h.run();
    assert!(h.state().canvas.menu.is_some(), "the menu opened");
    assert!(painted(&h).iter().any(|t| t == "Ctrl+Shift+G"), "Generate's keys as the menu bar writes them");
    let at = rect_of(&h, "workspace.canvas.menu.remove").center();
    click(&mut h, at);
    assert_eq!(node_ids(&h).len(), 1, "the node went; notice {:?}, active {:?}, ids {:?}", h.state().canvas.notice, h.state().canvas.active, ids(&h));
    let st = h.state().session.as_ref().unwrap().doc();
    assert!(st.alive().any(|o| archforge_core::name_of(st, o) == "Order"), "the table stays");
    // Shift+Delete on the selected Customer asks; Delete confirms and the table goes with its node.
    let at = rect_of(&h, &customer_node).center();
    click(&mut h, at);
    key(&mut h, egui::Key::Delete, egui::Modifiers::SHIFT);
    let d = h.state().explorer.delete.clone().expect("the dialog asks");
    assert_eq!(d.name, "Customer");
    assert_eq!(d.diagrams, 1);
    assert!(d.node.is_some());
    assert!(ids(&h).iter().any(|i| i == "workspace.explorer.delete.unplace"));
    let at = rect_of(&h, "workspace.explorer.delete.confirm").center();
    click(&mut h, at);
    assert!(h.state().explorer.delete.is_none());
    assert!(node_ids(&h).is_empty());
    let st = h.state().session.as_ref().unwrap().doc();
    assert!(!st.alive().any(|o| archforge_core::name_of(st, o) == "Customer"));
}

/// A table's menu as ER answers it: Show related tables first, in a section of its own and off,
/// then after Open in Models Add column and, on a pure mapping table alone, Collapse into a
/// relationship; Add column opens the table's new column in Properties and Collapse brings the
/// line back. Its keys are the platform's (⌘↩ on the Mac), and nothing but a table takes ER's menu.
#[test]
fn a_tables_menu_adds_its_own_rows_and_runs_them() {
    use archforge_shell_api::kinds::DiagramKind;
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, _customer, diagram) = diagram_harness_in(theme);
        let mapping = {
            let s = h.state_mut().session.as_mut().unwrap();
            let id = |v: serde_json::Value| v["id"].as_str().unwrap().to_string();
            let st = s.doc();
            let schema = st.uuid(st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_er::SCHEMA)).unwrap()).unwrap().to_string();
            let order = st.alive().find(|o| archforge_core::name_of(st, *o) == "Order").unwrap();
            let order_node = st.uuid(archforge_diagram::views_of(st, order).2[0]).unwrap().to_string();
            let order = st.uuid(order).unwrap().to_string();
            let tag = id(s.execute("er.entity.add", &serde_json::json!({ "schema": schema, "name": "Tag" })).unwrap().value);
            s.execute("er.attribute.add", &serde_json::json!({ "entity": tag, "name": "tag_id", "type": "INT", "primary_key": true })).unwrap();
            let line = id(s
                .execute(
                    "er.relationship.add",
                    &serde_json::json!({ "from": order, "to": tag, "from_end": "zero-or-more", "to_end": "zero-or-more" }),
                )
                .unwrap()
                .value);
            let t = id(s
                .execute("diagram.place", &serde_json::json!({ "diagram": diagram, "element": tag, "x": 900, "y": 400, "width": 240, "height": 120 }))
                .unwrap()
                .value);
            s.execute("diagram.connect", &serde_json::json!({ "diagram": diagram, "element": line, "source": order_node, "target": t })).unwrap();
            id(s.execute("er.relationship.resolve", &serde_json::json!({ "relationship": line })).unwrap().value)
        };
        h.run();
        let node_of = |h: &Harness<'static, Shell>, element: &str| {
            let st = h.state().session.as_ref().unwrap().doc();
            let e = st.alive().find(|o| archforge_core::name_of(st, *o) == element || st.uuid(*o).is_ok_and(|u| u.to_string() == element)).unwrap();
            format!("workspace.canvas.node.{}", st.uuid(archforge_diagram::views_of(st, e).2[0]).unwrap())
        };
        // ER answers for its tables alone, in the platform's keys.
        {
            let st = h.state().session.as_ref().unwrap().doc();
            let order = st.alive().find(|o| archforge_core::name_of(st, *o) == "Order").unwrap();
            let column = st.alive().find(|o| archforge_core::name_of(st, *o) == "order_id").unwrap();
            let own: Vec<_> = er_kind::ErKind.menu(st, order, false).unwrap().iter().map(|r| (r.key, r.label, r.live, r.first)).collect();
            assert_eq!(own, [("related", "Show related tables", false, true), ("add-column", "Add column", true, false)], "{theme:?}");
            let keys = |mac: bool| er_kind::ErKind.menu(st, order, mac).unwrap().iter().map(|r| r.keys).collect::<Vec<_>>();
            assert_eq!((keys(true), keys(false)), (vec!["R", "⌘↩"], vec!["R", "Ctrl Enter"]), "{theme:?}");
            assert!(er_kind::ErKind.menu(st, column, false).is_none(), "{theme:?}: a column takes no menu");
        }
        // Order: no collapse, as it is no mapping table.
        let at = rect_of(&h, &node_of(&h, "Order")).center();
        right_click(&mut h, at);
        let rows = |h: &Harness<'static, Shell>| menu_drawn(h).into_iter().map(|(key, gap)| format!("{key} {gap}")).collect::<Vec<_>>();
        assert_eq!(rows(&h), ["related 0", "open 1", "add-column 0", "generate 0", "remove 1", "delete 0"], "{theme:?}");
        let texts = painted(&h);
        assert!(["Show related tables", "R", "Add column", "Ctrl Enter"].iter().all(|t| texts.iter().any(|p| p == t)), "{theme:?}: {texts:?}");
        // Show related tables is off: its click runs nothing and falls to the canvas, which closes the menu.
        let at = rect_of(&h, "workspace.canvas.menu.related").center();
        click(&mut h, at);
        assert!(h.state().canvas.menu.is_none() && er_of(h.state()).column.is_none(), "{theme:?}: Show related tables is off");
        // Add column shows Properties in place of the panel that showed.
        h.state_mut().state.panel = archforge_shell_api::state::Panel::Skills;
        h.run();
        let at = rect_of(&h, &node_of(&h, "Order")).center();
        right_click(&mut h, at);
        let at = rect_of(&h, "workspace.canvas.menu.add-column").center();
        click(&mut h, at);
        assert!(h.state().canvas.menu.is_none(), "{theme:?}: the menu closed");
        let order = {
            let st = h.state().session.as_ref().unwrap().doc();
            st.uuid(st.alive().find(|o| archforge_core::name_of(st, *o) == "Order").unwrap()).unwrap()
        };
        assert!(
            er_of(h.state()).column.as_ref().is_some_and(|c| c.target.is_none() && c.entity == order),
            "{theme:?}: {:?}",
            er_of(h.state()).column
        );
        assert_eq!(h.state().state.panel, archforge_shell_api::state::Panel::Properties, "{theme:?}");
        // The mapping table: Collapse after Add column, and its click brings the line back.
        er_of_mut(h.state_mut()).column = None;
        h.run();
        let at = rect_of(&h, &node_of(&h, &mapping)).center();
        right_click(&mut h, at);
        assert_eq!(rows(&h), ["related 0", "open 1", "add-column 0", "collapse 0", "generate 0", "remove 1", "delete 0"], "{theme:?}");
        assert!(painted(&h).iter().any(|t| t == "Collapse into a relationship"), "{theme:?}");
        let at = rect_of(&h, "workspace.canvas.menu.collapse").center();
        click(&mut h, at);
        let st = h.state().session.as_ref().unwrap().doc();
        assert!(
            st.by_uuid(archforge_mof::Uuid::parse(&mapping).unwrap()).is_none_or(|m| !st.is_alive(m)),
            "{theme:?}: {:?}",
            h.state().canvas.notice
        );
        assert!(h.state().canvas.menu.is_none(), "{theme:?}");
    }
}

/// A table added in Models is placed on the open diagram, and its box grows with its columns.
#[test]
fn a_table_added_in_models_is_placed_on_the_open_diagram_and_grows_with_its_columns() {
    let (mut h, _customer, _diagram) = diagram_harness();
    let before = node_ids(&h).len();
    // Select the schema in Models and add a table: it lands on the diagram.
    let schema_row = ids(&h).into_iter().find(|i| i.starts_with("workspace.explorer.row.")).unwrap();
    let at = rect_of(&h, &schema_row).center();
    click(&mut h, at);
    archforge_ui_egui::explorer::add(h.state_mut(), "table");
    h.run();
    assert_eq!(node_ids(&h).len(), before + 1, "placed on the open diagram");
    key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
    // Its node starts at the empty height and grows with the first column.
    let new_node = node_ids(&h).into_iter().max_by(|a, b| rect_of(&h, a).left().total_cmp(&rect_of(&h, b).left())).unwrap();
    let id = new_node.trim_start_matches("workspace.canvas.node.").to_string();
    assert!((node_bounds(&h, &id).height - er_place::ENTITY_EMPTY_HEIGHT).abs() < 0.01);
    archforge_ui_egui::explorer::add(h.state_mut(), "column");
    h.run();
    assert!(
        (node_bounds(&h, &id).height - er_place::content_height(1)).abs() < 0.01,
        "{:?} notice {:?} undo {:?}",
        node_bounds(&h, &id),
        h.state().explorer.notice,
        h.state().session.as_ref().unwrap().undo_label()
    );
    // Growth and the column were one step.
    let s = h.state_mut().session.as_mut().unwrap();
    s.execute("edit.undo", &serde_json::Value::Null).unwrap();
    h.run();
    assert!((node_bounds(&h, &id).height - er_place::ENTITY_EMPTY_HEIGHT).abs() < 0.01);
}

/// A double click on a table's header renames it in place and one on a column opens its editor; a
/// refused name stays open with its refusal.
#[test]
fn double_clicks_rename_and_open_columns_and_a_refused_name_stays_open() {
    let (mut h, customer, _diagram) = diagram_harness();
    let nodes = node_ids(&h);
    let customer_node = nodes.iter().min_by(|a, b| rect_of(&h, a).left().total_cmp(&rect_of(&h, b).left())).unwrap().clone();
    let rect = rect_of(&h, &customer_node);
    // A double click on the header opens the name; "Order" is taken, so the rename is refused and stays open.
    let at = rect.left_top() + egui::vec2(60.0, 30.0);
    double_click(&mut h, at);
    assert!(h.state().canvas.rename.as_ref().is_some_and(|r| r.text == "Customer"), "{:?}", h.state().canvas.rename);
    h.state_mut().canvas.rename.as_mut().unwrap().text = "Order".into();
    key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
    let rename = h.state().canvas.rename.clone().expect("a refused name stays open");
    assert!(rename.refused.is_some());
    key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
    assert!(h.state().canvas.rename.is_none());
    assert_eq!(selected_name(&h), "Customer");
    // A double click on the second column row opens that column's editor.
    let at = rect.left_top() + egui::vec2(100.0, 77.0 + 20.0 + 6.0);
    double_click(&mut h, at);
    let edit = er_of(h.state()).column.clone().expect("the column editor opened");
    assert_eq!(edit.entity.to_string(), customer);
    assert_eq!(edit.name, "email", "the second row is the email column");
    // With the Entity tool, a click on a node selects it instead of placing one.
    let n = node_ids(&h).len();
    let at = rect_of(&h, "workspace.tools.entity").center();
    click(&mut h, at);
    let order_node = nodes.iter().find(|x| **x != customer_node).unwrap().clone();
    let at = rect_of(&h, &order_node).center();
    click(&mut h, at);
    assert_eq!(node_ids(&h).len(), n);
    assert_eq!(selected_name(&h), "Order");
    // The dialog's third button removes from the diagram only. Time passes first, so the next
    // click on Order is not read as a double click on the one before; the column editor, which
    // would keep the keyboard, is closed.
    key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
    for _ in 0..45 {
        h.step();
    }
    er_of_mut(h.state_mut()).column = None;
    let at = rect_of(&h, &order_node).center();
    click(&mut h, at);
    key(&mut h, egui::Key::Delete, egui::Modifiers::SHIFT);
    assert!(h.state().explorer.delete.is_some());
    let at = rect_of(&h, "workspace.explorer.delete.unplace").center();
    click(&mut h, at);
    assert!(h.state().explorer.delete.is_none());
    assert_eq!(node_ids(&h).len(), n - 1);
    let st = h.state().session.as_ref().unwrap().doc();
    assert!(st.alive().any(|o| archforge_core::name_of(st, o) == "Order"), "the table stays in the model");
}

/// The Relationship tool draws a line between two tables with its foreign key as one step, and
/// Delete removes it with a toast that offers Undo.
#[test]
fn the_relationship_tool_draws_a_line_with_its_foreign_key_as_one_step_and_delete_offers_undo() {
    let (mut h, customer, _order) = two_tables_harness();
    let nodes = node_ids(&h);
    let customer_node = nodes.iter().min_by(|a, b| rect_of(&h, a).left().total_cmp(&rect_of(&h, b).left())).unwrap().clone();
    let order_node = nodes.iter().find(|n| **n != customer_node).unwrap().clone();
    let at = rect_of(&h, "workspace.tools.relationship").center();
    click(&mut h, at);
    assert_eq!(h.state().canvas.tool, archforge_shell_api::canvas::Tool::Connect("relationship"));
    // Drag from Customer to Order: one to many, Customer has a primary key, so the key comes along.
    let (from, to) = (rect_of(&h, &customer_node).center(), rect_of(&h, &order_node).center());
    drag(&mut h, from, to);
    let rels = relationships(&h);
    assert_eq!(rels.len(), 1, "one relationship drawn");
    assert_eq!(h.state().canvas.tool, archforge_shell_api::canvas::Tool::Select, "drawing hands back to Select");
    let st = h.state().session.as_ref().unwrap().doc();
    let rel = rels[0];
    assert_eq!(st.get(rel, archforge_er::relationship::FROM).unwrap().as_ref().and_then(|o| st.uuid(o).ok()).unwrap().to_string(), customer);
    assert!(er_connect::has_foreign_key(st, rel), "the foreign key came with the line");
    assert!(ids(&h).iter().any(|i| i.starts_with("workspace.canvas.edge.")), "the edge is drawn");
    assert_eq!(h.state().session.as_ref().unwrap().view().selection, vec![rel], "the relationship is selected");
    assert!(ids(&h).iter().any(|i| i == "workspace.properties.name"), "Properties shows its name field");
    // The line, its edge and its label were one step.
    let s = h.state_mut().session.as_mut().unwrap();
    assert_eq!(s.undo_label(), Some("Add relationship"));
    s.execute("edit.undo", &serde_json::Value::Null).unwrap();
    h.run();
    assert!(relationships(&h).is_empty());
    let s = h.state_mut().session.as_mut().unwrap();
    s.execute("edit.redo", &serde_json::Value::Null).unwrap();
    h.run();
    // Escape returns to Select; a click on the edge selects the relationship; Delete removes it with a toast.
    key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
    let edge = ids(&h).into_iter().find(|i| i.starts_with("workspace.canvas.edge.")).unwrap();
    let at = rect_of(&h, &edge).center();
    click(&mut h, at);
    assert_eq!(h.state().session.as_ref().unwrap().view().selection.len(), 1);
    let keys_and_columns = |h: &Harness<'static, Shell>| {
        let st = h.state().session.as_ref().unwrap().doc();
        let count = |c| st.alive().filter(|o| st.class_of(*o).ok() == Some(c)).count();
        (count(archforge_er::FOREIGN_KEY), count(archforge_er::ATTRIBUTE))
    };
    let with_line = keys_and_columns(&h);
    key(&mut h, egui::Key::Delete, egui::Modifiers::NONE);
    assert!(relationships(&h).is_empty(), "the relationship went");
    assert_eq!(keys_and_columns(&h), (with_line.0 - 1, with_line.1 - 1), "its foreign key and its key column went with it");
    assert!(h.state().canvas.toast.is_some(), "the toast offers Undo");
    let at = rect_of(&h, "workspace.toast.undo").center();
    click(&mut h, at);
    assert_eq!(relationships(&h).len(), 1, "Undo brings it back");
    assert_eq!(keys_and_columns(&h), with_line, "with its key and column");
    assert!(h.state().canvas.toast.is_none());
    // A step after the removal takes the toast with it: Undo never undoes something else.
    let edge = ids(&h).into_iter().find(|i| i.starts_with("workspace.canvas.edge.")).unwrap();
    let at = rect_of(&h, &edge).center();
    click(&mut h, at);
    key(&mut h, egui::Key::Delete, egui::Modifiers::NONE);
    assert!(h.state().canvas.toast.is_some());
    let from = rect_of(&h, &customer_node).center();
    drag(&mut h, from, from + egui::vec2(30.0, 0.0));
    assert!(h.state().canvas.toast.is_none(), "the toast went with the move");
    assert!(!ids(&h).iter().any(|i| i == "workspace.toast.undo"));
}

/// A mapping table refused for its name says why in plain words on the offer, never with the
/// command's id or the parameter it is about. In light and dark (AR-428).
#[test]
fn a_mapping_table_refused_for_its_name_says_why_in_plain_words() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, customer, order) = two_tables_harness();
        h.state_mut().state.theme = theme;
        let line = {
            let s = h.state_mut().session.as_mut().unwrap();
            let rel = s.execute(
                "er.relationship.add",
                &serde_json::json!({ "from": customer, "to": order, "from_end": "zero-or-more", "to_end": "zero-or-more" }),
            );
            rel.unwrap().value["id"].as_str().unwrap().to_string()
        };
        let relationship = archforge_mof::Uuid::parse(&line).unwrap();
        er_of_mut(h.state_mut()).mapping = Some(er_connect::MappingOffer { relationship, name: "Customer".into(), refused: None });
        h.run();
        let at = rect_of(&h, "workspace.mapping.add").center();
        click(&mut h, at);
        let refused = er_of(h.state()).mapping.as_ref().and_then(|o| o.refused.clone()).unwrap_or_else(|| panic!("{theme:?}: refused"));
        assert!(
            refused.starts_with("There is a table named Customer") && !refused.contains("er.") && !refused.contains("name:"),
            "{theme:?}: {refused}"
        );
    }
}

/// ER's cardinality popover sets a new line's ends, and a many-to-many line offers its mapping table.
#[test]
fn the_cardinality_popover_sets_the_ends_and_a_many_to_many_line_offers_its_mapping_table() {
    let (mut h, _customer, _order) = two_tables_harness();
    let nodes = node_ids(&h);
    let customer_node = nodes.iter().min_by(|a, b| rect_of(&h, a).left().total_cmp(&rect_of(&h, b).left())).unwrap().clone();
    let order_node = nodes.iter().find(|n| **n != customer_node).unwrap().clone();
    let at = rect_of(&h, "workspace.tools.relationship-types").center();
    click(&mut h, at);
    assert!(h.state().canvas.popover.is_some() && h.state().canvas.tool == archforge_shell_api::canvas::Tool::Connect("relationship"));
    assert!(ids(&h).iter().any(|i| i == "workspace.cardinality.preset.3"), "{:?}", ids(&h));
    let at = rect_of(&h, "workspace.cardinality.preset.3").center();
    click(&mut h, at);
    assert!(er_of(h.state()).relationship.many_to_many());
    let at = rect_of(&h, "workspace.cardinality.identifying").center();
    click(&mut h, at);
    assert!(er_of(h.state()).relationship.identifying);
    let at = rect_of(&h, "workspace.cardinality.end.1").center();
    click(&mut h, at);
    assert_eq!(er_of(h.state()).relationship.to, archforge_er::Cardinality::ExactlyOne);
    let at = rect_of(&h, "workspace.cardinality.preset.3").center();
    click(&mut h, at);
    key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
    assert!(h.state().canvas.popover.is_none(), "Escape closes the popover");
    // The pick stays: the connect adorner of the selected Customer draws a many-to-many line.
    h.state_mut().canvas.tool = archforge_shell_api::canvas::Tool::Select;
    let at = rect_of(&h, &customer_node).center();
    click(&mut h, at);
    let from = rect_of(&h, "workspace.canvas.adorn.connect").center();
    let to = rect_of(&h, &order_node).center();
    drag(&mut h, from, to);
    let rels = relationships(&h);
    assert_eq!(rels.len(), 1);
    let st = h.state().session.as_ref().unwrap().doc();
    assert!(archforge_er::rules::is_many_to_many(st, rels[0]));
    assert!(!er_connect::has_foreign_key(st, rels[0]), "no key for a many-to-many line");
    let offer = er_of(h.state()).mapping.clone().expect("the mapping table is offered");
    assert_eq!(offer.name, "CustomerOrder");
    assert!(ids(&h).iter().any(|i| i == "workspace.mapping.add"));
    let at = rect_of(&h, "workspace.mapping.add").center();
    click(&mut h, at);
    assert!(er_of(h.state()).mapping.is_none(), "{:?}", er_of(h.state()).mapping);
    let st = h.state().session.as_ref().unwrap().doc();
    assert!(st.alive().any(|o| archforge_core::name_of(st, o) == "CustomerOrder"), "the mapping table exists");
}

/// The rules' problems badge the canvas, and the Note tool places a note typed in place.
#[test]
fn problems_badge_the_canvas_and_a_note_is_placed_and_typed_in_place() {
    let (mut h, _customer, _diagram) = diagram_harness();
    // Order's primary key goes: its node gets a badge; the schema's Properties lists and selects.
    {
        let s = h.state_mut().session.as_mut().unwrap();
        let (order, schema) = {
            let st = s.doc();
            let order = st.uuid(st.alive().find(|o| archforge_core::name_of(st, *o) == "Order").unwrap()).unwrap().to_string();
            let schema = st.uuid(st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_er::SCHEMA)).unwrap()).unwrap().to_string();
            (order, schema)
        };
        s.execute("er.primary-key.remove", &serde_json::json!({ "entity": order })).unwrap();
        s.execute("view.select", &serde_json::json!({ "ids": [schema] })).unwrap();
    }
    h.run();
    let badges: Vec<String> = ids(&h).into_iter().filter(|i| i.starts_with("workspace.canvas.problem.")).collect();
    assert_eq!(badges.len(), 1, "one badge on Order: {badges:?}");
    let at = rect_of(&h, "workspace.properties.problem.0").center();
    click(&mut h, at);
    assert_eq!(selected_name(&h), "Order", "the problem row selects its table");
    // The Note tool places a note and opens its text; typing and Cmd+Enter set it.
    let at = rect_of(&h, "workspace.tools.note").center();
    click(&mut h, at);
    assert_eq!(h.state().canvas.tool, archforge_shell_api::canvas::Tool::Note);
    let canvas = rect_of(&h, "workspace.canvas");
    let before = node_ids(&h).len();
    click(&mut h, canvas.left_top() + egui::vec2(canvas.width() - 120.0, 80.0));
    assert_eq!(node_ids(&h).len(), before + 1, "the note is a node: {:?}", h.state().canvas.notice);
    assert_eq!(h.state().canvas.tool, archforge_shell_api::canvas::Tool::Select, "placing hands back to Select");
    assert!(h.state().canvas.note_edit.is_some(), "its text is open");
    assert!(ids(&h).iter().any(|i| i == "workspace.canvas.note"));
    h.state_mut().canvas.note_edit.as_mut().unwrap().text = "Product.sku is unique per supplier".into();
    key(&mut h, egui::Key::Enter, egui::Modifiers::COMMAND);
    assert!(h.state().canvas.note_edit.is_none(), "{:?}", h.state().canvas.note_edit);
    let st = h.state().session.as_ref().unwrap().doc();
    let note = st
        .alive()
        .find(|o| {
            st.class_of(*o).ok() == Some(archforge_diagram::NODE)
                && st.get(*o, archforge_diagram::node::ELEMENT).ok().and_then(|v| v.as_ref()).is_none()
        })
        .unwrap();
    assert_eq!(st.get(note, archforge_diagram::node::TEXT).unwrap().as_str(), Some("Product.sku is unique per supplier"));
    // Removing a column with its key says so in a toast.
    key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
    let st = h.state().session.as_ref().unwrap().doc();
    let customer_id = st
        .uuid(
            st.alive()
                .find(|o| {
                    archforge_core::name_of(st, *o) == "customer_id"
                        && archforge_er::rules::entity_of(st, *o).is_some_and(|e| archforge_core::name_of(st, e) == "Customer")
                })
                .unwrap(),
        )
        .unwrap();
    archforge_ui_egui::snapshot::cx(h.state_mut(), |cx| {
        er_table::open_column(cx, archforge_mof::Uuid::parse(&_customer).unwrap(), Some(customer_id))
    });
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.execute("view.select", &serde_json::json!({ "ids": [_customer] })).unwrap();
    }
    h.run();
    let at = rect_of(&h, "workspace.properties.column.remove").center();
    click(&mut h, at);
    let toast = h.state().canvas.toast.clone().expect("the toast");
    assert!(toast.text.starts_with("customer_id removed · "), "{}", toast.text);
}

/// A table of another schema dragged from Models onto the diagram lands with its line, once.
#[test]
fn a_table_of_another_schema_dragged_from_the_explorer_lands_with_its_line_once() {
    let (mut h, _, order) = two_tables_harness();
    let (account, diagram) = {
        let s = h.state_mut().session.as_mut().unwrap();
        let id = |v: serde_json::Value| v["id"].as_str().unwrap().to_string();
        let crm = id(s.execute("er.schema.add", &serde_json::json!({ "name": "CRM" })).unwrap().value);
        let account = id(s.execute("er.entity.add", &serde_json::json!({ "schema": crm, "name": "Account" })).unwrap().value);
        s.execute("er.attribute.add", &serde_json::json!({ "entity": account, "name": "account_id", "type": "BIGINT", "primary_key": true }))
            .unwrap();
        s.execute(
            "er.relationship.add",
            &serde_json::json!({ "from": account, "to": order, "from_end": "exactly-one", "to_end": "zero-or-more", "create_foreign_key": true }),
        )
        .unwrap();
        let st = s.doc();
        let d = st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_diagram::DIAGRAM)).unwrap();
        (account, d)
    };
    h.run();
    let shown = |h: &Harness<'static, Shell>| {
        let st = h.state().session.as_ref().unwrap().doc();
        (st.list(diagram, archforge_diagram::diagram::NODE).unwrap().len(), st.list(diagram, archforge_diagram::diagram::EDGE).unwrap().len())
    };
    assert_eq!(shown(&h), (2, 0));
    // The new schema starts folded; Expand all opens it.
    let fold_all = rect_of(&h, "workspace.explorer.fold-all").center();
    click(&mut h, fold_all);
    h.run();
    let row = rect_of(&h, &format!("workspace.explorer.row.{account}")).center();
    let canvas = rect_of(&h, "workspace.canvas");
    let to = egui::pos2(canvas.center().x, canvas.bottom() - 200.0);
    drag(&mut h, row, to);
    assert_eq!(shown(&h), (3, 1), "the table and its relationship to Order, drawn together");
    assert_eq!(selected_name(&h), "Account");
    assert!(ids(&h).iter().any(|i| i.starts_with("workspace.canvas.node.")));
    // One undo takes both; a second drop of the same table is said and changes nothing.
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.execute("edit.undo", &serde_json::Value::Null).unwrap();
        s.execute("edit.redo", &serde_json::Value::Null).unwrap();
    }
    h.run();
    assert_eq!(shown(&h), (3, 1));
    let row = rect_of(&h, &format!("workspace.explorer.row.{account}")).center();
    drag(&mut h, row, to);
    assert_eq!(shown(&h), (3, 1));
    assert_eq!(h.state().canvas.notice.as_deref(), Some("Already on this diagram"));
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.execute("edit.undo", &serde_json::Value::Null).unwrap();
    }
    h.run();
    assert_eq!(shown(&h), (2, 0));
}

/// The key columns a new line adds grow the child table's box to show them, in the same step.
#[test]
fn key_columns_a_line_adds_grow_the_table_to_show_them_in_the_same_step() {
    let (mut h, _customer, order) = two_tables_harness();
    // Order's box just fits its one column.
    let order_node = {
        let st = h.state().session.as_ref().unwrap().doc();
        let e = st.by_uuid(archforge_mof::Uuid::parse(&order).unwrap()).unwrap();
        let n = st
            .alive()
            .find(|n| {
                st.class_of(*n).ok() == Some(archforge_diagram::NODE) && st.get(*n, archforge_diagram::node::ELEMENT).unwrap().as_ref() == Some(e)
            })
            .unwrap();
        st.uuid(n).unwrap().to_string()
    };
    let snug = er_place::content_height(1);
    h.state_mut()
        .session
        .as_mut()
        .unwrap()
        .execute("diagram.resize", &serde_json::json!({ "node": order_node, "width": 240, "height": snug }))
        .unwrap();
    h.run_steps(2);
    let nodes = node_ids(&h);
    let customer_node = nodes.iter().min_by(|a, b| rect_of(&h, a).left().total_cmp(&rect_of(&h, b).left())).unwrap().clone();
    let at = rect_of(&h, "workspace.tools.relationship").center();
    click(&mut h, at);
    let (a, b) = (rect_of(&h, &customer_node).center(), rect_of(&h, &format!("workspace.canvas.node.{order_node}")).center());
    drag(&mut h, a, b);
    let rels = relationships(&h);
    let st = h.state().session.as_ref().unwrap().doc();
    assert!(er_connect::has_foreign_key(st, rels[0]), "both tables have columns and Customer its key: the key comes on");
    let grown = node_bounds(&h, &order_node).height;
    assert_eq!(grown, er_place::content_height(2), "Order grew to show its new key column");
    h.state_mut().session.as_mut().unwrap().undo().unwrap();
    h.run_steps(2);
    assert_eq!(node_bounds(&h, &order_node).height, snug, "one undo takes the line and the growth back");
}

/// A table box shorter than its columns shows those that fit and counts the rest.
#[test]
fn a_table_box_shorter_than_its_columns_shows_those_that_fit_and_counts_the_rest() {
    let (mut h, customer, diagram) = diagram_harness();
    let s = h.state_mut().session.as_mut().unwrap();
    for name in ["name", "phone", "city"] {
        s.execute("er.attribute.add", &serde_json::json!({ "entity": customer, "name": name, "type": "TEXT" })).unwrap();
    }
    h.run();
    let st = h.state().session.as_ref().unwrap().doc();
    let (e, d) =
        (st.by_uuid(archforge_mof::Uuid::parse(&customer).unwrap()).unwrap(), st.by_uuid(archforge_mof::Uuid::parse(&diagram).unwrap()).unwrap());
    let rows: Vec<String> =
        archforge_diagram::Notation::card(&archforge_er::ErNotation, st, e, d).unwrap().compartments[0].iter().map(|r| r.text.clone()).collect();
    assert_eq!(rows.len(), 5);
    // Its box, 166 tall, holds four rows' places: three columns, then the two past it said in one.
    // A column is painted in the Models tree and, when it shows, in its box too.
    let texts = painted(&h);
    let times = |row: &String| texts.iter().filter(|t| *t == row).count();
    assert_eq!(rows.iter().map(times).collect::<Vec<_>>(), [2, 2, 2, 1, 1], "{texts:?}");
    assert!(texts.iter().any(|t| t == "+ 2 more columns"), "{texts:?}");
}

/// Generate scrolls its body in a short window and keeps its footer clear of it.
#[test]
fn generate_scrolls_its_body_in_a_short_window_and_keeps_its_footer_clear() {
    let (mut h, _customer, _diagram) = diagram_harness();
    let schema = {
        let st = h.state().session.as_ref().unwrap().doc();
        st.uuid(st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_er::SCHEMA)).unwrap()).unwrap()
    };
    h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [schema.to_string()] })).unwrap();
    // The smallest window the app allows.
    h.set_size(egui::vec2(800.0, 400.0));
    h.state_mut().open_generate();
    h.run();
    let foot = rect_of(&h, "generate.close").top() - 20.0;
    let request = |h: &Harness<'static, Shell>| ids(h).iter().any(|i| i == "generate.request").then(|| rect_of(h, "generate.request"));
    assert!(request(&h).is_none_or(|r| r.bottom() > foot), "the request does not fit above the footer before scrolling");
    // The wheel over the body brings the request into view above the footer.
    let over = rect_of(&h, "generate.output.ddl").center();
    h.event(egui::Event::PointerMoved(over));
    for _ in 0..6 {
        h.event(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, -1200.0),
            modifiers: egui::Modifiers::NONE,
            phase: egui::TouchPhase::Move,
            source: egui::MouseWheelSource::Wheel,
        });
        h.run_steps(30);
    }
    let r = request(&h).expect("the request in view");
    assert!(r.bottom() <= foot, "{r:?} ends above the footer at {foot}");
    assert_eq!(rect_of(&h, "generate.close").top() - 20.0, foot, "the footer stays put");
}

/// Generate hands a schema and its export skills to an agent as a request the user can copy.
#[test]
fn generate_hands_a_schema_and_its_export_skills_to_an_agent_as_a_request_it_can_copy() {
    let (mut h, _customer, _diagram) = diagram_harness();
    let schema = {
        let st = h.state().session.as_ref().unwrap().doc();
        st.uuid(st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_er::SCHEMA)).unwrap()).unwrap()
    };
    let run = |h: &mut Harness<'static, Shell>, command: &str, params: serde_json::Value| {
        h.state_mut().session.as_mut().unwrap().execute(command, &params).unwrap_or_else(|e| panic!("{command}: {e}"));
    };
    // With nothing selected it is from the open diagram.
    h.state_mut().open_generate();
    h.run();
    assert!(painted(&h).iter().any(|t| t == "Diagram · Shop schema"), "{:?}", painted(&h));
    for more in NOT_ALONE {
        key(&mut h, egui::Key::Escape, more);
        assert!(h.state().generate.is_some(), "Escape with {more:?} closes nothing");
    }
    key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
    assert!(h.state().generate.is_none());
    // A project's export skill and a schema's of the same name with its dialect: the schema's wins.
    run(&mut h, "skill.add", serde_json::json!({ "name": "DDL conventions", "body": "Plain CREATE TABLE.", "triggers": ["export"] }));
    run(
        &mut h,
        "skill.add",
        serde_json::json!({ "subject": schema.to_string(), "name": "DDL conventions", "body": "Named constraints.", "triggers": ["export"], "dialect": "MySQL" }),
    );
    run(&mut h, "view.select", serde_json::json!({ "ids": [schema.to_string()] }));
    h.state_mut().open_generate();
    h.run();
    assert_eq!(h.state().generate.as_ref().unwrap().dialect, "MySQL", "the dialect the skills in effect name");
    let texts = painted(&h);
    assert!(texts.iter().any(|t| t == "Schema · Shop"), "{texts:?}");
    assert!(texts.iter().any(|t| t == "DDL conventions · on this element · MySQL"), "{texts:?}");
    assert!(texts.iter().any(|t| t == "DDL conventions · from the project · overridden"), "{texts:?}");
    assert!(texts.iter().any(|t| t.starts_with("Agent access is off")), "the harness did not turn agent access on: {texts:?}");
    // The request names the resources to read and the revision it is of.
    let revision = h.state().session.as_ref().unwrap().revision();
    let request = texts.iter().find(|t| t.starts_with("Generate SQL DDL (MySQL)")).expect("the request").clone();
    assert!(request.contains(&format!("archforge://element/{schema}")), "{request}");
    assert!(request.contains(&format!("archforge://skills/effective/{schema}/export")), "{request}");
    assert!(request.contains(&format!("model revision {revision}")), "{request}");
    // Copied, it says so; a change after copying says to copy again.
    let at = rect_of(&h, "generate.copy").center();
    click(&mut h, at);
    assert_eq!(h.state().generate.as_ref().unwrap().copied, Some(revision));
    assert!(painted(&h).iter().any(|t| t.starts_with("Copied.")));
    run(&mut h, "er.entity.add", serde_json::json!({ "schema": schema.to_string(), "name": "Invoice" }));
    h.run();
    assert!(painted(&h).iter().any(|t| t.starts_with("The model changed after the request was copied")));
    assert_eq!(h.state().session.as_ref().unwrap().undo_label(), Some("Add entity"), "the dialog itself changes nothing");
    // Behind the dialog the workspace's keys rest: Delete removes nothing and undo undoes nothing.
    h.state_mut().canvas.focused = true;
    let revision = h.state().session.as_ref().unwrap().revision();
    key(&mut h, egui::Key::Delete, egui::Modifiers::NONE);
    key(&mut h, egui::Key::Z, egui::Modifiers::COMMAND);
    assert_eq!(h.state().session.as_ref().unwrap().revision(), revision, "nothing behind the dialog changed");
    // Documentation takes no dialect.
    let at = rect_of(&h, "generate.output.documentation").center();
    click(&mut h, at);
    assert!(!ids(&h).iter().any(|i| i == "generate.dialect"));
    assert!(painted(&h).iter().any(|t| t.starts_with("Write Markdown documentation")));
    let at = rect_of(&h, "generate.close").center();
    click(&mut h, at);
    assert!(h.state().generate.is_none());
    // On every platform: its keys open it, and so does a table's menu.
    key(&mut h, egui::Key::G, egui::Modifiers::COMMAND | egui::Modifiers::SHIFT);
    assert!(h.state().generate.is_some(), "Ctrl or Cmd with Shift and G");
    key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
    let node = {
        let st = h.state().session.as_ref().unwrap().doc();
        let c = st.by_uuid(archforge_mof::Uuid::parse(&_customer).unwrap()).unwrap();
        st.uuid(archforge_diagram::views_of(st, c).2[0]).unwrap()
    };
    let at = rect_of(&h, &format!("workspace.canvas.node.{node}")).center();
    h.event(egui::Event::PointerMoved(at));
    h.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Secondary, pressed: true, modifiers: egui::Modifiers::NONE });
    h.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Secondary, pressed: false, modifiers: egui::Modifiers::NONE });
    h.run();
    let at = rect_of(&h, "workspace.canvas.menu.generate").center();
    click(&mut h, at);
    assert!(h.state().generate.is_some(), "from the table's menu");
    assert!(painted(&h).iter().any(|t| t == "Table · Customer"), "{:?}", painted(&h));
    key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
    // With nothing to be from, there is no request to copy.
    run(&mut h, "view.select", serde_json::json!({ "ids": [] }));
    h.state_mut().canvas.active = None;
    h.state_mut().open_generate();
    h.run();
    let texts = painted(&h);
    assert!(texts.iter().any(|t| t == "Nothing selected") && texts.iter().any(|t| t.starts_with("Select a schema")), "{texts:?}");
    let at = rect_of(&h, "generate.copy").center();
    click(&mut h, at);
    assert_eq!(h.state().generate.as_ref().unwrap().copied, None);
}

/// An ER table shown on two diagrams: deleted from Models, the dialog says it disappears from both
/// (AR-187). A bare table shown there is asked about too, one shown nowhere goes at once, and the
/// schema the two diagrams belong to, shown on neither, disappears from none. In light and dark.
#[test]
fn deleting_a_table_shown_on_two_diagrams_from_models_says_two() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let mut h = workspace_harness(theme, egui::vec2(1600.0, 1120.0));
        {
            let s = h.state_mut().session.as_mut().unwrap();
            let mut id = |cmd: &str, p: serde_json::Value| s.execute(cmd, &p).unwrap().value["id"].as_str().unwrap_or_default().to_string();
            let schema = id("er.schema.add", serde_json::json!({ "name": "Shop" }));
            let customer = id("er.entity.add", serde_json::json!({ "schema": schema, "name": "Customer" }));
            id("er.attribute.add", serde_json::json!({ "entity": customer, "name": "customer_id", "type": "BIGINT", "primary_key": true }));
            let bare = id("er.entity.add", serde_json::json!({ "schema": schema, "name": "Bare" }));
            id("er.entity.add", serde_json::json!({ "schema": schema, "name": "Unplaced" }));
            for name in ["Shop schema", "Customers"] {
                let d = id("diagram.add", serde_json::json!({ "kind": archforge_er::DIAGRAM_KIND, "element": schema, "name": name }));
                for (table, x) in [(&customer, 100), (&bare, 500)] {
                    id("diagram.place", serde_json::json!({ "diagram": d, "element": table, "x": x, "y": 100, "width": 240, "height": 166 }));
                }
            }
        }
        h.run();
        let two = vec!["It disappears from 2 diagrams.".to_string()];
        select_named(&mut h, archforge_er::ENTITY, "Customer");
        archforge_ui_egui::explorer::remove_selection(h.state_mut());
        assert_eq!(asked_diagrams(&mut h), (2, two.clone()), "a table with a column");
        select_named(&mut h, archforge_er::ENTITY, "Bare");
        archforge_ui_egui::explorer::remove_selection(h.state_mut());
        assert_eq!(asked_diagrams(&mut h), (2, two), "a table with nothing else going");
        // The schema's own diagrams go with it, but it is shown on neither.
        select_named(&mut h, archforge_er::SCHEMA, "Shop");
        archforge_ui_egui::explorer::remove_selection(h.state_mut());
        assert_eq!(asked_diagrams(&mut h), (0, Vec::new()), "the schema");
        select_named(&mut h, archforge_er::ENTITY, "Unplaced");
        archforge_ui_egui::explorer::remove_selection(h.state_mut());
        assert!(h.state().explorer.delete.is_none(), "no dialog: nothing goes and no diagram shows it");
        assert!(schema_names(&h) == ["Shop"]);
        let st = h.state().session.as_ref().unwrap().doc();
        assert!(!st.alive().any(|o| archforge_core::name_of(st, o) == "Unplaced"), "removed at once");
    }
}

/// Escape on the canvas closes ER's mapping table offer, open over an ER diagram (AR-789); the
/// offer reads Escape itself too, so this holds the outcome whichever of the two ends it. In light
/// and dark.
#[test]
fn escape_on_the_canvas_closes_ers_mapping_offer() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, _, _) = diagram_harness_in(theme);
        let rel = {
            let st = h.state().session.as_ref().unwrap().doc();
            st.uuid(st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_er::RELATIONSHIP)).unwrap()).unwrap()
        };
        archforge_ui_egui::snapshot::cx(h.state_mut(), |cx| er_connect::offer_mapping(cx, rel));
        h.state_mut().canvas.focused = true;
        h.run();
        assert!(er_of(h.state()).mapping.is_some(), "{theme:?}: the offer is open");
        key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
        assert!(er_of(h.state()).mapping.is_none(), "{theme:?}: Escape closed it");
    }
}

/// A problem badge that offers a way out acts on a click as its notation says (AR-789): the badge
/// of a many-to-many line drawn from Customer to Order says the way out on hover and opens ER's
/// mapping table offer named after both; Order's badge, for its missing primary key, opens nothing.
/// In light and dark.
#[test]
fn a_many_to_many_badge_opens_the_mapping_offer_and_another_badge_nothing() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, _, _) = diagram_harness_in(theme);
        let node_of = |h: &Harness<'static, Shell>, name: &str| {
            let st = h.state().session.as_ref().unwrap().doc();
            let e = st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_er::ENTITY) && archforge_core::name_of(st, *o) == name).unwrap();
            st.uuid(archforge_diagram::views_of(st, e).2[0]).unwrap()
        };
        let (customer, order) = (node_of(&h, "Customer"), node_of(&h, "Order"));
        er_of_mut(h.state_mut()).relationship =
            er_connect::RelKind { from: archforge_er::Cardinality::ZeroOrMore, to: archforge_er::Cardinality::ZeroOrMore, identifying: false };
        let d = h.state().session.as_ref().unwrap().doc().by_uuid(h.state().canvas.active.unwrap()).unwrap();
        archforge_ui_egui::snapshot::cx(h.state_mut(), |cx| er_connect::connect(cx, d, customer, order));
        er_of_mut(h.state_mut()).mapping = None;
        let edge = {
            let s = h.state_mut().session.as_mut().unwrap();
            let st = s.doc();
            let rel = st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_er::RELATIONSHIP) && archforge_er::rules::is_many_to_many(st, *o));
            let edge = st.uuid(archforge_diagram::views_of(st, rel.unwrap()).1[0]).unwrap();
            let order = st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_er::ENTITY) && archforge_core::name_of(st, *o) == "Order");
            let order = st.uuid(order.unwrap()).unwrap().to_string();
            s.execute("er.primary-key.remove", &serde_json::json!({ "entity": order })).unwrap();
            edge
        };
        h.run();
        let badge = |what: archforge_mof::Uuid| format!("workspace.canvas.problem.{what}");
        // The line's badge says the way out while the pointer rests on it.
        let hint = "Many to many · add a mapping table";
        let before = painted(&h).iter().filter(|t| *t == hint).count();
        h.event(egui::Event::PointerMoved(rect_of(&h, &badge(edge)).center()));
        for _ in 0..40 {
            h.step();
        }
        assert_eq!(painted(&h).iter().filter(|t| *t == hint).count(), before + 1, "{theme:?}: its tooltip");
        let at = rect_of(&h, &badge(order)).center();
        click(&mut h, at);
        assert!(er_of(h.state()).mapping.is_none(), "{theme:?}: Order's badge offers nothing");
        let at = rect_of(&h, &badge(edge)).center();
        click(&mut h, at);
        assert_eq!(er_of(h.state()).mapping.as_ref().map(|m| m.name.as_str()), Some("CustomerOrder"), "{theme:?}: the offer opens");
    }
}

/// The Entity tool places a table where the canvas is clicked, its name typed in place, as one undo
/// step.
#[test]
fn the_entity_tool_places_a_table_named_in_place_as_one_undo_step() {
    let (mut h, _customer, _diagram) = diagram_harness();
    let at = rect_of(&h, "workspace.tools.entity").center();
    click(&mut h, at);
    assert_eq!(h.state().canvas.tool, archforge_ui_egui::canvas::Tool::Place("entity"));
    let canvas = rect_of(&h, "workspace.canvas");
    let before = node_ids(&h).len();
    click(&mut h, canvas.left_top() + egui::vec2(120.0, 320.0));
    let nodes = node_ids(&h);
    assert_eq!(nodes.len(), before + 1, "a node was placed: {nodes:?}");
    assert_eq!(h.state().canvas.tool, archforge_ui_egui::canvas::Tool::Select, "placing hands back to Select");
    let rename = h.state().canvas.rename.clone().expect("the name is open for typing");
    assert_eq!(rename.text, "Entity");
    assert_eq!(selected_name(&h), "Entity");
    assert!(ids(&h).iter().any(|i| i == "workspace.canvas.rename"));
    // Typing replaces the selected default; Enter commits the rename.
    h.event(egui::Event::Text("Payment".into()));
    h.run();
    key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
    assert!(h.state().canvas.rename.is_none(), "{:?}", h.state().canvas.rename);
    assert_eq!(selected_name(&h), "Payment");
    // Add and place were one step: one undo removes the node and the table together.
    let s = h.state_mut().session.as_mut().unwrap();
    s.execute("edit.undo", &serde_json::Value::Null).unwrap();
    s.execute("edit.undo", &serde_json::Value::Null).unwrap();
    h.run();
    assert_eq!(node_ids(&h).len(), before);
    let st = h.state().session.as_ref().unwrap().doc();
    assert!(!st.alive().any(|o| archforge_core::name_of(st, o) == "Entity"), "the table went with the node");
    // Escape returns to Select.
    key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
    assert_eq!(h.state().canvas.tool, archforge_ui_egui::canvas::Tool::Select);
}

#[test]
fn a_selected_notes_handle_links_it_to_a_table_or_a_relationships_line_and_delete_removes_a_link() {
    let (mut h, _customer, _order) = two_tables_harness();
    let nodes = node_ids(&h);
    let customer_node = nodes.iter().min_by(|a, b| rect_of(&h, a).left().total_cmp(&rect_of(&h, b).left())).unwrap().clone();
    let order_node = nodes.iter().find(|n| **n != customer_node).unwrap().clone();
    // A relationship between the two, and a note below them.
    let at = rect_of(&h, "workspace.tools.relationship").center();
    click(&mut h, at);
    let (from, to) = (rect_of(&h, &customer_node).center(), rect_of(&h, &order_node).center());
    drag(&mut h, from, to);
    let note = {
        let s = h.state_mut().session.as_mut().unwrap();
        let d = s.view().open_diagram.unwrap();
        let d = s.doc().uuid(d).unwrap().to_string();
        let out = s.execute("diagram.note", &serde_json::json!({ "diagram": d, "text": "Why", "x": 380, "y": 400 })).unwrap();
        let id = out.value["id"].as_str().unwrap().to_string();
        s.execute("view.select", &serde_json::json!({ "ids": [id] })).unwrap();
        id
    };
    h.run_steps(2);
    let links = |h: &Harness<'static, Shell>| {
        let st = h.state().session.as_ref().unwrap().doc();
        st.alive()
            .filter(|o| {
                st.class_of(*o).ok() == Some(archforge_diagram::EDGE)
                    && st.get(*o, archforge_diagram::edge::ELEMENT).unwrap().as_ref().is_none()
                    && st.get(*o, archforge_diagram::edge::SOURCE).unwrap().as_ref() == st.by_uuid(archforge_mof::Uuid::parse(&note).unwrap())
            })
            .map(|o| st.class_of(st.get(o, archforge_diagram::edge::TARGET).unwrap().as_ref().unwrap()).unwrap())
            .collect::<Vec<_>>()
    };
    // Its handle dragged onto Order links it to the table.
    let handle = rect_of(&h, "workspace.canvas.adorn.link").center();
    let onto = rect_of(&h, &order_node).center();
    drag(&mut h, handle, onto);
    assert_eq!(links(&h), [archforge_diagram::NODE], "{:?}", h.state().canvas.notice);
    // Selected again, dragged onto the middle of the relationship's line: linked to the line.
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.execute("view.select", &serde_json::json!({ "ids": [note] })).unwrap();
    }
    h.run_steps(2);
    let middle = rect_of(&h, &customer_node).center().lerp(rect_of(&h, &order_node).center(), 0.5);
    let handle = rect_of(&h, "workspace.canvas.adorn.link").center();
    drag(&mut h, handle, middle);
    let kinds = links(&h);
    assert_eq!(kinds.len(), 2, "{:?}", h.state().canvas.notice);
    assert!(kinds.contains(&archforge_diagram::EDGE), "one link ends on the relationship's line");
    // The link is selected after drawing; Delete removes it alone.
    key(&mut h, egui::Key::Delete, egui::Modifiers::NONE);
    assert_eq!(links(&h), [archforge_diagram::NODE]);
    assert_eq!(relationships(&h).len(), 1, "the relationship stays");
}

#[test]
fn a_cardinality_text_is_dragged_aside_and_a_double_click_puts_it_back() {
    let (mut h, _d, e, ..) = related_tables((500.0, 100.0));
    let labels = |h: &Harness<'static, Shell>| h.state().session.as_ref().unwrap().doc().list(e, archforge_diagram::edge::LABEL).unwrap().len();
    let first = h.state().canvas.ends.iter().find(|t| t.end == 1).cloned().expect("the line's texts are drawn");
    // Between the tables, clear of both.
    let to = first.rect.center() + egui::vec2(-40.0, 60.0);
    drag(&mut h, first.rect.center(), to);
    assert_eq!(labels(&h), 1, "the place is kept");
    h.run();
    let moved = h.state().canvas.ends.iter().find(|t| t.end == 1).cloned().unwrap();
    assert!((moved.rect.center() - to).length() < 2.0, "drawn where it was let go: {:?} {to:?}", moved.rect.center());
    click(&mut h, moved.rect.center());
    click(&mut h, moved.rect.center());
    h.run();
    assert_eq!(labels(&h), 0, "back beside the line");
}

/// With a schema selected, Models' `+` offers a table first and adds one; Delete asks before the
/// schema goes with what it holds.
#[test]
fn with_a_schema_selected_the_plus_adds_a_table_and_delete_asks_before_the_schema_goes() {
    let mut h = workspace_harness(ThemeKind::Light, egui::vec2(1600.0, 1120.0));
    archforge_ui_egui::explorer::add(h.state_mut(), "schema");
    h.run();
    key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
    assert!(h.state().explorer.rename.is_none());
    // The new schema is selected: + offers a table first.
    archforge_ui_egui::explorer::add(h.state_mut(), "table");
    h.run();
    key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
    let rows = archforge_ui_egui::testing::models_rows(
        archforge_ui_egui::testing::notation_uis(h.state()),
        h.state().session.as_ref().unwrap().doc(),
        &h.state().explorer,
    );
    assert_eq!(rows.iter().map(|r| r.label.as_str()).collect::<Vec<_>>(), ["Schema 1", "Table 1"]);
    assert_eq!(rows[1].depth, 1);
    // A column under the table.
    archforge_ui_egui::explorer::add(h.state_mut(), "column");
    h.run();
    key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
    let rows = archforge_ui_egui::testing::models_rows(
        archforge_ui_egui::testing::notation_uis(h.state()),
        h.state().session.as_ref().unwrap().doc(),
        &h.state().explorer,
    );
    assert_eq!(rows.iter().map(|r| r.label.as_str()).collect::<Vec<_>>(), ["Schema 1", "Table 1", "column_1"]);
    assert_eq!(rows[2].detail.as_deref(), Some("TEXT"));
    // Select the schema by its row and press Delete: it holds a table, so the dialog asks.
    let schema_row = format!("workspace.explorer.row.{}", rows[0].id);
    let at = rect_of(&h, &schema_row).center();
    click(&mut h, at);
    h.state_mut().explorer.focused = true;
    key(&mut h, egui::Key::Delete, egui::Modifiers::NONE);
    assert!(h.state().explorer.delete.is_some(), "the dialog asks");
    let at = rect_of(&h, "workspace.explorer.delete.cancel").center();
    click(&mut h, at);
    assert!(h.state().explorer.delete.is_none());
    assert_eq!(schema_names(&h), ["Schema 1"]);
    h.state_mut().explorer.focused = true;
    key(&mut h, egui::Key::Delete, egui::Modifiers::NONE);
    let at = rect_of(&h, "workspace.explorer.delete.confirm").center();
    click(&mut h, at);
    assert!(schema_names(&h).is_empty(), "the schema and its table are gone");
    key(&mut h, egui::Key::Z, egui::Modifiers::COMMAND);
    assert_eq!(schema_names(&h), ["Schema 1"], "undo brings them back");
}
