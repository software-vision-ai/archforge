//! UML's window tests of the canvas, its tools and tabs, and relationships: cards, lines and
//! labels drawn, placed, selected, moved, resized, routed and deleted on the canvas; the toolbar's
//! tools and their gestures; the diagram tabs; the notices the canvas draws at its top.

use super::*;

/// A constraint without a name, shown as a note, asks with ⇧⌫ on its note and the title reads it
/// as its row in Constraints does, by its braces (AR-216). In light and dark.
#[test]
fn deleting_an_unnamed_constraint_from_the_canvas_names_it_by_its_braces() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, made) = templates_sample(theme);
        let s = h.state_mut().session.as_mut().unwrap();
        let c =
            s.execute("uml.constraint.add", &serde_json::json!({ "context": made["Repository"], "expression": "size < 100" })).unwrap().value["id"]
                .as_str()
                .unwrap()
                .to_string();
        let note = s
            .execute("uml.constraint.show", &serde_json::json!({ "diagram": made["diagram"], "constraint": c, "as": "note", "x": 700, "y": 200 }))
            .unwrap()
            .value["id"]
            .as_str()
            .unwrap()
            .to_string();
        // Its note selected on the canvas, which has the keys.
        archforge_ui_egui::snapshot::run(h.state_mut(), "view.select", serde_json::json!({ "ids": [note] })).unwrap();
        h.state_mut().canvas.focused = true;
        h.run();
        key(&mut h, egui::Key::Delete, egui::Modifiers::SHIFT);
        h.run();
        let asked = h.state().explorer.delete.clone().expect("asks first");
        assert_eq!(asked.name, "{size < 100}", "{theme:?}");
        assert_eq!(delete_title(&h), "Delete {size < 100} from the model?", "{theme:?}");
    }
}

/// An n-ary association drawn without a name, as it is by default, asks with ⇧⌫ on its diamond
/// and its title says what goes: this n-ary association (AR-216). In light and dark.
#[test]
fn deleting_an_unnamed_n_ary_association_from_the_canvas_names_its_kind() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, ids) = nary_sample(theme, false);
        let s = h.state_mut().session.as_mut().unwrap();
        let d = {
            let st = s.doc();
            let n = st.by_uuid(archforge_mof::Uuid::parse(&ids["customer"]).unwrap()).unwrap();
            st.uuid(archforge_diagram::diagram_of(st, n).unwrap()).unwrap().to_string()
        };
        let classes = [ids["Customer"].clone(), ids["Product"].clone(), ids["Catalog"].clone()];
        let a = s.execute("uml.association.add-nary", &serde_json::json!({ "classes": classes })).unwrap().value;
        let a_id = a["id"].as_str().unwrap().to_string();
        let n = s
            .execute("diagram.place", &serde_json::json!({ "diagram": d, "element": a_id, "x": 400, "y": 360, "width": 44, "height": 28 }))
            .unwrap()
            .value["id"]
            .as_str()
            .unwrap()
            .to_string();
        let ends: Vec<String> = a["ends"].as_array().unwrap().iter().map(|e| e.as_str().unwrap().to_string()).collect();
        for (end, card) in ends.iter().zip([&ids["customer"], &ids["product"], &ids["catalog"]]) {
            s.execute("diagram.connect", &serde_json::json!({ "diagram": d, "element": end, "source": n, "target": card })).unwrap();
        }
        h.run();
        let at = card(&h, &n);
        click(&mut h, at);
        key(&mut h, egui::Key::Delete, egui::Modifiers::SHIFT);
        h.run();
        let asked = h.state().explorer.delete.clone().expect("asks first");
        assert_eq!(asked.name, "this n-ary association");
        assert_eq!(delete_title(&h), "Delete this n-ary association from the model?", "{theme:?}");
    }
}

/// The Package tool makes the package, its card, its own diagram and the binding that names that
/// diagram its default as one undo step, and leaves the package selected; Escape before the click
/// makes nothing (AR-581).
#[test]
fn the_package_tool_makes_the_package_its_diagram_and_its_default_as_one_undo_step() {
    use archforge_uml::defaults::{DefaultDiagram, resolve};
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let mut h = workspace_harness(theme, egui::vec2(1600.0, 1000.0));
        let top = {
            let s = h.state_mut().session.as_mut().unwrap();
            let root = s.execute("uml.model.root", &serde_json::json!({})).unwrap().value["id"].as_str().unwrap().to_string();
            let d = s
                .execute("diagram.add", &serde_json::json!({ "kind": archforge_uml::STRUCTURE_DIAGRAM, "element": root, "name": "Top" }))
                .unwrap()
                .value;
            archforge_mof::Uuid::parse(d["id"].as_str().unwrap()).unwrap()
        };
        archforge_ui_egui::canvas::open(h.state_mut(), top);
        h.run();
        let made =
            |h: &Harness<'static, Shell>| (packages_diagrams_bindings(h), h.state().session.as_ref().unwrap().undo_label().map(str::to_string));
        let before = made(&h);
        // Escape puts the tool down: nothing is made.
        let tool = rect_of(&h, "workspace.tools.package").center();
        click(&mut h, tool);
        h.state_mut().canvas.focused = true;
        key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
        assert_eq!(h.state().canvas.tool, archforge_shell_api::canvas::Tool::Select, "{theme:?}: the tool is put down");
        let centre = rect_of(&h, "workspace.canvas").center();
        click(&mut h, centre);
        assert_eq!(made(&h), before, "{theme:?}: nothing made");
        // Placed: the package, its diagram and its binding, one step.
        click(&mut h, tool);
        click(&mut h, centre);
        let [packages, diagrams, bindings] = before.0;
        assert_eq!(packages_diagrams_bindings(&h), [packages + 1, diagrams + 1, bindings + 1], "{theme:?}");
        let s = h.state().session.as_ref().unwrap();
        assert_eq!(s.undo_label(), Some("Add package"), "{theme:?}: one undo step");
        let package = h.state().canvas.rename.as_ref().expect("named in place").element;
        let (st, p) = (s.doc(), s.doc().by_uuid(package).unwrap());
        assert_eq!(s.view().selection, [p], "{theme:?}: the package selected, not its diagram");
        let Ok(DefaultDiagram::Bound(d)) = resolve(st, p) else { panic!("{theme:?}: no default for the package") };
        assert_eq!(st.get(d, archforge_diagram::diagram::ELEMENT).unwrap().as_ref(), Some(p), "{theme:?}: its own diagram");
        assert_eq!(archforge_ui_egui::diagrams::diagram_name(st, d), "Package 1", "{theme:?}: named like it");
        key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
        h.state_mut().session.as_mut().unwrap().undo().unwrap();
        h.run();
        assert_eq!(packages_diagrams_bindings(&h), before.0, "{theme:?}: one Undo takes all three back");
    }
}

/// A package dragged from Models onto a class diagram gets a diagram of its own, bound as its
/// default in the same step, only when it has no structure diagram yet; one that has one keeps it
/// and its default (AR-581).
#[test]
fn a_package_dropped_from_models_gets_a_default_diagram_only_when_it_has_none() {
    use archforge_uml::defaults::{DefaultDiagram, resolve};
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let mut h = workspace_harness(theme, egui::vec2(1600.0, 1000.0));
        let id = |v: serde_json::Value| archforge_mof::Uuid::parse(v["id"].as_str().unwrap()).unwrap();
        let (root, top, sales, billing, billings) = {
            let s = h.state_mut().session.as_mut().unwrap();
            let root = id(s.execute("uml.model.root", &serde_json::json!({})).unwrap().value);
            let top = own_diagram(s, root);
            let sales = id(s.execute("uml.package.add", &serde_json::json!({ "name": "Sales" })).unwrap().value);
            let billing = id(s.execute("uml.package.add", &serde_json::json!({ "name": "Billing" })).unwrap().value);
            (root, top, sales, billing, own_diagram(s, billing))
        };
        archforge_ui_egui::canvas::open(h.state_mut(), top);
        h.state_mut().state.show_explorer(archforge_shell_api::state::Explorer::Models);
        h.state_mut().explorer.expanded.insert(root);
        h.run();
        let default = |h: &Harness<'static, Shell>, p: archforge_mof::Uuid| {
            let st = h.state().session.as_ref().unwrap().doc();
            match resolve(st, st.by_uuid(p).unwrap()) {
                Ok(DefaultDiagram::Bound(d)) => Some(st.uuid(d).unwrap()),
                other => {
                    assert_eq!(other, Ok(DefaultDiagram::NoDiagramYet), "{theme:?}");
                    None
                }
            }
        };
        assert_eq!(default(&h, sales), None, "{theme:?}: Sales has no diagram yet");
        let canvas = rect_of(&h, "workspace.canvas");
        let before = packages_diagrams_bindings(&h);
        // Sales, without one: placed, with its diagram and its binding, one step.
        let from = rect_of(&h, &format!("workspace.explorer.row.{sales}")).center();
        drag(&mut h, from, canvas.center() - egui::vec2(300.0, 0.0));
        assert_eq!(h.state().session.as_ref().unwrap().undo_label(), Some("Place package"), "{theme:?}: {:?}", h.state().canvas.notice);
        assert_eq!(packages_diagrams_bindings(&h), [before[0], before[1] + 1, before[2] + 1], "{theme:?}");
        let made = default(&h, sales).expect("Sales's default");
        let st = h.state().session.as_ref().unwrap().doc();
        assert_eq!(archforge_ui_egui::diagrams::diagram_name(st, st.by_uuid(made).unwrap()), "Sales", "{theme:?}: named like it");
        // Billing, which has one: placed, nothing made, its default as it was.
        let from = rect_of(&h, &format!("workspace.explorer.row.{billing}")).center();
        drag(&mut h, from, canvas.center() + egui::vec2(300.0, 0.0));
        assert_eq!(packages_diagrams_bindings(&h), [before[0], before[1] + 1, before[2] + 1], "{theme:?}: no second diagram");
        assert_eq!(default(&h, billing), Some(billings), "{theme:?}");
        let st = h.state().session.as_ref().unwrap().doc();
        assert!(archforge_shell_api::editing::on_diagram(st, st.by_uuid(top).unwrap(), st.by_uuid(billing).unwrap()), "{theme:?}: Billing placed");
        // One Undo takes Billing's card back, the next Sales's card, diagram and binding.
        let s = h.state_mut().session.as_mut().unwrap();
        s.undo().unwrap();
        s.undo().unwrap();
        h.run();
        assert_eq!((packages_diagrams_bindings(&h), default(&h, sales)), (before, None), "{theme:?}");
    }
}

/// A package of another branch dragged from Models onto a class diagram is shown there once, its
/// owner line naming where it lives, and keeps its owner; dropped again it is said to be there and
/// nothing changes; its card removed, it is still in Models (AR-569).
#[test]
fn a_package_of_another_branch_dropped_from_models_is_shown_once_and_keeps_its_owner() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, root, _) = owning_path_harness(theme);
        let add = |h: &mut Harness<'static, Shell>, name: &str, parent: &str| {
            let id = owning_path_made(h, "uml.package.add", serde_json::json!({ "name": name, "parent": parent }));
            archforge_mof::Uuid::parse(&id).unwrap()
        };
        let commerce = add(&mut h, "Commerce", &root);
        let operations = add(&mut h, "Operations", &root);
        let archive = add(&mut h, "Archive", &operations.to_string());
        let shown = own_diagram(h.state_mut().session.as_mut().unwrap(), commerce);
        archforge_ui_egui::canvas::open(h.state_mut(), shown);
        h.state_mut().state.show_explorer(archforge_shell_api::state::Explorer::Models);
        h.state_mut().explorer.expanded.extend([archforge_mof::Uuid::parse(&root).unwrap(), operations]);
        h.run();
        // Archive's owner, and its cards with the diagram each is on.
        let owner_and_cards = |h: &Harness<'static, Shell>| {
            let st = h.state().session.as_ref().unwrap().doc();
            let package = st.by_uuid(archive).unwrap();
            let owner = archforge_uml::commands::package_of(st, package).map(|p| st.uuid(p).unwrap());
            let on = |n: archforge_mof::Obj| archforge_diagram::diagram_of(st, n).map(|d| st.uuid(d).unwrap());
            (owner, archforge_diagram::views_of(st, package).2.into_iter().map(|n| (st.uuid(n).unwrap(), on(n))).collect::<Vec<_>>())
        };
        let canvas = rect_of(&h, "workspace.canvas");
        let row = rect_of(&h, &format!("workspace.explorer.row.{archive}")).center();
        drag(&mut h, row, canvas.center());
        assert_eq!(h.state().session.as_ref().unwrap().undo_label(), Some("Place package"), "{theme:?}: {:?}", h.state().canvas.notice);
        let (owner, cards) = owner_and_cards(&h);
        let [(card, Some(on))] = cards[..] else { panic!("{theme:?}: one card of Archive: {cards:?}") };
        assert_eq!((owner, on), (Some(operations), shown), "{theme:?}: on Commerce's diagram, still owned by Operations");
        let line = h.state().canvas.card_labels.iter().find(|b| b.node == card).map(|b| b.whole.clone());
        assert_eq!(line.as_deref(), Some("Owner: Commerce platform › Operations"), "{theme:?}: its owner line");
        // Dropped again: said so, nothing changed.
        let state = |h: &Harness<'static, Shell>| {
            let s = h.state().session.as_ref().unwrap();
            (s.revision(), s.undo_label().map(str::to_string), packages_diagrams_bindings(h), owner_and_cards(h))
        };
        let before = state(&h);
        drag(&mut h, row, canvas.center() + egui::vec2(200.0, 0.0));
        assert_eq!(h.state().canvas.notice.as_deref(), Some("Already on this diagram"), "{theme:?}");
        assert_eq!(state(&h), before, "{theme:?}: nothing changed");
        // Its card removed from the diagram, Archive stays in Models, under Operations.
        archforge_ui_egui::testing::unplace(h.state_mut(), vec![card]);
        h.run();
        assert_eq!(owner_and_cards(&h), (Some(operations), Vec::new()), "{theme:?}: no card, the same owner");
        assert!(ids(&h).contains(&format!("workspace.explorer.row.{archive}")), "{theme:?}: still in Models");
    }
}

/// The Package tool places a package with its own diagram, named in place as one undo step; that
/// diagram opens from the selected card, the path bar leads back to the view as it was left, ⌘↓ and
/// ⌘↑ do the same, and zooming in on the card enters it and out comes back. In light and dark.
#[test]
fn a_package_placed_on_a_class_diagram_leads_to_its_own_diagram_by_open_the_path_keys_and_zoom() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let mut h = workspace_harness(theme, egui::vec2(1600.0, 1000.0));
        let root_diagram = {
            let s = h.state_mut().session.as_mut().unwrap();
            let root = s.execute("uml.model.root", &serde_json::json!({})).unwrap().value["id"].as_str().unwrap().to_string();
            let d = s
                .execute(
                    "diagram.add",
                    &serde_json::json!({ "kind": archforge_uml::STRUCTURE_DIAGRAM, "element": root, "name": "Commerce platform" }),
                )
                .unwrap()
                .value;
            archforge_mof::Uuid::parse(d["id"].as_str().unwrap()).unwrap()
        };
        archforge_ui_egui::canvas::open(h.state_mut(), root_diagram);
        h.run();
        let active = |h: &Harness<'static, Shell>| h.state().canvas.active;
        // The Package tool: a package in the root, its card and its own diagram, its name open.
        let at = rect_of(&h, "workspace.tools.package").center();
        click(&mut h, at);
        let centre = rect_of(&h, "workspace.canvas").center();
        click(&mut h, centre);
        let rename = h.state().canvas.rename.clone().expect("the package is named in place");
        assert_eq!(h.state().session.as_ref().unwrap().undo_label(), Some("Add package"), "{theme:?}: one undo step");
        h.event(egui::Event::Text("Sales".into()));
        h.run();
        key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
        let (package, inner) = {
            let st = h.state().session.as_ref().unwrap().doc();
            let p = st.by_uuid(rename.element).unwrap();
            assert_eq!(archforge_core::name_of(st, p), "Sales");
            let d = default_of(st, p).expect("its own diagram");
            assert_eq!(archforge_ui_egui::diagrams::diagram_name(st, d), "Sales", "{theme:?}: named with the package");
            (rename.element, st.uuid(d).unwrap())
        };
        // Open on the selected package; the path bar leads back, to the view as it was left.
        h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [package.to_string()] })).unwrap();
        h.run();
        let left = (h.state().canvas.pan, h.state().canvas.zoom);
        let at = rect_of(&h, &format!("workspace.canvas.node.{}.enter", rename.node)).center();
        click(&mut h, at);
        assert_eq!(active(&h), Some(inner), "{theme:?}: Open opens the package's diagram");
        assert_eq!(h.state().canvas.zoom, 1.0, "{theme:?}: at 100 % (AR-110)");
        let found = ids(&h);
        assert!(found.iter().any(|i| i == "workspace.canvas.path.0") && found.iter().any(|i| i == "workspace.canvas.path.1"), "{found:?}");
        rest(&mut h);
        let at = rect_of(&h, "workspace.canvas.path.0").center();
        click(&mut h, at);
        assert_eq!(active(&h), Some(root_diagram));
        assert_eq!((h.state().canvas.pan, h.state().canvas.zoom), left, "{theme:?}: the parent as it was left");
        // ⌘↓ and ⌘↑ do the same from the keyboard, in both key shapes; with Shift, Alt or a Mac's
        // Control more they do nothing.
        let (mac, shift, alt) = (egui::Modifiers::MAC_CMD | egui::Modifiers::COMMAND, egui::Modifiers::SHIFT, egui::Modifiers::ALT);
        for (shape, extras) in [(mac, vec![shift, alt, egui::Modifiers::CTRL]), (egui::Modifiers::CTRL | egui::Modifiers::COMMAND, vec![shift, alt])]
        {
            rest(&mut h);
            h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [package.to_string()] })).unwrap();
            h.state_mut().canvas.focused = true;
            h.run();
            for (arrow, here, there) in [(egui::Key::ArrowDown, root_diagram, inner), (egui::Key::ArrowUp, inner, root_diagram)] {
                for &more in &extras {
                    key(&mut h, arrow, shape | more);
                    assert_eq!(active(&h), Some(here), "{theme:?}: {arrow:?} with {:?}", shape | more);
                }
                key(&mut h, arrow, shape);
                assert_eq!(active(&h), Some(there), "{theme:?}: {arrow:?} with {shape:?}");
                rest(&mut h);
                h.state_mut().canvas.focused = true;
            }
        }
        // Zooming in until the package fills the view enters it; zooming out on it comes back.
        rest(&mut h);
        let node = {
            let st = h.state().session.as_ref().unwrap().doc();
            st.by_uuid(rename.node).unwrap()
        };
        let card = rect_of(&h, &format!("workspace.canvas.node.{}", rename.node)).center();
        h.event(egui::Event::PointerMoved(card));
        for _ in 0..40 {
            if active(&h) == Some(inner) {
                break;
            }
            h.event(egui::Event::Zoom(1.2));
            h.step();
        }
        assert_eq!(active(&h), Some(inner), "{theme:?}: zooming in entered Sales");
        h.run();
        assert_eq!(h.state().canvas.zoom, 1.0, "{theme:?}: entered at 100 %");
        rest(&mut h);
        h.event(egui::Event::PointerMoved(rect_of(&h, "workspace.canvas").center()));
        for _ in 0..40 {
            if active(&h) == Some(root_diagram) {
                break;
            }
            h.event(egui::Event::Zoom(0.8));
            h.step();
        }
        assert_eq!(active(&h), Some(root_diagram), "{theme:?}: zooming out came back");
        h.run();
        // The parent at 100 %, the package left centred in the canvas.
        assert_eq!(h.state().canvas.zoom, 1.0, "{theme:?}: the parent at 100 %");
        let canvas = rect_of(&h, "workspace.canvas");
        let shown = rect_of(&h, &format!("workspace.canvas.node.{}", rename.node));
        assert!((shown.center() - canvas.center()).length() < 2.0, "{theme:?}: the package left centred, {shown:?} in {canvas:?}");
        // A double click still renames; it does not navigate.
        rest(&mut h);
        let _ = node;
        double_click(&mut h, shown.center());
        assert_eq!(h.state().canvas.rename.as_ref().map(|r| r.element), Some(package), "{theme:?}: renaming");
        assert_eq!(active(&h), Some(root_diagram));
        key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
        // Saved and opened again, the package leads to the same diagram.
        let rows = {
            let (tables, junctions) = archforge_notations::NOTATIONS.storage();
            archforge_mof::rows::snapshot(h.state().session.as_ref().unwrap().doc(), &tables, &junctions).unwrap()
        };
        let mut again = workspace_harness(theme, egui::vec2(1600.0, 1000.0));
        again.state_mut().open_project(archforge_ui_egui::Opened {
            name: "Commerce platform".into(),
            path: "/tmp/Again.archforge".into(),
            model: Some(std::sync::Arc::new(rows)),
            ..Default::default()
        });
        again.run();
        let st = again.state().session.as_ref().unwrap().doc();
        let p = st.by_uuid(package).expect("the package");
        assert_eq!(default_of(st, p).and_then(|d| st.uuid(d).ok()), Some(inner), "{theme:?}: the same diagram");
    }
}

/// Open, ⌘↓ and zooming in enter a package through its default diagram, the one its binding names,
/// never the first found: Sales's first diagram is made first, and its second is then chosen with
/// `uml.package.default.set`. The query agents ask, `uml.package.diagram`, names that same diagram.
/// Billing, with no diagram yet, opens nothing and makes nothing, the model as it was, and the
/// notice under the path bar says why in the chosen language (AR-582). In light and dark.
#[test]
fn entry_reaches_the_bound_default_never_the_first_found_and_makes_nothing_without_one() {
    use serde_json::json;
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let mut h = workspace_harness(theme, egui::vec2(1600.0, 1000.0));
        let id = |v: serde_json::Value| archforge_mof::Uuid::parse(v["id"].as_str().unwrap()).unwrap();
        let (top, [sales, billing], first, chosen, cards) = {
            let s = h.state_mut().session.as_mut().unwrap();
            let root = id(s.execute("uml.model.root", &json!({})).unwrap().value);
            let top = own_diagram(s, root);
            let sales = id(s.execute("uml.package.add", &json!({ "name": "Sales" })).unwrap().value);
            let billing = id(s.execute("uml.package.add", &json!({ "name": "Billing" })).unwrap().value);
            let first = own_diagram(s, sales);
            let detail = json!({ "kind": archforge_uml::STRUCTURE_DIAGRAM, "element": sales.to_string(), "name": "Detail" });
            let chosen = id(s.execute("diagram.add", &detail).unwrap().value);
            let at = s.token();
            let token = json!({ "history": at.history, "revision": at.revision });
            let choose = json!({ "package": sales.to_string(), "diagram": chosen.to_string(), "expected_revision": token });
            s.execute("uml.package.default.set", &choose).unwrap();
            // Sales's card big enough for zooming in on it to make it cover the canvas.
            let mut place = |p: archforge_mof::Uuid, x: i32, width: i32| {
                let at = json!({ "diagram": top.to_string(), "element": p.to_string(), "x": x, "y": 160, "width": width, "height": 260 });
                id(s.execute("diagram.place", &at).unwrap().value)
            };
            let cards = [place(sales, 120, 400), place(billing, 600, 240)];
            (top, [sales, billing], first, chosen, cards)
        };
        let asked = h.state_mut().session.as_mut().unwrap().execute("uml.package.diagram", &json!({ "package": sales.to_string() }));
        assert_eq!(asked.map(|o| id(o.value)).ok(), Some(chosen), "{theme:?}: agents ask for the same diagram, not the first {first}");
        let active = |h: &Harness<'static, Shell>| h.state().canvas.active;
        let back_on_top = |h: &mut Harness<'static, Shell>, p: archforge_mof::Uuid| {
            archforge_ui_egui::canvas::open(h.state_mut(), top);
            h.run();
            rest(h);
            h.state_mut().session.as_mut().unwrap().execute("view.select", &json!({ "ids": [p.to_string()] })).unwrap();
            h.state_mut().canvas.focused = true;
            h.run();
        };
        let down = egui::Modifiers::MAC_CMD | egui::Modifiers::COMMAND;
        // Open, ⌘↓ and zooming in on Sales each reach the chosen diagram.
        back_on_top(&mut h, sales);
        let at = rect_of(&h, &format!("workspace.canvas.node.{}.enter", cards[0])).center();
        click(&mut h, at);
        assert_eq!(active(&h), Some(chosen), "{theme:?}: Open");
        back_on_top(&mut h, sales);
        key(&mut h, egui::Key::ArrowDown, down);
        assert_eq!(active(&h), Some(chosen), "{theme:?}: ⌘↓");
        back_on_top(&mut h, sales);
        h.event(egui::Event::PointerMoved(rect_of(&h, &format!("workspace.canvas.node.{}", cards[0])).center()));
        for _ in 0..40 {
            if active(&h) != Some(top) {
                break;
            }
            h.event(egui::Event::Zoom(1.2));
            h.step();
        }
        assert_eq!(active(&h), Some(chosen), "{theme:?}: zooming in");
        h.run(); // The chosen diagram lands; a landing never carries over to the next tab (AR-666).
        // Billing has none: Open and ⌘↓ stay, make nothing and say why, in German.
        h.state_mut().state.language = "de".into();
        back_on_top(&mut h, billing);
        let rows = |h: &Harness<'static, Shell>| {
            let (tables, junctions) = archforge_notations::NOTATIONS.storage();
            let s = h.state().session.as_ref().unwrap();
            (archforge_mof::rows::snapshot(s.doc(), &tables, &junctions).unwrap(), s.undo_label().map(str::to_string))
        };
        let before = rows(&h);
        let at = rect_of(&h, &format!("workspace.canvas.node.{}.enter", cards[1])).center();
        click(&mut h, at);
        let said = "Billing hat noch kein Klassendiagramm, daher gibt es nichts zu öffnen. Erstellen Sie zuerst ein Klassendiagramm dafür.";
        assert_eq!((active(&h), h.state().canvas.notice.as_deref()), (Some(top), Some(said)), "{theme:?}: Open");
        assert!(ids(&h).iter().any(|i| i == "workspace.canvas.notice"), "{theme:?}: the notice is shown");
        h.state_mut().canvas.notice = None;
        h.state_mut().canvas.focused = true;
        key(&mut h, egui::Key::ArrowDown, down);
        assert_eq!((active(&h), h.state().canvas.notice.as_deref()), (Some(top), Some(said)), "{theme:?}: ⌘↓");
        assert!(rows(&h) == before, "{theme:?}: the model and its history as they were");
    }
    archforge_ui_egui::i18n::set_language("en");
}

/// A package's card dragged over another package or an ancestor in the path bar only arranges the
/// diagram: no ring, no prompt, its owner stays and one Undo puts the card back (AR-566). Its owner
/// changes in Properties and in Models, which refuse it into a package it holds, said in the chosen
/// language with history untouched (AR-102): its cards stay, their owner lines follow, nothing is
/// added to the new owner's diagram, and Undo brings both back (AR-568).
#[test]
fn a_package_dragged_onto_a_package_or_the_path_bar_only_moves_and_changes_owner_in_properties_and_models() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let mut h = workspace_harness(theme, egui::vec2(1600.0, 1000.0));
        let id = |v: serde_json::Value| archforge_mof::Uuid::parse(v["id"].as_str().unwrap()).unwrap();
        let (top, sales, billing, sales_card, billing_card) = {
            let s = h.state_mut().session.as_mut().unwrap();
            let root = id(s.execute("uml.model.root", &serde_json::json!({})).unwrap().value).to_string();
            let top = id(s
                .execute("diagram.add", &serde_json::json!({ "kind": archforge_uml::STRUCTURE_DIAGRAM, "element": root, "name": "Top" }))
                .unwrap()
                .value);
            let mut made = Vec::new();
            for (name, x) in [("Sales", 80), ("Billing", 520)] {
                let p = id(s.execute("uml.package.add", &serde_json::json!({ "name": name })).unwrap().value);
                let card = id(s
                    .execute(
                        "diagram.place",
                        &serde_json::json!({ "diagram": top.to_string(), "element": p.to_string(), "x": x, "y": 120, "width": 240, "height": 148 }),
                    )
                    .unwrap()
                    .value);
                own_diagram(s, p);
                made.push((p, card));
            }
            (top, made[0].0, made[1].0, made[0].1, made[1].1)
        };
        // Sales shown on a second diagram of the root as well.
        let (overview, sales_card2) = {
            let s = h.state_mut().session.as_mut().unwrap();
            let root = s.execute("uml.model.root", &serde_json::json!({})).unwrap().value["id"].as_str().unwrap().to_string();
            let d = id(s
                .execute("diagram.add", &serde_json::json!({ "kind": archforge_uml::STRUCTURE_DIAGRAM, "element": root, "name": "Overview" }))
                .unwrap()
                .value);
            let at = serde_json::json!({ "diagram": d.to_string(), "element": sales.to_string(), "x": 80, "y": 120, "width": 240, "height": 148 });
            (d, id(s.execute("diagram.place", &at).unwrap().value))
        };
        archforge_ui_egui::canvas::open(h.state_mut(), top);
        h.run();
        let parent = |h: &Harness<'static, Shell>, p: archforge_mof::Uuid| {
            let st = h.state().session.as_ref().unwrap().doc();
            archforge_uml::commands::package_of(st, st.by_uuid(p).unwrap()).map(|o| archforge_core::name_of(st, o))
        };
        let card_at = |h: &Harness<'static, Shell>, card: archforge_mof::Uuid| {
            let st = h.state().session.as_ref().unwrap().doc();
            let b = archforge_diagram::grid::bounds_of(st, st.by_uuid(card).unwrap());
            (b.x, b.y)
        };
        // Whether the last frame ringed `target` in the accent, more boldly than a selection is.
        let accent = archforge_ui_egui::ux::palette(theme == ThemeKind::Dark).accent;
        let ringed = |h: &Harness<'static, Shell>, target: egui::Rect| {
            /// Whether `s`, or a shape it holds, is a rectangle round `target` stroked in `accent`
            /// at least 2 points wide.
            fn walk(s: &egui::Shape, accent: archforge_ui_egui::ux::Colour, target: egui::Rect) -> bool {
                match s {
                    egui::Shape::Rect(r) => r.stroke.color == egui::Color32::from(accent) && r.stroke.width >= 2.0 && r.rect.contains_rect(target),
                    egui::Shape::Vec(v) => v.iter().any(|s| walk(s, accent, target)),
                    _ => false,
                }
            }
            h.output().shapes.iter().any(|c| walk(&c.shape, accent, target))
        };
        // Whether any text the last frame painted names both `a` and `b`, as a move prompt would.
        let prompt = |h: &Harness<'static, Shell>, a: &str, b: &str| painted(h).iter().any(|t| t.contains(a) && t.contains(b));
        // Held over Billing: neither ringed nor a prompt; let go, Sales only moves on the diagram.
        let (from, to) = (
            rect_of(&h, &format!("workspace.canvas.node.{sales_card}")).center(),
            rect_of(&h, &format!("workspace.canvas.node.{billing_card}")).center(),
        );
        let (billing_rect, before) = (rect_of(&h, &format!("workspace.canvas.node.{billing_card}")), card_at(&h, sales_card));
        h.event(egui::Event::PointerMoved(from));
        h.event(egui::Event::PointerButton { pos: from, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE });
        h.run();
        h.event(egui::Event::PointerMoved(to));
        h.run();
        assert!(!ringed(&h, billing_rect), "{theme:?}: Billing is not ringed");
        assert!(!prompt(&h, "Sales", "Billing"), "{theme:?}: no prompt: {:?}", painted(&h));
        h.event(egui::Event::PointerButton { pos: to, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE });
        h.run();
        assert_eq!(parent(&h, sales).as_deref(), Some("Commerce platform"), "{theme:?}: the owner stays");
        assert_eq!(h.state().session.as_ref().unwrap().undo_label(), Some("Move"), "{theme:?}: a diagram step");
        assert_ne!(card_at(&h, sales_card), before, "{theme:?}: the card moved");
        assert!(!prompt(&h, "Sales", "Billing"), "{theme:?}: no toast");
        // One Undo puts the card back, its owner untouched.
        h.state_mut().session.as_mut().unwrap().undo().unwrap();
        h.run();
        assert_eq!(card_at(&h, sales_card), before, "{theme:?}: back where it was");
        assert_eq!(parent(&h, sales).as_deref(), Some("Commerce platform"));
        // Both selected and dragged: one diagram step moves both, and one Undo puts both back.
        h.state_mut()
            .session
            .as_mut()
            .unwrap()
            .execute("view.select", &serde_json::json!({ "ids": [sales.to_string(), billing.to_string()] }))
            .unwrap();
        h.run();
        let both = (card_at(&h, sales_card), card_at(&h, billing_card));
        drag(&mut h, from, from + egui::vec2(0.0, 260.0));
        let moved = (card_at(&h, sales_card), card_at(&h, billing_card));
        assert!(moved.0 != both.0 && moved.1 != both.1, "{theme:?}: both moved: {both:?} to {moved:?}");
        assert_eq!(h.state().session.as_ref().unwrap().undo_label(), Some("Move"), "{theme:?}: a diagram step");
        h.state_mut().session.as_mut().unwrap().undo().unwrap();
        h.run();
        assert_eq!((card_at(&h, sales_card), card_at(&h, billing_card)), both, "{theme:?}: one Undo puts both back");
        assert_eq!((parent(&h, sales).as_deref(), parent(&h, billing).as_deref()), (Some("Commerce platform"), Some("Commerce platform")));
        // Properties: the parent package, the keyboard's way.
        h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [sales.to_string()] })).unwrap();
        h.state_mut().state.show_panel(archforge_shell_api::state::Panel::Properties);
        h.run();
        let at = rect_of(&h, "workspace.properties.package.parent").center();
        click(&mut h, at);
        let choices: Vec<String> = ids(&h).into_iter().filter(|i| i.starts_with("workspace.properties.package.parent.")).collect();
        assert_eq!(choices.len(), 2, "{theme:?}: the root and Billing, never Sales itself: {choices:?}");
        let at = rect_of(&h, "workspace.properties.package.parent.1").center();
        click(&mut h, at);
        assert_eq!(parent(&h, sales).as_deref(), Some("Billing"));
        assert_eq!(h.state().session.as_ref().unwrap().undo_label(), Some("Move package"), "{theme:?}: one step");
        assert_eq!(h.state().canvas.toast.as_ref().map(|t| t.text.as_str()), Some("Sales moved into Billing"), "{theme:?}");
        // Its two cards stay, their owner lines naming Billing; Billing's diagram gets none (AR-568).
        let inner = {
            let st = h.state().session.as_ref().unwrap().doc();
            st.uuid(default_of(st, st.by_uuid(billing).unwrap()).unwrap()).unwrap()
        };
        let lines_say = |h: &mut Harness<'static, Shell>, owner: &str| {
            let mut said = Vec::new();
            for (d, card) in [(overview, sales_card2), (top, sales_card)] {
                archforge_ui_egui::canvas::open(h.state_mut(), d);
                h.run();
                said.push(h.state().canvas.card_labels.iter().find(|b| b.node == card).map(|b| b.whole.clone()));
            }
            let st = h.state().session.as_ref().unwrap().doc();
            let mut on: Vec<_> = archforge_diagram::views_of(st, st.by_uuid(sales).unwrap()).2.into_iter().map(|n| st.uuid(n).unwrap()).collect();
            on.sort();
            let mut cards = vec![sales_card, sales_card2];
            cards.sort();
            assert_eq!(on, cards, "{theme:?}: its two cards, no other");
            assert_eq!(said, vec![Some(format!("Owner: {owner}")); 2], "{theme:?}: both owner lines");
        };
        lines_say(&mut h, "Commerce platform › Billing");
        // Placed on Billing's diagram later, by its own step.
        let card = {
            let s = h.state_mut().session.as_mut().unwrap();
            let at =
                serde_json::json!({ "diagram": inner.to_string(), "element": sales.to_string(), "x": 80, "y": 120, "width": 240, "height": 148 });
            id(s.execute("diagram.place", &at).unwrap().value)
        };
        archforge_ui_egui::canvas::open(h.state_mut(), inner);
        h.run();
        // Held over the root in the path bar: neither ringed nor a prompt; let go, Sales only moves
        // on Billing's diagram, which stays open.
        let (from, pill, before) =
            (rect_of(&h, &format!("workspace.canvas.node.{card}")).center(), rect_of(&h, "workspace.canvas.path.0"), card_at(&h, card));
        h.event(egui::Event::PointerMoved(from));
        h.event(egui::Event::PointerButton { pos: from, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE });
        h.run();
        h.event(egui::Event::PointerMoved(pill.center()));
        h.run();
        assert!(!ringed(&h, pill), "{theme:?}: the root is not ringed");
        assert!(!prompt(&h, "Sales", "Commerce platform"), "{theme:?}: no prompt: {:?}", painted(&h));
        h.event(egui::Event::PointerButton {
            pos: pill.center(),
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        });
        h.run();
        assert_eq!(parent(&h, sales).as_deref(), Some("Billing"), "{theme:?}: the path bar takes no package");
        assert_eq!(h.state().canvas.active, Some(inner), "{theme:?}: still on Billing's diagram");
        assert_eq!(h.state().session.as_ref().unwrap().undo_label(), Some("Move"), "{theme:?}: a diagram step");
        assert_ne!(card_at(&h, card), before, "{theme:?}: the card moved");
        // Undo puts the card back, takes the placement back, then the Properties move: the owner
        // lines name the root again, and both cards stay.
        h.state_mut().session.as_mut().unwrap().undo().unwrap();
        h.run();
        assert_eq!((card_at(&h, card), parent(&h, sales).as_deref()), (before, Some("Billing")), "{theme:?}: only the card back");
        h.state_mut().session.as_mut().unwrap().undo().unwrap();
        lines_say(&mut h, "Commerce platform › Billing");
        h.state_mut().session.as_mut().unwrap().undo().unwrap();
        assert_eq!(parent(&h, sales).as_deref(), Some("Commerce platform"), "{theme:?}: Properties' move undone");
        lines_say(&mut h, "Commerce platform");
        // In Models, a package dropped onto a package it holds is refused, said in German, and
        // the history stays as it was.
        let lines = id(h
            .state_mut()
            .session
            .as_mut()
            .unwrap()
            .execute("uml.package.add", &serde_json::json!({ "name": "Lines", "parent": sales.to_string() }))
            .unwrap()
            .value);
        h.state_mut().state.show_explorer(archforge_shell_api::state::Explorer::Models);
        h.state_mut().state.language = "de".into();
        let root = {
            let st = h.state().session.as_ref().unwrap().doc();
            st.uuid(archforge_uml::commands::root_package(st).unwrap()).unwrap()
        };
        h.state_mut().explorer.expanded.extend([root, sales]);
        h.run();
        let history = |h: &Harness<'static, Shell>| {
            let s = h.state().session.as_ref().unwrap();
            (s.revision(), s.undo_label().map(str::to_string), s.redo_label().map(str::to_string))
        };
        let was = history(&h);
        let (from, target) =
            (rect_of(&h, &format!("workspace.explorer.row.{sales}")).center(), rect_of(&h, &format!("workspace.explorer.row.{lines}")).center());
        drag(&mut h, from, target);
        assert_eq!(parent(&h, sales).as_deref(), Some("Commerce platform"), "{theme:?}: not moved");
        let said = "Ein Paket kann nicht in sich selbst oder in ein Paket verschoben werden, das es enthält.";
        assert_eq!(h.state().explorer.notice.as_deref(), Some(said), "{theme:?}");
        assert_eq!(history(&h), was, "{theme:?}: history untouched");
        h.state_mut().state.language = "en".into();
        h.state_mut().explorer.notice = None;
        h.run();
        // Onto Billing's row: moved, its cards kept with their owner lines, none on Billing's diagram.
        let (from, target) =
            (rect_of(&h, &format!("workspace.explorer.row.{sales}")).center(), rect_of(&h, &format!("workspace.explorer.row.{billing}")).center());
        drag(&mut h, from, target);
        assert_eq!(parent(&h, sales).as_deref(), Some("Billing"), "{theme:?}: moved in Models");
        assert_eq!(h.state().session.as_ref().unwrap().undo_label(), Some("Move package"), "{theme:?}: one step");
        assert_eq!(h.state().canvas.toast.as_ref().map(|t| t.text.as_str()), Some("Sales moved into Billing"), "{theme:?}");
        lines_say(&mut h, "Commerce platform › Billing");
        // Undo brings the owner and the lines back.
        h.state_mut().session.as_mut().unwrap().undo().unwrap();
        assert_eq!(parent(&h, sales).as_deref(), Some("Commerce platform"), "{theme:?}: Models' move undone");
        lines_say(&mut h, "Commerce platform");
    }
    archforge_ui_egui::i18n::set_language("en");
}

/// The path bar follows ownership, never the way in (AR-666): Sales was entered from the root's
/// Second, yet the root's pill opens the root's default, First, and starts a new route in its tab.
#[test]
fn the_path_bar_opens_the_parents_default_not_the_diagram_it_was_entered_from() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let mut h = workspace_harness(theme, egui::vec2(1600.0, 1000.0));
        let id = |v: serde_json::Value| archforge_mof::Uuid::parse(v["id"].as_str().unwrap()).unwrap();
        // The root has two class diagrams, First its default; Sales is on the second.
        let (first, second, sales) = {
            let s = h.state_mut().session.as_mut().unwrap();
            let root = id(s.execute("uml.model.root", &serde_json::json!({})).unwrap().value).to_string();
            let first = id(s
                .execute("diagram.add", &serde_json::json!({ "kind": archforge_uml::STRUCTURE_DIAGRAM, "element": root, "name": "First" }))
                .unwrap()
                .value);
            let second = id(s
                .execute("diagram.add", &serde_json::json!({ "kind": archforge_uml::STRUCTURE_DIAGRAM, "element": root, "name": "Second" }))
                .unwrap()
                .value);
            let sales = id(s.execute("uml.package.add", &serde_json::json!({ "name": "Sales" })).unwrap().value);
            let p =
                serde_json::json!({ "diagram": second.to_string(), "element": sales.to_string(), "x": 200, "y": 160, "width": 240, "height": 148 });
            s.execute("diagram.place", &p).unwrap();
            own_diagram(s, sales);
            (first, second, sales)
        };
        archforge_ui_egui::canvas::open(h.state_mut(), second);
        h.run();
        archforge_ui_egui::snapshot::cx(h.state_mut(), |cx| uml_package_nav::enter(cx, sales, 0.0));
        h.run();
        rest(&mut h);
        let at = rect_of(&h, "workspace.canvas.path.0").center();
        click(&mut h, at);
        assert_eq!(h.state().canvas.active, Some(first), "{theme:?}: the root's default, not Second");
        assert_eq!(route_of(&h, first), vec![], "{theme:?}: a new route");
    }
}

/// Each tab keeps the route it was entered by (AR-662): entering gives the new tab the route of the
/// tab left plus one step, which names the diagram left, its view and the session's history, and
/// leaves the tab left its own. Going back by the path bar opens the parent's default and starts a
/// new route in its tab (AR-666), and browsing changes nothing in the model, its undo history or
/// what is saved.
#[test]
fn each_tab_keeps_the_route_it_was_entered_by_and_browsing_changes_nothing() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let mut h = workspace_harness(theme, egui::vec2(1600.0, 1000.0));
        let id = |v: serde_json::Value| archforge_mof::Uuid::parse(v["id"].as_str().unwrap()).unwrap();
        // Root > Sales > Orders. The root's First and Second both show Sales; Sales' diagram shows
        // Orders and the root.
        let (first, second, root, sales, orders, sales_d, orders_d) = {
            let s = h.state_mut().session.as_mut().unwrap();
            let root = id(s.execute("uml.model.root", &serde_json::json!({})).unwrap().value);
            let add = |s: &mut archforge_notations::Session, name: &str| {
                let p = serde_json::json!({ "kind": archforge_uml::STRUCTURE_DIAGRAM, "element": root.to_string(), "name": name });
                id(s.execute("diagram.add", &p).unwrap().value)
            };
            let (first, second) = (add(s, "First"), add(s, "Second"));
            let sales = id(s.execute("uml.package.add", &serde_json::json!({ "name": "Sales" })).unwrap().value);
            let orders = id(s.execute("uml.package.add", &serde_json::json!({ "name": "Orders", "parent": sales.to_string() })).unwrap().value);
            let (sales_d, orders_d) = (own_diagram(s, sales), own_diagram(s, orders));
            for (on, what) in [(first, sales), (second, sales), (sales_d, orders), (sales_d, root)] {
                let p =
                    serde_json::json!({ "diagram": on.to_string(), "element": what.to_string(), "x": 200, "y": 160, "width": 240, "height": 148 });
                s.execute("diagram.place", &p).unwrap();
            }
            s.mark_saved().unwrap();
            (first, second, root, sales, orders, sales_d, orders_d)
        };
        let unchanged = |h: &Harness<'static, Shell>| {
            let s = h.state().session.as_ref().unwrap();
            (s.revision(), s.can_undo(), s.can_redo(), s.is_dirty())
        };
        let before = unchanged(&h);
        let history = h.state().session.as_ref().unwrap().history();
        let route = |h: &Harness<'static, Shell>, tab| -> Vec<(archforge_mof::Uuid, archforge_mof::Uuid)> {
            uml_of(h.state()).nav.routes.get(&tab).map(|r| r.steps().map(|s| (s.source, s.destination)).collect()).unwrap_or_default()
        };
        archforge_ui_egui::canvas::open(h.state_mut(), second);
        h.run();
        let left = (h.state().canvas.pan, h.state().canvas.zoom);
        archforge_ui_egui::snapshot::cx(h.state_mut(), |cx| uml_package_nav::enter(cx, sales, 0.0));
        h.run();
        let entry = uml_of(h.state()).nav.routes.get(&sales_d).and_then(|r| r.steps().next_back()).cloned().unwrap();
        let expected = uml_nav_route::Step { source: second, destination: sales_d, source_name: "Second".into(), view: Some(left), history };
        assert_eq!(entry, expected, "{theme:?}: the step names what was left, its view and the history");
        assert_eq!(route(&h, second), vec![], "{theme:?}: the tab left had no route and still has none");
        // Sales > Orders: the Orders tab carries the way into Sales too; Sales keeps its own.
        rest(&mut h);
        archforge_ui_egui::snapshot::cx(h.state_mut(), |cx| uml_package_nav::enter(cx, orders, 0.0));
        h.run();
        assert_eq!(route(&h, orders_d), vec![(second, sales_d), (sales_d, orders_d)], "{theme:?}");
        assert_eq!(route(&h, sales_d), vec![(second, sales_d)], "{theme:?}: Sales' tab keeps its own route");
        // Back to Sales by the path bar, which starts Sales' tab a new route, then into the root's
        // First and from it into Sales again: Sales' tab came by First, and Second is gone from it.
        rest(&mut h);
        let at = rect_of(&h, "workspace.canvas.path.1").center();
        click(&mut h, at);
        assert_eq!(h.state().canvas.active, Some(sales_d), "{theme:?}");
        rest(&mut h);
        archforge_ui_egui::snapshot::cx(h.state_mut(), |cx| uml_package_nav::enter(cx, root, 0.0));
        h.run();
        assert_eq!(h.state().canvas.active, Some(first), "{theme:?}: the root's default");
        rest(&mut h);
        archforge_ui_egui::snapshot::cx(h.state_mut(), |cx| uml_package_nav::enter(cx, sales, 0.0));
        h.run();
        assert_eq!(route(&h, sales_d), vec![(sales_d, first), (first, sales_d)], "{theme:?}");
        rest(&mut h);
        let at = rect_of(&h, "workspace.canvas.path.0").center();
        click(&mut h, at);
        assert_eq!(h.state().canvas.active, Some(first), "{theme:?}: the root's default");
        assert_eq!(unchanged(&h), before, "{theme:?}: no change, no undo step, nothing to save");
    }
}

/// A path bar six packages deep keeps within the canvas: its first ancestors fold into …, the open
/// package and its parent show whole, and both the parent and … lead where they say.
#[test]
fn a_long_package_path_folds_its_first_ancestors_and_keeps_the_nearest_ones_clickable() {
    let mut h = workspace_harness(ThemeKind::Light, egui::vec2(1400.0, 900.0));
    let id = |v: serde_json::Value| archforge_mof::Uuid::parse(v["id"].as_str().unwrap()).unwrap();
    // Six packages deep, each with a long name, and the deepest one's diagram open.
    let (chain, deepest) = {
        let s = h.state_mut().session.as_mut().unwrap();
        let mut parent = String::new();
        let mut chain = Vec::new();
        for i in 0..6 {
            let p = id(s
                .execute("uml.package.add", &serde_json::json!({ "name": format!("Package with a long name {i}"), "parent": parent }))
                .unwrap()
                .value);
            parent = p.to_string();
            chain.push(p);
        }
        // Each with its own diagram, which the path bar leads back to; entry makes none (AR-582).
        let d = chain.iter().map(|p| own_diagram(s, *p)).last().unwrap();
        (chain, d)
    };
    archforge_ui_egui::canvas::open(h.state_mut(), deepest);
    h.run();
    let found = ids(&h);
    let bar = rect_of(&h, "workspace.canvas.path");
    let canvas = rect_of(&h, "workspace.canvas");
    assert!(bar.right() <= canvas.right(), "the bar keeps within the canvas");
    assert!(found.iter().any(|i| i == "workspace.canvas.path.more"), "the first ancestors fold into …: {found:?}");
    // The root, then the six packages: the open one and its parent are shown whole.
    let (here, parent) = (chain.len(), chain.len() - 1);
    for i in [here, parent] {
        let pill = rect_of(&h, &format!("workspace.canvas.path.{i}"));
        assert!(bar.contains_rect(pill), "path.{i} in the bar");
    }
    // The parent is a click away; … leads to the nearest ancestor it folds.
    let at = rect_of(&h, &format!("workspace.canvas.path.{parent}")).center();
    click(&mut h, at);
    let open_package = |h: &Harness<'static, Shell>| {
        let st = h.state().session.as_ref().unwrap().doc();
        let d = st.by_uuid(h.state().canvas.active.unwrap()).unwrap();
        st.uuid(uml_package_nav::package_of_diagram(st, d).unwrap()).unwrap()
    };
    assert_eq!(open_package(&h), chain[chain.len() - 2]);
    // Narrower still, the parent folds too, and … leads to it.
    rest(&mut h);
    archforge_ui_egui::canvas::open(h.state_mut(), deepest);
    h.state_mut().state.explorer_collapsed = false;
    h.set_size(egui::vec2(1000.0, 900.0));
    h.run();
    // … leads to the ancestor just before the first one still shown (path.0 is the root).
    let found = ids(&h);
    let first_shown = (1..=chain.len()).find(|i| found.iter().any(|f| f == &format!("workspace.canvas.path.{i}"))).unwrap();
    assert!(first_shown >= 2, "some ancestors are folded: {found:?}");
    let at = rect_of(&h, "workspace.canvas.path.more").center();
    click(&mut h, at);
    assert_eq!(open_package(&h), chain[first_shown - 2], "… leads to the nearest folded ancestor");
}

/// ⇧⌫ on a selected package card, with the canvas holding the keys, opens the delete dialog for the
/// package.
#[test]
fn shift_delete_on_a_selected_package_card_asks_to_delete_the_package() {
    let mut h = workspace_harness(ThemeKind::Light, egui::vec2(1600.0, 1000.0));
    let id = |v: serde_json::Value| archforge_mof::Uuid::parse(v["id"].as_str().unwrap()).unwrap();
    let (top, sales) = {
        let s = h.state_mut().session.as_mut().unwrap();
        let root = id(s.execute("uml.model.root", &serde_json::json!({})).unwrap().value).to_string();
        let top = id(s
            .execute("diagram.add", &serde_json::json!({ "kind": archforge_uml::STRUCTURE_DIAGRAM, "element": root, "name": "Top" }))
            .unwrap()
            .value);
        let sales = id(s.execute("uml.package.add", &serde_json::json!({ "name": "Sales" })).unwrap().value);
        s.execute(
            "diagram.place",
            &serde_json::json!({ "diagram": top.to_string(), "element": sales.to_string(), "x": 200, "y": 160, "width": 240, "height": 148 }),
        )
        .unwrap();
        (top, sales)
    };
    archforge_ui_egui::canvas::open(h.state_mut(), top);
    h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [sales.to_string()] })).unwrap();
    h.state_mut().canvas.focused = true;
    h.run();
    key(&mut h, egui::Key::Delete, egui::Modifiers::SHIFT);
    assert_eq!(h.state().explorer.delete.as_ref().map(|d| d.ids.clone()), Some(vec![sales]), "the delete dialog, for the package");
}

/// A package card names its package in a tab over its top left, fitted to the name, widening with a
/// longer one up to the card's width and cut with an ellipsis past it, the whole name on hover. In
/// light and dark.
#[test]
fn a_package_card_names_its_package_in_a_tab_that_fits_the_name_and_follows_a_rename() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, _, sales, card, _, _) = package_preview_harness(theme);
        let body = rect_of(&h, &format!("workspace.canvas.node.{card}"));
        let tab = |h: &Harness<'static, Shell>| rect_of(h, &format!("workspace.canvas.node.{card}.tab"));
        let short = tab(&h);
        assert!(painted(&h).iter().any(|t| t == "Sales"), "the name is in the tab");
        assert!(
            (short.left() - body.left()).abs() < 0.5 && (short.bottom() - body.top() - 1.0).abs() < 0.5,
            "over the card's top left: {short:?} {body:?}"
        );
        assert!(short.width() >= 56.0 && short.width() < 100.0, "fitted to Sales and its padding: {short:?}");
        assert!(short.height() > 20.0 && short.height() < 24.0);
        // A longer name widens it; one longer than the card stops at the card and is cut.
        let rename = |h: &mut Harness<'static, Shell>, name: &str| {
            h.state_mut()
                .session
                .as_mut()
                .unwrap()
                .execute("uml.package.rename", &serde_json::json!({ "package": sales.to_string(), "name": name }))
                .unwrap();
            h.run();
        };
        rename(&mut h, "Sales and Marketing");
        let longer = tab(&h);
        assert!(longer.width() > short.width() + 40.0 && longer.width() < body.width(), "{longer:?}");
        assert!(painted(&h).iter().any(|t| t == "Sales and Marketing"));
        rename(&mut h, "Customer Relationship Management Integrations");
        let longest = tab(&h);
        assert!(longest.width() <= body.width() + 0.5 && longest.width() > body.width() - 30.0, "never wider than the card: {longest:?}");
        let whole = "Customer Relationship Management Integrations";
        // In the tab its glyphs stop short of the whole name, with the ellipsis.
        let in_tab = texts_in(&h, longest);
        assert!(in_tab.iter().any(|(t, glyphs)| t == whole && *glyphs < whole.chars().count()), "cut: {in_tab:?}");
        let before = painted(&h).iter().filter(|t| *t == whole).count();
        assert!(h.state().canvas.card_parts.iter().any(|t| t.hover.as_deref() == Some("Customer Relationship Management Integrations")));
        // On the tab, the pointer shows the whole name.
        h.event(egui::Event::PointerMoved(longest.center()));
        for _ in 0..40 {
            h.step();
        }
        assert_eq!(painted(&h).iter().filter(|t| *t == whole).count(), before + 1, "the whole name on hover, as well as in Models");
    }
}

/// A package card shows its own diagram scaled down inside its margin, follows a move there, and
/// says Empty diagram once nothing is left on it.
#[test]
fn a_package_cards_thumbnail_is_its_own_diagram_scaled_and_follows_it() {
    let (mut h, _, _, card, own, [order, line]) = package_preview_harness(ThemeKind::Light);
    let body = rect_of(&h, &format!("workspace.canvas.node.{card}"));
    let area = rect_of(&h, &format!("workspace.canvas.node.{card}.preview"));
    assert!(body.contains_rect(area) && area.width() > body.width() - 24.0, "the thumbnail fills the card inside its margin: {area:?}");
    assert!(!painted(&h).iter().any(|t| t == "Order" || t == "OrderLine"), "no list of class names any more");
    let shown = |h: &Harness<'static, Shell>| {
        let s = h.state().session.as_ref().unwrap();
        let st = s.doc();
        uml_package::preview(st, s.view(), st.by_uuid(own).unwrap(), area).expect("a thumbnail")
    };
    let first = shown(&h);
    assert_eq!(first.cards.len(), 2, "both classes");
    assert_eq!(first.lines.len(), 1, "and the association between them");
    assert!(first.cards.iter().all(|c| area.contains_rect(c.rect) && c.look == uml_package::MiniLook::Plain && c.rows == vec![0, 0]));
    let (a, b) = (first.cards[0].rect, first.cards[1].rect);
    assert!(a.right() < b.left() && (a.top() - b.top()).abs() < 0.5, "laid out as on the diagram: side by side");
    assert!(first.scale > 0.0 && first.scale <= 0.5);
    // Moving OrderLine below Order there moves it in the thumbnail too.
    let s = h.state_mut().session.as_mut().unwrap();
    s.execute("diagram.move", &serde_json::json!({ "nodes": [line.to_string()], "dx": -400, "dy": 300 })).unwrap();
    h.run();
    let moved = shown(&h);
    let (a, b) = (moved.cards[0].rect, moved.cards[1].rect);
    assert!(b.top() > a.bottom() && (a.left() - b.left()).abs() < 0.5, "now one under the other: {a:?} {b:?}");
    // With nothing left on the diagram, the card says so.
    let s = h.state_mut().session.as_mut().unwrap();
    s.execute("diagram.unplace", &serde_json::json!({ "nodes": [order.to_string(), line.to_string()] })).unwrap();
    h.run();
    let st = h.state().session.as_ref().unwrap();
    assert!(uml_package::preview(st.doc(), st.view(), st.doc().by_uuid(own).unwrap(), area).is_none());
    assert!(painted(&h).iter().any(|t| t == "Empty diagram"));
}

/// A package of another branch says its whole owning path under its card, to a screen reader and
/// on hover; the root's card says none (AR-565).
#[test]
fn a_package_card_says_where_its_package_lives_under_it_and_the_root_says_nothing() {
    use egui_kittest::kittest::{NodeT, Queryable};
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, root, top) = owning_path_harness(theme);
        // Archive lives in Operations, another branch than the diagram's root; the root is shown too.
        let operations = owning_path_made(&mut h, "uml.package.add", serde_json::json!({ "name": "Operations", "parent": root }));
        let archive = owning_path_made(&mut h, "uml.package.add", serde_json::json!({ "name": "Archive", "parent": operations }));
        let place = |element: &str, x: i64| serde_json::json!({ "diagram": top, "element": element, "x": x, "y": 80, "width": 240, "height": 148 });
        let card = owning_path_made(&mut h, "diagram.place", place(&archive, 80));
        let root_card = owning_path_made(&mut h, "diagram.place", place(&root, 480));
        h.run();
        let whole = "Owner: Commerce platform › Operations";
        let body = rect_of(&h, &format!("workspace.canvas.node.{card}"));
        let path = rect_of(&h, &format!("workspace.canvas.node.{card}.path"));
        assert!(
            (path.top() - body.bottom() - 6.0).abs() < 0.5 && (path.left() - body.left()).abs() < 0.5,
            "{theme:?}: under the card: {path:?} {body:?}"
        );
        assert!(path.width() <= body.width() + 0.5 && path.height() > 12.0, "{theme:?}: {path:?}");
        assert!(painted(&h).iter().any(|t| t == whole), "{theme:?}: the whole path, said in words");
        assert!(painted(&h).iter().any(|t| t == "Archive"), "{theme:?}: the name stays in the tab");
        let said = h.get_by_label(whole).accesskit_node();
        assert_eq!(said.role(), egui::accesskit::Role::Label, "{theme:?}: read out whole");
        // The root, which no package owns, says no path.
        let found = ids(&h);
        assert!(found.contains(&format!("workspace.canvas.node.{root_card}.tab")), "{theme:?}: the root's card shows");
        assert!(!found.contains(&format!("workspace.canvas.node.{root_card}.path")), "{theme:?}: but no path under it");
        // The pointer on the path shows it whole.
        let before = painted(&h).iter().filter(|t| *t == whole).count();
        h.event(egui::Event::PointerMoved(path.center()));
        for _ in 0..40 {
            h.step();
        }
        assert_eq!(painted(&h).iter().filter(|t| *t == whole).count(), before + 1, "{theme:?}: its tooltip");
    }
}

/// A path too long for its card folds its first owners into `…`; a card too narrow even for that
/// shows the nearest owner alone, cut but legible; both say the whole path (AR-565).
#[test]
fn a_long_owning_path_folds_its_first_owners_and_a_narrow_card_keeps_the_nearest() {
    use egui_kittest::kittest::Queryable;
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, root, top) = owning_path_harness(theme);
        let mut parent = root;
        for name in ["Operations and logistics", "Regional fulfilment"] {
            parent = owning_path_made(&mut h, "uml.package.add", serde_json::json!({ "name": name, "parent": parent }));
        }
        let place = |name: &str, x: i64, width: i64, h: &mut Harness<'static, Shell>| {
            let p = owning_path_made(h, "uml.package.add", serde_json::json!({ "name": name, "parent": parent }));
            owning_path_made(h, "diagram.place", serde_json::json!({ "diagram": top, "element": p, "x": x, "y": 80, "width": width, "height": 148 }))
        };
        let wide = place("Archive", 80, 240, &mut h);
        let narrow = place("Ledger", 480, 100, &mut h);
        h.run();
        let whole = "Owner: Commerce platform › Operations and logistics › Regional fulfilment";
        let badge = |h: &Harness<'static, Shell>, card: &str| {
            h.state().canvas.card_labels.iter().find(|b| b.node.to_string() == card).cloned().unwrap_or_else(|| panic!("a path under {card}"))
        };
        // Too long for its card: the first owners fold, the nearest stays.
        let long = badge(&h, &wide);
        assert_eq!((long.shown.as_str(), long.whole.as_str()), ("Owner: … › Regional fulfilment", whole), "{theme:?}");
        let body = rect_of(&h, &format!("workspace.canvas.node.{wide}"));
        assert!(rect_of(&h, &format!("workspace.canvas.node.{wide}.path")).width() <= body.width() + 0.5, "{theme:?}");
        assert!(painted(&h).iter().any(|t| t == "Archive"), "{theme:?}: the package's name stays in its tab");
        // Too narrow even for that: the nearest owner alone, cut at its end, in the caption's size.
        let short = badge(&h, &narrow);
        assert_eq!((short.shown.as_str(), short.whole.as_str()), ("Owner: Regional fulfilment", whole), "{theme:?}");
        let body = rect_of(&h, &format!("workspace.canvas.node.{narrow}"));
        let path = rect_of(&h, &format!("workspace.canvas.node.{narrow}.path"));
        assert!(path.width() <= body.width() + 0.5 && path.height() > 12.0, "{theme:?}: legible, inside the card's width: {path:?}");
        let glyphs = texts_in(&h, path).into_iter().find(|(t, _)| t == "Owner: Regional fulfilment").map(|(_, g)| g);
        assert!(glyphs.is_some_and(|g| g > 6 && g < "Owner: Regional fulfilment".chars().count()), "{theme:?}: cut, not gone: {glyphs:?}");
        // Both read out whole.
        assert_eq!(h.query_all_by_label(whole).count(), 2, "{theme:?}");
    }
}

/// A package selected with a class while its card lies out of view, dragged with the class until
/// it covers another class: during the drag, the package's card is drawn under that class, as when
/// it lies still (AR-570).
#[test]
fn a_package_dragged_into_view_stays_under_the_class_it_covers() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, root, top) = owning_path_harness(theme);
        // At 100 % with no pan, a diagram unit is a point from the canvas's corner: Sales lies
        // just past its right edge, Stay 360 points to the left of where Sales lies.
        let (canvas, w) = {
            let c = rect_of(&h, "workspace.canvas");
            (c, c.width() as i64)
        };
        let place = |element: &str, (x, y, width, height): (i64, i64, i64, i64)| serde_json::json!({ "diagram": top, "element": element, "x": x, "y": y, "width": width, "height": height });
        let mut cards = Vec::new();
        for (name, at) in [("Stay", (w - 360, 160, 140, 100)), ("Mover", (500, 400, 120, 80))] {
            let c = owning_path_made(&mut h, "uml.class.add", serde_json::json!({ "package": root, "name": name }));
            cards.push((c.clone(), owning_path_made(&mut h, "diagram.place", place(&c, at))));
        }
        let sales = owning_path_made(&mut h, "uml.package.add", serde_json::json!({ "name": "Sales", "parent": root }));
        let card = owning_path_made(&mut h, "diagram.place", place(&sales, (w + 40, 120, 240, 148)));
        let ((mover, mover_card), stay) = (cards[1].clone(), cards[0].1.clone());
        h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [mover, sales] })).unwrap();
        h.run();
        assert!(!ids(&h).contains(&format!("workspace.canvas.node.{card}")), "{theme:?}: Sales lies out of view of {canvas:?}");
        // Mover held 360 points to the left, Sales with it: its card now covers Stay.
        let from = rect_of(&h, &format!("workspace.canvas.node.{mover_card}")).center();
        let to = from - egui::vec2(360.0, 0.0);
        h.event(egui::Event::PointerMoved(from));
        h.event(egui::Event::PointerButton { pos: from, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE });
        h.run();
        h.event(egui::Event::PointerMoved(to));
        h.run();
        let (stay_rect, card_rect) = (rect_of(&h, &format!("workspace.canvas.node.{stay}")), rect_of(&h, &format!("workspace.canvas.node.{card}")));
        assert!(card_rect.intersects(stay_rect), "{theme:?}: Sales covers Stay: {card_rect:?} {stay_rect:?}");
        let (stay_at, card_at) = (filled_at(&h, stay_rect), filled_at(&h, card_rect));
        assert!(card_at.is_some() && stay_at > card_at, "{theme:?}: Stay is painted after the card: {stay_at:?} {card_at:?}");
        h.event(egui::Event::PointerButton { pos: to, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE });
        h.run();
    }
}

/// A class placed on a diagram before a package whose card and tab then cover it: the card is
/// drawn first and the class over it, a press or a relationship's end on the class takes the class,
/// on the card as on the tab, and a press on the card where nothing lies takes the package. Drawing
/// and pressing leave the history and the rows as they were (AR-570).
#[test]
fn a_class_over_a_package_card_is_drawn_and_pressed_in_front_of_the_card() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, root, top) = owning_path_harness(theme);
        let invoice = owning_path_made(&mut h, "uml.class.add", serde_json::json!({ "package": root, "name": "Invoice" }));
        let place = |element: &str, (x, y, width, height): (i64, i64, i64, i64)| serde_json::json!({ "diagram": top, "element": element, "x": x, "y": y, "width": width, "height": height });
        let class_card = owning_path_made(&mut h, "diagram.place", place(&invoice, (210, 120, 140, 100)));
        let sales = owning_path_made(&mut h, "uml.package.add", serde_json::json!({ "name": "Sales", "parent": root }));
        let card = owning_path_made(&mut h, "diagram.place", place(&sales, (200, 160, 320, 200)));
        h.run();
        let history = |h: &Harness<'static, Shell>| {
            let s = h.state().session.as_ref().unwrap();
            (s.revision(), s.undo_label().map(str::to_owned), s.can_redo())
        };
        let (rows, before) = (rows_now(&h), history(&h));
        let (class_rect, card_rect) =
            (rect_of(&h, &format!("workspace.canvas.node.{class_card}")), rect_of(&h, &format!("workspace.canvas.node.{card}")));
        // Where the class covers the package's tab, above its card.
        let on_tab = class_rect.intersect(rect_of(&h, &format!("workspace.canvas.node.{card}.tab"))).center();
        assert!(card_rect.contains(class_rect.center()) && !card_rect.contains(on_tab), "{theme:?}: {class_rect:?} {card_rect:?} {on_tab:?}");
        let (class_at, card_at) = (filled_at(&h, class_rect), filled_at(&h, card_rect));
        assert!(card_at.is_some() && class_at > card_at, "{theme:?}: the class is painted after the card: {class_at:?} {card_at:?}");
        let selected = |h: &Harness<'static, Shell>| {
            let s = h.state().session.as_ref().unwrap();
            s.view().selection.iter().filter_map(|o| s.doc().uuid(*o).ok()).map(|id| id.to_string()).collect::<Vec<_>>()
        };
        // Each press past egui's double-click window, so it is a click of its own.
        for (at, takes) in [(class_rect.center(), &invoice), (on_tab, &invoice), (card_rect.max - egui::vec2(24.0, 24.0), &sales)] {
            for _ in 0..40 {
                h.step();
            }
            click(&mut h, at);
            assert_eq!(selected(&h), std::slice::from_ref(takes), "{theme:?}: a press at {at:?}");
        }
        assert_eq!(history(&h), before, "{theme:?}: no step of its own");
        assert!(*rows_now(&h) == *rows, "{theme:?}: the rows are as they were");
        // A relationship dragged onto the class where it covers the tab ends at the class's card.
        let party = owning_path_made(&mut h, "uml.class.add", serde_json::json!({ "package": root, "name": "Party" }));
        let party_card = owning_path_made(&mut h, "diagram.place", place(&party, (700, 200, 120, 80)));
        h.run();
        h.state_mut().canvas.tool = archforge_shell_api::canvas::Tool::Connect("relationship");
        let from = rect_of(&h, &format!("workspace.canvas.node.{party_card}")).center();
        drag(&mut h, from, on_tab);
        let st = h.state().session.as_ref().unwrap().doc();
        let end = |e: archforge_mof::Obj, prop| st.get(e, prop).ok().and_then(|v| v.as_ref()).and_then(|n| st.uuid(n).ok()).map(|id| id.to_string());
        let lines: Vec<_> = st
            .alive()
            .filter(|o| st.class_of(*o).ok() == Some(archforge_diagram::EDGE))
            .map(|e| (end(e, archforge_diagram::edge::SOURCE), end(e, archforge_diagram::edge::TARGET)))
            .collect();
        assert_eq!(lines, [(Some(party_card), Some(class_card))], "{theme:?}: {:?}", h.state().canvas.notice);
    }
}

/// A click on a package's tab selects the package, also where a line runs across the tab.
#[test]
fn a_click_on_a_packages_tab_selects_the_package_though_a_line_crosses_the_tab() {
    let (mut h, top, sales, card, _, _) = package_preview_harness(ThemeKind::Light);
    // Two classes on Top, their association running across Sales's tab, above its card (y 150).
    {
        let s = h.state_mut().session.as_mut().unwrap();
        let root = s.execute("uml.model.root", &serde_json::json!({})).unwrap().value["id"].as_str().unwrap().to_string();
        let mut class = |name: &str, x: i64| {
            let c =
                s.execute("uml.class.add", &serde_json::json!({ "package": root, "name": name })).unwrap().value["id"].as_str().unwrap().to_string();
            s.execute(
                "diagram.place",
                &serde_json::json!({ "diagram": top.to_string(), "element": c, "x": x, "y": 100, "width": 100, "height": 100 }),
            )
            .unwrap();
            c
        };
        let (a, b) = (class("Left", 0), class("Right", 600));
        s.execute("uml.association.add", &serde_json::json!({ "from": a, "to": b })).unwrap();
        let _ = s.execute("diagram.connect-all", &serde_json::json!({ "diagram": top.to_string() }));
    }
    h.run();
    let tab = rect_of(&h, &format!("workspace.canvas.node.{card}.tab"));
    let (view, store) = (h.state().canvas.zoom, h.state().session.as_ref().unwrap().doc());
    let d = store.by_uuid(top).unwrap();
    let grid = archforge_diagram::Grid::build(store, d);
    let origin = rect_of(&h, &format!("workspace.canvas.node.{card}")).min - egui::vec2(200.0, 160.0) * view;
    let x = f64::from((tab.center().x - origin.x) / view);
    // Where the line runs across the tab: its horizontal segment over the tab's middle.
    let y = grid
        .lines()
        .flat_map(|(_, p)| p.windows(2).map(|w| (w[0], w[1])).collect::<Vec<_>>())
        .find(|((x0, y0), (x1, y1))| (y0 - y1).abs() < 1e-6 && x0.min(*x1) <= x && x <= x0.max(*x1))
        .map(|((_, y0), _)| y0)
        .expect("a segment across the tab");
    let at = egui::pos2(tab.center().x, origin.y + y as f32 * view);
    assert!(tab.contains(at), "the line crosses the tab: {at:?} {tab:?}");
    assert!(matches!(grid.hit(x, y), Some(archforge_diagram::Hit::Edge(_))), "and only the line is there in the grid");
    click(&mut h, at);
    let s = h.state().session.as_ref().unwrap();
    let picked: Vec<_> = s.view().selection.iter().filter_map(|o| s.doc().uuid(*o).ok()).collect();
    assert_eq!(picked, vec![sales], "the tab, not the line under it");
}

/// A card drawn over a package's tab takes the click there, and keeps it once the package is placed
/// again: a package's card and tab stay under the other figures (AR-570).
#[test]
fn a_packages_tab_drawn_over_another_card_takes_the_click_and_the_card_on_top_of_a_tab_keeps_it() {
    let (mut h, top, sales, card, _, _) = package_preview_harness(ThemeKind::Light);
    // A class whose card lies under Sales's tab: placed after Sales, it is drawn over the tab.
    let under = {
        let s = h.state_mut().session.as_mut().unwrap();
        let root = s.execute("uml.model.root", &serde_json::json!({})).unwrap().value["id"].as_str().unwrap().to_string();
        let c =
            s.execute("uml.class.add", &serde_json::json!({ "package": root, "name": "Under" })).unwrap().value["id"].as_str().unwrap().to_string();
        s.execute("diagram.place", &serde_json::json!({ "diagram": top.to_string(), "element": c, "x": 150, "y": 60, "width": 140, "height": 110 }))
            .unwrap();
        archforge_mof::Uuid::parse(&c).unwrap()
    };
    h.run();
    let ids = |h: &Harness<'static, Shell>| {
        let s = h.state().session.as_ref().unwrap();
        s.view().selection.iter().filter_map(|o| s.doc().uuid(*o).ok()).collect::<Vec<_>>()
    };
    let at = rect_of(&h, &format!("workspace.canvas.node.{card}.tab")).center();
    click(&mut h, at);
    assert_eq!(ids(&h), vec![under], "the class, drawn over the tab, takes the click");
    // Sales placed again comes after the class, but a package's card and tab stay under the other
    // figures (AR-570): the class keeps the click.
    let card = {
        let s = h.state_mut().session.as_mut().unwrap();
        s.execute("diagram.unplace", &serde_json::json!({ "nodes": [card.to_string()] })).unwrap();
        let v = s
            .execute(
                "diagram.place",
                &serde_json::json!({ "diagram": top.to_string(), "element": sales.to_string(), "x": 200, "y": 160, "width": 240, "height": 148 }),
            )
            .unwrap()
            .value;
        archforge_mof::Uuid::parse(v["id"].as_str().unwrap()).unwrap()
    };
    for _ in 0..45 {
        h.step();
    }
    h.run();
    let at = rect_of(&h, &format!("workspace.canvas.node.{card}.tab")).center();
    click(&mut h, at);
    assert_eq!(ids(&h), vec![under], "the class, still drawn over the tab, takes the click");
    // A package nested in Sales's card, its tab over Sales's body: the nested tab, drawn after its
    // parent, takes the click.
    let (inner, inner_card) = {
        let s = h.state_mut().session.as_mut().unwrap();
        let root = s.execute("uml.model.root", &serde_json::json!({})).unwrap().value["id"].as_str().unwrap().to_string();
        let p =
            s.execute("uml.package.add", &serde_json::json!({ "name": "Inner", "parent": root })).unwrap().value["id"].as_str().unwrap().to_string();
        let v = s
            .execute(
                "diagram.place",
                &serde_json::json!({ "diagram": top.to_string(), "element": p, "parent": card.to_string(), "x": 220, "y": 230, "width": 160, "height": 60 }),
            )
            .unwrap()
            .value;
        (archforge_mof::Uuid::parse(&p).unwrap(), archforge_mof::Uuid::parse(v["id"].as_str().unwrap()).unwrap())
    };
    for _ in 0..45 {
        h.step();
    }
    h.run();
    let inner_tab = rect_of(&h, &format!("workspace.canvas.node.{inner_card}.tab"));
    assert!(rect_of(&h, &format!("workspace.canvas.node.{card}")).contains(inner_tab.center()), "the nested tab lies on Sales's body");
    click(&mut h, inner_tab.center());
    assert_eq!(ids(&h), vec![inner], "the nested package's tab, not its parent's card");
}

/// A package's tab selects its package, drags its card, and on a double click renames it in place,
/// the tab growing with the name typed.
#[test]
fn a_packages_tab_selects_drags_and_renames_its_card_and_the_name_is_typed_in_the_tab() {
    let (mut h, _, sales, card, _, _) = package_preview_harness(ThemeKind::Light);
    let tab = rect_of(&h, &format!("workspace.canvas.node.{card}.tab"));
    let body = rect_of(&h, &format!("workspace.canvas.node.{card}"));
    click(&mut h, tab.center());
    assert_eq!(
        h.state()
            .session
            .as_ref()
            .unwrap()
            .view()
            .selection
            .iter()
            .filter_map(|o| h.state().session.as_ref().unwrap().doc().uuid(*o).ok())
            .collect::<Vec<_>>(),
        vec![sales],
        "a click on the tab selects the package"
    );
    // A drag by the tab moves the card.
    for _ in 0..45 {
        h.step();
    }
    drag(&mut h, tab.center(), tab.center() + egui::vec2(80.0, 40.0));
    let moved = rect_of(&h, &format!("workspace.canvas.node.{card}"));
    assert!((moved.left() - body.left() - 80.0).abs() < 1.0 && (moved.top() - body.top() - 40.0).abs() < 1.0, "{moved:?} from {body:?}");
    // A double click on the tab renames it there, and the tab grows with the name typed.
    for _ in 0..45 {
        h.step();
    }
    let tab = rect_of(&h, &format!("workspace.canvas.node.{card}.tab"));
    double_click(&mut h, tab.center());
    assert!(h.state().canvas.rename.as_ref().is_some_and(|r| r.text == "Sales"), "{:?}", h.state().canvas.rename);
    let field = rect_of(&h, "workspace.canvas.rename");
    assert!(tab.contains_rect(field), "the field is in the tab: {field:?} {tab:?}");
    h.state_mut().canvas.rename.as_mut().unwrap().text = "Sales and Marketing".into();
    h.run();
    let typed = rect_of(&h, &format!("workspace.canvas.node.{card}.tab"));
    assert!(typed.width() > tab.width() + 40.0, "the tab follows the typing: {typed:?}");
    key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
    let st = h.state().session.as_ref().unwrap().doc();
    assert_eq!(archforge_core::name_of(st, st.by_uuid(sales).unwrap()), "Sales and Marketing");
}

/// A double click on a package's card opens its name in its tab in the very frame of the second
/// click, before the card is drawn renamed, in light and dark (AR-113).
#[test]
fn a_packages_name_is_typed_in_its_tab_from_the_frame_the_double_click_opens_it() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, _, _, card, _, _) = package_preview_harness(theme);
        let tab = rect_of(&h, &format!("workspace.canvas.node.{card}.tab"));
        let body = rect_of(&h, &format!("workspace.canvas.node.{card}"));
        let at = body.center();
        h.event(egui::Event::PointerMoved(at));
        for _ in 0..2 {
            for pressed in [true, false] {
                h.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE });
            }
            h.step();
        }
        assert!(h.state().canvas.rename.as_ref().is_some_and(|r| r.text == "Sales"), "{theme:?}: {:?}", h.state().canvas.rename);
        let field = rect_of(&h, "workspace.canvas.rename");
        assert!(tab.contains_rect(field), "{theme:?}: in the tab from the first frame: {field:?} {tab:?}");
    }
}

/// A package dropped where a class lies over another package's card is only moved on the diagram:
/// the class on top takes the drop.
#[test]
fn a_class_over_a_package_card_keeps_a_package_dropped_on_it_where_it_was() {
    let mut h = workspace_harness(ThemeKind::Light, egui::vec2(1600.0, 1000.0));
    let id = |v: serde_json::Value| archforge_mof::Uuid::parse(v["id"].as_str().unwrap()).unwrap();
    let (top, sales, sales_card, class_card) = {
        let s = h.state_mut().session.as_mut().unwrap();
        let root = id(s.execute("uml.model.root", &serde_json::json!({})).unwrap().value);
        let top = id(s
            .execute("diagram.add", &serde_json::json!({ "kind": archforge_uml::STRUCTURE_DIAGRAM, "element": root.to_string(), "name": "Top" }))
            .unwrap()
            .value);
        let sales = id(s.execute("uml.package.add", &serde_json::json!({ "name": "Sales" })).unwrap().value);
        let billing = id(s.execute("uml.package.add", &serde_json::json!({ "name": "Billing" })).unwrap().value);
        let place = |s: &mut archforge_notations::Session, e: archforge_mof::Uuid, x: i32| {
            id(s.execute(
                "diagram.place",
                &serde_json::json!({ "diagram": top.to_string(), "element": e.to_string(), "x": x, "y": 120, "width": 240, "height": 148 }),
            )
            .unwrap()
            .value)
        };
        let sales_card = place(s, sales, 40);
        place(s, billing, 600);
        // A class drawn over Billing's card, on top of it.
        let order = id(s.execute("uml.class.add", &serde_json::json!({ "package": root.to_string(), "name": "Order" })).unwrap().value);
        let class_card = place(s, order, 620);
        (top, sales, sales_card, class_card)
    };
    archforge_ui_egui::canvas::open(h.state_mut(), top);
    h.state_mut().canvas.fit_pending = false;
    h.state_mut().canvas.zoom = 1.0;
    h.state_mut().canvas.pan = egui::Vec2::ZERO;
    h.run();
    let from = rect_of(&h, &format!("workspace.canvas.node.{sales_card}")).center();
    let onto = rect_of(&h, &format!("workspace.canvas.node.{class_card}")).center();
    drag(&mut h, from, onto);
    let st = h.state().session.as_ref().unwrap().doc();
    let parent = archforge_uml::commands::package_of(st, st.by_uuid(sales).unwrap()).map(|p| archforge_core::name_of(st, p));
    assert_eq!(parent.as_deref(), Some("Commerce platform"), "the class on top took the drop: only a move on the diagram");
}

/// A package let go over the Models explorer, where another package's card lies out of sight, is
/// not moved into that package.
#[test]
fn a_package_let_go_outside_the_canvas_stays_where_it_was() {
    let mut h = workspace_harness(ThemeKind::Light, egui::vec2(1600.0, 1000.0));
    let id = |v: serde_json::Value| archforge_mof::Uuid::parse(v["id"].as_str().unwrap()).unwrap();
    let (top, sales, sales_card) = {
        let s = h.state_mut().session.as_mut().unwrap();
        let root = id(s.execute("uml.model.root", &serde_json::json!({})).unwrap().value);
        let top = id(s
            .execute("diagram.add", &serde_json::json!({ "kind": archforge_uml::STRUCTURE_DIAGRAM, "element": root.to_string(), "name": "Top" }))
            .unwrap()
            .value);
        let sales = id(s.execute("uml.package.add", &serde_json::json!({ "name": "Sales" })).unwrap().value);
        let billing = id(s.execute("uml.package.add", &serde_json::json!({ "name": "Billing" })).unwrap().value);
        let sales_card = id(s
            .execute(
                "diagram.place",
                &serde_json::json!({ "diagram": top.to_string(), "element": sales.to_string(), "x": 400, "y": 120, "width": 240, "height": 148 }),
            )
            .unwrap()
            .value);
        // Billing far to the left, its card under the Models explorer once the view is at 0,0.
        s.execute(
            "diagram.place",
            &serde_json::json!({ "diagram": top.to_string(), "element": billing.to_string(), "x": -300, "y": 120, "width": 240, "height": 148 }),
        )
        .unwrap();
        (top, sales, sales_card)
    };
    archforge_ui_egui::canvas::open(h.state_mut(), top);
    h.state_mut().canvas.fit_pending = false;
    h.state_mut().canvas.zoom = 1.0;
    h.state_mut().canvas.pan = egui::Vec2::ZERO;
    h.run();
    let canvas = rect_of(&h, "workspace.canvas");
    let from = rect_of(&h, &format!("workspace.canvas.node.{sales_card}")).center();
    // Over the Models explorer, where Billing's card lies out of sight.
    let outside = egui::pos2(canvas.left() - 120.0, canvas.top() + 120.0 + 74.0);
    drag(&mut h, from, outside);
    let st = h.state().session.as_ref().unwrap().doc();
    let parent = archforge_uml::commands::package_of(st, st.by_uuid(sales).unwrap()).map(|p| archforge_core::name_of(st, p));
    assert_eq!(parent.as_deref(), Some("Commerce platform"), "not moved into a package out of sight");
}

/// Zooming out to 10 % goes up one package level per gesture, landing at 100 %, and stops at the
/// root; the zoom never goes below 10 %. In light and dark.
#[test]
fn zooming_out_to_ten_percent_climbs_one_level_per_gesture_each_at_a_hundred_and_stops_at_the_root() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let mut h = workspace_harness(theme, egui::vec2(1600.0, 1000.0));
        let id = |v: serde_json::Value| archforge_mof::Uuid::parse(v["id"].as_str().unwrap()).unwrap();
        // Root > Sales > Orders, each with its diagram, the next one's card on it.
        let (top, sales_d, orders_d) = {
            let s = h.state_mut().session.as_mut().unwrap();
            let root = id(s.execute("uml.model.root", &serde_json::json!({})).unwrap().value);
            let top = id(s
                .execute("diagram.add", &serde_json::json!({ "kind": archforge_uml::STRUCTURE_DIAGRAM, "element": root.to_string(), "name": "Top" }))
                .unwrap()
                .value);
            let sales = id(s.execute("uml.package.add", &serde_json::json!({ "name": "Sales" })).unwrap().value);
            let orders = id(s.execute("uml.package.add", &serde_json::json!({ "name": "Orders", "parent": sales.to_string() })).unwrap().value);
            let (sales_d, orders_d) = (own_diagram(s, sales), own_diagram(s, orders));
            s.execute(
                "diagram.place",
                &serde_json::json!({ "diagram": top.to_string(), "element": sales.to_string(), "x": 100, "y": 100, "width": 240, "height": 148 }),
            )
            .unwrap();
            s.execute("diagram.place", &serde_json::json!({ "diagram": sales_d.to_string(), "element": orders.to_string(), "x": 300, "y": 200, "width": 240, "height": 148 })).unwrap();
            (top, sales_d, orders_d)
        };
        archforge_ui_egui::canvas::open(h.state_mut(), orders_d);
        h.state_mut().canvas.fit_pending = false;
        h.state_mut().canvas.zoom = 1.0;
        h.run();
        rest(&mut h);
        let active = |h: &Harness<'static, Shell>| h.state().canvas.active;
        h.event(egui::Event::PointerMoved(rect_of(&h, "workspace.canvas").center()));
        // Above 10 % it stays; at 10 % it goes up one level, at 100 %. The same gesture going on
        // stays on Sales, whatever its zoom.
        let mut zooms = Vec::new();
        for _ in 0..60 {
            h.event(egui::Event::Zoom(0.8));
            h.step();
            if active(&h) == Some(orders_d) {
                zooms.push(h.state().canvas.zoom);
            }
        }
        assert!(zooms.iter().all(|z| *z >= archforge_shell_api::canvas::ZOOM_RANGE.0), "{theme:?}: never below 10 %");
        assert!(zooms.windows(2).all(|w| w[1] <= w[0]), "{theme:?}: Orders zoomed out steadily before leaving");
        assert_eq!(active(&h), Some(sales_d), "{theme:?}: one level up, not two, from one long gesture");
        // Once the gesture rests, the next one goes up again, landing at 100 %.
        rest(&mut h);
        h.event(egui::Event::PointerMoved(rect_of(&h, "workspace.canvas").center()));
        for _ in 0..30 {
            if active(&h) == Some(top) {
                break;
            }
            h.event(egui::Event::Zoom(0.8));
            h.step();
        }
        assert_eq!(active(&h), Some(top), "{theme:?}: Sales to the root");
        h.run();
        assert_eq!(h.state().canvas.zoom, 1.0, "{theme:?}: the root at 100 %");
        // At the root, 10 % stays put and says so.
        rest(&mut h);
        h.event(egui::Event::PointerMoved(rect_of(&h, "workspace.canvas").center()));
        for _ in 0..30 {
            h.event(egui::Event::Zoom(0.8));
            h.step();
        }
        h.run();
        assert_eq!(active(&h), Some(top), "{theme:?}: the root stays");
        assert!((h.state().canvas.zoom - archforge_shell_api::canvas::ZOOM_RANGE.0).abs() < 1e-4, "{theme:?}: at 10 %");
        assert!(painted(&h).iter().any(|t| t == "This is the top package: zooming out goes no further"), "{theme:?}: says so");
        // The − key reaches 10 % too, the keyboard's way.
        archforge_ui_egui::canvas::open(h.state_mut(), orders_d);
        h.state_mut().canvas.zoom = 0.2;
        h.state_mut().canvas.focused = true;
        h.run();
        rest(&mut h);
        h.state_mut().canvas.focused = true;
        for _ in 0..12 {
            if active(&h) != Some(orders_d) {
                break;
            }
            key(&mut h, egui::Key::Minus, egui::Modifiers::NONE);
            rest(&mut h);
            h.state_mut().canvas.focused = true;
        }
        assert_eq!(active(&h), Some(sales_d), "{theme:?}: − to 10 % goes up one level");
        assert_eq!(h.state().canvas.zoom, 1.0, "{theme:?}: at 100 %");
    }
}

/// Zooming out at 10 % returns to the diagram a package was entered from though it is in another
/// branch (AR-664, Decision 4): the Commerce overview shows Archive, owned by Operations; inside
/// Archive the path bar shows Operations, yet zooming out lands on the overview. The return takes
/// its step and makes none: one gesture makes one switch, and the next goes up by ownership instead
/// of bouncing back into Archive. Browsing changes nothing in the model or its undo history.
#[test]
fn zooming_out_returns_across_branches_to_the_diagram_entered_from_without_a_bounce() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let mut h = workspace_harness(theme, egui::vec2(1600.0, 1000.0));
        let id = |v: serde_json::Value| archforge_mof::Uuid::parse(v["id"].as_str().unwrap()).unwrap();
        // Root > Commerce and Root > Operations > Archive; Commerce's overview shows Archive.
        let (top, overview, operations_d, archive, archive_d) = {
            let s = h.state_mut().session.as_mut().unwrap();
            let root = id(s.execute("uml.model.root", &serde_json::json!({})).unwrap().value);
            let add = |s: &mut archforge_notations::Session, on: archforge_mof::Uuid, name: &str| {
                let p = serde_json::json!({ "kind": archforge_uml::STRUCTURE_DIAGRAM, "element": on.to_string(), "name": name });
                id(s.execute("diagram.add", &p).unwrap().value)
            };
            let top = add(s, root, "Top");
            let commerce = id(s.execute("uml.package.add", &serde_json::json!({ "name": "Commerce" })).unwrap().value);
            let operations = id(s.execute("uml.package.add", &serde_json::json!({ "name": "Operations" })).unwrap().value);
            let archive =
                id(s.execute("uml.package.add", &serde_json::json!({ "name": "Archive", "parent": operations.to_string() })).unwrap().value);
            let overview = add(s, commerce, "Commerce overview");
            let (operations_d, archive_d) = (own_diagram(s, operations), own_diagram(s, archive));
            let p = serde_json::json!({ "diagram": overview.to_string(), "element": archive.to_string(), "x": 200, "y": 160, "width": 240, "height": 148 });
            s.execute("diagram.place", &p).unwrap();
            s.mark_saved().unwrap();
            (top, overview, operations_d, archive, archive_d)
        };
        let unchanged = |h: &Harness<'static, Shell>| {
            let s = h.state().session.as_ref().unwrap();
            (s.revision(), s.can_undo(), s.can_redo(), s.is_dirty())
        };
        let before = unchanged(&h);
        archforge_ui_egui::canvas::open(h.state_mut(), overview);
        h.run();
        archforge_ui_egui::snapshot::cx(h.state_mut(), |cx| uml_package_nav::enter(cx, archive, 0.0));
        h.run();
        assert_eq!(h.state().canvas.active, Some(archive_d), "{theme:?}");
        // The path bar follows ownership, not the way in: the root, Operations, Archive.
        let path: Vec<String> = texts_in(&h, rect_of(&h, "workspace.canvas.path")).into_iter().map(|(t, _)| t).collect();
        assert_eq!(path.get(1..), Some(&["Operations".to_string(), "Archive".to_string()][..]), "{theme:?}: {path:?}");
        // One long gesture: back on the overview, not Operations' diagram, and no further.
        rest(&mut h);
        assert_eq!(zoom_out_gesture(&mut h, 40, false), vec![archive_d, overview], "{theme:?}: one switch, across branches");
        assert_ne!(h.state().canvas.active, Some(operations_d), "{theme:?}");
        assert_eq!(route_of(&h, overview), vec![], "{theme:?}: the step taken, no reverse entry made");
        assert_eq!(route_of(&h, archive_d), vec![], "{theme:?}: Archive's tab has no way back left");
        // The next gesture has no route: up by ownership to the root's diagram, not back into Archive.
        rest(&mut h);
        assert_eq!(zoom_out_gesture(&mut h, 40, false), vec![overview, top], "{theme:?}: no bounce");
        assert_eq!(unchanged(&h), before, "{theme:?}: no change, no undo step, nothing to save");
    }
}

/// Repeated visits A → B → A → B return B → A → B → A, one step a gesture, each landing as it was
/// left, fitted here, so at 100 % with the package left centred; once the route is spent, the owning
/// parent's default opens.
#[test]
fn zooming_out_walks_repeated_visits_back_one_step_a_gesture_then_follows_ownership() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let mut h = workspace_harness(theme, egui::vec2(1600.0, 1000.0));
        let id = |v: serde_json::Value| archforge_mof::Uuid::parse(v["id"].as_str().unwrap()).unwrap();
        // A is Commerce's diagram and shows Archive; B is Archive's and shows Commerce.
        let (top, commerce, archive, a, b, archive_card, commerce_card) = {
            let s = h.state_mut().session.as_mut().unwrap();
            let root = id(s.execute("uml.model.root", &serde_json::json!({})).unwrap().value);
            let p = serde_json::json!({ "kind": archforge_uml::STRUCTURE_DIAGRAM, "element": root.to_string(), "name": "Top" });
            let top = id(s.execute("diagram.add", &p).unwrap().value);
            let commerce = id(s.execute("uml.package.add", &serde_json::json!({ "name": "Commerce" })).unwrap().value);
            let operations = id(s.execute("uml.package.add", &serde_json::json!({ "name": "Operations" })).unwrap().value);
            let archive =
                id(s.execute("uml.package.add", &serde_json::json!({ "name": "Archive", "parent": operations.to_string() })).unwrap().value);
            let (a, b) = (own_diagram(s, commerce), own_diagram(s, archive));
            let mut place = |on: archforge_mof::Uuid, what: archforge_mof::Uuid| {
                let p =
                    serde_json::json!({ "diagram": on.to_string(), "element": what.to_string(), "x": 200, "y": 160, "width": 240, "height": 148 });
                id(s.execute("diagram.place", &p).unwrap().value)
            };
            let (archive_card, commerce_card) = (place(a, archive), place(b, commerce));
            (top, commerce, archive, a, b, archive_card, commerce_card)
        };
        archforge_ui_egui::canvas::open(h.state_mut(), a);
        h.run();
        for package in [archive, commerce, archive] {
            archforge_ui_egui::snapshot::cx(h.state_mut(), |cx| uml_package_nav::enter(cx, package, 0.0));
            h.run();
            rest(&mut h);
        }
        assert_eq!(h.state().canvas.active, Some(b), "{theme:?}");
        assert_eq!(route_of(&h, b), vec![(a, b), (b, a), (a, b)], "{theme:?}");
        // Back B → A → B → A: each gesture one switch and one step consumed, none added.
        for (from, to, card, left) in [(b, a, archive_card, vec![(a, b), (b, a)]), (a, b, commerce_card, vec![(a, b)]), (b, a, archive_card, vec![])]
        {
            assert_eq!(zoom_out_gesture(&mut h, 40, true), vec![from, to], "{theme:?}");
            assert_eq!(route_of(&h, to), left, "{theme:?}: from {from} to {to}");
            assert_eq!(route_of(&h, from), vec![], "{theme:?}: the tab left keeps no way back");
            assert_eq!(h.state().canvas.zoom, 1.0, "{theme:?}: at 100 %");
            let canvas = rect_of(&h, "workspace.canvas");
            let shown = rect_of(&h, &format!("workspace.canvas.node.{card}"));
            assert!((shown.center() - canvas.center()).length() < 2.0, "{theme:?}: the package left, centred: {shown:?} in {canvas:?}");
            rest(&mut h);
        }
        // The route spent: Commerce's owning parent's default.
        assert_eq!(zoom_out_gesture(&mut h, 40, true), vec![a, top], "{theme:?}: by ownership");
    }
}

/// A zoom out whose way in leads only to a removed diagram spends the route and opens the owning
/// parent's default instead of staying put on a diagram that is gone.
#[test]
fn zooming_out_past_a_removed_way_in_opens_the_owning_parents_default() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let mut h = workspace_harness(theme, egui::vec2(1600.0, 1000.0));
        let id = |v: serde_json::Value| archforge_mof::Uuid::parse(v["id"].as_str().unwrap()).unwrap();
        // Commerce's second diagram, the overview, shows Archive, owned by Operations.
        let (overview, operations_d, archive, archive_d) = {
            let s = h.state_mut().session.as_mut().unwrap();
            let commerce = id(s.execute("uml.package.add", &serde_json::json!({ "name": "Commerce" })).unwrap().value);
            let operations = id(s.execute("uml.package.add", &serde_json::json!({ "name": "Operations" })).unwrap().value);
            let archive =
                id(s.execute("uml.package.add", &serde_json::json!({ "name": "Archive", "parent": operations.to_string() })).unwrap().value);
            own_diagram(s, commerce);
            let p = serde_json::json!({ "kind": archforge_uml::STRUCTURE_DIAGRAM, "element": commerce.to_string(), "name": "Commerce overview" });
            let overview = id(s.execute("diagram.add", &p).unwrap().value);
            let (operations_d, archive_d) = (own_diagram(s, operations), own_diagram(s, archive));
            let p = serde_json::json!({ "diagram": overview.to_string(), "element": archive.to_string(), "x": 200, "y": 160, "width": 240, "height": 148 });
            s.execute("diagram.place", &p).unwrap();
            (overview, operations_d, archive, archive_d)
        };
        archforge_ui_egui::canvas::open(h.state_mut(), overview);
        h.run();
        archforge_ui_egui::snapshot::cx(h.state_mut(), |cx| uml_package_nav::enter(cx, archive, 0.0));
        h.run();
        h.state_mut().session.as_mut().unwrap().execute("diagram.remove", &serde_json::json!({ "diagram": overview.to_string() })).unwrap();
        h.run();
        rest(&mut h);
        assert_eq!(zoom_out_gesture(&mut h, 40, true), vec![archive_d, operations_d], "{theme:?}: Operations' default");
        assert_eq!(route_of(&h, archive_d), vec![], "{theme:?}: the route spent");
    }
}

/// Leaving by the path bar or ⌘↑ follows ownership and starts a new route (AR-666, Decision 4):
/// after Archive, owned by Operations, is entered from the Commerce overview, the Operations pill
/// and ⌘↑ both open Operations' default, not the overview. That tab came from the overview before,
/// yet its route starts empty, so the next zoom out goes up by ownership to the root's default
/// rather than back to the overview. Browsing changes nothing in the model or its undo history.
#[test]
fn the_path_bar_and_command_up_follow_ownership_and_start_a_new_route() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        for by_key in [false, true] {
            let mut h = workspace_harness(theme, egui::vec2(1600.0, 1000.0));
            let id = |v: serde_json::Value| archforge_mof::Uuid::parse(v["id"].as_str().unwrap()).unwrap();
            // Root > Commerce and Root > Operations > Archive; Commerce's overview shows Operations
            // and Archive.
            let (top, overview, operations, operations_d, archive, archive_d) = {
                let s = h.state_mut().session.as_mut().unwrap();
                let root = id(s.execute("uml.model.root", &serde_json::json!({})).unwrap().value);
                let p = serde_json::json!({ "kind": archforge_uml::STRUCTURE_DIAGRAM, "element": root.to_string(), "name": "Top" });
                let top = id(s.execute("diagram.add", &p).unwrap().value);
                let commerce = id(s.execute("uml.package.add", &serde_json::json!({ "name": "Commerce" })).unwrap().value);
                let operations = id(s.execute("uml.package.add", &serde_json::json!({ "name": "Operations" })).unwrap().value);
                let archive =
                    id(s.execute("uml.package.add", &serde_json::json!({ "name": "Archive", "parent": operations.to_string() })).unwrap().value);
                let overview = own_diagram(s, commerce);
                let (operations_d, archive_d) = (own_diagram(s, operations), own_diagram(s, archive));
                for (what, x) in [(operations, 200), (archive, 600)] {
                    let p = serde_json::json!({ "diagram": overview.to_string(), "element": what.to_string(), "x": x, "y": 160, "width": 240, "height": 148 });
                    s.execute("diagram.place", &p).unwrap();
                }
                s.mark_saved().unwrap();
                (top, overview, operations, operations_d, archive, archive_d)
            };
            let unchanged = |h: &Harness<'static, Shell>| {
                let s = h.state().session.as_ref().unwrap();
                (s.revision(), s.can_undo(), s.can_redo(), s.is_dirty())
            };
            let before = unchanged(&h);
            // Operations' tab first comes from the overview; then Archive is entered from it too.
            archforge_ui_egui::canvas::open(h.state_mut(), overview);
            h.run();
            archforge_ui_egui::snapshot::cx(h.state_mut(), |cx| uml_package_nav::enter(cx, operations, 0.0));
            h.run();
            assert_eq!(route_of(&h, operations_d), vec![(overview, operations_d)], "{theme:?}");
            rest(&mut h);
            archforge_ui_egui::canvas::open(h.state_mut(), overview);
            h.run();
            archforge_ui_egui::snapshot::cx(h.state_mut(), |cx| uml_package_nav::enter(cx, archive, 0.0));
            h.run();
            assert_eq!(route_of(&h, archive_d), vec![(overview, archive_d)], "{theme:?}");
            rest(&mut h);
            if by_key {
                h.state_mut().canvas.focused = true;
                h.run();
                key(&mut h, egui::Key::ArrowUp, egui::Modifiers::MAC_CMD | egui::Modifiers::COMMAND);
            } else {
                // The path bar: the root, Operations, Archive.
                let at = rect_of(&h, "workspace.canvas.path.1").center();
                click(&mut h, at);
            }
            assert_eq!(h.state().canvas.active, Some(operations_d), "{theme:?} by key {by_key}: Operations' default, not the overview");
            assert_eq!(route_of(&h, operations_d), vec![], "{theme:?} by key {by_key}: a new route, no step made up from Archive");
            rest(&mut h);
            assert_eq!(zoom_out_gesture(&mut h, 40, true), vec![operations_d, top], "{theme:?} by key {by_key}: up by ownership");
            assert_eq!(unchanged(&h), before, "{theme:?} by key {by_key}: no change, no undo step, nothing to save");
        }
    }
}

/// A switch fades in over the canvas, whatever draws there (AR-788): the frame after a package is
/// entered covers the canvas with its own colour, part see-through; once the fade has passed, the
/// canvas is clear. In light and dark.
#[test]
fn a_switch_fades_in_over_the_canvas() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let mut h = workspace_harness(theme, egui::vec2(1600.0, 1000.0));
        let (_, first, sales, _) = first_and_sales(&mut h);
        archforge_ui_egui::canvas::open(h.state_mut(), first);
        h.run();
        let veiled = |h: &Harness<'static, Shell>| {
            /// Every shape `s` holds, nested lists opened.
            fn walk<'a>(s: &'a egui::Shape, out: &mut Vec<&'a egui::Shape>) {
                match s {
                    egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
                    s => out.push(s),
                }
            }
            let canvas = rect_of(h, "workspace.canvas");
            let mut flat = Vec::new();
            h.output().shapes.iter().for_each(|c| walk(&c.shape, &mut flat));
            flat.iter().any(|s| {
                matches!(s, egui::Shape::Rect(f) if (1..255).contains(&f.fill.a()) && f.rect.min.distance(canvas.min) < 0.5 && f.rect.max.distance(canvas.max) < 0.5)
            })
        };
        assert!(!veiled(&h), "{theme:?}: no fade before a switch");
        let now = h.ctx.input(|i| i.time);
        archforge_ui_egui::snapshot::cx(h.state_mut(), |cx| uml_package_nav::enter(cx, sales, now));
        h.step();
        assert!(veiled(&h), "{theme:?}: the switch fades in");
        h.run();
        assert!(!veiled(&h), "{theme:?}: the fade has passed");
    }
}

/// The path bar restores a diagram as it was last left, however it was left (AR-666): First, left
/// once by entering Sales, is shown again by its tab, zoomed out and left by a tab switch or by
/// closing its tab; the path bar from Sales then opens First at that latest view.
#[test]
fn the_path_bar_restores_the_view_a_diagram_was_last_left_with_by_a_tab_switch_or_close() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        for by_close in [false, true] {
            let mut h = workspace_harness(theme, egui::vec2(1600.0, 1000.0));
            let (_, first, sales, sales_d) = first_and_sales(&mut h);
            archforge_ui_egui::canvas::open(h.state_mut(), first);
            h.run();
            archforge_ui_egui::snapshot::cx(h.state_mut(), |cx| uml_package_nav::enter(cx, sales, 0.0));
            h.run();
            rest(&mut h);
            let carried = (h.state().canvas.pan, h.state().canvas.zoom);
            let at = rect_of(&h, &format!("workspace.tabs.{first}")).center();
            click(&mut h, at);
            let c = &h.state().canvas;
            assert_eq!((c.active, (c.pan, c.zoom)), (Some(first), carried), "{theme:?}: an ordinary switch keeps the view, as before");
            let latest = (egui::vec2(-37.0, 52.0), 0.5);
            (h.state_mut().canvas.pan, h.state_mut().canvas.zoom) = latest;
            h.run();
            let tab = if by_close { format!("workspace.tabs.{first}.close") } else { format!("workspace.tabs.{sales_d}") };
            let at = rect_of(&h, &tab).center();
            click(&mut h, at);
            assert_eq!(h.state().canvas.active, Some(sales_d), "{theme:?} by close {by_close}");
            // Sales gets a view of its own, so only the view kept for First can bring `latest` back.
            (h.state_mut().canvas.pan, h.state_mut().canvas.zoom) = (egui::vec2(11.0, -7.0), 1.25);
            rest(&mut h);
            let at = rect_of(&h, "workspace.canvas.path.0").center();
            click(&mut h, at);
            assert_eq!(h.state().canvas.active, Some(first), "{theme:?} by close {by_close}");
            assert_eq!((h.state().canvas.pan, h.state().canvas.zoom), latest, "{theme:?} by close {by_close}: the view First was last left with");
        }
    }
}

/// A view is kept only once the canvas has drawn it for its tab (AR-666). After a zoom out of
/// Sales, or entering Sales and opening First again, First has Sales' pan until its next frame.
/// Leaving First for Sales before that frame keeps nothing for First, so the path bar later opens
/// First as it was last shown, not at a mix of two views.
#[test]
fn a_tab_left_before_its_view_is_drawn_keeps_the_view_it_was_last_shown_with() {
    for (theme, at_once) in [(ThemeKind::Light, false), (ThemeKind::Dark, false), (ThemeKind::Light, true), (ThemeKind::Dark, true)] {
        let mut h = workspace_harness(theme, egui::vec2(1600.0, 1000.0));
        let (_, first, sales, sales_d) = first_and_sales(&mut h);
        archforge_ui_egui::canvas::open(h.state_mut(), first);
        h.run();
        let shown = (egui::vec2(-37.0, 52.0), 0.5);
        (h.state_mut().canvas.pan, h.state_mut().canvas.zoom) = shown;
        h.run();
        archforge_ui_egui::snapshot::cx(h.state_mut(), |cx| uml_package_nav::enter(cx, sales, 0.0));
        if at_once {
            archforge_ui_egui::canvas::open(h.state_mut(), first);
        } else {
            h.run();
            rest(&mut h);
            zoom_out_to(&mut h, first);
        }
        assert_eq!(h.state().canvas.active, Some(first), "{theme:?} at once {at_once}: back on First");
        archforge_ui_egui::canvas::open(h.state_mut(), sales_d);
        h.run();
        rest(&mut h);
        let at = rect_of(&h, "workspace.canvas.path.0").center();
        click(&mut h, at);
        assert_eq!(h.state().canvas.active, Some(first), "{theme:?} at once {at_once}");
        assert_eq!((h.state().canvas.pan, h.state().canvas.zoom), shown, "{theme:?} at once {at_once}: First as it was last shown");
    }
}

/// A landing or fit package navigation leaves pending is for the tab it opened (AR-666). Support, a
/// wide diagram opened in a new tab before First or Sales draws, is fitted below 100 %, neither
/// landed at 100 % nor centred. Taking over from First before it draws, by First's removal or by
/// reopening its closed tab, Support shows as last shown, or fitted when it never was; so does First
/// after Support, opened new. First opened again before it draws still lands, at the view it was
/// left with (AR-668).
#[test]
fn a_landing_pending_for_one_tab_never_places_another() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        for case in
            ["out, Support", "in, Support", "out, First gone", "out, First gone, unseen", "out, reopened", "out, Support, First", "out, First again"]
        {
            let mut h = workspace_harness(theme, egui::vec2(1600.0, 1000.0));
            let (root, first, sales, sales_d) = first_and_sales(&mut h);
            let support = {
                let s = h.state_mut().session.as_mut().unwrap();
                let id = |v: serde_json::Value| archforge_mof::Uuid::parse(v["id"].as_str().unwrap()).unwrap();
                let d = id(s
                    .execute("diagram.add", &serde_json::json!({ "kind": archforge_uml::STRUCTURE_DIAGRAM, "element": root, "name": "Support" }))
                    .unwrap()
                    .value);
                for (name, x) in [("Ticket", 0), ("Agent", 4000)] {
                    let c = id(s.execute("uml.class.add", &serde_json::json!({ "package": root, "name": name })).unwrap().value);
                    let p = serde_json::json!({ "diagram": d.to_string(), "element": c.to_string(), "x": x, "y": 0, "width": 200, "height": 120 });
                    s.execute("diagram.place", &p).unwrap();
                }
                d
            };
            let (shown, first_view) = ((egui::vec2(5.0, 6.0), 0.3), (egui::vec2(-50.0, 30.0), 0.6));
            if case.starts_with("out, First gone") || case == "out, reopened" {
                archforge_ui_egui::canvas::open(h.state_mut(), support);
                if !case.ends_with("unseen") {
                    h.run();
                    (h.state_mut().canvas.pan, h.state_mut().canvas.zoom) = shown;
                }
            }
            archforge_ui_egui::canvas::open(h.state_mut(), first);
            h.run();
            (h.state_mut().canvas.pan, h.state_mut().canvas.zoom) = first_view;
            if case == "out, reopened" {
                archforge_ui_egui::canvas::close_tab(h.state_mut(), support);
            }
            archforge_ui_egui::snapshot::cx(h.state_mut(), |cx| uml_package_nav::enter(cx, sales, 0.0));
            if case != "in, Support" {
                h.run();
                rest(&mut h);
                zoom_out_to(&mut h, first);
            }
            match case {
                c if c.starts_with("out, First gone") => {
                    archforge_ui_egui::canvas::close_tab(h.state_mut(), sales_d);
                    let s = h.state_mut().session.as_mut().unwrap();
                    let (at, d) = (s.token(), first.to_string());
                    let p = serde_json::json!({ "diagram": d, "replacement_diagram": support.to_string(), "expected_revision": { "history": at.history, "revision": at.revision } });
                    s.execute("diagram.remove", &p).unwrap();
                }
                "out, First again" => archforge_ui_egui::canvas::open(h.state_mut(), first),
                "out, Support, First" => [support, first].into_iter().for_each(|d| archforge_ui_egui::canvas::open(h.state_mut(), d)),
                _ => archforge_ui_egui::canvas::open(h.state_mut(), support),
            }
            h.run();
            let (pan, zoom, size) = (h.state().canvas.pan, h.state().canvas.zoom, h.state().canvas.size);
            match case {
                "out, First again" => assert_eq!((pan, zoom), first_view, "{theme:?}: First landed as it was left"),
                "out, First gone" | "out, reopened" => assert_eq!((h.state().canvas.active, (pan, zoom)), (Some(support), shown), "{theme:?} {case}"),
                "out, Support, First" => assert_eq!((h.state().canvas.active, (pan, zoom)), (Some(first), first_view), "{theme:?}: as last shown"),
                _ => {
                    let seen = [0.0, 4200.0].iter().all(|x| (0.0..=size.x).contains(&(pan.x + x * zoom)));
                    assert!(h.state().canvas.active == Some(support) && seen, "{theme:?} {case}: both Support cards in view, not landed at {zoom}");
                }
            }
        }
    }
}

/// A pending fit and a view restored but not yet drawn stay on their own tab across tab changes
/// in one frame (AR-758). First is removed with its landing pending, so Support takes over at the
/// view it was last shown with; Overview, chosen before Support draws, shows its own view, not
/// Support's. Support, opened new and left for First before its fit, is fitted when chosen later
/// from First drawn, both its cards in view, not at First's view, and so when reopened after a
/// close. First's tab closed with its landing pending hands over to Sales as it was last left.
#[test]
fn a_pending_fit_or_restored_view_stays_on_its_tab_across_switches_in_one_frame() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        for case in ["restored", "unseen", "closed"] {
            let restored = case == "restored";
            let mut h = workspace_harness(theme, egui::vec2(1600.0, 1000.0));
            let (root, first, sales, sales_d) = first_and_sales(&mut h);
            let id = |v: serde_json::Value| archforge_mof::Uuid::parse(v["id"].as_str().unwrap()).unwrap();
            let [overview, support] = ["Overview", "Support"].map(|name| {
                let s = h.state_mut().session.as_mut().unwrap();
                id(s.execute("diagram.add", &serde_json::json!({ "kind": archforge_uml::STRUCTURE_DIAGRAM, "element": root, "name": name }))
                    .unwrap()
                    .value)
            });
            let s = h.state_mut().session.as_mut().unwrap();
            for (name, x) in [("Ticket", 0), ("Agent", 4000)] {
                let c = id(s.execute("uml.class.add", &serde_json::json!({ "package": root, "name": name })).unwrap().value);
                let p = serde_json::json!({ "diagram": support.to_string(), "element": c.to_string(), "x": x, "y": 0, "width": 200, "height": 120 });
                s.execute("diagram.place", &p).unwrap();
            }
            let views = [(egui::vec2(70.0, -20.0), 0.8), (egui::vec2(5.0, 6.0), 0.3), (egui::vec2(-50.0, 30.0), 0.6)];
            let shown = if restored { vec![overview, support, first] } else { vec![first] };
            for (d, view) in shown.into_iter().zip(if restored { &views[..] } else { &views[2..] }) {
                archforge_ui_egui::canvas::open(h.state_mut(), d);
                h.run();
                (h.state_mut().canvas.pan, h.state_mut().canvas.zoom) = *view;
                h.run();
            }
            archforge_ui_egui::snapshot::cx(h.state_mut(), |cx| uml_package_nav::enter(cx, sales, 0.0));
            h.run();
            rest(&mut h);
            zoom_out_to(&mut h, first);
            let seen = |c: &archforge_shell_api::canvas::State| [0.0, 4200.0].iter().all(|x| (0.0..=c.size.x).contains(&(c.pan.x + x * c.zoom)));
            if case == "closed" {
                let kept = h.state().canvas.views.get(&sales_d).copied();
                archforge_ui_egui::canvas::close_tab(h.state_mut(), first);
                h.run();
                let c = &h.state().canvas;
                assert_eq!((c.active, Some((c.pan, c.zoom))), (Some(sales_d), kept), "{theme:?}: Sales as last left");
            } else if restored {
                archforge_ui_egui::canvas::close_tab(h.state_mut(), sales_d);
                let s = h.state_mut();
                let session = s.session.as_mut().unwrap();
                let (at, d) = (session.token(), first.to_string());
                let p = serde_json::json!({ "diagram": d, "replacement_diagram": support.to_string(), "expected_revision": { "history": at.history, "revision": at.revision } });
                session.execute("diagram.remove", &p).unwrap();
                archforge_ui_egui::canvas::retain_valid(&mut s.canvas, s.session.as_ref().unwrap().doc());
                assert_eq!((s.canvas.active, (s.canvas.pan, s.canvas.zoom)), (Some(support), views[1]), "{theme:?}: Support as last shown");
                archforge_ui_egui::canvas::open(h.state_mut(), overview);
                h.run();
                let c = &h.state().canvas;
                assert_eq!((c.active, (c.pan, c.zoom)), (Some(overview), views[0]), "{theme:?}: Overview's own view, not Support's");
            } else {
                [support, first].into_iter().for_each(|d| archforge_ui_egui::canvas::open(h.state_mut(), d));
                h.run();
                archforge_ui_egui::canvas::open(h.state_mut(), support);
                h.run();
                let c = &h.state().canvas;
                assert!(c.active == Some(support) && seen(c), "{theme:?}: Support fitted, both cards in view, not at {:?}", (c.pan, c.zoom));
                // Closed and opened again from Sales drawn at a view of its own, Support is a new
                // tab, fitted again.
                archforge_ui_egui::canvas::close_tab(h.state_mut(), support);
                h.run();
                (h.state_mut().canvas.pan, h.state_mut().canvas.zoom) = (egui::vec2(11.0, -7.0), 1.25);
                h.run();
                archforge_ui_egui::canvas::open(h.state_mut(), support);
                h.run();
                assert!(seen(&h.state().canvas), "{theme:?}: Support reopened, fitted, not at Sales' view");
            }
        }
    }
}

/// Entering the package whose diagram is open lands that diagram in place, at 100 % on its next
/// frame; left before that frame, it keeps no view it was never drawn with (AR-666).
#[test]
fn a_landing_in_place_left_before_it_draws_keeps_no_view_never_drawn() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let mut h = workspace_harness(theme, egui::vec2(1600.0, 1000.0));
        let (root, first, sales, sales_d) = first_and_sales(&mut h);
        let p = serde_json::json!({ "diagram": sales_d.to_string(), "element": sales.to_string(), "x": 40, "y": 40, "width": 240, "height": 148 });
        h.state_mut().session.as_mut().unwrap().execute("diagram.place", &p).unwrap();
        archforge_ui_egui::canvas::open(h.state_mut(), sales_d);
        h.run();
        let drawn = (egui::vec2(-37.0, 52.0), 0.5);
        (h.state_mut().canvas.pan, h.state_mut().canvas.zoom) = drawn;
        h.run();
        archforge_ui_egui::snapshot::cx(h.state_mut(), |cx| uml_package_nav::enter(cx, sales, 0.0));
        assert_eq!((h.state().canvas.active, h.state().canvas.zoom), (Some(sales_d), 1.0), "{theme:?}: landing in place");
        archforge_ui_egui::snapshot::cx(h.state_mut(), |cx| uml_package_nav::leave(cx, archforge_mof::Uuid::parse(&root).unwrap(), 0.0));
        assert_eq!(h.state().canvas.active, Some(first), "{theme:?}");
        let kept = h.state().canvas.views.get(&sales_d).copied();
        assert!(kept.is_none() || kept == Some(drawn), "{theme:?}: Sales keeps no view it was never drawn with, {kept:?}");
    }
}

/// After a pause, a new zoom-out gesture that reaches 10 % goes up a level, though the window drew
/// no idle frame in between.
#[test]
fn a_new_gesture_after_a_pause_goes_up_though_no_idle_frame_was_drawn_between() {
    let mut h = workspace_harness(ThemeKind::Light, egui::vec2(1600.0, 1000.0));
    let id = |v: serde_json::Value| archforge_mof::Uuid::parse(v["id"].as_str().unwrap()).unwrap();
    let (sales_d, orders_d) = {
        let s = h.state_mut().session.as_mut().unwrap();
        let sales = id(s.execute("uml.package.add", &serde_json::json!({ "name": "Sales" })).unwrap().value);
        let orders = id(s.execute("uml.package.add", &serde_json::json!({ "name": "Orders", "parent": sales.to_string() })).unwrap().value);
        let (sales_d, orders_d) = (own_diagram(s, sales), own_diagram(s, orders));
        (sales_d, orders_d)
    };
    archforge_ui_egui::canvas::open(h.state_mut(), orders_d);
    h.state_mut().canvas.fit_pending = false;
    h.run();
    h.event(egui::Event::PointerMoved(rect_of(&h, "workspace.canvas").center()));
    h.run();
    // Just above 10 %, the last switch and the last zoom long past, and still disarmed: as an idle
    // window leaves it, having drawn no frame since.
    let now = h.ctx.input(|i| i.time);
    h.state_mut().canvas.switched = Some(now - 5.0);
    let nav = &mut uml_of_mut(h.state_mut()).nav;
    nav.disarmed = true;
    nav.last_zoom = Some(now - 5.0);
    h.state_mut().canvas.zoom = 0.11;
    // The new gesture's first frame reaches 10 % and goes up.
    h.event(egui::Event::Zoom(0.8));
    h.step();
    assert_eq!(h.state().canvas.active, Some(sales_d), "the pause re-armed it");
}

/// Zoomed out to a parent whose diagram has no card of the package left, the parent's diagram opens
/// at 100 % with what it shows centred.
#[test]
fn a_zoom_out_to_a_parent_without_the_packages_card_centres_the_parents_content() {
    let mut h = workspace_harness(ThemeKind::Light, egui::vec2(1600.0, 1000.0));
    let id = |v: serde_json::Value| archforge_mof::Uuid::parse(v["id"].as_str().unwrap()).unwrap();
    // Sales' diagram shows a class far from the origin, and no card of Orders.
    let (sales_d, orders_d, ledger_card) = {
        let s = h.state_mut().session.as_mut().unwrap();
        let sales = id(s.execute("uml.package.add", &serde_json::json!({ "name": "Sales" })).unwrap().value);
        let orders = id(s.execute("uml.package.add", &serde_json::json!({ "name": "Orders", "parent": sales.to_string() })).unwrap().value);
        let (sales_d, orders_d) = (own_diagram(s, sales), own_diagram(s, orders));
        let ledger = id(s.execute("uml.class.add", &serde_json::json!({ "package": sales.to_string(), "name": "Ledger" })).unwrap().value);
        let card = id(s
            .execute("diagram.place", &serde_json::json!({ "diagram": sales_d.to_string(), "element": ledger.to_string(), "x": 2400, "y": 1800 }))
            .unwrap()
            .value);
        (sales_d, orders_d, card)
    };
    archforge_ui_egui::canvas::open(h.state_mut(), orders_d);
    h.state_mut().canvas.fit_pending = false;
    h.state_mut().canvas.zoom = 0.11;
    h.state_mut().canvas.pan = egui::vec2(-900.0, -700.0);
    h.run();
    rest(&mut h);
    h.event(egui::Event::PointerMoved(rect_of(&h, "workspace.canvas").center()));
    h.run();
    h.event(egui::Event::Zoom(0.8));
    h.step();
    h.run();
    assert_eq!(h.state().canvas.active, Some(sales_d));
    assert_eq!(h.state().canvas.zoom, 1.0);
    let canvas = rect_of(&h, "workspace.canvas");
    let card = rect_of(&h, &format!("workspace.canvas.node.{ledger_card}"));
    assert!((card.center() - canvas.center()).length() < 2.0, "what Sales shows, centred: {card:?} in {canvas:?}");
}

/// A class diagram is made in three steps, and its classifier tool places a class named in place.
#[test]
fn a_new_project_makes_a_class_diagram_in_three_steps_and_its_class_tool_places_a_class_named_in_place() {
    let mut h = workspace_harness(ThemeKind::Light, egui::vec2(1600.0, 1000.0));
    // 1. The tab strip's + opens New diagram; 2. the UML structure diagram card, Continue; 3. Create.
    let at = rect_of(&h, "workspace.tabs.add").center();
    click(&mut h, at);
    let at = rect_of(&h, "workspace.new-diagram.type.uml.structure").center();
    click(&mut h, at);
    let at = rect_of(&h, "workspace.new-diagram.continue").center();
    click(&mut h, at);
    let edit = h.state().diagrams.new.clone().expect("the dialog is open");
    assert_eq!(edit.kind, archforge_uml::STRUCTURE_DIAGRAM);
    assert_eq!(edit.owner, None, "a project without a package gets its root package");
    assert!(painted(&h).iter().any(|t| t == "Commerce platform"), "the Package field names the root, after the project");
    h.state_mut().diagrams.new.as_mut().unwrap().name = "Domain".into();
    let at = rect_of(&h, "workspace.new-diagram.create").center();
    click(&mut h, at);
    assert!(h.state().diagrams.new.is_none(), "{:?}", h.state().diagrams.new);
    assert_eq!(class_diagram_shown(&h), (archforge_uml::STRUCTURE_DIAGRAM.to_string(), Vec::new()), "an empty class diagram is open");
    // The diagram is of the model's root package, named after the project; no other package.
    let st = h.state().session.as_ref().unwrap().doc();
    let root = archforge_uml::commands::root_package(st).expect("the root package is made");
    assert_eq!(archforge_uml::commands::all_packages(st), vec![root]);
    assert_eq!(archforge_core::name_of(st, root), "Commerce platform");
    let shown = st.by_uuid(h.state().canvas.active.expect("a diagram is open")).unwrap();
    assert_eq!(st.get(shown, archforge_diagram::diagram::ELEMENT).unwrap().as_ref(), Some(root));
    // Its toolbar: Class, Relationship with its kinds, Note and Link note; ER's tools and its key
    // status are not there.
    let found = ids(&h);
    for tool in ["select", "pan", "classifier", "classifier-kinds", "relationship", "relationship-kinds", "note", "link", "grid", "arrange"] {
        assert!(found.iter().any(|i| *i == format!("workspace.tools.{tool}")), "{tool}");
    }
    for er in ["entity", "relationship-types", "key-status"] {
        assert!(!found.iter().any(|i| *i == format!("workspace.tools.{er}")), "{er}");
    }
    // A click in Select adds nothing; with the Class tool it places a class, named in place.
    let canvas = rect_of(&h, "workspace.canvas");
    click(&mut h, canvas.center());
    assert!(class_diagram_shown(&h).1.is_empty(), "Select adds nothing");
    let at = rect_of(&h, "workspace.tools.classifier").center();
    click(&mut h, at);
    click(&mut h, canvas.center());
    assert_eq!(class_diagram_shown(&h).1, ["Class 1"]);
    assert_eq!(h.state().canvas.tool, archforge_shell_api::canvas::Tool::Select, "the tool hands back to Select");
    let rename = h.state().canvas.rename.clone().expect("the class's name is open for typing");
    assert_eq!(rename.text, "Class 1");
    h.state_mut().canvas.rename.as_mut().unwrap().text = "Order".into();
    key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
    assert_eq!(class_diagram_shown(&h).1, ["Order"]);
    // The tree shows the root package, after the project, with its class; the search finds it.
    let st = h.state().session.as_ref().unwrap().doc();
    let pkg = st.uuid(archforge_uml::commands::packages(st)[0]).unwrap();
    h.state_mut().explorer.expanded.insert(pkg);
    let rows: Vec<String> = archforge_ui_egui::testing::models_rows(
        archforge_ui_egui::testing::notation_uis(h.state()),
        h.state().session.as_ref().unwrap().doc(),
        &h.state().explorer,
    )
    .into_iter()
    .map(|r| r.label)
    .collect();
    assert_eq!(rows, ["Commerce platform", "Order"]);
    let index = archforge_ui_egui::testing::search_index(
        archforge_ui_egui::testing::kinds(h.state()),
        archforge_ui_egui::testing::notation_uis(h.state()),
        h.state().session.as_ref().unwrap().doc(),
    );
    assert!(archforge_ui_egui::testing::search_hits(&index, "ord").1.iter().any(|e| e.label == "Order"));
    // Saved and opened again, as the project's rows: the class diagram comes back.
    let (tables, junctions) = archforge_notations::NOTATIONS.storage();
    let rows = archforge_mof::rows::snapshot(h.state().session.as_ref().unwrap().doc(), &tables, &junctions).unwrap();
    let back = archforge_notations::NOTATIONS.session_from_rows(&rows, Box::new(|| archforge_mof::Uuid::v7(1_761_670_900_000, [9; 10]))).unwrap();
    let st = back.doc();
    let d = st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_diagram::DIAGRAM)).unwrap();
    assert_eq!(st.get(d, archforge_diagram::diagram::KIND).unwrap().as_str(), Some(archforge_uml::STRUCTURE_DIAGRAM));
    assert_eq!(st.list(d, archforge_diagram::diagram::NODE).unwrap().len(), 1);
}

/// The class diagram New diagram makes first is its package's default, bound as an agent's
/// `diagram.add` binds it: `uml.package.default.get` answers it, and a second made the same way
/// leaves the default where it was (AR-579). In both appearances.
#[test]
fn the_first_class_diagram_new_diagram_makes_is_its_packages_default() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let mut h = workspace_harness(theme, egui::vec2(1600.0, 1000.0));
        let mut made = |name: &str| {
            for id in ["workspace.tabs.add", "workspace.new-diagram.type.uml.structure", "workspace.new-diagram.continue"] {
                let at = rect_of(&h, id).center();
                click(&mut h, at);
            }
            h.state_mut().diagrams.new.as_mut().expect("the dialog is open").name = name.into();
            let at = rect_of(&h, "workspace.new-diagram.create").center();
            click(&mut h, at);
            h.state().canvas.active.expect("the new diagram is open")
        };
        let (first, second) = (made("Domain"), made("Detail"));
        assert_ne!(first, second, "{theme:?}");
        let session = h.state_mut().session.as_mut().unwrap();
        let root = archforge_uml::commands::root_package(session.doc()).map(|r| session.doc().uuid(r).unwrap().to_string()).unwrap();
        let answer = session.execute("uml.package.default.get", &serde_json::json!({ "package": root })).unwrap().value;
        assert_eq!((answer["state"].as_str(), answer["diagram"].as_str()), (Some("bound"), Some(first.to_string().as_str())), "{theme:?}");
        assert_eq!(archforge_diagram::defaults::bindings(session.doc()).len(), 1, "{theme:?}: the second binds nothing");
    }
}

/// A class diagram draws every line kind with its ends, dashes and texts; a hovered line turns to
/// the accent, a multiplicity text drags aside and stays, and a segment slides across.
#[test]
fn a_class_diagram_draws_every_line_kind_and_its_lines_highlight_and_their_texts_and_segments_drag() {
    let (mut h, d, e) = read_class_diagram("paid by");
    let st = h.state().session.as_ref().unwrap().doc();
    // Every line kind is on the diagram, each drawn as its notation describes it.
    let lines: Vec<archforge_diagram::figure::Line> = st
        .list(d, archforge_diagram::diagram::EDGE)
        .unwrap()
        .iter()
        .filter_map(archforge_mof::Value::as_ref)
        .filter_map(|edge| st.get(edge, archforge_diagram::edge::ELEMENT).ok()?.as_ref())
        .filter_map(|el| archforge_uml::figures::line(st, el))
        .collect();
    use archforge_diagram::figure::EndMark as M;
    let marks: Vec<[M; 2]> = lines.iter().map(|l| [l.ends[0].mark, l.ends[1].mark]).collect();
    for kind in [M::OpenArrow, M::FilledDiamond, M::HollowDiamond, M::Triangle] {
        assert!(marks.iter().any(|m| m.contains(&kind)), "{kind:?} in {marks:?}");
    }
    assert!(marks.contains(&[M::OpenArrow, M::OpenArrow]), "navigable both ways: an arrow at each end");
    assert!(lines.iter().any(|l| l.dashed), "the dependency is dashed");
    let texts = painted(&h);
    for t in ["CLASS", "Payment", "+ amount : Money", "+ authorise() : Boolean", "+ price : Money [0..1]", "paid by", "1..*", "0..1", "lines"] {
        assert!(texts.iter().any(|x| x == t), "{t} is drawn: {texts:?}");
    }
    // The hovered line turns to the accent.
    assert!(!painted_accent(&h), "nothing is selected or hovered yet");
    let route = |h: &Harness<'static, Shell>| archforge_diagram::Grid::build(h.state().session.as_ref().unwrap().doc(), d).line(e).unwrap().to_vec();
    let before = route(&h);
    assert!(before.len() >= 3, "a right-angled route: {before:?}");
    let view =
        archforge_shell_api::canvas::View { origin: rect_of(&h, "workspace.canvas").min, pan: h.state().canvas.pan, zoom: h.state().canvas.zoom };
    let middle = view.to_screen((before[1].0 + before[2].0) / 2.0, (before[1].1 + before[2].1) / 2.0);
    h.event(egui::Event::PointerMoved(middle));
    h.run();
    assert!(painted_accent(&h), "the hovered line is highlighted");
    // A multiplicity text drags aside and stays where it was let go.
    let edge = h.state().session.as_ref().unwrap().doc().uuid(e).unwrap();
    let text = h.state().canvas.ends.iter().find(|t| t.edge == edge && t.end == 1).cloned().expect("the line's end text is drawn");
    let to = text.rect.center() + egui::vec2(30.0, 30.0);
    drag(&mut h, text.rect.center(), to);
    h.run();
    let moved = h.state().canvas.ends.iter().find(|t| t.edge == edge && t.end == 1).cloned().unwrap();
    assert!((moved.rect.center() - to).length() < 2.0, "drawn where it was let go: {:?} {to:?}", moved.rect.center());
    // A segment slides across.
    let level = (before[1].1 - before[2].1).abs() < 1e-6;
    let shift = if level { egui::vec2(0.0, -40.0) } else { egui::vec2(-40.0, 0.0) };
    let middle = view.to_screen((before[1].0 + before[2].0) / 2.0, (before[1].1 + before[2].1) / 2.0);
    drag(&mut h, middle, middle + shift * view.zoom);
    let after = route(&h);
    let slid = if level { (after[1].1 - (before[1].1 - 40.0)).abs() < 1.0 } else { (after[1].0 - (before[1].0 - 40.0)).abs() < 1.0 };
    assert!(slid, "{before:?} → {after:?}");
}

/// Every compartment of a class's card is padded as the card is (AR-191): the last attribute's
/// text ends as far over the dividing line as the last operation's over the card's foot, and the
/// first operation starts as far under that line as the first attribute under the header's; a new
/// class's default size holds a row in each, and a card one unit short says the row it hides; in
/// light and dark.
#[test]
fn a_class_cards_compartments_are_padded_so_no_row_sits_on_a_dividing_line() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (h, order, _) = nested_orders(theme);
        let card = rect_of(&h, &format!("workspace.canvas.node.{}", node_of(&h, order)));
        let boxes = text_boxes(&h);
        let row =
            |t: &str| boxes.iter().find(|(s, r)| s == t && card.contains(r.center())).map(|(_, r)| *r).unwrap_or_else(|| panic!("{t} on the card"));
        let (first_attribute, last_attribute, operation) = (row("+ number : Integer"), row("+ total : Money"), row("+ pay() : Boolean"));
        // The lines across the card: the header's, then the one between its compartments.
        let mut lines: Vec<f32> = rect_shapes(&h)
            .into_iter()
            .filter(|(r, _, _)| (r.height() - 1.0).abs() < 0.01 && (r.width() - card.width()).abs() < 0.5 && card.contains(r.center()))
            .map(|(r, _, _)| r.top())
            .collect();
        lines.sort_by(f32::total_cmp);
        let [header, divider] = lines[..] else { panic!("{theme:?}: two lines across the card, not {lines:?}") };
        assert!(divider > last_attribute.bottom() + 8.0, "{theme:?}: the line {divider} clear of the row ending at {}", last_attribute.bottom());
        let near = |a: f32, b: f32| (a - b).abs() < 0.5;
        assert!(near(divider - last_attribute.bottom(), card.bottom() - operation.bottom()), "{theme:?}: a compartment's foot is the card's");
        assert!(near(operation.top() - divider, first_attribute.top() - header), "{theme:?}: a compartment's head is the first one's");
    }
    // A new class shows a row in each compartment at its default size; the card height grows by
    // a pitch for each row, and made too short for its last row it counts the rows it hides, the
    // last one that fits saying so.
    assert_eq!(uml_ui::members_height(1, 1), archforge_uml::CLASS_SIZE.1);
    assert_eq!(uml_ui::members_height(2, 1) - uml_ui::members_height(1, 1), f64::from(archforge_shell_api::canvas::CARD_ROW));
    let (mut h, order, _) = nested_orders(ThemeKind::Light);
    let node = node_of(&h, order);
    let height = uml_ui::members_height(2, 1) - 11.0;
    h.state_mut().session.as_mut().unwrap().execute("diagram.resize", &serde_json::json!({ "node": node, "width": 280, "height": height })).unwrap();
    h.run();
    let texts = painted(&h);
    assert!(texts.iter().any(|t| t == "+ 2 more members") && texts.iter().any(|t| t == "+ number : Integer"), "{texts:?}");
}

/// A static member's row is underlined on its card (AR-136): the line runs under the row's text,
/// from where it starts to no further than it ends, in the text's ink, at least a pixel thick, in
/// light and dark and at the detail zoom, 100 % and 200 %; a static abstract operation is
/// underlined and slanted; a plain row has none; Properties' rows read as the card's; a row cut
/// short by a narrow card keeps its underline inside the card; clearing Static takes it away.
#[test]
fn a_static_members_row_is_underlined_on_its_card() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, order, _) = nested_orders(theme);
        let p = archforge_ui_egui::ux::palette(theme == ThemeKind::Dark);
        let (number, pay) = {
            let st = h.state().session.as_ref().unwrap().doc();
            let o = st.by_uuid(order).unwrap();
            let number = st.uuid(archforge_uml::members::attributes(st, o)[0]).unwrap();
            let pay = st.uuid(archforge_uml::members::operations(st, o)[0]).unwrap();
            (number, pay)
        };
        let session = |h: &mut Harness<'static, Shell>, command: &str, params: serde_json::Value| {
            h.state_mut().session.as_mut().unwrap().execute(command, &params).unwrap_or_else(|e| panic!("{command}: {e}"));
            h.run();
        };
        session(&mut h, "uml.attribute.set", serde_json::json!({ "attribute": number.to_string(), "static": true }));
        let static_abstract =
            serde_json::json!({ "operation": pay.to_string(), "name": "pay", "returns": "Boolean", "static": true, "abstract": true });
        session(&mut h, "uml.operation.set", static_abstract);
        // The underline under the row `text` within `area`, when there is one.
        let under = |h: &Harness<'static, Shell>, area: egui::Rect, text: &str| {
            let (_, r) =
                text_boxes(h).into_iter().find(|(t, r)| t == text && area.contains(r.center())).unwrap_or_else(|| panic!("{text} in {area:?}"));
            level_lines(h).into_iter().find(|(left, right, y, _, _)| {
                (left - r.left()).abs() < 1.0 && *right <= r.right() + 0.5 && *right > r.center().x && *y > r.center().y && *y < r.bottom()
            })
        };
        for zoom in [archforge_shell_api::canvas::DETAIL_ZOOM, 1.0, 2.0] {
            h.state_mut().canvas.zoom = zoom;
            h.run();
            let card = rect_of(&h, &format!("workspace.canvas.node.{}", node_of(&h, order)));
            for row in ["+ number : Integer", "+ pay() : Boolean"] {
                let (_, _, _, width, colour) = under(&h, card, row).unwrap_or_else(|| panic!("{theme:?} at {zoom}: {row} is underlined"));
                assert_eq!(colour, egui::Color32::from(p.text), "{theme:?} at {zoom}: {row} in the text's ink");
                assert!(width >= 1.0, "{theme:?} at {zoom}: {row} at least a pixel thick");
            }
            assert!(under(&h, card, "+ total : Money").is_none(), "{theme:?} at {zoom}: a plain row has no underline");
        }
        h.state_mut().canvas.zoom = 1.0;
        h.run();
        // Properties' rows read as the card's.
        session(&mut h, "view.select", serde_json::json!({ "ids": [order.to_string()] }));
        h.state_mut().state.show_panel(archforge_shell_api::state::Panel::Properties);
        h.run();
        let row = rect_of(&h, &format!("workspace.properties.attribute.{number}"));
        assert!(under(&h, row, "+ number : Integer").is_some(), "{theme:?}: Properties' row is underlined");
        let row = rect_of(&h, &format!("workspace.properties.operation.{pay}"));
        assert!(under(&h, row, "+ pay() : Boolean").is_some(), "{theme:?}: Properties' operation row is underlined");
        // A narrow card cuts the row short: its underline stays inside the card.
        let node = node_of(&h, order);
        let height = uml_ui::members_height(2, 1);
        session(&mut h, "diagram.resize", serde_json::json!({ "node": node, "width": 120, "height": height }));
        let card = rect_of(&h, &format!("workspace.canvas.node.{node}"));
        let (_, right, _, _, _) = under(&h, card, "+ number : Integer").unwrap_or_else(|| panic!("{theme:?}: the cut row is underlined"));
        assert!(right < card.right(), "{theme:?}: the underline {right} ends inside the card {card:?}");
        // Static cleared: the underline goes; undone, it is back.
        session(&mut h, "uml.attribute.set", serde_json::json!({ "attribute": number.to_string(), "static": false }));
        let card = rect_of(&h, &format!("workspace.canvas.node.{node}"));
        assert!(under(&h, card, "+ number : Integer").is_none(), "{theme:?}: cleared, no underline");
        h.state_mut().session.as_mut().unwrap().undo().unwrap();
        h.run();
        assert!(under(&h, card, "+ number : Integer").is_some(), "{theme:?}: undone, underlined again");
    }
}

/// A literal's draft row is spaced as an attribute's and an operation's (AR-191): each ring takes
/// its row's pitch, clear of the row over it, the typed text on the line the row's text takes, and
/// its ⋯ on the surface in the same ink; in light and dark.
#[test]
fn a_literal_draft_row_is_spaced_as_an_attribute_draft_row_and_clear_of_the_row_over_it() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, order, status) = nested_orders(theme);
        let p = archforge_ui_egui::ux::palette(theme == ThemeKind::Dark);
        let mut rings = Vec::new();
        for (class, kind, over) in [
            (status, uml_panel::MemberKind::Literal, "Shipped"),
            (order, uml_panel::MemberKind::Attribute, "+ total : Money"),
            (order, uml_panel::MemberKind::Operation, "+ pay() : Boolean"),
        ] {
            assert!(archforge_ui_egui::snapshot::cx(h.state_mut(), |cx| uml_inline::open(cx, class, kind, None)), "{kind:?}'s draft opens");
            uml_of_mut(h.state_mut()).inline.as_mut().unwrap().edit.name = "Returned".into();
            h.run();
            // Its ring: the accent's outline round the tinted row, a row's height, not a card's.
            let tinted: Vec<egui::Rect> =
                rect_shapes(&h).into_iter().filter(|(_, fill, _)| *fill == egui::Color32::from(p.accent_tint)).map(|(r, _, _)| r).collect();
            let ring =
                rect_shapes(&h).into_iter().find(|(r, _, stroke)| *stroke == egui::Color32::from(p.accent) && tinted.contains(r)).map(|(r, _, _)| r);
            let ring = ring.unwrap_or_else(|| panic!("{theme:?}: {kind:?}'s draft ringed"));
            let boxes = text_boxes(&h);
            let card = rect_of(&h, &format!("workspace.canvas.node.{}", node_of(&h, class)));
            let above = boxes.iter().find(|(t, r)| t == over && card.contains(r.center())).map(|(_, r)| *r).expect("the row over the draft");
            let typed = boxes.iter().find(|(t, r)| t == "Returned" && ring.contains(r.center())).map(|(_, r)| *r).expect("the typed name");
            assert!(ring.top() >= above.bottom(), "{theme:?}: {kind:?}'s ring at {} clear of {over} ending at {}", ring.top(), above.bottom());
            assert!((ring.height() - archforge_shell_api::canvas::CARD_ROW).abs() < 0.5, "{theme:?}: a row's pitch, not {}", ring.height());
            // The typed text sits on the line a row's text takes: a pitch under the row over it.
            let pitch = typed.center().y - above.center().y;
            assert!((pitch - archforge_shell_api::canvas::CARD_ROW).abs() < 1.0, "{theme:?}: {kind:?}'s draft a pitch under {over}, not {pitch}");
            // Its ⋯ on the surface at the ring's right.
            assert!(
                rect_shapes(&h)
                    .iter()
                    .any(|(r, fill, _)| *fill == egui::Color32::from(p.surface) && ring.contains_rect(*r) && r.right() > ring.right() - 24.0),
                "{theme:?}: {kind:?}'s ⋯ on the surface"
            );
            rings.push((ring.top() - above.bottom(), ring.height()));
            key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
            assert!(uml_of(h.state()).inline.is_none());
        }
        assert_eq!(rings[0], rings[1], "{theme:?}: a literal's draft is spaced as an attribute's");
        assert_eq!(rings[0], rings[2], "{theme:?}: a literal's draft is spaced as an operation's");
        assert_eq!(uml_inline::details_ink(p), p.text_secondary, "one ⋯ ink for every member's row");
    }
}

/// A new class is placed at its default size, grows on every diagram as operations are added (one
/// undo step taking back the growth), keeps a size set by hand and then says the rows it hides in
/// its `+ n more` row, at any zoom; in light and dark.
#[test]
fn a_new_class_shows_the_operations_added_to_it_and_a_card_made_short_says_what_it_hides() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let mut h = workspace_harness(theme, egui::vec2(1600.0, 1000.0));
        let run = |h: &mut Harness<'static, Shell>, command: &str, params: serde_json::Value| {
            let out = h.state_mut().session.as_mut().unwrap().execute(command, &params).unwrap_or_else(|e| panic!("{command}: {e}"));
            out.value["id"].as_str().map(|i| archforge_mof::Uuid::parse(i).unwrap())
        };
        let pkg = run(&mut h, "uml.package.add", serde_json::json!({ "name": "Billing" })).unwrap();
        let d = run(
            &mut h,
            "diagram.add",
            serde_json::json!({ "kind": archforge_uml::STRUCTURE_DIAGRAM, "element": pkg.to_string(), "name": "Billing" }),
        )
        .unwrap();
        archforge_ui_egui::canvas::open(h.state_mut(), d);
        let class = run(&mut h, "uml.class.add", serde_json::json!({ "package": pkg.to_string(), "name": "ReviewInvoice" })).unwrap();
        let node =
            run(&mut h, "diagram.place", serde_json::json!({ "diagram": d.to_string(), "element": class.to_string(), "x": 200, "y": 200 })).unwrap();
        let height = |h: &Harness<'static, Shell>| {
            let st = h.state().session.as_ref().unwrap().doc();
            archforge_diagram::grid::bounds_of(st, st.by_uuid(node).unwrap()).height
        };
        assert_eq!(height(&h), archforge_uml::CLASS_SIZE.1, "a new class is as tall as a row in each compartment");
        // The same class on two more diagrams: wider and short, and already tall.
        let mut others = Vec::new();
        for (name, width, tall) in [("Wide", 300, 150), ("Tall", 260, 400)] {
            let other =
                run(&mut h, "diagram.add", serde_json::json!({ "kind": archforge_uml::STRUCTURE_DIAGRAM, "element": pkg.to_string(), "name": name }))
                    .unwrap();
            let n = run(
                &mut h,
                "diagram.place",
                serde_json::json!({ "diagram": other.to_string(), "element": class.to_string(), "x": 0, "y": 0, "width": width, "height": tall }),
            )
            .unwrap();
            others.push(n);
        }
        let size = |h: &Harness<'static, Shell>, n: archforge_mof::Uuid| {
            let st = h.state().session.as_ref().unwrap().doc();
            let b = archforge_diagram::grid::bounds_of(st, st.by_uuid(n).unwrap());
            (b.width, b.height)
        };
        h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [class.to_string()] })).unwrap();
        h.state_mut().state.show_panel(archforge_shell_api::state::Panel::Properties);
        h.run();
        // The first operation, added in Properties, shows on the card at its default size.
        let add_operation = |h: &mut Harness<'static, Shell>, name: &str| {
            let at = rect_of(h, "workspace.properties.operations.add").center();
            click(h, at);
            h.event(egui::Event::Text(name.into()));
            h.run();
            let at = rect_of(h, "workspace.properties.member.apply").center();
            click(h, at);
            assert!(uml_of(h.state()).member.is_none(), "{:?}", uml_of(h.state()).member);
            h.run();
        };
        add_operation(&mut h, "calculateTotal");
        assert_eq!(height(&h), archforge_uml::CLASS_SIZE.1, "nothing grows while every row fits");
        // Each member is painted twice: on the card and as its row in Properties.
        let on_card = |h: &Harness<'static, Shell>, row: &str| painted(h).iter().filter(|t| *t == row).count() == 2;
        assert!(on_card(&h, "+ calculateTotal()"), "{theme:?}: {:?}", painted(&h));
        assert!(!painted(&h).iter().any(|t| t.contains("more member")));
        // A second one grows the card to show both, in the same undo step; its width stays.
        add_operation(&mut h, "approve");
        let grown = height(&h);
        assert!(grown > archforge_uml::CLASS_SIZE.1, "{grown}");
        assert!(on_card(&h, "+ calculateTotal()") && on_card(&h, "+ approve()"), "{:?}", painted(&h));
        // Every card of the class grows on every diagram, keeping its width; a taller one is left.
        assert_eq!((size(&h, others[0]), size(&h, others[1])), ((300.0, grown), (260.0, 400.0)));
        h.state_mut().session.as_mut().unwrap().undo().unwrap();
        assert_eq!((member_rows(&h, class, true).len(), height(&h)), (1, archforge_uml::CLASS_SIZE.1), "one undo takes back both");
        assert_eq!((size(&h, others[0]), size(&h, others[1])), ((300.0, 150.0), (260.0, 400.0)), "and every card's height");
        h.state_mut().session.as_mut().unwrap().redo().unwrap();
        assert_eq!((member_rows(&h, class, true).len(), height(&h)), (2, grown));
        assert_eq!(size(&h, others[0]), (300.0, grown));
        // Made short by hand it stays so, and says what it hides rather than looking empty.
        run(&mut h, "diagram.resize", serde_json::json!({ "node": node.to_string(), "width": 240, "height": 120 }));
        h.run();
        let texts = painted(&h);
        assert!(texts.iter().any(|t| t == "+ 2 more members"), "{theme:?}: {texts:?}");
        assert!(!on_card(&h, "+ calculateTotal()") && !on_card(&h, "+ approve()"), "{texts:?}");
        assert_eq!(height(&h), 120.0);
        // Shorter than a row's place under the header, it says so at its foot, then beside its eyebrow.
        for short in [90, 60] {
            run(&mut h, "diagram.resize", serde_json::json!({ "node": node.to_string(), "width": 240, "height": short }));
            h.run();
            assert!(painted(&h).iter().any(|t| t == "+ 2 more members"), "{short} tall: {:?}", painted(&h));
        }
        run(&mut h, "diagram.resize", serde_json::json!({ "node": node.to_string(), "width": 240, "height": 120 }));
        h.run();
        // Seen at a reduced zoom, the card still says it.
        let zoom = h.state().canvas.zoom;
        h.state_mut().canvas.zoom = 0.6;
        h.run();
        assert!(painted(&h).iter().any(|t| t == "+ 2 more members"), "at 60 %");
        h.state_mut().canvas.zoom = zoom;
        // With an attribute it shows, the hint takes an operation's place and counts the rest.
        run(&mut h, "uml.attribute.add", serde_json::json!({ "class": class.to_string(), "name": "number" }));
        run(&mut h, "diagram.resize", serde_json::json!({ "node": node.to_string(), "width": 240, "height": archforge_uml::CLASS_SIZE.1 }));
        h.run();
        let texts = painted(&h);
        assert!(on_card(&h, "+ number"), "{texts:?}");
        assert!(texts.iter().any(|t| t == "+ 2 more members"), "{texts:?}");
        assert!(!on_card(&h, "+ calculateTotal()") && !on_card(&h, "+ approve()"), "{texts:?}");
        // Editing a member grows it again.
        let st = h.state().session.as_ref().unwrap().doc();
        let op = st.uuid(archforge_uml::members::operations(st, st.by_uuid(class).unwrap())[1]).unwrap();
        h.run();
        let at = rect_of(&h, &format!("workspace.properties.operation.{op}.edit")).center();
        click(&mut h, at);
        uml_of_mut(h.state_mut()).member.as_mut().unwrap().name = "approveAll".into();
        let at = rect_of(&h, "workspace.properties.member.apply").center();
        click(&mut h, at);
        h.run();
        assert_eq!(height(&h), grown);
        assert!(on_card(&h, "+ approveAll()"), "{:?}", painted(&h));
        // Saved and opened again from the project's rows, the card keeps its size.
        let (tables, junctions) = archforge_notations::NOTATIONS.storage();
        let rows = archforge_mof::rows::snapshot(h.state().session.as_ref().unwrap().doc(), &tables, &junctions).unwrap();
        let back = archforge_notations::NOTATIONS.session_from_rows(&rows, Box::new(|| archforge_mof::Uuid::v7(1_761_670_900_000, [9; 10]))).unwrap();
        let st = back.doc();
        assert_eq!(archforge_diagram::grid::bounds_of(st, st.by_uuid(node).unwrap()).height, grown);
        assert_eq!(archforge_uml::members::operations(st, st.by_uuid(class).unwrap()).len(), 2);
    }
}

/// Members a tool adds over the control channel as Chat does, in one `engine.batch`, grow every
/// card of their class to show them all, no `+ n more` row left (AR-241): the command grows the
/// card, the window takes no part. A card sized by hand keeps its width, a taller one its height,
/// and one undo over the channel takes the members and the growth back, redo both; light and dark.
#[test]
fn members_a_tool_adds_grow_the_class_card_in_the_same_undo_step() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let mut h = workspace_harness(theme, egui::vec2(1600.0, 1000.0));
        let run = |h: &mut Harness<'static, Shell>, command: &str, params: serde_json::Value| {
            let out = h.state_mut().session.as_mut().unwrap().execute(command, &params).unwrap_or_else(|e| panic!("{command}: {e}"));
            archforge_mof::Uuid::parse(out.value["id"].as_str().unwrap()).unwrap()
        };
        let pkg = run(&mut h, "uml.package.add", serde_json::json!({ "name": "Sales" }));
        let d =
            run(&mut h, "diagram.add", serde_json::json!({ "kind": archforge_uml::STRUCTURE_DIAGRAM, "element": pkg.to_string(), "name": "Sales" }));
        archforge_ui_egui::canvas::open(h.state_mut(), d);
        let class = run(&mut h, "uml.class.add", serde_json::json!({ "package": pkg.to_string(), "name": "Customer" }));
        let node = run(&mut h, "diagram.place", serde_json::json!({ "diagram": d.to_string(), "element": class.to_string(), "x": 200, "y": 200 }));
        // The same class on two more diagrams: made wider and short by hand, and already tall.
        let mut others = Vec::new();
        for (name, width, tall) in [("Wide", 360, 150), ("Tall", 260, 600)] {
            let other =
                run(&mut h, "diagram.add", serde_json::json!({ "kind": archforge_uml::STRUCTURE_DIAGRAM, "element": pkg.to_string(), "name": name }));
            let place =
                serde_json::json!({ "diagram": other.to_string(), "element": class.to_string(), "x": 0, "y": 0, "width": width, "height": tall });
            others.push(run(&mut h, "diagram.place", place));
        }
        let before = [node_size(&h, node), node_size(&h, others[0]), node_size(&h, others[1])];
        assert_eq!(before, [(240.0, archforge_uml::CLASS_SIZE.1), (360.0, 150.0), (260.0, 600.0)]);
        let c = class.to_string();
        let steps = serde_json::json!([
            { "command": "uml.attribute.add", "params": { "class": c, "name": "firstName", "type": "String" } },
            { "command": "uml.attribute.add", "params": { "class": c, "name": "lastName", "type": "String" } },
            { "command": "uml.operation.add", "params": { "class": c, "name": "setFirstName" } },
            { "command": "uml.operation.add", "params": { "class": c, "name": "setLastName" } },
            { "command": "uml.operation.add", "params": { "class": c, "name": "setEmail" } },
        ]);
        let answer =
            archforge_ui_egui::control::answer(h.state_mut(), "engine.batch", &serde_json::json!({ "label": "Agent changes", "steps": steps }))
                .unwrap();
        assert_eq!(answer["undo"], "Agent changes", "one undo step");
        h.run();
        let texts = painted(&h);
        assert!(!texts.iter().any(|t| t.contains("more member")), "{theme:?}: {texts:?}");
        // Each row is painted twice, on the card and in Properties, so a row Properties alone shows fails.
        for row in ["+ firstName : String", "+ lastName : String", "+ setFirstName()", "+ setLastName()", "+ setEmail()"] {
            assert_eq!(texts.iter().filter(|t| *t == row).count(), 2, "{theme:?}: {row} on the card: {texts:?}");
        }
        // Two attribute rows, the line, three operation rows.
        let grown = uml_ui::members_height(2, 3);
        assert_eq!([node_size(&h, node), node_size(&h, others[0]), node_size(&h, others[1])], [(240.0, grown), (360.0, grown), (260.0, 600.0)]);
        archforge_ui_egui::control::answer(h.state_mut(), "engine.undo", &serde_json::json!({})).unwrap();
        assert_eq!(member_rows(&h, class, false).len() + member_rows(&h, class, true).len(), 0, "one undo takes every member back");
        assert_eq!([node_size(&h, node), node_size(&h, others[0]), node_size(&h, others[1])], before, "{theme:?}: and every card's height");
        archforge_ui_egui::control::answer(h.state_mut(), "engine.redo", &serde_json::json!({})).unwrap();
        assert_eq!([node_size(&h, node), node_size(&h, others[0]), node_size(&h, others[1])], [(240.0, grown), (360.0, grown), (260.0, 600.0)]);
    }
}

/// Literals a tool adds to an enumeration or renames, and stereotype values it sets and shows, grow
/// the cards that would hide them in the command's own step (AR-241), so no `+ n more` row is left;
/// a value set while the diagram hides values, hiding them again, and an n-ary association's
/// diamond with a value leave their nodes as they are; in light and dark.
#[test]
fn literals_and_values_a_tool_writes_grow_the_cards_that_show_them() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, nodes) = related_sample_in(theme, egui::vec2(1600.0, 1100.0));
        let (_, _, entity) = archforge_ui_egui::snapshot::cx(h.state_mut(), |cx| uml_profiles::put_profiles(cx, true)).unwrap();
        let execute = |h: &mut Harness<'static, Shell>, command: &str, params: serde_json::Value| {
            let out =
                archforge_ui_egui::control::answer(h.state_mut(), "engine.execute", &serde_json::json!({ "command": command, "params": params }));
            out.unwrap_or_else(|e| panic!("{command}: {e}"))["result"].clone()
        };
        let d = h.state().canvas.active.unwrap();
        let (sample, order) = {
            let st = h.state().session.as_ref().unwrap().doc();
            let o = st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_uml::CLASS) && archforge_core::name_of(st, *o) == "Order").unwrap();
            (st.uuid(st.container(o).unwrap().unwrap().0).unwrap(), st.uuid(o).unwrap())
        };
        // An enumeration made short and wide by hand grows a row a literal, its width kept.
        let status = execute(&mut h, "uml.enumeration.add", serde_json::json!({ "package": sample.to_string(), "name": "Stage" }));
        let status = archforge_mof::Uuid::parse(status["id"].as_str().unwrap()).unwrap();
        let place = serde_json::json!({ "diagram": d.to_string(), "element": status.to_string(), "x": 1200, "y": 700, "width": 300, "height": 100 });
        let card = archforge_mof::Uuid::parse(execute(&mut h, "diagram.place", place)["id"].as_str().unwrap()).unwrap();
        for name in ["Draft", "Placed", "Paid", "Shipped"] {
            execute(&mut h, "uml.literal.add", serde_json::json!({ "enumeration": status.to_string(), "name": name }));
        }
        assert_eq!(node_size(&h, card), (300.0, uml_ui::literals_height(4)), "{theme:?}");
        h.run();
        let texts = painted(&h);
        assert!(!texts.iter().any(|t| t.contains("more literal")) && texts.iter().any(|t| t == "Shipped"), "{theme:?}: {texts:?}");
        archforge_ui_egui::control::answer(h.state_mut(), "engine.undo", &serde_json::json!({})).unwrap();
        assert_eq!(node_size(&h, card), (300.0, uml_ui::literals_height(3)), "undo takes the last literal's growth back");
        // Made short by hand again, a literal renamed grows it back, its width kept.
        execute(&mut h, "diagram.resize", serde_json::json!({ "node": card.to_string(), "width": 300, "height": 100 }));
        let paid = {
            let st = h.state().session.as_ref().unwrap().doc();
            let e = st.by_uuid(status).unwrap();
            st.uuid(archforge_uml::commands::literals(st, e)[2]).unwrap()
        };
        execute(&mut h, "uml.literal.rename", serde_json::json!({ "literal": paid.to_string(), "name": "Settled" }));
        assert_eq!(node_size(&h, card), (300.0, uml_ui::literals_height(3)), "{theme:?}: an edit grows it too");
        // An n-ary association of three sample classes, its diamond at its own size, with «Reference».
        let (reference, classes, counts) = {
            let st = h.state().session.as_ref().unwrap().doc();
            let named = |kind, n: &str| st.alive().find(|o| st.class_of(*o).ok() == Some(kind) && archforge_core::name_of(st, *o) == n).unwrap();
            let o = st.by_uuid(order).unwrap();
            let counts = (archforge_uml::members::attributes(st, o).len(), archforge_uml::members::operations(st, o).len());
            let classes = ["Customer", "Product", "Order"].map(|n| st.uuid(named(archforge_uml::CLASS, n)).unwrap().to_string());
            (st.uuid(named(archforge_uml::STEREOTYPE, "Reference")).unwrap().to_string(), classes, counts)
        };
        let nary = execute(&mut h, "uml.association.add-nary", serde_json::json!({ "classes": classes }))["id"].as_str().unwrap().to_string();
        let place = serde_json::json!({ "diagram": d.to_string(), "element": nary, "x": 1300, "y": 500, "width": 44, "height": 28 });
        let diamond = archforge_mof::Uuid::parse(execute(&mut h, "diagram.place", place)["id"].as_str().unwrap()).unwrap();
        execute(&mut h, "uml.stereotype.apply", serde_json::json!({ "element": nary, "stereotype": reference }));
        execute(
            &mut h,
            "uml.stereotype.set-value",
            serde_json::json!({ "element": nary, "stereotype": reference, "property": "lazy", "values": ["false"] }),
        );
        // A value set while the diagram hides values leaves Order's card; shown, the card grows.
        let order_node = archforge_mof::Uuid::parse(&nodes["Order"]).unwrap();
        let low = node_size(&h, order_node);
        let value = serde_json::json!({ "element": order.to_string(), "stereotype": entity, "property": "table", "values": ["orders"] });
        execute(&mut h, "uml.stereotype.set-value", value);
        assert_eq!(node_size(&h, order_node), low, "{theme:?}: no value shows yet");
        execute(&mut h, "uml.stereotype.show-values", serde_json::json!({ "diagram": d.to_string(), "show": true }));
        // Its members, then the line, «Entity» and table = "orders".
        let shown = uml_ui::members_height(counts.0, counts.1) + 3.0 * f64::from(archforge_shell_api::canvas::CARD_ROW);
        assert_eq!(node_size(&h, order_node), (low.0, shown), "{theme:?}");
        assert_eq!(node_size(&h, diamond), (44.0, 28.0), "{theme:?}: a diamond keeps its size");
        h.run();
        let texts = painted(&h);
        assert!(texts.iter().any(|t| t == "table = \"orders\"") && !texts.iter().any(|t| t.contains("more member")), "{theme:?}: {texts:?}");
        let value = serde_json::json!({ "element": order.to_string(), "stereotype": entity, "property": "cached", "values": ["true"] });
        execute(&mut h, "uml.stereotype.set-value", value);
        h.run();
        let texts = painted(&h);
        assert!(texts.iter().any(|t| t == "table = \"orders\"") && texts.iter().any(|t| t == "cached = true"), "{theme:?}: {texts:?}");
        assert!(!texts.iter().any(|t| t.contains("more member")), "{theme:?}: {texts:?}");
        // Hidden again, no card is made shorter.
        let tall = node_size(&h, order_node);
        execute(&mut h, "uml.stereotype.show-values", serde_json::json!({ "diagram": d.to_string(), "show": false }));
        assert_eq!((node_size(&h, order_node), node_size(&h, diamond)), (tall, (44.0, 28.0)), "{theme:?}");
    }
}

/// A class selected on the sample diagram takes attributes and operations typed on its card, each
/// one undo step.
#[test]
fn a_selected_class_takes_attributes_and_operations_typed_on_its_card_each_one_undo_step() {
    let mut h = workspace_harness(ThemeKind::Light, egui::vec2(1600.0, 1000.0));
    archforge_ui_egui::snapshot::cx(h.state_mut(), uml_ui::open_sample).unwrap();
    let st = h.state().session.as_ref().unwrap().doc();
    let c = st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_uml::CLASS) && archforge_core::name_of(st, *o) == "Catalog").unwrap();
    let catalog = st.uuid(c).unwrap();
    let select = |h: &mut Harness<'static, Shell>| {
        h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [catalog.to_string()] })).unwrap();
        h.run();
    };
    select(&mut h);
    let revision = |h: &Harness<'static, Shell>| h.state().session.as_ref().unwrap().revision();
    // The attributes' + opens a draft row with the name taking the keys; Tab moves to the type.
    let at = rect_of(&h, "workspace.canvas.class.add-attribute").center();
    click(&mut h, at);
    assert!(uml_of(h.state()).inline.as_ref().is_some_and(|i| i.edit.kind == uml_panel::MemberKind::Attribute && i.edit.target.is_none()));
    h.event(egui::Event::Text("code".into()));
    h.run();
    key(&mut h, egui::Key::Tab, egui::Modifiers::NONE);
    h.event(egui::Event::Text("Integer".into()));
    h.run();
    key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
    assert_eq!(member_rows(&h, catalog, false), ["+ name : String", "+ code : Integer"]);
    assert_eq!(h.state().session.as_ref().unwrap().undo_label(), Some("Add attribute"), "one undo step");
    let written = revision(&h);
    assert!(painted(&h).iter().any(|t| t == "+ code : Integer"), "the card shows it");
    // The next draft is open; a type the model does not have is refused beside it, nothing written.
    assert!(uml_of(h.state()).inline.as_ref().is_some_and(|i| i.edit.name.is_empty() && i.edit.target.is_none()));
    h.event(egui::Event::Text("rating".into()));
    h.run();
    key(&mut h, egui::Key::Tab, egui::Modifiers::NONE);
    h.event(egui::Event::Text("Currency".into()));
    h.run();
    key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
    let refused = uml_of(h.state()).inline.as_ref().and_then(|i| i.refused.clone()).expect("refused");
    assert!(refused.contains("Currency"), "{refused}");
    assert!(painted(&h).contains(&refused), "said beside the row");
    assert_eq!(uml_of(h.state()).inline.as_ref().unwrap().edit.name, "rating", "the text stays");
    assert_eq!((member_rows(&h, catalog, false).len(), revision(&h)), (2, written), "nothing written");
    // Escape drops only the draft; undo takes back the member written.
    key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
    assert!(uml_of(h.state()).inline.is_none());
    assert_eq!(member_rows(&h, catalog, false).len(), 2);
    h.state_mut().session.as_mut().unwrap().undo().unwrap();
    assert_eq!(member_rows(&h, catalog, false), ["+ name : String"]);
    h.state_mut().session.as_mut().unwrap().redo().unwrap();
    h.run();
    // The operations' + and the type menu: the pick fills the return type.
    select(&mut h);
    let at = rect_of(&h, "workspace.canvas.class.add-operation").center();
    click(&mut h, at);
    assert_eq!(uml_of(h.state()).inline.as_ref().map(|i| i.edit.kind), Some(uml_panel::MemberKind::Operation));
    h.event(egui::Event::Text("reprice".into()));
    h.run();
    let at = rect_of(&h, "workspace.canvas.member.type.menu").center();
    click(&mut h, at);
    let real = uml_panel::type_choices(h.state().session.as_ref().unwrap().doc(), None).iter().position(|(l, _)| l == "Real").unwrap();
    let at = rect_of(&h, &format!("workspace.canvas.member.type.{real}")).center();
    click(&mut h, at);
    assert_eq!(uml_of(h.state()).inline.as_ref().unwrap().ty, "Real");
    key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
    assert_eq!(member_rows(&h, catalog, true), ["+ reprice() : Real"]);
    key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
    // A draft with a name is written when the pointer goes elsewhere; an empty one only goes.
    select(&mut h);
    let at = rect_of(&h, "workspace.canvas.class.add-attribute").center();
    click(&mut h, at);
    h.event(egui::Event::Text("notes".into()));
    h.run();
    let canvas = rect_of(&h, "workspace.canvas");
    click(&mut h, canvas.left_top() + egui::vec2(30.0, 64.0));
    assert!(uml_of(h.state()).inline.is_none());
    assert_eq!(member_rows(&h, catalog, false).last().map(String::as_str), Some("+ notes"));
    // So is one whose type menu was open when the pointer went elsewhere.
    select(&mut h);
    let at = rect_of(&h, "workspace.canvas.class.add-attribute").center();
    click(&mut h, at);
    h.event(egui::Event::Text("tags".into()));
    h.run();
    let at = rect_of(&h, "workspace.canvas.member.type.menu").center();
    click(&mut h, at);
    assert_eq!(h.state().panels.menu, Some(uml_state::menus::INLINE_TYPE));
    click(&mut h, canvas.left_top() + egui::vec2(30.0, 64.0));
    assert!(uml_of(h.state()).inline.is_none(), "{:?}", uml_of(h.state()).inline);
    assert_eq!(member_rows(&h, catalog, false).last().map(String::as_str), Some("+ tags"));
    // ⋯ takes the draft to Properties' editor with what was typed.
    select(&mut h);
    let at = rect_of(&h, "workspace.canvas.class.add-attribute").center();
    click(&mut h, at);
    h.event(egui::Event::Text("reference".into()));
    h.state_mut().state.show_panel(archforge_shell_api::state::Panel::Skills);
    h.run();
    let at = rect_of(&h, "workspace.canvas.member.details").center();
    click(&mut h, at);
    assert!(uml_of(h.state()).inline.is_none());
    let edit = uml_of(h.state()).member.clone().expect("Properties' editor");
    assert_eq!((edit.name.as_str(), edit.target, edit.class), ("reference", None, catalog));
    assert_eq!(h.state().state.panel, archforge_shell_api::state::Panel::Properties, "Properties shows the editor");
    uml_of_mut(h.state_mut()).member = None;
    // ⇧⌘↩ opens an operation's row from the keys.
    select(&mut h);
    h.state_mut().canvas.focused = true;
    key(&mut h, egui::Key::Enter, egui::Modifiers { command: true, shift: true, ..Default::default() });
    assert_eq!(uml_of(h.state()).inline.as_ref().map(|i| i.edit.kind), Some(uml_panel::MemberKind::Operation));
    // An agent removing the class drops the row.
    h.state_mut().session.as_mut().unwrap().execute("element.remove", &serde_json::json!({ "ids": [catalog.to_string()] })).unwrap();
    h.run();
    assert!(uml_of(h.state()).inline.is_none());
    h.state_mut().session.as_mut().unwrap().undo().unwrap();
    h.run();
    // Drawn too small for its rows, the card leaves the member to Properties' editor, which the
    // right side then shows: ⌘↩'s attribute, and ⇧⌘↩'s operation.
    h.state_mut().canvas.zoom = 0.4;
    for (shift, kind) in [(false, uml_panel::MemberKind::Attribute), (true, uml_panel::MemberKind::Operation)] {
        uml_of_mut(h.state_mut()).member = None;
        h.state_mut().state.show_panel(archforge_shell_api::state::Panel::Skills);
        select(&mut h);
        h.state_mut().canvas.focused = true;
        key(&mut h, egui::Key::Enter, egui::Modifiers { command: true, shift, ..Default::default() });
        assert!(uml_of(h.state()).inline.is_none());
        assert!(uml_of(h.state()).member.as_ref().is_some_and(|e| e.class == catalog && e.target.is_none() && e.kind == kind), "{kind:?}");
        assert_eq!(h.state().state.panel, archforge_shell_api::state::Panel::Properties, "{kind:?}: Properties shows the editor");
    }
}

/// A row typed on a card keeps its type by what it is, refuses a name two types share, and keeps
/// rows it moves in sight.
#[test]
fn an_inline_row_keeps_a_type_by_what_it_is_refuses_a_name_two_types_share_and_keeps_moved_rows_in_sight() {
    let mut h = workspace_harness(ThemeKind::Light, egui::vec2(1600.0, 1000.0));
    archforge_ui_egui::snapshot::cx(h.state_mut(), uml_ui::open_sample).unwrap();
    let run = |h: &mut Harness<'static, Shell>, command: &str, params: serde_json::Value| {
        let out = h.state_mut().session.as_mut().unwrap().execute(command, &params).unwrap_or_else(|e| panic!("{command}: {e}"));
        out.value["id"].as_str().map(|i| archforge_mof::Uuid::parse(i).unwrap())
    };
    let class = |h: &Harness<'static, Shell>, name: &str| {
        let st = h.state().session.as_ref().unwrap().doc();
        st.uuid(st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_uml::CLASS) && archforge_core::name_of(st, *o) == name).unwrap()).unwrap()
    };
    let catalog = class(&h, "Catalog");
    // A second Money, in another package; Catalog's fee is of that one.
    let billing = run(&mut h, "uml.package.add", serde_json::json!({ "name": "Billing" })).unwrap();
    let money = run(&mut h, "uml.class.add", serde_json::json!({ "package": billing.to_string(), "name": "Money" })).unwrap();
    let fee =
        run(&mut h, "uml.attribute.add", serde_json::json!({ "class": catalog.to_string(), "name": "fee", "type": money.to_string() })).unwrap();
    let type_of = |h: &Harness<'static, Shell>| {
        let st = h.state().session.as_ref().unwrap().doc();
        let t = st.get(st.by_uuid(fee).unwrap(), archforge_uml::typed_element::TYPE).unwrap().as_ref().unwrap();
        st.uuid(t).unwrap()
    };
    // Renamed in place, it keeps that Money.
    assert!(archforge_ui_egui::snapshot::cx(h.state_mut(), |cx| uml_inline::open(cx, catalog, uml_panel::MemberKind::Attribute, Some(fee))));
    h.run();
    uml_of_mut(h.state_mut()).inline.as_mut().unwrap().edit.name = "charge".into();
    key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
    assert!(uml_of(h.state()).inline.is_none(), "{:?}", uml_of(h.state()).inline);
    assert_eq!(type_of(&h), money, "the type stays the one it was");
    // Typing a name two types share is refused, the type unchanged.
    assert!(archforge_ui_egui::snapshot::cx(h.state_mut(), |cx| uml_inline::open(cx, catalog, uml_panel::MemberKind::Attribute, Some(fee))));
    h.run();
    uml_of_mut(h.state_mut()).inline.as_mut().unwrap().ty = "money".into();
    key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
    let refused = uml_of(h.state()).inline.as_ref().and_then(|i| i.refused.clone()).expect("refused");
    assert!(refused.contains("money"), "{refused}");
    assert_eq!(type_of(&h), money);
    // A type no type has stays with the row when ⋯ is pressed, rather than going over as another.
    uml_of_mut(h.state_mut()).inline.as_mut().unwrap().ty = "Currency".into();
    let at = rect_of(&h, "workspace.canvas.member.details").center();
    click(&mut h, at);
    assert!(uml_of(h.state()).member.is_none(), "Properties' editor stays closed");
    assert!(uml_of(h.state()).inline.as_ref().and_then(|i| i.refused.clone()).is_some_and(|r| r.contains("Currency")));
    key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
    // A draft at the end of Order's attributes moves its operation down, still drawn in the card.
    let order = class(&h, "Order");
    assert!(archforge_ui_egui::snapshot::cx(h.state_mut(), |cx| uml_inline::open(cx, order, uml_panel::MemberKind::Attribute, None)));
    h.run();
    let canvas = rect_of(&h, "workspace.canvas");
    /// Collects the texts of `s` and of the shapes it holds into `out`, each with its place and
    /// `clip`.
    fn texts(s: &egui::Shape, clip: egui::Rect, out: &mut Vec<(String, egui::Pos2, egui::Rect)>) {
        match s {
            egui::Shape::Text(t) => out.push((t.galley.text().to_string(), t.pos, clip)),
            egui::Shape::Vec(v) => v.iter().for_each(|s| texts(s, clip, out)),
            _ => {}
        }
    }
    let mut found = Vec::new();
    h.output().shapes.iter().for_each(|c| texts(&c.shape, c.clip_rect, &mut found));
    let (_, pos, clip) = found.iter().find(|(t, pos, _)| t == "+ total() : Money" && canvas.contains(*pos)).expect("the operation drawn on the card");
    assert!(clip.contains(*pos + egui::vec2(1.0, 8.0)), "{pos:?} inside {clip:?}");
}

/// A class dragged from Models onto a second diagram stays one element, and leaves that diagram by
/// Delete or the model by Shift+Delete.
#[test]
fn a_class_dragged_from_models_onto_a_second_diagram_stays_one_element_and_leaves_by_delete_or_shift_delete() {
    let mut h = workspace_harness(ThemeKind::Light, egui::vec2(1600.0, 1000.0));
    archforge_ui_egui::snapshot::cx(h.state_mut(), uml_ui::open_sample).unwrap();
    let find = |h: &Harness<'static, Shell>, name: &str| {
        let st = h.state().session.as_ref().unwrap().doc();
        let c = st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_uml::CLASS) && archforge_core::name_of(st, *o) == name).unwrap();
        st.uuid(c).unwrap()
    };
    let (customer, order) = (find(&h, "Customer"), find(&h, "Order"));
    let (pkg, d2) = {
        let s = h.state_mut().session.as_mut().unwrap();
        let st = s.doc();
        let pkg = st.uuid(archforge_uml::commands::packages(st)[0]).unwrap();
        let out = s
            .execute("diagram.add", &serde_json::json!({ "kind": archforge_uml::STRUCTURE_DIAGRAM, "element": pkg.to_string(), "name": "Ordering" }))
            .unwrap();
        (pkg, archforge_mof::Uuid::parse(out.value["id"].as_str().unwrap()).unwrap())
    };
    archforge_ui_egui::canvas::open(h.state_mut(), d2);
    h.state_mut().explorer.expanded.insert(pkg);
    h.run();
    let shown = |h: &Harness<'static, Shell>| {
        let st = h.state().session.as_ref().unwrap().doc();
        let d = st.by_uuid(d2).unwrap();
        (st.list(d, archforge_diagram::diagram::NODE).unwrap().len(), st.list(d, archforge_diagram::diagram::EDGE).unwrap().len())
    };
    assert_eq!(shown(&h), (0, 0));
    // Customer, then Order: the association between them draws with the second, one undo step.
    let canvas = rect_of(&h, "workspace.canvas");
    let row = rect_of(&h, &format!("workspace.explorer.row.{customer}")).center();
    drag(&mut h, row, canvas.center() - egui::vec2(200.0, 0.0));
    let row = rect_of(&h, &format!("workspace.explorer.row.{order}")).center();
    drag(&mut h, row, canvas.center() + egui::vec2(200.0, 0.0));
    assert_eq!(shown(&h), (2, 1), "notice {:?}", h.state().canvas.notice);
    h.state_mut().session.as_mut().unwrap().undo().unwrap();
    assert_eq!(shown(&h), (1, 0));
    h.state_mut().session.as_mut().unwrap().redo().unwrap();
    h.run();
    assert_eq!(shown(&h), (2, 1));
    // One element, two views; the card is as tall as its members.
    let st = h.state().session.as_ref().unwrap().doc();
    let (_, _, nodes) = archforge_diagram::views_of(st, st.by_uuid(order).unwrap());
    assert_eq!(nodes.len(), 2, "Order on both diagrams");
    let row = rect_of(&h, &format!("workspace.explorer.row.{order}")).center();
    drag(&mut h, row, canvas.center());
    assert_eq!(shown(&h), (2, 1));
    assert_eq!(h.state().canvas.notice.as_deref(), Some("Already on this diagram"));
    // The selected class's adorner opens a new attribute's row on the card; Enter writes it and
    // opens the next, which Escape drops.
    h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [order.to_string()] })).unwrap();
    h.run();
    let at = rect_of(&h, "workspace.canvas.adorn.add-attribute").center();
    click(&mut h, at);
    assert!(uml_of(h.state()).inline.as_ref().is_some_and(|i| i.edit.target.is_none() && i.edit.class == order));
    h.event(egui::Event::Text("status".into()));
    h.run();
    key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
    assert_eq!(member_rows(&h, order, false), ["+ id : Integer", "+ placedAt : Date", "+ status"]);
    assert!(uml_of(h.state()).inline.as_ref().is_some_and(|i| i.edit.target.is_none() && i.edit.name.is_empty()), "the next draft");
    key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
    assert!(uml_of(h.state()).inline.is_none());
    // A double click on the operation's row of the card edits it in place.
    let st = h.state().session.as_ref().unwrap().doc();
    let node = archforge_diagram::views_of(st, st.by_uuid(order).unwrap())
        .2
        .into_iter()
        .find(|n| st.container(*n).ok().flatten().is_some_and(|(d, _)| st.uuid(d).ok() == Some(d2)))
        .unwrap();
    let node_id = st.uuid(node).unwrap().to_string();
    let r = rect_of(&h, &format!("workspace.canvas.node.{node_id}"));
    let z = h.state().canvas.zoom;
    // Two attributes shown when it was placed, the dividing line, then the operation.
    let y = r.top() + (archforge_shell_api::canvas::CARD_FIRST_ROW + 3.5 * archforge_shell_api::canvas::CARD_ROW + 6.0) * z;
    let at = egui::pos2(r.center().x, y);
    h.event(egui::Event::PointerMoved(at));
    for _ in 0..2 {
        h.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE });
        h.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE });
    }
    h.run();
    h.run();
    let st = h.state().session.as_ref().unwrap().doc();
    let total = st.uuid(archforge_uml::members::operations(st, st.by_uuid(order).unwrap())[0]).unwrap();
    assert_eq!(uml_of(h.state()).inline.as_ref().and_then(|i| i.edit.target), Some(total), "{:?}", uml_of(h.state()).inline);
    key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
    assert!(uml_of(h.state()).inline.is_none());
    // Delete takes it off this diagram only; Shift+Delete asks, then takes it from the model.
    let at = rect_of(&h, &format!("workspace.canvas.node.{node_id}")).center();
    click(&mut h, at);
    key(&mut h, egui::Key::Delete, egui::Modifiers::NONE);
    assert_eq!(shown(&h).0, 1);
    let st = h.state().session.as_ref().unwrap().doc();
    assert!(st.by_uuid(order).is_some_and(|o| st.is_alive(o)), "the class stays in the model");
    let node = node_ids(&h)
        .into_iter()
        .find(|i| {
            i.ends_with(&{
                let st = h.state().session.as_ref().unwrap().doc();
                let c = st.by_uuid(customer).unwrap();
                let (_, _, nodes) = archforge_diagram::views_of(st, c);
                nodes
                    .into_iter()
                    .filter_map(|n| st.uuid(n).ok())
                    .find(|n| {
                        let o = st.by_uuid(*n).unwrap();
                        st.container(o).ok().flatten().is_some_and(|(d, _)| st.uuid(d).ok() == Some(d2))
                    })
                    .unwrap()
                    .to_string()
            })
        })
        .unwrap();
    let at = rect_of(&h, &node).center();
    click(&mut h, at);
    key(&mut h, egui::Key::Delete, egui::Modifiers::SHIFT);
    let ask = h.state().explorer.delete.clone().expect("the dialog asks");
    assert_eq!((ask.name.as_str(), ask.diagrams), ("Customer", 2));
    assert_eq!(ask.consequences, ["1 relationship goes with it."], "its association goes with it");
    let at = rect_of(&h, "workspace.explorer.delete.confirm").center();
    click(&mut h, at);
    let st = h.state().session.as_ref().unwrap().doc();
    assert!(st.by_uuid(customer).is_none_or(|c| !st.is_alive(c)), "Customer left the model");
    assert_eq!(shown(&h).0, 0);
    h.state_mut().session.as_mut().unwrap().undo().unwrap();
    let st = h.state().session.as_ref().unwrap().doc();
    assert!(st.by_uuid(customer).is_some_and(|c| st.is_alive(c)), "undo brings it back");
}

/// The Relationship tool's menu ticks its current kind at the row's right, as the classifier
/// tool's menu ticks its own (AR-191): one tick, in the current row, at the same place; in light
/// and dark.
#[test]
fn the_relationship_menu_ticks_its_current_kind_as_the_classifier_menu_does() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, _) = related_sample_in(theme, egui::vec2(1600.0, 1000.0));
        let accent = archforge_ui_egui::ux::palette(theme == ThemeKind::Dark).accent;
        // Where each menu ticks its current row: the tick's centre from the row's right.
        let tick_in = |h: &Harness<'static, Shell>, row: egui::Rect| -> Vec<f32> {
            icon_boxes(h, accent)
                .into_iter()
                .filter(|b| row.contains(b.center()) && b.center().x > row.right() - 28.0)
                .map(|b| row.right() - b.center().x)
                .collect()
        };
        let at = rect_of(&h, "workspace.tools.classifier-kinds").center();
        click(&mut h, at);
        let classifier = tick_in(&h, rect_of(&h, "workspace.classifier.class"));
        assert_eq!(classifier.len(), 1, "{theme:?}: the classifier menu ticks Class");
        key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
        for kind in ["composition", "generalization"] {
            let at = rect_of(&h, "workspace.tools.relationship-kinds").center();
            click(&mut h, at);
            let at = rect_of(&h, &format!("workspace.relate.{kind}")).center();
            click(&mut h, at);
            let at = rect_of(&h, "workspace.tools.relationship-kinds").center();
            click(&mut h, at);
            assert_eq!(h.state().canvas.popover, Some("relationship-kinds"));
            let rows = ["association", "directed", "aggregation", "composition", "generalization", "dependency", "realization", "association-class"];
            for row in rows {
                let ticks = tick_in(&h, rect_of(&h, &format!("workspace.relate.{row}")));
                if row == kind {
                    assert_eq!(ticks.len(), 1, "{theme:?}: {row}, the current kind, is ticked");
                    assert!((ticks[0] - classifier[0]).abs() < 0.5, "{theme:?}: ticked where the classifier menu ticks: {ticks:?} {classifier:?}");
                } else {
                    assert!(ticks.is_empty(), "{theme:?}: {row} is not ticked while {kind} is current");
                }
            }
            key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
        }
    }
}

/// A selected line's end handles sit on the line just past its ends' marks, so a generalization's
/// triangle, a realization's, a navigable end's arrowhead and a composition's diamond show whole;
/// an end without a mark keeps its handle, and its small square, on the card's border; past a mark
/// no square is drawn on the tip (AR-191).
#[test]
fn a_selected_lines_end_handles_sit_past_its_end_marks() {
    use archforge_diagram::figure::EndMark;
    let (mut h, _) = related_sample_in(ThemeKind::Light, egui::vec2(1600.0, 1000.0));
    // Order realizes an interface Payable placed beside the sample, so a realization is drawn too.
    let d = h.state().canvas.active.unwrap();
    let (package, order) = {
        let st = h.state().session.as_ref().unwrap().doc();
        let order = st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_uml::CLASS) && archforge_core::name_of(st, *o) == "Order").unwrap();
        (st.uuid(archforge_uml::commands::package_of(st, order).unwrap()).unwrap(), st.uuid(order).unwrap())
    };
    let s = h.state_mut().session.as_mut().unwrap();
    let id = |v: serde_json::Value| v["id"].as_str().unwrap().to_string();
    let payable = id(s.execute("uml.interface.add", &serde_json::json!({ "package": package.to_string(), "name": "Payable" })).unwrap().value);
    s.execute("diagram.place", &serde_json::json!({ "diagram": d.to_string(), "element": payable, "x": 560, "y": 700, "width": 290, "height": 146 }))
        .unwrap();
    s.execute("uml.realization.add", &serde_json::json!({ "class": order.to_string(), "interface": payable })).unwrap();
    s.execute("diagram.connect-all", &serde_json::json!({ "diagram": d.to_string() })).unwrap();
    h.run();
    let (lines, nodes) = {
        let st = h.state().session.as_ref().unwrap().doc();
        let dg = st.by_uuid(d).unwrap();
        let lines: Vec<(archforge_mof::Uuid, [EndMark; 2], bool)> = st
            .list(dg, archforge_diagram::diagram::EDGE)
            .unwrap()
            .iter()
            .filter_map(archforge_mof::Value::as_ref)
            .filter_map(|e| {
                let a = st.get(e, archforge_diagram::edge::ELEMENT).ok()?.as_ref()?;
                let line = archforge_uml::figures::line(st, a)?;
                Some((st.uuid(a).ok()?, [line.ends[0].mark, line.ends[1].mark], line.styled))
            })
            .collect();
        let nodes: Vec<String> = st
            .list(dg, archforge_diagram::diagram::NODE)
            .unwrap()
            .iter()
            .filter_map(archforge_mof::Value::as_ref)
            .filter_map(|n| st.uuid(n).ok().map(|u| u.to_string()))
            .collect();
        (lines, nodes)
    };
    let marks: Vec<EndMark> = lines.iter().flat_map(|(_, m, _)| *m).collect();
    for kind in [EndMark::Triangle, EndMark::OpenArrow, EndMark::FilledDiamond, EndMark::None] {
        assert!(marks.contains(&kind), "the sample has an end with {kind:?}");
    }
    // At zooms that show the whole sample, the marks at their full size and at their least.
    let mut checked = std::collections::BTreeSet::new();
    let mut squared = 0;
    for zoom in [0.6, 0.5] {
        h.state_mut().canvas.zoom = zoom;
        h.state_mut().canvas.pan = egui::Vec2::ZERO;
        h.state_mut().canvas.fit_pending = false;
        for (line, ends, styled) in &lines {
            h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [line.to_string()] })).unwrap();
            h.run();
            let cards: Vec<egui::Rect> = nodes.iter().map(|n| rect_of(&h, &format!("workspace.canvas.node.{n}"))).collect();
            // The small square at a selected relationship line's end (a thin generalization or
            // realization draws none) shows only where the round handle stayed on the end: past a
            // mark the handle marks the end, and a square would sit on the mark.
            let accent = archforge_ui_egui::ux::palette(false).accent;
            let squares = rect_shapes(&h)
                .iter()
                .filter(|(r, _, stroke)| *stroke == egui::Color32::from(accent) && (r.width() - 6.0).abs() < 0.01 && (r.height() - 6.0).abs() < 0.01)
                .count();
            let unmoved = ends
                .iter()
                .filter(|m| !matches!(m, EndMark::Triangle | EndMark::OpenArrow | EndMark::HollowDiamond | EndMark::FilledDiamond))
                .count();
            assert_eq!(squares, if *styled { unmoved } else { 0 }, "{line} at {zoom}: {ends:?}");
            squared += squares;
            for (end, mark) in ["source", "target"].into_iter().zip(ends) {
                let id = format!("workspace.canvas.line-end.{end}");
                if !ids(&h).contains(&id) {
                    continue;
                }
                let at = rect_of(&h, &id).center();
                let off = cards.iter().map(|c| c.distance_to_pos(at)).fold(f32::INFINITY, f32::min);
                let reach = match mark {
                    EndMark::Triangle | EndMark::OpenArrow => 12.0,
                    EndMark::HollowDiamond | EndMark::FilledDiamond => 20.0,
                    _ => 0.0,
                } * f32::max(zoom, 0.6);
                checked.insert(format!("{mark:?}"));
                if reach > 0.0 {
                    assert!(
                        off >= reach + archforge_shell_api::canvas::END_HANDLE - 0.5,
                        "{line} {end} {mark:?} at {zoom}: the handle {off} off the card covers its mark"
                    );
                } else {
                    assert!(off < 0.5, "{line} {end} at {zoom}: an end without a mark keeps its handle on the border, not {off} off it");
                }
            }
        }
    }
    assert_eq!(checked.len(), 5, "every kind of end was looked at: {checked:?}");
    assert!(squared > 0, "an end without a mark keeps its square");
}

/// On a short line between two cards the two end handles keep a hit box apart and each takes its
/// own press, in light and dark (AR-191).
#[test]
fn a_short_lines_two_end_handles_stay_apart_and_each_takes_its_own_press() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, order, _) = nested_orders(theme);
        let (orders, d) = {
            let st = h.state().session.as_ref().unwrap().doc();
            let p = st.uuid(archforge_uml::commands::package_of(st, st.by_uuid(order).unwrap()).unwrap()).unwrap();
            (p, h.state().canvas.active.unwrap())
        };
        // Refund, 24 points right of Order, specializes it: the line runs between their borders.
        let s = h.state_mut().session.as_mut().unwrap();
        let id = |v: serde_json::Value| v["id"].as_str().unwrap().to_string();
        let refund = id(s.execute("uml.class.add", &serde_json::json!({ "package": orders.to_string(), "name": "Refund" })).unwrap().value);
        let place = serde_json::json!({ "diagram": d.to_string(), "element": refund, "x": 384, "y": 80, "width": 80, "height": uml_ui::members_height(2, 1) });
        s.execute("diagram.place", &place).unwrap();
        let g = id(s.execute("uml.generalization.add", &serde_json::json!({ "specific": refund, "general": order.to_string() })).unwrap().value);
        s.execute("diagram.connect-all", &serde_json::json!({ "diagram": d.to_string() })).unwrap();
        s.execute("view.select", &serde_json::json!({ "ids": [g] })).unwrap();
        h.run();
        let (source, target) = (rect_of(&h, "workspace.canvas.line-end.source"), rect_of(&h, "workspace.canvas.line-end.target"));
        assert!(!source.expand(2.0).intersects(target.expand(2.0)), "{theme:?}: {source:?} and {target:?} apart");
        // Each takes its own press: a drag from either starts moving that end.
        for (rect, is_source) in [(source, true), (target, false)] {
            h.event(egui::Event::PointerMoved(rect.center()));
            h.event(egui::Event::PointerButton {
                pos: rect.center(),
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            });
            h.run();
            h.event(egui::Event::PointerMoved(rect.center() + egui::vec2(0.0, 40.0)));
            h.run();
            let moving =
                matches!(h.state().canvas.gesture, Some(archforge_shell_api::editing::Gesture::Reconnect { source, .. }) if source == is_source);
            assert!(moving, "{theme:?}: the {} handle takes its press, {:?}", if is_source { "source" } else { "target" }, h.state().canvas.gesture);
            // Let go with the gesture dropped, so the line stays as it is for the other end.
            h.state_mut().canvas.gesture = None;
            h.event(egui::Event::PointerButton {
                pos: rect.center(),
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            });
            h.run();
        }
    }
}

/// On the canvas, a composition routed in a short U between two small cards, its ends close at
/// 50 %, shows its two end handles with hit boxes apart, and each press reconnects its own end; in
/// light and dark (AR-191).
#[test]
fn a_short_u_shaped_lines_end_handles_stay_apart_and_each_takes_its_own_press() {
    use archforge_diagram::figure::EndMark;
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, order, _) = nested_orders(theme);
        let (orders, d) = {
            let st = h.state().session.as_ref().unwrap().doc();
            let p = st.uuid(archforge_uml::commands::package_of(st, st.by_uuid(order).unwrap()).unwrap()).unwrap();
            (p, h.state().canvas.active.unwrap())
        };
        let s = h.state_mut().session.as_mut().unwrap();
        let id = |v: serde_json::Value| v["id"].as_str().unwrap().to_string();
        let mut add = |name: &str, y: i32| {
            let c = id(s.execute("uml.class.add", &serde_json::json!({ "package": orders.to_string(), "name": name })).unwrap().value);
            s.execute("diagram.place", &serde_json::json!({ "diagram": d.to_string(), "element": c, "x": 100, "y": y, "width": 40, "height": 20 }))
                .unwrap();
            c
        };
        let (whole, part) = (add("Whole", 600), add("Part", 624));
        let a = id(s.execute("uml.association.add", &serde_json::json!({ "from": whole, "to": part, "kind": "composition" })).unwrap().value);
        s.execute("uml.association.set-end", &serde_json::json!({ "association": a, "end": 1, "navigable": true })).unwrap();
        s.execute("diagram.connect-all", &serde_json::json!({ "diagram": d.to_string() })).unwrap();
        // Out of both cards' right sides and back: a U whose ends are 24 apart, 12 at 50 %.
        let edge = {
            let st = s.doc();
            let a = st.by_uuid(archforge_mof::Uuid::parse(&a).unwrap()).unwrap();
            st.uuid(archforge_diagram::views_of(st, a).1[0]).unwrap()
        };
        let route = serde_json::json!({ "edge": edge.to_string(), "points": [{ "x": 200, "y": 610 }, { "x": 200, "y": 634 }] });
        s.execute("diagram.route", &route).unwrap();
        s.execute("view.select", &serde_json::json!({ "ids": [a] })).unwrap();
        let marks = {
            let st = s.doc();
            let line = archforge_uml::figures::line(st, st.by_uuid(archforge_mof::Uuid::parse(&a).unwrap()).unwrap()).unwrap();
            [line.ends[0].mark, line.ends[1].mark]
        };
        assert!(marks.iter().all(|m| *m != EndMark::None), "a mark at both ends: {marks:?}");
        h.state_mut().canvas.zoom = 0.5;
        h.run();
        let (source, target) = (rect_of(&h, "workspace.canvas.line-end.source"), rect_of(&h, "workspace.canvas.line-end.target"));
        assert!(!source.expand(2.0).intersects(target.expand(2.0)), "{theme:?}: hit boxes {source:?} and {target:?} apart");
        for (rect, is_source) in [(source, true), (target, false)] {
            h.event(egui::Event::PointerMoved(rect.center()));
            h.event(egui::Event::PointerButton {
                pos: rect.center(),
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            });
            h.run();
            h.event(egui::Event::PointerMoved(rect.center() + egui::vec2(40.0, 40.0)));
            h.run();
            let moving =
                matches!(h.state().canvas.gesture, Some(archforge_shell_api::editing::Gesture::Reconnect { source, .. }) if source == is_source);
            assert!(
                moving,
                "{theme:?}: the {} handle takes its own press, {:?}",
                if is_source { "source" } else { "target" },
                h.state().canvas.gesture
            );
            h.state_mut().canvas.gesture = None;
            h.event(egui::Event::PointerButton {
                pos: rect.center(),
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            });
            h.run();
        }
    }
}

/// The Relationship tool draws the kind its menu chose, refuses a cycle, and makes a class on empty canvas.
#[test]
fn the_relationship_tool_draws_the_kind_its_menu_chose_refuses_a_cycle_and_a_drop_on_empty_canvas_makes_a_class() {
    let (mut h, nodes) = related_sample();
    let node = |h: &Harness<'static, Shell>, name: &str| rect_of(h, &format!("workspace.canvas.node.{}", nodes[name])).center();
    let before = edge_count(&h);
    // The chevron's menu: Composition.
    let at = rect_of(&h, "workspace.tools.relationship-kinds").center();
    click(&mut h, at);
    let at = rect_of(&h, "workspace.relate.composition").center();
    click(&mut h, at);
    assert_eq!(uml_of(h.state()).relation, uml_relate::Relate::Composition);
    assert!(h.state().canvas.popover.is_none(), "a pick closes the menu");
    // Drawn from the whole, Catalog, to the part, Customer: one step, the line on the diagram.
    let at = rect_of(&h, "workspace.tools.relationship").center();
    click(&mut h, at);
    let (from, to) = (node(&h, "Catalog"), node(&h, "Customer"));
    drag(&mut h, from, to);
    assert_eq!(edge_count(&h), before + 1, "notice {:?}", h.state().canvas.notice);
    let composed =
        related(&mut h, "Catalog").into_iter().find(|r| r["to"].is_string() && r["ends"][1]["aggregation"] == "composite").expect("the composition");
    assert_eq!(composed["kind"], "association");
    assert_eq!(h.state().canvas.tool, archforge_shell_api::canvas::Tool::Select, "the tool hands back to Select");
    h.state_mut().session.as_mut().unwrap().undo().unwrap();
    assert_eq!(edge_count(&h), before);
    h.state_mut().session.as_mut().unwrap().redo().unwrap();
    h.run();
    assert_eq!(edge_count(&h), before + 1);
    // A generalization that would make a cycle is refused and said; nothing is drawn.
    uml_of_mut(h.state_mut()).relation = uml_relate::Relate::Generalization;
    let at = rect_of(&h, "workspace.tools.relationship").center();
    click(&mut h, at);
    let (from, to) = (node(&h, "Payment"), node(&h, "CardPayment"));
    drag(&mut h, from, to);
    assert_eq!(edge_count(&h), before + 1);
    let notice = h.state().canvas.notice.clone().expect("the refusal is said");
    assert!(notice.contains("cycle"), "{notice}");
    // A dependency dropped on empty canvas: a new class there, its name open for typing.
    uml_of_mut(h.state_mut()).relation = uml_relate::Relate::Dependency;
    let at = rect_of(&h, "workspace.tools.relationship").center();
    click(&mut h, at);
    let from = node(&h, "Catalog");
    let canvas = rect_of(&h, "workspace.canvas");
    drag(&mut h, from, egui::pos2(from.x, canvas.bottom() - 60.0));
    let rename = h.state().canvas.rename.clone().expect("the new class's name is open");
    assert_eq!(rename.text, "Class 1");
    assert!(related(&mut h, "Class 1").iter().any(|r| r["kind"] == "dependency"));
    assert_eq!(edge_count(&h), before + 2);
}

/// The classifier tool's menu, its keys, the kind it places and keeps, in both appearances.
#[test]
fn the_classifier_tool_places_the_kind_its_menu_chose_walked_by_the_keys_and_a_new_window_starts_at_class() {
    use uml_classifier::Classifier;
    assert_eq!(uml_state::UmlState::default().classifier, Classifier::Class, "a new window starts at Class");
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, nodes) = related_sample_in(theme, egui::vec2(1600.0, 1000.0));
        let found = ids(&h);
        for id in ["workspace.tools.classifier", "workspace.tools.classifier-kinds", "workspace.tools.package", "workspace.tools.relationship"] {
            assert!(found.iter().any(|i| i == id), "{id}");
        }
        assert_eq!(classifier_hint(&h), "Class", "the tooltip names the kind");
        // The chevron opens the kinds, the current one marked, and arms the tool.
        let at = rect_of(&h, "workspace.tools.classifier-kinds").center();
        click(&mut h, at);
        assert_eq!(h.state().canvas.popover, Some("classifier-kinds"));
        assert_eq!(h.state().canvas.tool, archforge_shell_api::canvas::Tool::Place(uml_classifier::TOOL));
        let found = ids(&h);
        for kind in ["class", "interface"] {
            assert!(found.iter().any(|i| *i == format!("workspace.classifier.{kind}")), "{kind} in {found:?}");
        }
        assert!(!found.iter().any(|i| i.starts_with("workspace.relate.")), "the Relationship menu stays shut");
        // The arrows walk the kinds and Enter picks one: the menu closes, the tool stays armed.
        key(&mut h, egui::Key::ArrowDown, egui::Modifiers::NONE);
        assert_eq!(h.state().canvas.menu_focus, Some(1));
        key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
        assert_eq!(uml_of(h.state()).classifier, Classifier::Interface);
        assert_eq!(h.state().canvas.popover, None, "a pick closes the menu");
        assert_eq!(h.state().canvas.tool, archforge_shell_api::canvas::Tool::Place(uml_classifier::TOOL), "and arms the tool");
        assert_eq!(classifier_hint(&h), "Interface");
        // A click on the canvas places an interface named in place, as one undo step, and hands
        // back to Select.
        let before = h.state().session.as_ref().unwrap().revision();
        let at = empty_spot(&h, &nodes);
        click(&mut h, at);
        assert_eq!(h.state().canvas.tool, archforge_shell_api::canvas::Tool::Select, "the tool hands back to Select");
        let rename = h.state().canvas.rename.clone().expect("its name is open for typing");
        assert_eq!(rename.text, "Interface 1");
        assert_eq!(h.state().session.as_ref().unwrap().undo_label(), Some("Add interface"));
        h.state_mut().canvas.rename.as_mut().unwrap().text = "Payable".into();
        key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
        let payable = named(&h, archforge_uml::INTERFACE, "Payable");
        let st = h.state().session.as_ref().unwrap().doc();
        let home = archforge_uml::commands::package_of(st, st.by_uuid(payable).unwrap()).map(|p| archforge_core::name_of(st, p));
        assert_eq!(home.as_deref(), Some("Commerce"), "in the diagram's package");
        assert!(painted(&h).iter().any(|t| t == "«interface»"), "its eyebrow is its keyword, as written");
        h.state_mut().session.as_mut().unwrap().undo().unwrap();
        h.state_mut().session.as_mut().unwrap().undo().unwrap();
        assert_eq!(h.state().session.as_ref().unwrap().revision(), before, "the rename, then the placing, each one step");
        h.state_mut().session.as_mut().unwrap().redo().unwrap();
        h.run();
        // The kind stays for the window; Escape closes the menu alone, the tool staying armed.
        let at = rect_of(&h, "workspace.tools.classifier-kinds").center();
        click(&mut h, at);
        assert!(h.state().canvas.focused, "the canvas has the keys");
        key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
        assert_eq!((h.state().canvas.popover, uml_of(h.state()).classifier), (None, Classifier::Interface));
        assert_eq!(h.state().canvas.tool, archforge_shell_api::canvas::Tool::Place(uml_classifier::TOOL), "Escape keeps the armed tool");
        // A press on the canvas outside the open menu closes it and places nothing; the next one
        // places with the tool still armed.
        let interfaces = |h: &Harness<'static, Shell>| {
            let st = h.state().session.as_ref().unwrap().doc();
            st.alive().filter(|o| st.class_of(*o).ok() == Some(archforge_uml::INTERFACE)).count()
        };
        let free = {
            let above = rect_of(&h, &format!("workspace.canvas.node.{}", nodes["OrderLine"])).center();
            let below = empty_spot(&h, &nodes);
            egui::pos2(above.x, (above.y + below.y) / 2.0)
        };
        let before = interfaces(&h);
        let at = rect_of(&h, "workspace.tools.classifier-kinds").center();
        click(&mut h, at);
        click(&mut h, free);
        assert_eq!(h.state().canvas.popover, None, "the press closed the menu");
        assert_eq!(interfaces(&h), before, "and placed nothing");
        assert_eq!(h.state().canvas.tool, archforge_shell_api::canvas::Tool::Place(uml_classifier::TOOL));
        click(&mut h, free);
        assert_eq!(interfaces(&h), before + 1, "the next press places one");
        h.state_mut().canvas.rename = None;
        h.state_mut().session.as_mut().unwrap().undo().unwrap();
        h.run();
        // Up and Enter go back to Class; a click on a kind picks it too.
        let at = rect_of(&h, "workspace.tools.classifier-kinds").center();
        click(&mut h, at);
        key(&mut h, egui::Key::ArrowUp, egui::Modifiers::NONE);
        key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
        assert_eq!(uml_of(h.state()).classifier, Classifier::Class);
        let at = rect_of(&h, "workspace.tools.classifier-kinds").center();
        click(&mut h, at);
        let at = rect_of(&h, "workspace.classifier.interface").center();
        click(&mut h, at);
        assert_eq!((h.state().canvas.popover, uml_of(h.state()).classifier), (None, Classifier::Interface));
        // The arrows while the menu is open move no card.
        let order = nodes["Order"].clone();
        let at = rect_of(&h, &format!("workspace.canvas.node.{order}")).center();
        h.state_mut().canvas.tool = archforge_shell_api::canvas::Tool::Select;
        click(&mut h, at);
        let was = rect_of(&h, &format!("workspace.canvas.node.{order}"));
        let at = rect_of(&h, "workspace.tools.classifier-kinds").center();
        click(&mut h, at);
        key(&mut h, egui::Key::ArrowDown, egui::Modifiers::NONE);
        assert_eq!(rect_of(&h, &format!("workspace.canvas.node.{order}")), was, "the menu took the key");
    }
}

/// Models lists interfaces, adds them with `+`, finds them and drags them onto a diagram or a package.
#[test]
fn interfaces_are_listed_under_their_package_added_with_the_plus_found_and_dragged_onto_the_diagram() {
    use archforge_shell_api::notation_ui::NotationUi;
    let (mut h, nodes) = related_sample();
    let commerce = select_named(&mut h, archforge_uml::PACKAGE, "Commerce");
    // Models' + offers each classifier kind and a package in the selected package.
    let st = h.state().session.as_ref().unwrap().doc();
    let offers: Vec<&str> = uml_ui::UmlUi.offers(st, st.by_uuid(commerce)).into_iter().map(|o| o.key).collect();
    assert_eq!(offers, ["class", "interface", "enumeration", "data-type", "primitive-type", "package"]);
    archforge_ui_egui::explorer::add(h.state_mut(), "interface");
    let rename = h.state().explorer.rename.clone().expect("named in place");
    assert_eq!(rename.text, "Interface 1");
    h.state_mut().explorer.rename.as_mut().unwrap().text = "Payable".into();
    key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
    h.run();
    let payable = named(&h, archforge_uml::INTERFACE, "Payable");
    let rows = archforge_ui_egui::testing::models_rows(
        archforge_ui_egui::testing::notation_uis(h.state()),
        h.state().session.as_ref().unwrap().doc(),
        &h.state().explorer,
    );
    let row = rows.iter().find(|r| r.label == "Payable").expect("listed under Commerce");
    assert_eq!((row.kind, row.depth), ("interface", 1), "{:?}", tree_labels(&h));
    // A name a class of the package has is refused, in place.
    let refused = archforge_ui_egui::snapshot::run(
        h.state_mut(),
        "uml.interface.rename",
        serde_json::json!({ "interface": payable.to_string(), "name": "order" }),
    );
    assert!(refused.is_err_and(|e| e.contains("class named Order")), "it names the class that has the name");
    // The search finds it as an interface of Commerce.
    let index = archforge_ui_egui::testing::search_index(
        archforge_ui_egui::testing::kinds(h.state()),
        archforge_ui_egui::testing::notation_uis(h.state()),
        h.state().session.as_ref().unwrap().doc(),
    );
    let hit = archforge_ui_egui::testing::search_hits(&index, "payable").1.into_iter().find(|e| e.label == "Payable").map(|e| e.detail.clone());
    assert_eq!(hit.as_deref(), Some("Interface · Commerce"));
    // Dragged from Models onto the diagram: its card, as one undo step; a second drag says so.
    let nodes_now = |h: &Harness<'static, Shell>| {
        let st = h.state().session.as_ref().unwrap().doc();
        st.list(st.by_uuid(h.state().canvas.active.unwrap()).unwrap(), archforge_diagram::diagram::NODE).unwrap().len()
    };
    let before = nodes_now(&h);
    let from = rect_of(&h, &format!("workspace.explorer.row.{payable}")).center();
    let to = empty_spot(&h, &nodes);
    drag(&mut h, from, to);
    assert_eq!(nodes_now(&h), before + 1, "notice {:?}", h.state().canvas.notice);
    assert_eq!(h.state().session.as_ref().unwrap().undo_label(), Some("Place interface"));
    let from = rect_of(&h, &format!("workspace.explorer.row.{payable}")).center();
    drag(&mut h, from, to);
    assert_eq!(h.state().canvas.notice.as_deref(), Some("Already on this diagram"));
    // Dragged onto another package's row, it moves there.
    let billing = {
        let s = h.state_mut().session.as_mut().unwrap();
        let out = s.execute("uml.package.add", &serde_json::json!({ "name": "Billing", "parent": commerce.to_string() })).unwrap();
        archforge_mof::Uuid::parse(out.value["id"].as_str().unwrap()).unwrap()
    };
    h.run();
    let from = rect_of(&h, &format!("workspace.explorer.row.{payable}")).center();
    let to = rect_of(&h, &format!("workspace.explorer.row.{billing}")).center();
    drag(&mut h, from, to);
    let st = h.state().session.as_ref().unwrap().doc();
    assert_eq!(archforge_uml::commands::package_of(st, st.by_uuid(payable).unwrap()), st.by_uuid(billing), "{:?}", h.state().explorer.notice);
    assert_eq!(h.state().session.as_ref().unwrap().undo_label(), Some("Move interface"));
}

/// An interface takes members on its card and in Properties, all public, and is offered as a type.
#[test]
fn an_interface_takes_members_on_its_card_and_in_properties_and_is_offered_as_a_type() {
    let (mut h, _) = related_sample();
    let d = h.state().canvas.active.unwrap().to_string();
    let payable = {
        let s = h.state_mut().session.as_mut().unwrap();
        let st = s.doc();
        let commerce =
            st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_uml::PACKAGE) && archforge_core::name_of(st, *o) == "Commerce").unwrap();
        let commerce = st.uuid(commerce).unwrap().to_string();
        let out = s.execute("uml.interface.add", &serde_json::json!({ "package": commerce, "name": "Payable" })).unwrap();
        let id = out.value["id"].as_str().unwrap().to_string();
        s.execute("diagram.place", &serde_json::json!({ "diagram": d, "element": id, "x": 560, "y": 700, "width": 290, "height": 136 })).unwrap();
        s.execute("view.select", &serde_json::json!({ "ids": [id] })).unwrap();
        archforge_mof::Uuid::parse(&id).unwrap()
    };
    h.state_mut().state.show_panel(archforge_shell_api::state::Panel::Properties);
    h.run();
    let texts = painted(&h);
    assert!(texts.iter().any(|t| t == "Interface · model, in every diagram"), "{texts:?}");
    assert!(!ids(&h).iter().any(|i| i == "workspace.properties.class.abstract"), "an interface has no Abstract");
    // On its card: the attributes' + opens a draft row; Enter writes it as one step.
    let at = rect_of(&h, "workspace.canvas.class.add-attribute").center();
    click(&mut h, at);
    h.event(egui::Event::Text("due".into()));
    h.run();
    key(&mut h, egui::Key::Tab, egui::Modifiers::NONE);
    h.event(egui::Event::Text("Real".into()));
    h.run();
    key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
    key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
    assert_eq!(member_rows(&h, payable, false), ["+ due : Real"]);
    assert_eq!(h.state().session.as_ref().unwrap().undo_label(), Some("Add attribute"));
    assert!(painted(&h).iter().any(|t| t == "+ due : Real"), "the card shows it");
    // In Properties: an operation with its return type.
    h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [payable.to_string()] })).unwrap();
    h.run();
    let at = rect_of(&h, "workspace.properties.operations.add").center();
    click(&mut h, at);
    h.event(egui::Event::Text("pay".into()));
    h.run();
    pick_type(&mut h, "workspace.properties.member.type", "Boolean");
    let at = rect_of(&h, "workspace.properties.member.apply").center();
    click(&mut h, at);
    assert!(uml_of(h.state()).member.is_none(), "{:?}", uml_of(h.state()).member);
    assert_eq!(member_rows(&h, payable, true), ["+ pay() : Boolean"]);
    // Its members are public: the editor's Visibility offers Public alone.
    let at = rect_of(&h, "workspace.properties.attributes.add").center();
    click(&mut h, at);
    let at = rect_of(&h, "workspace.properties.member.visibility").center();
    click(&mut h, at);
    let offered: Vec<String> = ids(&h).into_iter().filter(|i| i.starts_with("workspace.properties.member.visibility.")).collect();
    assert_eq!(offered, ["workspace.properties.member.visibility.0"]);
    let at = rect_of(&h, "workspace.properties.member.cancel").center();
    click(&mut h, at);
    // It is offered as a type: Customer's new attribute takes it.
    assert!(
        uml_panel::type_choices(h.state().session.as_ref().unwrap().doc(), None).iter().any(|(l, id)| l == "Payable" && *id == payable.to_string())
    );
    let customer = select_named(&mut h, archforge_uml::CLASS, "Customer");
    let at = rect_of(&h, "workspace.properties.attributes.add").center();
    click(&mut h, at);
    h.event(egui::Event::Text("wallet".into()));
    h.run();
    pick_type(&mut h, "workspace.properties.member.type", "Payable");
    let at = rect_of(&h, "workspace.properties.member.apply").center();
    click(&mut h, at);
    assert!(member_rows(&h, customer, false).iter().any(|r| r == "+ wallet : Payable"), "{:?}", member_rows(&h, customer, false));
}

/// No subclasses (final) in a classifier's Properties (AR-137), in light and dark: described by its
/// explanation, ticked it makes the classifier final in one undo step and its card says `{final}`;
/// the Relationship tool then refuses a generalization to it, and a generalization's general end
/// dragged onto it, at the canvas's top, changing nothing; ticked on a classifier with subclasses
/// it stays clear and Properties names them, with a button that selects their generalizations.
#[test]
fn no_subclasses_makes_a_classifier_final_and_specialising_it_is_refused() {
    use egui_kittest::kittest::{NodeT, Queryable};
    use uml_panel::FINAL_ID;
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, nodes) = related_sample_in(theme, egui::vec2(1600.0, 1000.0));
        let class = |h: &Harness<'static, Shell>, name: &str| {
            let st = h.state().session.as_ref().unwrap().doc();
            st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_uml::CLASS) && archforge_core::name_of(st, *o) == name).unwrap()
        };
        let select = |h: &mut Harness<'static, Shell>, id: String| {
            h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [id] })).unwrap();
            h.state_mut().state.show_panel(archforge_shell_api::state::Panel::Properties);
            h.run();
        };
        let is_final = |h: &Harness<'static, Shell>, name: &str| {
            let st = h.state().session.as_ref().unwrap().doc();
            archforge_uml::relations::is_final(st, class(h, name))
        };
        let pricing = class(&h, "PricingService");
        let pricing_id = h.state().session.as_ref().unwrap().doc().uuid(pricing).unwrap().to_string();
        select(&mut h, pricing_id.clone());
        // Its accessible name and the explanation that describes it.
        let node = h.get_by_label("No subclasses (final)").accesskit_node().data().clone();
        assert_eq!(node.description(), Some("Nothing may specialise PricingService. Shown as {final} after its name."));
        assert_eq!(node.toggled(), Some(egui::accesskit::Toggled::False));
        assert!(painted(&h).iter().any(|t| t == "Nothing may specialise PricingService. Shown as {final} after its name."));
        // Ticked: final in one step, `{final}` after its name on its card.
        let at = rect_of(&h, FINAL_ID).center();
        click(&mut h, at);
        assert!(is_final(&h, "PricingService"));
        assert_eq!(h.state().session.as_ref().unwrap().undo_label(), Some("Set classifier"));
        h.run();
        assert!(painted(&h).iter().any(|t| t == "{final}"), "the card says {{final}}");
        assert_eq!(h.get_by_label("No subclasses (final)").accesskit_node().data().toggled(), Some(egui::accesskit::Toggled::True));
        // A generalization drawn from OrderLine to it is refused at the canvas's top.
        let refusal = "PricingService is final: nothing may specialise it. Clear \"No subclasses\" in its Properties first.";
        let revision = h.state().session.as_ref().unwrap().revision();
        let edges = edge_count(&h);
        uml_of_mut(h.state_mut()).relation = uml_relate::Relate::Generalization;
        let at = rect_of(&h, "workspace.tools.relationship").center();
        click(&mut h, at);
        let from = rect_of(&h, &format!("workspace.canvas.node.{}", nodes["OrderLine"])).center();
        let to = rect_of(&h, &format!("workspace.canvas.node.{}", nodes["PricingService"])).center();
        drag(&mut h, from, to);
        assert_eq!(h.state().canvas.notice.as_deref(), Some(refusal));
        assert_eq!((edge_count(&h), h.state().session.as_ref().unwrap().revision()), (edges, revision), "nothing drawn");
        // CardPayment's generalization: its general end dragged onto it is refused the same way.
        let generalization = {
            let st = h.state().session.as_ref().unwrap().doc();
            let (g, _) = archforge_uml::relations::specializations(st, class(&h, "Payment"))[0];
            st.uuid(g).unwrap().to_string()
        };
        h.state_mut().canvas.notice = None;
        select(&mut h, generalization.clone());
        let handle = rect_of(&h, "workspace.canvas.line-end.target").center();
        drag(&mut h, handle, to);
        assert_eq!(h.state().canvas.notice.as_deref(), Some(refusal));
        assert_eq!(h.state().session.as_ref().unwrap().revision(), revision, "nothing moved");
        // Payment has a subclass: ticked, the box stays clear, nothing is written, and the
        // refusal names CardPayment; its button selects the generalization.
        let payment = h.state().session.as_ref().unwrap().doc().uuid(class(&h, "Payment")).unwrap().to_string();
        select(&mut h, payment);
        let at = rect_of(&h, FINAL_ID).center();
        click(&mut h, at);
        h.run();
        assert!(!is_final(&h, "Payment"));
        assert_eq!(h.state().session.as_ref().unwrap().revision(), revision, "nothing entered the model or its history");
        let said = "Payment has 1 subclass: CardPayment. Remove that generalization before making it final.";
        assert!(painted(&h).iter().any(|t| t == said), "{:?}", painted(&h));
        assert!(ids(&h).contains(&format!("{FINAL_ID}.refusal")));
        let node = h.get_by_label("No subclasses (final)").accesskit_node().data().clone();
        assert!(node.description().is_some_and(|d| d.ends_with(said)), "{:?}", node.description());
        let at = rect_of(&h, &format!("{FINAL_ID}.select")).center();
        click(&mut h, at);
        let st = h.state().session.as_ref().unwrap().doc();
        let selected: Vec<String> = h.state().session.as_ref().unwrap().view().selection.iter().map(|o| st.uuid(*o).unwrap().to_string()).collect();
        assert_eq!(selected, [generalization]);
        // Cleared on PricingService: one step, undone and redone; a generalization to it is drawn.
        select(&mut h, pricing_id.clone());
        let at = rect_of(&h, FINAL_ID).center();
        click(&mut h, at);
        assert!(!is_final(&h, "PricingService"));
        h.state_mut().session.as_mut().unwrap().undo().unwrap();
        assert!(is_final(&h, "PricingService"));
        h.state_mut().session.as_mut().unwrap().redo().unwrap();
        assert!(!is_final(&h, "PricingService"));
        h.run();
        assert!(!painted(&h).iter().any(|t| t == "{final}"));
        h.state_mut().canvas.notice = None;
        let at = rect_of(&h, "workspace.tools.relationship").center();
        click(&mut h, at);
        drag(&mut h, from, to);
        assert_eq!(edge_count(&h), edges + 1, "notice {:?}", h.state().canvas.notice);
    }
}

/// Generalization sets (AR-129, design page 08, B1 to B4), in light and dark: three generalizations
/// to Payment selected, a right click on one of their lines opens the set actions, Create
/// generalization set… makes one undo step and selects the set, its name ready in Properties, which
/// shows unfolded, its label `name
/// {incomplete, overlapping}` drawn beside the lines; Properties' Completeness and Disjointness
/// write in one step each, undone and redone, the label following; a member is taken out; ⌥⌘G
/// makes a second set; the generalization in both lists both; a mixed selection disables Create in
/// the menu and in Properties with the reason; reconnecting a member is refused, said at the canvas's
/// top and in its Properties, changing nothing; Delete on a set keeps its generalizations.
#[test]
fn generalization_sets_are_made_from_the_menu_the_shortcut_and_properties() {
    use archforge_uml::generalization_sets as sets;
    use egui_kittest::kittest::Queryable;
    use uml_sets::{CREATE_ID, PANEL_CREATE_ID};
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, _) = related_sample_in(theme, egui::vec2(1600.0, 1000.0));
        let (gens, payment) = archforge_ui_egui::snapshot::cx(h.state_mut(), uml_ui::put_payment_kinds).unwrap();
        h.state_mut().canvas.zoom = 0.75;
        h.state_mut().canvas.pan = egui::vec2(-470.0, -230.0);
        h.run();
        let obj = |h: &Harness<'static, Shell>, id: &str| {
            h.state().session.as_ref().unwrap().doc().by_uuid(archforge_mof::Uuid::parse(id).unwrap()).unwrap()
        };
        let select = |h: &mut Harness<'static, Shell>, ids: Vec<String>| {
            h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": ids })).unwrap();
            h.state_mut().state.show_panel(archforge_shell_api::state::Panel::Properties);
            h.run();
        };
        let all_sets = |h: &Harness<'static, Shell>| {
            let st = h.state().session.as_ref().unwrap().doc();
            let mut v: Vec<archforge_mof::Obj> = st.alive().filter(|o| sets::is_set(st, *o)).collect();
            v.sort();
            v
        };
        let edge_of = |h: &Harness<'static, Shell>, g: &str| {
            let st = h.state().session.as_ref().unwrap().doc();
            st.uuid(archforge_diagram::views_of(st, obj(h, g)).1[0]).unwrap()
        };
        // B1: the three selected, a right click on one of their lines opens the set actions.
        select(&mut h, gens.to_vec());
        let at = point_on_edge(&h, edge_of(&h, &gens[1]));
        h.event(egui::Event::PointerMoved(at));
        for pressed in [true, false] {
            h.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Secondary, pressed, modifiers: egui::Modifiers::NONE });
        }
        h.run();
        assert!(uml_of(h.state()).line_menu.is_some(), "the menu opened");
        assert_eq!(h.state().session.as_ref().unwrap().view().selection.len(), 3, "the selection is kept");
        assert!(painted(&h).iter().any(|t| t == "3 GENERALIZATIONS · TO PAYMENT"), "{:?}", painted(&h));
        assert!(painted(&h).iter().any(|t| t == "none for Payment yet"));
        assert!(h.query_by_label("Create generalization set…").is_some(), "its accessible name");
        for id in [
            CREATE_ID,
            "workspace.canvas.contextMenu.generalizationSet.add",
            "workspace.canvas.contextMenu.remove",
            "workspace.canvas.contextMenu.delete",
        ] {
            assert!(ids(&h).iter().any(|i| i == id), "{id}");
        }
        // Another panel, folded away, gives way to the new set's Properties.
        (h.state_mut().state.panel, h.state_mut().state.panel_collapsed) = (archforge_shell_api::state::Panel::Skills, true);
        let at = rect_of(&h, CREATE_ID).center();
        click(&mut h, at);
        assert_eq!(h.state().session.as_ref().unwrap().undo_label(), Some("Create generalization set"));
        assert_eq!(
            (h.state().state.panel, h.state().state.panel_collapsed),
            (archforge_shell_api::state::Panel::Properties, false),
            "{theme:?}: Properties shows"
        );
        let made = all_sets(&h);
        assert_eq!(made.len(), 1);
        let set = made[0];
        let set_id = h.state().session.as_ref().unwrap().doc().uuid(set).unwrap();
        assert_eq!(h.state().session.as_ref().unwrap().view().selection, vec![set], "the new set is selected");
        h.run();
        assert!(painted(&h).iter().any(|t| t == "GeneralizationSet 1"), "its fresh name");
        assert!(painted(&h).iter().any(|t| t == "{incomplete, overlapping}"), "its label beside the lines");
        assert!(ids(&h).contains(&format!("workspace.canvas.generalizationSet.{set_id}")));
        // B2: Completeness and Disjointness, one step each, the label following.
        let at = rect_of(&h, "workspace.properties.generalizationSet.complete").center();
        click(&mut h, at);
        let at = rect_of(&h, "workspace.properties.generalizationSet.disjoint").center();
        click(&mut h, at);
        assert_eq!(sets::flags(h.state().session.as_ref().unwrap().doc(), set), (true, true));
        // The arrows flip the pair with the keyboard; Newer version's chord, off here, is no arrow.
        h.ctx.memory_mut(|m| m.request_focus(archforge_ui_egui::widgets::egui_id("workspace.properties.generalizationSet.disjoint")));
        h.run();
        for (held, flags) in
            [(egui::Modifiers::MAC_CMD | egui::Modifiers::COMMAND | egui::Modifiers::ALT, (true, true)), (egui::Modifiers::NONE, (true, false))]
        {
            key(&mut h, egui::Key::ArrowRight, held);
            assert_eq!(sets::flags(h.state().session.as_ref().unwrap().doc(), set), flags, "{held:?}");
        }
        key(&mut h, egui::Key::ArrowLeft, egui::Modifiers::NONE);
        assert_eq!(sets::flags(h.state().session.as_ref().unwrap().doc(), set), (true, true));
        assert!(painted(&h).iter().any(|t| t == "{complete, disjoint}"));
        assert!(painted(&h).iter().any(|t| t == "Shown beside the lines as {complete, disjoint}: every Payment is exactly one of these kinds."));
        h.state_mut().session.as_mut().unwrap().undo().unwrap();
        assert_eq!(sets::flags(h.state().session.as_ref().unwrap().doc(), set), (true, false));
        h.state_mut().session.as_mut().unwrap().redo().unwrap();
        assert_eq!(sets::flags(h.state().session.as_ref().unwrap().doc(), set), (true, true));
        h.run();
        // Its label dragged up: where it sits is this diagram's own, a node nested in Payment's.
        let d = h.state().session.as_ref().unwrap().doc().by_uuid(h.state().canvas.active.unwrap()).unwrap();
        let label = rect_of(&h, &format!("workspace.canvas.generalizationSet.{set_id}"));
        drag(&mut h, label.center(), label.center() + egui::vec2(30.0, -24.0));
        assert_eq!(h.state().session.as_ref().unwrap().undo_label(), Some("Place generalization set label"));
        let node = sets::label_node(h.state().session.as_ref().unwrap().doc(), d, set).expect("a node holds its place");
        let moved = rect_of(&h, &format!("workspace.canvas.generalizationSet.{set_id}"));
        assert!((moved.min - label.min - egui::vec2(30.0, -24.0)).length() < 2.0, "{label:?} {moved:?}");
        // Dragged again, the node moves.
        drag(&mut h, moved.center(), moved.center() + egui::vec2(-30.0, 0.0));
        let again = rect_of(&h, &format!("workspace.canvas.generalizationSet.{set_id}"));
        assert!((again.min.x - (moved.min.x - 30.0)).abs() < 2.0, "{moved:?} {again:?}");
        assert_eq!(sets::label_node(h.state().session.as_ref().unwrap().doc(), d, set), Some(node));
        // A member taken out from the set's Properties.
        let wallet = obj(&h, &gens[2]);
        let at = rect_of(&h, &format!("workspace.properties.generalizationSet.member.{}.remove", gens[2])).center();
        click(&mut h, at);
        assert_eq!(sets::members(h.state().session.as_ref().unwrap().doc(), set).len(), 2);
        assert!(!sets::members(h.state().session.as_ref().unwrap().doc(), set).contains(&wallet));
        // ⌥⌘G: CardPayment's and WalletPayment's generalizations into a second set, in both key
        // shapes; with Shift or a Mac's Control more, or without Alt, it makes none. The window's own
        // bar and its Alt letters, as on Windows and Linux, leave it be (AR-538).
        h.state_mut().in_window_menu = true;
        select(&mut h, vec![gens[0].clone(), gens[2].clone()]);
        h.state_mut().canvas.focused = true;
        let (mac, alt) = (egui::Modifiers::MAC_CMD | egui::Modifiers::COMMAND, egui::Modifiers::ALT);
        for (n, shape) in [mac, egui::Modifiers::CTRL | egui::Modifiers::COMMAND].into_iter().enumerate() {
            for other in [shape | alt | egui::Modifiers::SHIFT, shape, mac | alt | egui::Modifiers::CTRL] {
                key(&mut h, egui::Key::G, other);
                assert_eq!(all_sets(&h).len(), 1, "{theme:?}: {other:?}");
            }
            key(&mut h, egui::Key::G, shape | alt);
            assert_eq!(all_sets(&h).len(), 2, "{theme:?} {shape:?}: notice {:?}", h.state().canvas.notice);
            if n == 0 {
                h.state_mut().session.as_mut().unwrap().undo().unwrap();
                select(&mut h, vec![gens[0].clone(), gens[2].clone()]);
                h.state_mut().canvas.focused = true;
            }
        }
        // B4: CardPayment's generalization lists both sets.
        select(&mut h, vec![gens[0].clone()]);
        assert!(painted(&h).iter().any(|t| t == "Generalization sets · 2"), "{:?}", painted(&h));
        for s in all_sets(&h) {
            let sid = h.state().session.as_ref().unwrap().doc().uuid(s).unwrap();
            assert!(ids(&h).contains(&format!("workspace.properties.generalization.set.{sid}")));
        }
        // Its general end dragged onto Product is refused, said at the canvas's top and in Properties.
        let revision = h.state().session.as_ref().unwrap().revision();
        let handle = rect_of(&h, "workspace.canvas.line-end.target").center();
        let bank_node = {
            let st = h.state().session.as_ref().unwrap().doc();
            let c =
                st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_uml::CLASS) && archforge_core::name_of(st, *o) == "BankTransfer").unwrap();
            st.uuid(archforge_diagram::views_of(st, c).2[0]).unwrap()
        };
        let to = rect_of(&h, &format!("workspace.canvas.node.{bank_node}")).center();
        drag(&mut h, handle, to);
        let notice = h.state().canvas.notice.clone().unwrap_or_default();
        assert!(notice.starts_with("CardPayment → Payment is in GeneralizationSet 1 and GeneralizationSet 2"), "{notice}");
        assert!(notice.ends_with("Remove it from those sets before reconnecting it to BankTransfer."), "{notice}");
        assert_eq!(h.state().session.as_ref().unwrap().revision(), revision, "nothing moved");
        h.run();
        assert!(ids(&h).iter().any(|i| i == "workspace.properties.generalization.sets.refusal"));
        // B3: a generalization and a class: Create is disabled, with the reason, in Properties.
        select(&mut h, vec![gens[0].clone(), payment.clone()]);
        assert!(painted(&h).iter().any(|t| t == "Selection · mixed"));
        assert!(ids(&h).iter().any(|i| i == PANEL_CREATE_ID));
        assert!(ids(&h).iter().any(|i| i == "workspace.properties.selection.generalizationSet.reason"));
        let before = all_sets(&h).len();
        let at = rect_of(&h, PANEL_CREATE_ID).center();
        click(&mut h, at);
        assert_eq!(all_sets(&h).len(), before, "disabled: nothing made");
        // Delete on a selected set removes it and keeps its generalizations.
        let gens_alive = |h: &Harness<'static, Shell>| gens.iter().all(|g| h.state().session.as_ref().unwrap().doc().is_alive(obj(h, g)));
        select(&mut h, vec![set_id.to_string()]);
        h.state_mut().canvas.focused = true;
        key(&mut h, egui::Key::Delete, egui::Modifiers::NONE);
        assert_eq!(all_sets(&h).len(), 1);
        assert!(gens_alive(&h), "its generalizations stay");
    }
}

/// A generalization set's label that draws nothing is never hit (AR-129), in light and dark: its
/// label moved, so a node holds its place, and then every member line taken off the diagram, a
/// click or a drag where the label was selects and moves nothing of the set, and the node keeps
/// its place; the lines back, the label draws there again and takes the click.
#[test]
fn a_set_label_that_draws_nothing_takes_no_press() {
    use archforge_uml::generalization_sets as sets;
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, _) = related_sample_in(theme, egui::vec2(1600.0, 1000.0));
        let (gens, _) = archforge_ui_egui::snapshot::cx(h.state_mut(), uml_ui::put_payment_kinds).unwrap();
        h.state_mut().canvas.zoom = 0.75;
        h.state_mut().canvas.pan = egui::vec2(-470.0, -230.0);
        let out = h
            .state_mut()
            .session
            .as_mut()
            .unwrap()
            .execute("uml.generalization-set.add", &serde_json::json!({ "generalizations": gens, "name": "method" }))
            .unwrap();
        let set_id = out.value["id"].as_str().unwrap().to_string();
        h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [] })).unwrap();
        h.run();
        let label_id = format!("workspace.canvas.generalizationSet.{set_id}");
        let label = rect_of(&h, &label_id);
        drag(&mut h, label.center(), label.center() + egui::vec2(40.0, -20.0));
        let (d, set) = {
            let st = h.state().session.as_ref().unwrap().doc();
            (st.by_uuid(h.state().canvas.active.unwrap()).unwrap(), st.by_uuid(archforge_mof::Uuid::parse(&set_id).unwrap()).unwrap())
        };
        let node = sets::label_node(h.state().session.as_ref().unwrap().doc(), d, set).expect("a node holds its place");
        let node_id = h.state().session.as_ref().unwrap().doc().uuid(node).unwrap();
        assert!(!ids(&h).contains(&format!("workspace.canvas.node.{node_id}")), "{theme:?}: the label's node draws no card of its own");
        let bounds = |h: &Harness<'static, Shell>| archforge_diagram::grid::bounds_of(h.state().session.as_ref().unwrap().doc(), node);
        let placed = bounds(&h);
        let at = rect_of(&h, &label_id).center();
        // Every member line off this diagram: the label draws nothing and takes no press.
        let edges: Vec<String> = {
            let st = h.state().session.as_ref().unwrap().doc();
            gens.iter()
                .flat_map(|g| archforge_diagram::views_of(st, st.by_uuid(archforge_mof::Uuid::parse(g).unwrap()).unwrap()).1)
                .map(|e| st.uuid(e).unwrap().to_string())
                .collect()
        };
        for e in &edges {
            h.state_mut().session.as_mut().unwrap().execute("diagram.disconnect", &serde_json::json!({ "edge": e })).unwrap();
        }
        h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [] })).unwrap();
        h.run();
        assert!(!ids(&h).contains(&label_id), "nothing drawn");
        assert!(h.state().canvas.extra_labels.is_empty());
        click(&mut h, at);
        assert!(!h.state().session.as_ref().unwrap().view().selection.contains(&set), "the hidden label is not selectable");
        drag(&mut h, at, at + egui::vec2(60.0, 40.0));
        assert!(!h.state().session.as_ref().unwrap().view().selection.contains(&set));
        assert_eq!(bounds(&h), placed, "nor does it move");
        // The lines back, the label draws at its place again and takes the click.
        let diagram = h.state().canvas.active.unwrap().to_string();
        h.state_mut().session.as_mut().unwrap().execute("diagram.connect-all", &serde_json::json!({ "diagram": diagram })).unwrap();
        h.run();
        let back = rect_of(&h, &label_id).center();
        click(&mut h, back);
        assert_eq!(h.state().session.as_ref().unwrap().view().selection, vec![set]);
    }
}

/// The shared removals say what UML worked out (AR-789), as before: Delete on a line alone in a
/// set names the set and then the rule it changes; the Models tree's removal at once says the rule
/// after `{name} deleted`, and a nested package's after `{name} removed`. In light and dark.
#[test]
fn a_removal_says_what_its_notation_worked_out() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, gens, payment) = sample_with_a_set_and_a_rule(theme);
        let toast = |h: &Harness<'static, Shell>| h.state().canvas.toast.as_ref().map(|t| t.text.clone());
        h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [gens[1]] })).unwrap();
        h.run();
        let at = rect_of(&h, "workspace.canvas").center();
        h.event(egui::Event::PointerMoved(at));
        h.state_mut().canvas.focused = true;
        key(&mut h, egui::Key::Delete, egui::Modifiers::NONE);
        let want = "Generalization removed · Kinds had no other member and went too · settled now constrains Payment only";
        assert_eq!(toast(&h).as_deref(), Some(want), "{theme:?}");
        h.state_mut().session.as_mut().unwrap().undo().unwrap();
        h.state_mut().canvas.toast = None;
        h.run();
        // A class on no diagram, named by a rule: removed at once, the rule said after the lead.
        let pkg = {
            let st = h.state().session.as_ref().unwrap().doc();
            let p = archforge_uml::commands::package_of(st, st.by_uuid(archforge_mof::Uuid::parse(&payment).unwrap()).unwrap()).unwrap();
            st.uuid(p).unwrap().to_string()
        };
        let s = h.state_mut().session.as_mut().unwrap();
        let id = |v: serde_json::Value| v["id"].as_str().unwrap().to_string();
        let extra = id(s.execute("uml.class.add", &serde_json::json!({ "package": pkg, "name": "Extra" })).unwrap().value);
        let rule = serde_json::json!({ "context": payment, "name": "spare", "expression": "x", "constrained": [extra, payment] });
        s.execute("uml.constraint.add", &rule).unwrap();
        s.execute("view.select", &serde_json::json!({ "ids": [extra] })).unwrap();
        h.run();
        archforge_ui_egui::explorer::remove_selection(h.state_mut());
        assert!(h.state().explorer.delete.is_none(), "{theme:?}: nothing else goes, so no dialog");
        assert_eq!(toast(&h).as_deref(), Some("Extra deleted · spare now constrains Payment only"), "{theme:?}");
        h.state_mut().canvas.toast = None;
        // A nested package named by a rule hands up what it holds: the rule after its toast.
        let s = h.state_mut().session.as_mut().unwrap();
        let inner = id(s.execute("uml.package.add", &serde_json::json!({ "parent": pkg, "name": "Inner" })).unwrap().value);
        let rule = serde_json::json!({ "context": payment, "name": "kept", "expression": "x", "constrained": [inner, payment] });
        s.execute("uml.constraint.add", &rule).unwrap();
        s.execute("view.select", &serde_json::json!({ "ids": [inner] })).unwrap();
        h.run();
        archforge_ui_egui::explorer::remove_selection(h.state_mut());
        assert!(h.state().explorer.delete.is_none(), "{theme:?}: a nested package hands up, no dialog");
        assert_eq!(toast(&h).as_deref(), Some("Inner removed · kept now constrains Payment only"), "{theme:?}");
    }
}

/// The Relationship tool joins interfaces and refuses what would make a class specialize an interface.
#[test]
fn the_relationship_tool_joins_interfaces_and_refuses_what_would_make_a_class_specialize_an_interface() {
    let (mut h, nodes) = related_sample();
    // Two interfaces on the diagram, under OrderLine and right of it.
    let d = h.state().canvas.active.unwrap().to_string();
    let placed: Vec<String> = {
        let s = h.state_mut().session.as_mut().unwrap();
        let st = s.doc();
        let commerce =
            st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_uml::PACKAGE) && archforge_core::name_of(st, *o) == "Commerce").unwrap();
        let commerce = st.uuid(commerce).unwrap().to_string();
        [("Payable", 560, 1000), ("Refundable", 1040, 1000)]
            .into_iter()
            .map(|(name, x, y)| {
                let id = s.execute("uml.interface.add", &serde_json::json!({ "package": commerce, "name": name })).unwrap().value["id"]
                    .as_str()
                    .unwrap()
                    .to_string();
                let n = s
                    .execute("diagram.place", &serde_json::json!({ "diagram": d, "element": id, "x": x, "y": y, "width": 290, "height": 136 }))
                    .unwrap();
                n.value["id"].as_str().unwrap().to_string()
            })
            .collect()
    };
    h.state_mut().canvas.fit_pending = true;
    h.run();
    let node = |h: &Harness<'static, Shell>, id: &str| rect_of(h, &format!("workspace.canvas.node.{id}")).center();
    let (payable, refundable) = (placed[0].clone(), placed[1].clone());
    let before = edge_count(&h);
    let relate = |h: &mut Harness<'static, Shell>, kind: uml_relate::Relate, from: egui::Pos2, to: egui::Pos2| {
        uml_of_mut(h.state_mut()).relation = kind;
        h.state_mut().canvas.notice = None;
        let at = rect_of(h, "workspace.tools.relationship").center();
        click(h, at);
        drag(h, from, to);
    };
    // An interface specializes an interface: one line, one step.
    let (from, to) = (node(&h, &refundable), node(&h, &payable));
    relate(&mut h, uml_relate::Relate::Generalization, from, to);
    assert_eq!(edge_count(&h), before + 1, "notice {:?}", h.state().canvas.notice);
    assert_eq!(h.state().session.as_ref().unwrap().undo_label(), Some("Add generalization"));
    // An interface never specializes a class, nor a class an interface: said, nothing drawn.
    let revision = h.state().session.as_ref().unwrap().revision();
    for (from, to, why) in [
        (payable.clone(), nodes["Order"].clone(), "An interface specializes only an interface"),
        (nodes["OrderLine"].clone(), payable.clone(), "A class cannot specialize an interface"),
    ] {
        let (a, b) = (node(&h, &from), node(&h, &to));
        relate(&mut h, uml_relate::Relate::Generalization, a, b);
        let notice = h.state().canvas.notice.clone().expect("the refusal is said");
        assert!(notice.contains(why), "{notice}");
        assert_eq!((edge_count(&h), h.state().session.as_ref().unwrap().revision()), (before + 1, revision), "nothing changed");
    }
    // An association between two interfaces, and one from a class to an interface.
    let (from, to) = (node(&h, &payable), node(&h, &refundable));
    relate(&mut h, uml_relate::Relate::Directed, from, to);
    let (from, to) = (node(&h, &nodes["OrderLine"]), node(&h, &payable));
    relate(&mut h, uml_relate::Relate::Association, from, to);
    assert_eq!(edge_count(&h), before + 3, "notice {:?}", h.state().canvas.notice);
    // A generalization dragged out of an interface onto empty canvas makes its new supertype an
    // interface, named in place, as one step.
    let empty = egui::pos2(node(&h, &nodes["Catalog"]).x, node(&h, &payable).y);
    let from = node(&h, &payable);
    relate(&mut h, uml_relate::Relate::Generalization, from, empty);
    let rename = h.state().canvas.rename.clone().expect("its name is open for typing");
    assert_eq!(rename.text, "Interface 1", "notice {:?}", h.state().canvas.notice);
    assert_eq!(h.state().session.as_ref().unwrap().undo_label(), Some("Add interface"));
    let made = named(&h, archforge_uml::INTERFACE, "Interface 1");
    let related = {
        let s = h.state_mut().session.as_mut().unwrap();
        let id = made.to_string();
        s.execute("uml.relationship.list", &serde_json::json!({ "class": id })).unwrap().value["relationships"].clone()
    };
    assert_eq!(related[0]["kind"], "generalization", "{related}");
    assert_eq!(related[0]["general"], made.to_string());
    assert_eq!(edge_count(&h), before + 4);
}

/// The Relationship tool draws an interface realization from a class to an interface, dashed to a
/// hollow triangle, as one step, and says its direction; from anything but a class it starts no
/// line, and a drop on anything but an interface, or a second realization of the same interface, is
/// said at the canvas's top and draws nothing; dropped on empty canvas it makes an interface. Its
/// menu row sits after a separator; in light and dark.
#[test]
fn the_relationship_tool_draws_an_interface_realization_from_a_class_to_an_interface_and_refuses_the_rest() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, nodes, [payable, refundable]) = realization_sample(theme);
        let node = |h: &Harness<'static, Shell>, id: &str| rect_of(h, &format!("workspace.canvas.node.{id}")).center();
        let before = edge_count(&h);
        // The menu: Interface realization, apart from the others under a separator.
        let at = rect_of(&h, "workspace.tools.relationship-kinds").center();
        click(&mut h, at);
        let (dependency, realization) = (rect_of(&h, "workspace.relate.dependency"), rect_of(&h, "workspace.relate.realization"));
        assert!(realization.top() - dependency.bottom() > 8.0, "a separator between {dependency:?} and {realization:?}");
        {
            use egui_kittest::kittest::Queryable;
            assert!(h.query_by_label("Interface realization: from the implementing class to its interface").is_some(), "the row says its direction");
        }
        click(&mut h, realization.center());
        assert_eq!(uml_of(h.state()).relation, uml_relate::Relate::Realization);
        // The tool says which way it draws, in its tooltip and its accessible name.
        let hint = {
            use archforge_shell_api::kinds::DiagramKind;
            uml_ui::UmlKind.tool_hint(&archforge_ui_egui::testing::cx_ref(h.state()), "relationship")
        };
        assert_eq!(hint.as_deref(), Some("Interface realization: from the implementing class to its interface"));
        let relate = |h: &mut Harness<'static, Shell>, from: egui::Pos2, to: egui::Pos2| {
            h.state_mut().canvas.notice = None;
            let at = rect_of(h, "workspace.tools.relationship").center();
            click(h, at);
            drag(h, from, to);
        };
        // From the class OrderLine to the interface Payable: one line, one step, no name label.
        let (from, to) = (node(&h, &nodes["OrderLine"]), node(&h, &payable));
        relate(&mut h, from, to);
        assert_eq!(edge_count(&h), before + 1, "notice {:?}", h.state().canvas.notice);
        assert_eq!(h.state().session.as_ref().unwrap().undo_label(), Some("Add interface realization"));
        let made = related(&mut h, "OrderLine").into_iter().find(|r| r["kind"] == "realization").expect("the realization");
        let line = {
            let st = h.state().session.as_ref().unwrap().doc();
            let r = st.by_uuid(archforge_mof::Uuid::parse(made["id"].as_str().unwrap()).unwrap()).unwrap();
            let edge = archforge_diagram::views_of(st, r).1[0];
            assert!(st.list(edge, archforge_diagram::edge::LABEL).unwrap().is_empty(), "a realization's line has no name");
            archforge_uml::figures::line(st, r).unwrap()
        };
        assert_eq!(line.ends.map(|e| e.mark), [archforge_diagram::figure::EndMark::None, archforge_diagram::figure::EndMark::Triangle]);
        assert!(line.dashed);
        // Refused, said, and nothing changes: again to Payable, to a class, to a package-less spot of
        // another kind, and from an interface, which starts no line at all.
        let revision = h.state().session.as_ref().unwrap().revision();
        for (from, to, why) in [
            (nodes["OrderLine"].clone(), payable.clone(), "This class realizes that interface already"),
            (nodes["OrderLine"].clone(), nodes["Product"].clone(), "A class realizes only an interface"),
            (payable.clone(), refundable.clone(), "Only a class implements an interface"),
        ] {
            let (a, b) = (node(&h, &from), node(&h, &to));
            relate(&mut h, a, b);
            assert_eq!(h.state().canvas.notice.as_deref(), Some(why));
            assert_eq!((edge_count(&h), h.state().session.as_ref().unwrap().revision()), (before + 1, revision), "nothing changed: {why}");
        }
        assert!(h.state().canvas.gesture.is_none(), "no line was begun from the interface");
        // Several classes realize one interface.
        let (from, to) = (node(&h, &nodes["Order"]), node(&h, &payable));
        relate(&mut h, from, to);
        assert_eq!(edge_count(&h), before + 2, "notice {:?}", h.state().canvas.notice);
        // Dropped on empty canvas: a new interface there, its name open for typing, as one step.
        let empty = egui::pos2(node(&h, &nodes["Catalog"]).x, node(&h, &payable).y);
        let from = node(&h, &nodes["Customer"]);
        relate(&mut h, from, empty);
        let rename = h.state().canvas.rename.clone().expect("its name is open for typing");
        assert_eq!(rename.text, "Interface 1", "notice {:?}", h.state().canvas.notice);
        let new = named(&h, archforge_uml::INTERFACE, "Interface 1").to_string();
        let realized = related(&mut h, "Customer").into_iter().find(|r| r["kind"] == "realization").expect("Customer realizes it");
        assert_eq!(realized["contract"].as_str(), Some(new.as_str()));
        assert_eq!(edge_count(&h), before + 3);
        h.state_mut().session.as_mut().unwrap().undo().unwrap();
        h.run();
        assert_eq!(edge_count(&h), before + 2, "one step: the interface and its line went together");
    }
}

/// A selected realization's Properties name its implementing class and its interface, with no
/// name; its interface end drags onto another interface, and onto a class it is refused and said.
#[test]
fn a_realization_shows_its_class_and_interface_in_properties_and_its_ends_reconnect_by_dragging() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, nodes, [payable, refundable]) = realization_sample(theme);
        let id = {
            let s = h.state_mut().session.as_mut().unwrap();
            let st = s.doc();
            let at = |node: &str| {
                let n = st.by_uuid(archforge_mof::Uuid::parse(node).unwrap()).unwrap();
                st.uuid(st.get(n, archforge_diagram::node::ELEMENT).unwrap().as_ref().unwrap()).unwrap().to_string()
            };
            let (class, interface, d) = (at(&nodes["OrderLine"]), at(&payable), h.state().canvas.active.unwrap().to_string());
            let s = h.state_mut().session.as_mut().unwrap();
            let r = s.execute("uml.realization.add", &serde_json::json!({ "class": class, "interface": interface })).unwrap().value["id"]
                .as_str()
                .unwrap()
                .to_string();
            s.execute("diagram.connect", &serde_json::json!({ "diagram": d, "element": r, "source": nodes["OrderLine"], "target": payable }))
                .unwrap();
            s.execute("view.select", &serde_json::json!({ "ids": [r] })).unwrap();
            r
        };
        h.state_mut().state.show_panel(archforge_shell_api::state::Panel::Properties);
        h.run();
        let found = ids(&h);
        for want in ["workspace.properties.relationship.from", "workspace.properties.relationship.to"] {
            assert!(found.contains(&want.to_string()), "{want} in {found:?}");
        }
        assert!(!found.contains(&"workspace.properties.name".to_string()), "a realization has no name to edit");
        let shown = painted(&h);
        for label in [
            "Implementing class",
            "Realized interface",
            "OrderLine",
            "Payable",
            "Interface realization",
            "Drag the class end onto another class, the interface end onto another interface.",
        ] {
            assert!(shown.iter().any(|t| t == label), "{label} is shown in {shown:?}");
        }
        // The interface end dragged onto Refundable: the realization and its line move there.
        let handle = rect_of(&h, "workspace.canvas.line-end.target").center();
        let to = rect_of(&h, &format!("workspace.canvas.node.{refundable}")).center();
        drag(&mut h, handle, to);
        let now = related(&mut h, "OrderLine").into_iter().find(|r| r["kind"] == "realization").unwrap();
        let st = h.state().session.as_ref().unwrap().doc();
        let element = |node: &str| {
            let n = st.by_uuid(archforge_mof::Uuid::parse(node).unwrap()).unwrap();
            st.uuid(st.get(n, archforge_diagram::node::ELEMENT).unwrap().as_ref().unwrap()).unwrap().to_string()
        };
        assert_eq!(now["contract"].as_str(), Some(element(&refundable).as_str()), "notice {:?}", h.state().canvas.notice);
        // Onto a class it is refused, said at the canvas's top, and changes nothing.
        h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [id] })).unwrap();
        h.run();
        let revision = h.state().session.as_ref().unwrap().revision();
        let handle = rect_of(&h, "workspace.canvas.line-end.target").center();
        let to = rect_of(&h, &format!("workspace.canvas.node.{}", nodes["Product"])).center();
        drag(&mut h, handle, to);
        assert_eq!(h.state().canvas.notice.as_deref(), Some("A class realizes only an interface"));
        assert_eq!(h.state().session.as_ref().unwrap().revision(), revision);
    }
}

/// A dependency's end dragged onto another class moves there, the canvas sending the revision the
/// drag began on, which a dependency's reconnect needs, so a model no longer at it when the end is
/// dropped makes the drop stale; dragged onto the client and supplier another dependency has, one
/// with no line on the diagram, it is refused in words naming that one and nothing changes; in
/// light and dark (AR-697).
#[test]
fn a_dependency_end_dragged_onto_another_class_carries_the_revision_and_a_repeated_pair_is_refused() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, nodes) = related_sample_in(theme, egui::vec2(1600.0, 1000.0));
        let (dep, order, payment) = {
            let st = h.state().session.as_ref().unwrap().doc();
            let dep = st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_uml::DEPENDENCY)).unwrap();
            let named = |n: &str| st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_uml::CLASS) && archforge_core::name_of(st, *o) == n);
            let uuid = |o: archforge_mof::Obj| st.uuid(o).unwrap().to_string();
            (uuid(dep), uuid(named("Order").unwrap()), uuid(named("Payment").unwrap()))
        };
        let supplier = |h: &Harness<'static, Shell>| {
            let st = h.state().session.as_ref().unwrap().doc();
            let (_, to) = archforge_uml::figures::ends(st, st.by_uuid(archforge_mof::Uuid::parse(&dep).unwrap()).unwrap()).unwrap();
            archforge_core::name_of(st, to)
        };
        assert_eq!(supplier(&h), "PricingService", "{theme:?}");
        h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [dep] })).unwrap();
        h.run();
        let handle = rect_of(&h, "workspace.canvas.line-end.target").center();
        let to = rect_of(&h, &format!("workspace.canvas.node.{}", nodes["Product"])).center();
        drag(&mut h, handle, to);
        assert_eq!(supplier(&h), "Product", "{theme:?}: moved, notice {:?}", h.state().canvas.notice);
        assert_eq!(h.state().session.as_ref().unwrap().undo_label(), Some("Reconnect relationship"), "{theme:?}");
        // A change made while the end is dragged, as an agent's: the drop, on the revision the drag
        // began on, is refused as stale and changes nothing.
        let handle = rect_of(&h, "workspace.canvas.line-end.target").center();
        let to = rect_of(&h, &format!("workspace.canvas.node.{}", nodes["Customer"])).center();
        h.event(egui::Event::PointerMoved(handle));
        h.event(egui::Event::PointerButton { pos: handle, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE });
        h.run();
        h.event(egui::Event::PointerMoved(handle + egui::vec2(0.0, 40.0)));
        h.run();
        assert!(matches!(h.state().canvas.gesture, Some(archforge_shell_api::editing::Gesture::Reconnect { .. })), "{theme:?}: dragging the end");
        let s = h.state_mut().session.as_mut().unwrap();
        s.execute("uml.relationship.rename", &serde_json::json!({ "relationship": dep, "name": "prices" })).unwrap();
        let revision = s.revision();
        h.event(egui::Event::PointerMoved(to));
        h.run();
        h.event(egui::Event::PointerButton { pos: to, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE });
        h.run();
        let stale = "The model has changed since it was read, or another project was read. Read it again and retry. Nothing was changed.";
        assert_eq!(h.state().canvas.notice.as_deref(), Some(stale), "{theme:?}");
        assert_eq!((supplier(&h), h.state().session.as_ref().unwrap().revision()), ("Product".to_string(), revision), "{theme:?}: unchanged");
        // Another dependency from Order to Payment, with no line on this diagram: moving this one
        // onto that pair is refused, naming it.
        let s = h.state_mut().session.as_mut().unwrap();
        s.execute("uml.dependency.add", &serde_json::json!({ "client": order, "supplier": payment, "name": "pays" })).unwrap();
        s.execute("view.select", &serde_json::json!({ "ids": [dep] })).unwrap();
        h.run();
        let revision = h.state().session.as_ref().unwrap().revision();
        let handle = rect_of(&h, "workspace.canvas.line-end.target").center();
        let to = rect_of(&h, &format!("workspace.canvas.node.{}", nodes["Payment"])).center();
        drag(&mut h, handle, to);
        let said = "This connection already exists: the dependency pays. Select the existing relationship.";
        assert_eq!(h.state().canvas.notice.as_deref(), Some(said), "{theme:?}");
        assert_eq!((supplier(&h), h.state().session.as_ref().unwrap().revision()), ("Product".to_string(), revision), "{theme:?}: unchanged");
    }
}

/// The classifier tool places an enumeration named in place, as one step, its kind kept and its
/// face and tooltip naming it; the card says «enumeration» and takes literals, not attributes or
/// operations, from the canvas: its `+`, its adorner, its menu's row and ⌘↩ open a literal's draft,
/// ⇧⌘↩ nothing; in light and dark.
#[test]
fn the_classifier_tool_places_an_enumeration_named_in_place_whose_card_takes_literals() {
    use uml_classifier::Classifier;
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, nodes) = related_sample_in(theme, egui::vec2(1600.0, 1000.0));
        // The menu lists Enumeration after Interface; a click picks it and arms the tool.
        let at = rect_of(&h, "workspace.tools.classifier-kinds").center();
        click(&mut h, at);
        let (interface, enumeration) = (rect_of(&h, "workspace.classifier.interface"), rect_of(&h, "workspace.classifier.enumeration"));
        assert!(enumeration.top() >= interface.bottom(), "Enumeration under Interface");
        click(&mut h, enumeration.center());
        assert_eq!((h.state().canvas.popover, uml_of(h.state()).classifier), (None, Classifier::Enumeration));
        assert_eq!(h.state().canvas.tool, archforge_shell_api::canvas::Tool::Place(uml_classifier::TOOL));
        assert_eq!(classifier_hint(&h), "Enumeration", "the tooltip and accessible name name the kind");
        assert_eq!(*Classifier::Enumeration.tool_icon(), archforge_uml_ui::snapshot::icons::TOOL_ENUMERATION, "the face shows the kind's icon");
        // A click places one in the diagram's package, named in place, as one undo step.
        let at = empty_spot(&h, &nodes);
        click(&mut h, at);
        let rename = h.state().canvas.rename.clone().expect("its name is open for typing");
        assert_eq!(rename.text, "Enumeration 1");
        assert_eq!(h.state().session.as_ref().unwrap().undo_label(), Some("Add enumeration"));
        h.state_mut().canvas.rename.as_mut().unwrap().text = "OrderStatus".into();
        key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
        let status = named(&h, archforge_uml::ENUMERATION, "OrderStatus");
        let st = h.state().session.as_ref().unwrap().doc();
        let home = archforge_uml::commands::package_of(st, st.by_uuid(status).unwrap()).map(|p| archforge_core::name_of(st, p));
        assert_eq!(home.as_deref(), Some("Commerce"), "in the diagram's package");
        assert!(painted(&h).iter().any(|t| t == "«enumeration»"), "its eyebrow is its keyword, as written");
        assert_eq!(uml_of(h.state()).classifier, Classifier::Enumeration, "the kind stays for the window");
        // Selected, its card offers Connect and a literal: the adorner and its compartment's +,
        // no attribute or operation. (Selected by the model: a click where it was placed would
        // count as the placing click's second.)
        let node = node_of(&h, status);
        let at = rect_of(&h, &format!("workspace.canvas.node.{node}")).center();
        h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [status.to_string()] })).unwrap();
        h.run();
        let found = ids(&h);
        for id in ["workspace.canvas.adorn.connect", "workspace.canvas.adorn.add-literal", "workspace.canvas.enumeration.add-literal"] {
            assert!(found.iter().any(|i| i == id), "{id} in {found:?}");
        }
        for none in ["workspace.canvas.adorn.add-attribute", "workspace.canvas.class.add-attribute", "workspace.canvas.class.add-operation"] {
            assert!(!found.iter().any(|i| i == none), "{none} on an enumeration's card");
        }
        // ⇧⌘↩ opens nothing on it; ⌘↩ a literal's draft.
        h.state_mut().canvas.focused = true;
        key(&mut h, egui::Key::Enter, egui::Modifiers::COMMAND | egui::Modifiers::SHIFT);
        assert!(uml_of(h.state()).inline.is_none() && uml_of(h.state()).member.is_none(), "no operation's draft");
        key(&mut h, egui::Key::Enter, egui::Modifiers::COMMAND);
        let inline = uml_of(h.state()).inline.clone().expect("a literal's draft");
        assert_eq!((inline.edit.kind, inline.edit.target), (uml_panel::MemberKind::Literal, None));
        key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
        assert!(uml_of(h.state()).inline.is_none());
        // Its menu's row adds a literal; a class's adds an attribute.
        right_click(&mut h, at);
        assert!(painted(&h).iter().any(|t| t == "Add literal"), "the menu names the literal");
        let row = rect_of(&h, "workspace.canvas.menu.add-row").center();
        click(&mut h, row);
        assert_eq!(uml_of(h.state()).inline.as_ref().map(|i| i.edit.kind), Some(uml_panel::MemberKind::Literal));
        key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
        let at = rect_of(&h, &format!("workspace.canvas.node.{}", nodes["Order"])).center();
        right_click(&mut h, at);
        assert!(painted(&h).iter().any(|t| t == "Add attribute"), "a class's menu keeps its row");
    }
}

/// An enumeration's literals are typed on its card: its compartment's `+` and its adorner open a
/// name-only draft row at the end; Enter writes one, one undo step, and opens the next; Escape
/// drops the draft; a click away writes a draft with a name and drops an empty one; a name another
/// literal has, whatever the case, is refused under the row, the text kept and the model
/// untouched; a double click on a literal's row edits it in place. In light and dark.
#[test]
fn an_enumeration_takes_literals_typed_on_its_card_each_one_undo_step() {
    use uml_panel::MemberKind;
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, nodes, status, node) = literal_sample(theme, &[]);
        let revision = |h: &Harness<'static, Shell>| h.state().session.as_ref().unwrap().revision();
        // The compartment's + opens a draft row: a name, no type.
        let at = rect_of(&h, "workspace.canvas.enumeration.add-literal").center();
        click(&mut h, at);
        let inline = uml_of(h.state()).inline.clone().expect("a draft row");
        assert_eq!((inline.edit.kind, inline.edit.target, inline.edit.class), (MemberKind::Literal, None, status));
        let found = ids(&h);
        assert!(found.iter().any(|i| i == "workspace.canvas.member.name") && found.iter().any(|i| i == "workspace.canvas.member.details"));
        assert!(!found.iter().any(|i| i.starts_with("workspace.canvas.member.type")), "a literal has no type: {found:?}");
        // Enter writes it as one undo step and opens the next draft.
        h.event(egui::Event::Text("Placed".into()));
        h.run();
        key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
        assert_eq!(literal_names(&h, status), ["Placed"]);
        assert_eq!(h.state().session.as_ref().unwrap().undo_label(), Some("Add literal"), "one undo step");
        assert!(uml_of(h.state()).inline.as_ref().is_some_and(|i| i.edit.name.is_empty() && i.edit.target.is_none()), "the next draft");
        h.event(egui::Event::Text("Paid".into()));
        h.run();
        key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
        assert_eq!(literal_names(&h, status), ["Placed", "Paid"]);
        let written = revision(&h);
        // A name another literal has, whatever the case: refused under the row, the text kept.
        h.event(egui::Event::Text("placed".into()));
        h.run();
        key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
        let refused = uml_of(h.state()).inline.as_ref().and_then(|i| i.refused.clone()).expect("refused");
        assert_eq!(refused, "There is a literal named Placed already.");
        assert!(painted(&h).contains(&refused), "said under the row");
        assert_eq!(uml_of(h.state()).inline.as_ref().unwrap().edit.name, "placed", "the text stays");
        assert_eq!((literal_names(&h, status).len(), revision(&h)), (2, written), "nothing written");
        // Escape drops only the draft; undo takes back the literal written, redo brings it again.
        key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
        assert!(uml_of(h.state()).inline.is_none());
        assert_eq!(literal_names(&h, status), ["Placed", "Paid"]);
        h.state_mut().session.as_mut().unwrap().undo().unwrap();
        assert_eq!(literal_names(&h, status), ["Placed"]);
        h.state_mut().session.as_mut().unwrap().redo().unwrap();
        h.run();
        assert!(painted(&h).iter().any(|t| t == "Paid"), "the card shows it");
        // The adorner opens a draft; a click away writes one with a name.
        h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [status.to_string()] })).unwrap();
        h.run();
        let at = rect_of(&h, "workspace.canvas.adorn.add-literal").center();
        click(&mut h, at);
        assert_eq!(uml_of(h.state()).inline.as_ref().map(|i| i.edit.kind), Some(MemberKind::Literal));
        h.event(egui::Event::Text("Shipped".into()));
        h.run();
        let away = empty_spot(&h, &nodes);
        click(&mut h, away);
        assert!(uml_of(h.state()).inline.is_none());
        assert_eq!(literal_names(&h, status), ["Placed", "Paid", "Shipped"]);
        // An empty one only goes.
        h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [status.to_string()] })).unwrap();
        h.run();
        let written = revision(&h);
        let at = rect_of(&h, "workspace.canvas.enumeration.add-literal").center();
        click(&mut h, at);
        assert!(uml_of(h.state()).inline.is_some());
        let away = empty_spot(&h, &nodes);
        click(&mut h, away);
        assert!(uml_of(h.state()).inline.is_none());
        assert_eq!((literal_names(&h, status).len(), revision(&h)), (3, written), "nothing written");
        // The card grew to show every literal: no "more" row.
        assert!(!painted(&h).iter().any(|t| t.contains("more literal")), "{:?}", painted(&h));
        // A double click on a literal's row edits it in place: the same literal, renamed.
        let paid = literal_named(&h, status, "Paid");
        let at = card_row(&h, &node, 1);
        double_click(&mut h, at);
        let inline = uml_of(h.state()).inline.clone().expect("the row opens in place");
        assert_eq!((inline.edit.kind, inline.edit.target, inline.edit.name.as_str()), (MemberKind::Literal, Some(paid), "Paid"));
        uml_of_mut(h.state_mut()).inline.as_mut().unwrap().edit.name = "SHIPPED".into();
        h.run();
        key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
        assert_eq!(uml_of(h.state()).inline.as_ref().and_then(|i| i.refused.clone()).as_deref(), Some("There is a literal named Shipped already."));
        uml_of_mut(h.state_mut()).inline.as_mut().unwrap().edit.name = "Settled".into();
        h.run();
        key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
        assert!(uml_of(h.state()).inline.is_none(), "an edited row closes when written");
        assert_eq!(literal_names(&h, status), ["Placed", "Settled", "Shipped"]);
        assert_eq!(literal_named(&h, status, "Settled"), paid, "it keeps its identity");
        assert_eq!(h.state().session.as_ref().unwrap().undo_label(), Some("Rename literal"));
    }
}

/// A card that leaves members out (past `MAX_CARD_ROWS`) takes a new member in Properties' editor,
/// not as a draft row in the middle of its rows, from ⌘↩ and from its compartment's +, the right
/// side showing Properties; in light and dark.
#[test]
fn a_card_that_lists_only_some_of_its_members_adds_one_in_properties() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let mut h = workspace_harness(theme, egui::vec2(1600.0, 1000.0));
        archforge_ui_egui::snapshot::cx(h.state_mut(), uml_ui::open_sample).unwrap();
        let catalog = named(&h, archforge_uml::CLASS, "Catalog");
        {
            let s = h.state_mut().session.as_mut().unwrap();
            s.batch("Add attribute", |b| {
                for i in 0..archforge_uml::figures::MAX_CARD_ROWS {
                    b.execute("uml.attribute.add", &serde_json::json!({ "class": catalog.to_string(), "name": format!("a{i}") }))?;
                }
                Ok(())
            })
            .unwrap();
            s.execute("view.select", &serde_json::json!({ "ids": [catalog.to_string()] })).unwrap();
        }
        // At a zoom that shows the card's rows, its top in view, so only what it leaves out decides.
        h.run();
        let st = h.state().session.as_ref().unwrap().doc();
        let card = archforge_diagram::grid::bounds_of(st, st.by_uuid(archforge_mof::Uuid::parse(&node_of(&h, catalog)).unwrap()).unwrap());
        h.state_mut().canvas.fit_pending = false;
        h.state_mut().canvas.zoom = 1.0;
        h.state_mut().canvas.pan = egui::vec2(160.0 - card.x as f32, 120.0 - card.y as f32);
        h.state_mut().canvas.focused = true;
        h.state_mut().state.show_panel(archforge_shell_api::state::Panel::Skills);
        h.run();
        assert!(h.state().canvas.zoom >= archforge_shell_api::canvas::DETAIL_ZOOM, "{theme:?}: the card shows its rows");
        key(&mut h, egui::Key::Enter, egui::Modifiers::COMMAND);
        assert!(uml_of(h.state()).inline.is_none(), "{theme:?}: no draft row among the rows");
        assert!(uml_of(h.state()).member.as_ref().is_some_and(|e| e.class == catalog && e.target.is_none()), "{theme:?}: Properties' editor instead");
        assert_eq!(h.state().state.panel, archforge_shell_api::state::Panel::Properties, "{theme:?}: Properties shows the editor");
        // So does the attributes' +.
        uml_of_mut(h.state_mut()).member = None;
        h.state_mut().state.show_panel(archforge_shell_api::state::Panel::Skills);
        h.run();
        let at = rect_of(&h, "workspace.canvas.class.add-attribute").center();
        click(&mut h, at);
        assert!(uml_of(h.state()).inline.is_none(), "{theme:?}: no draft row from the +");
        let new = uml_of(h.state()).member.as_ref().map(|e| (e.class, e.target, e.kind));
        assert_eq!(new, Some((catalog, None, uml_panel::MemberKind::Attribute)), "{theme:?}: the +'s new attribute");
        assert_eq!(h.state().state.panel, archforge_shell_api::state::Panel::Properties, "{theme:?}: Properties shows it");
    }
}

/// Constraints (AR-133) in Properties and on the canvas: `+` in Constraints opens the editor, an
/// empty expression is refused in place with nothing written, Apply adds one in one undo step and
/// shows it on the open diagram as braces for a short expression; the pencil reopens it, a second
/// constrained element and Linked note make it a constraint note linked to both cards by dashes,
/// which says it is not evaluated; deleting a constrained element drops it and the toast says so, and a
/// constraint left naming nothing goes with it, said too; a double click on the note opens its
/// editor, Properties shown on the right side. In light and dark, every control found by its
/// automation id.
#[test]
fn constraints_are_added_in_properties_shown_as_braces_or_a_note_and_follow_a_deleted_element() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, nodes) = related_sample_in(theme, egui::vec2(1600.0, 1000.0));
        let order = named(&h, archforge_uml::CLASS, "Order");
        let payment = named(&h, archforge_uml::CLASS, "Payment");
        h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [order.to_string()] })).unwrap();
        h.state_mut().state.show_panel(archforge_shell_api::state::Panel::Properties);
        h.run();
        assert!(painted(&h).iter().any(|t| t == "Constraints · 0"), "the section shows on a class");
        let at = rect_of(&h, "workspace.properties.constraints.add").center();
        click(&mut h, at);
        let edit = uml_of(h.state()).constraint.clone().expect("the editor");
        assert_eq!((edit.target, edit.context, edit.elements.clone()), (None, order, vec![order]), "the selected element is constrained");
        assert_eq!(edit.name, "Constraint 1");
        let found = ids(&h);
        for id in ["name", "language", "language.menu", "expression", "element.0", "elements.add", "show.note", "show.braces", "cancel", "apply"] {
            assert!(found.iter().any(|i| *i == format!("workspace.properties.constraint.{id}")), "{id}");
        }
        assert!(!found.iter().any(|i| i == "workspace.properties.constraint.remove"), "a new constraint has nothing to remove");
        assert!(!found.iter().any(|i| i == "workspace.properties.constraint.element.0.remove"), "the last element stays");
        assert!(painted(&h).iter().any(|t| t.starts_with("Stored as written.")), "the editor says it is never evaluated");
        // An empty expression is refused in place; nothing is written.
        let revision = h.state().session.as_ref().unwrap().revision();
        let at = rect_of(&h, "workspace.properties.constraint.apply").center();
        click(&mut h, at);
        let refused = "Type an expression. A constraint without one states no rule.";
        assert_eq!(uml_of(h.state()).constraint.as_ref().and_then(|e| e.refused.clone()).as_deref(), Some(refused));
        assert!(painted(&h).iter().any(|t| t == refused), "said in the editor");
        assert_eq!(h.state().session.as_ref().unwrap().revision(), revision, "nothing written");
        // Typed and applied: one step, braces on the open diagram for a short expression.
        let edit = uml_of_mut(h.state_mut()).constraint.as_mut().unwrap();
        edit.name = "nonEmpty".into();
        edit.expression = "lines->notEmpty()".into();
        h.run();
        assert!(uml_of(h.state()).constraint.as_ref().unwrap().refused.is_none(), "the refusal goes once there is text");
        let at = rect_of(&h, "workspace.properties.constraint.apply").center();
        click(&mut h, at);
        assert!(uml_of(h.state()).constraint.is_none(), "{:?}", uml_of(h.state()).constraint);
        assert_eq!(constraint_rows(&h, "Order"), ["nonEmpty {lines->notEmpty()}"]);
        assert_eq!(h.state().session.as_ref().unwrap().undo_label(), Some("Add constraint"), "one undo step");
        h.run();
        let painted_now = painted(&h);
        assert!(painted_now.iter().any(|t| t == "{lines->notEmpty()}"), "braces by Order");
        assert!(painted_now.iter().any(|t| t == "Constraints · 1"));
        let (c, d) = {
            let st = h.state().session.as_ref().unwrap().doc();
            let c = archforge_uml::constraints::constraints_of(st, st.by_uuid(order).unwrap())[0];
            (st.uuid(c).unwrap(), st.by_uuid(h.state().canvas.active.unwrap()).unwrap())
        };
        let form = |h: &Harness<'static, Shell>| {
            let st = h.state().session.as_ref().unwrap().doc();
            archforge_uml::constraints::shown_on(st, d, st.by_uuid(c).unwrap()).map(|(n, f)| (st.uuid(n).unwrap(), f))
        };
        assert_eq!(form(&h).map(|f| f.1), Some(archforge_uml::constraints::Form::Braces));
        // The pencil reopens it; Payment picked from the element menu and Linked note make it a note.
        let at = rect_of(&h, &format!("workspace.properties.constraint.{c}.edit")).center();
        click(&mut h, at);
        assert_eq!(uml_of(h.state()).constraint.as_ref().map(|e| e.target), Some(Some(c)));
        let at = rect_of(&h, "workspace.properties.constraint.elements.add").center();
        click(&mut h, at);
        let menu: Vec<String> = ids(&h).into_iter().filter(|i| i.starts_with("workspace.properties.constraint.elements.menu.")).collect();
        assert!(!menu.is_empty(), "the model's elements are offered");
        let at = rect_of(&h, "workspace.properties.constraint.elements.menu.0").center();
        click(&mut h, at);
        assert_eq!(uml_of(h.state()).constraint.as_ref().unwrap().elements.len(), 2, "a pick adds it");
        assert!(ids(&h).iter().any(|i| i == "workspace.properties.constraint.element.1.remove"), "with two, each may go");
        uml_of_mut(h.state_mut()).constraint.as_mut().unwrap().elements = vec![order, payment];
        h.run();
        assert_eq!(uml_of(h.state()).constraint.as_ref().unwrap().elements, vec![order, payment]);
        let at = rect_of(&h, "workspace.properties.constraint.show.note").center();
        click(&mut h, at);
        uml_of_mut(h.state_mut()).constraint.as_mut().unwrap().expression = "payment.amount = total()\nwhen status = Paid".into();
        let at = rect_of(&h, "workspace.properties.constraint.apply").center();
        click(&mut h, at);
        assert_eq!(h.state().session.as_ref().unwrap().undo_label(), Some("Edit constraint"), "one undo step");
        let (note, f) = form(&h).expect("shown");
        assert_eq!(f, archforge_uml::constraints::Form::Note);
        h.run();
        let painted_now = painted(&h);
        assert!(painted_now.iter().any(|t| t == "CONSTRAINT · NOT EVALUATED"), "{painted_now:?}");
        assert!(painted_now.iter().any(|t| t == "{ payment.amount = total()"));
        assert!(ids(&h).contains(&format!("workspace.canvas.node.{note}")));
        // Its dashed links, in purple while it is not selected: a dash along the route from the
        // note's border to each of Order's and Payment's cards.
        let purple = egui::Color32::from(archforge_ui_egui::ux::palette(theme == ThemeKind::Dark).purple);
        let dashes: Vec<[egui::Pos2; 2]> = h
            .output()
            .shapes
            .iter()
            .filter_map(|c| match &c.shape {
                egui::Shape::LineSegment { points, stroke } if stroke.color == purple => Some(*points),
                _ => None,
            })
            .collect();
        let note_rect = rect_of(&h, &format!("workspace.canvas.node.{note}"));
        for name in ["Order", "Payment"] {
            let card = rect_of(&h, &format!("workspace.canvas.node.{}", nodes[name]));
            let (from, to) = (
                archforge_shell_api::canvas::exit_point(note_rect, card.center()),
                archforge_shell_api::canvas::exit_point(card, note_rect.center()),
            );
            let on_route = |q: egui::Pos2| {
                let along = ((q - from).dot(to - from) / (to - from).length_sq()).clamp(0.0, 1.0);
                (from + (to - from) * along).distance(q) < 0.5
            };
            assert!(dashes.iter().any(|[a, b]| on_route(*a) && on_route(*b)), "{theme:?}: a dash from the note to {name}'s card: {dashes:?}");
        }
        // A double click on the note opens its editor, in Properties brought to the right side.
        h.state_mut().state.show_panel(archforge_shell_api::state::Panel::Skills);
        h.state_mut().canvas.fit_pending = true;
        h.run();
        let at = rect_of(&h, &format!("workspace.canvas.node.{note}")).center();
        double_click(&mut h, at);
        assert_eq!(uml_of(h.state()).constraint.as_ref().map(|e| e.target), Some(Some(c)), "the note's double click opens it");
        assert_eq!(h.state().state.panel, archforge_shell_api::state::Panel::Properties, "{theme:?}: Properties shows the note's editor");
        let at = rect_of(&h, "workspace.properties.constraint.cancel").center();
        click(&mut h, at);
        // Deleting Payment drops it, one step, and the toast says what nonEmpty constrains now.
        let _ = &nodes;
        h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [payment.to_string()] })).unwrap();
        h.run();
        archforge_ui_egui::explorer::remove_selection(h.state_mut());
        h.run();
        if h.state().explorer.delete.is_some() {
            let at = rect_of(&h, "workspace.explorer.delete.confirm").center();
            click(&mut h, at);
        }
        let toast = h.state().canvas.toast.clone().expect("a toast").text;
        assert!(toast.ends_with("nonEmpty now constrains Order only"), "{toast}");
        assert_eq!(constraint_rows(&h, "Order"), ["nonEmpty {payment.amount = total() when status = Paid}"]);
        let st = h.state().session.as_ref().unwrap().doc();
        assert_eq!(archforge_uml::constraints::constrained(st, st.by_uuid(c).unwrap()), vec![st.by_uuid(order).unwrap()]);
        h.state_mut().session.as_mut().unwrap().undo().unwrap();
        h.run();
        // A constraint naming Payment alone goes with it, and the toast names it.
        h.state_mut()
            .session
            .as_mut()
            .unwrap()
            .execute("uml.constraint.set", &serde_json::json!({ "constraint": c.to_string(), "constrained": [payment.to_string()] }))
            .unwrap();
        h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [payment.to_string()] })).unwrap();
        h.run();
        archforge_ui_egui::explorer::remove_selection(h.state_mut());
        h.run();
        if h.state().explorer.delete.is_some() {
            let at = rect_of(&h, "workspace.explorer.delete.confirm").center();
            click(&mut h, at);
        }
        let toast = h.state().canvas.toast.clone().expect("a toast").text;
        assert!(toast.ends_with("nonEmpty constrained nothing else and went with it"), "{toast}");
        assert!(constraint_rows(&h, "Order").is_empty());
        h.state_mut().session.as_mut().unwrap().undo().unwrap();
        h.run();
        assert_eq!(constraint_rows(&h, "Order").len(), 1, "one undo step brings it back");
        // An association class's Properties end with its constraints too, as the class it is.
        let s = h.state_mut().session.as_mut().unwrap();
        let line = s
            .execute("uml.association-class.add", &serde_json::json!({ "from": order.to_string(), "to": payment.to_string(), "name": "Settlement" }))
            .unwrap()
            .value["id"]
            .as_str()
            .unwrap()
            .to_string();
        let rule = s
            .execute("uml.constraint.add", &serde_json::json!({ "context": line, "name": "settled", "expression": "amount > 0" }))
            .unwrap()
            .value["id"]
            .as_str()
            .unwrap()
            .to_string();
        s.execute("view.select", &serde_json::json!({ "ids": [line] })).unwrap();
        h.run();
        assert!(painted(&h).iter().any(|t| t == "Constraints · 1"), "the association class's section");
        // It follows the operations, at the foot of a long panel: scrolled into view.
        h.state_mut().panels.scroll = 10_000.0;
        h.run();
        let found = ids(&h);
        assert!(found.iter().any(|i| i == "workspace.properties.constraints.add"));
        assert!(found.iter().any(|i| *i == format!("workspace.properties.constraint.{rule}.edit")), "its row with its pencil");
    }
}

/// Models lists enumerations under their package with their literals below them; a literal row
/// renames in place and Delete removes it, each one undo step, and `+` on a selected enumeration
/// adds a literal named in place; `+` adds an enumeration, the search finds it, and it drags onto a
/// diagram or into another package.
#[test]
fn enumerations_are_listed_with_their_literals_edited_in_place_added_with_the_plus_found_and_dragged_onto_the_diagram() {
    use archforge_shell_api::notation_ui::NotationUi;
    let (mut h, nodes) = related_sample();
    let commerce = select_named(&mut h, archforge_uml::PACKAGE, "Commerce");
    let st = h.state().session.as_ref().unwrap().doc();
    let offers: Vec<&str> = uml_ui::UmlUi.offers(st, st.by_uuid(commerce)).into_iter().map(|o| o.key).collect();
    assert_eq!(offers, ["class", "interface", "enumeration", "data-type", "primitive-type", "package"]);
    archforge_ui_egui::explorer::add(h.state_mut(), "enumeration");
    let rename = h.state().explorer.rename.clone().expect("named in place");
    assert_eq!(rename.text, "Enumeration 1");
    h.state_mut().explorer.rename.as_mut().unwrap().text = "OrderStatus".into();
    key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
    h.run();
    let status = named(&h, archforge_uml::ENUMERATION, "OrderStatus");
    archforge_ui_egui::snapshot::cx(h.state_mut(), |cx| uml_ui::put_literals(cx, status, &["Placed", "Paid"])).unwrap();
    h.state_mut().explorer.expanded.insert(status);
    h.run();
    // Its row under Commerce, its literals below it, in their order.
    let rows = archforge_ui_egui::testing::models_rows(
        archforge_ui_egui::testing::notation_uis(h.state()),
        h.state().session.as_ref().unwrap().doc(),
        &h.state().explorer,
    );
    let at = rows.iter().position(|r| r.label == "OrderStatus").expect("listed under Commerce");
    assert_eq!((rows[at].kind, rows[at].depth, rows[at].expandable), ("enumeration", 1, true), "{:?}", tree_labels(&h));
    let literals: Vec<(&str, &str, u8)> = rows[at + 1..at + 3].iter().map(|r| (r.label.as_str(), r.kind, r.depth)).collect();
    assert_eq!(literals, [("Placed", "literal", 2), ("Paid", "literal", 2)]);
    // A literal row renames in place with a second click: the same literal, one undo step; a name
    // another literal has, whatever the case, is refused under the tree and changes nothing.
    let placed = rows[at + 1].id;
    h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [placed.to_string()] })).unwrap();
    h.state_mut().explorer.focused = true;
    h.run();
    let row = rect_of(&h, &format!("workspace.explorer.row.{placed}")).center();
    click(&mut h, row);
    let rename = h.state().explorer.rename.clone().expect("a second click opens the literal's name");
    assert_eq!((rename.target, rename.text.as_str()), (placed, "Placed"));
    h.state_mut().explorer.rename.as_mut().unwrap().text = "PAID".into();
    key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
    let refused = h.state().explorer.rename.as_ref().and_then(|r| r.refused.clone()).expect("refused");
    assert_eq!(refused, "There is a literal named Paid already.", "said as the card and Properties say it");
    assert_eq!(literal_names(&h, status), ["Placed", "Paid"], "nothing changed");
    // F2 opens it again, with its name as it is.
    h.state_mut().explorer.rename = None;
    h.state_mut().explorer.focused = true;
    key(&mut h, egui::Key::F2, egui::Modifiers::NONE);
    assert_eq!(h.state().explorer.rename.as_ref().map(|r| (r.target, r.text.as_str())), Some((placed, "Placed")));
    h.state_mut().explorer.rename.as_mut().unwrap().text = "Ordered".into();
    h.run();
    key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
    assert!(h.state().explorer.rename.is_none(), "{:?}", h.state().explorer.rename);
    assert_eq!(literal_names(&h, status), ["Ordered", "Paid"]);
    assert_eq!(literal_named(&h, status, "Ordered"), placed, "the same literal");
    assert_eq!(h.state().session.as_ref().unwrap().undo_label(), Some("Rename literal"));
    // + on a selected literal (or its enumeration) offers a literal first and adds one at its
    // enumeration's end, named in place.
    let st = h.state().session.as_ref().unwrap().doc();
    let offers: Vec<&str> = uml_ui::UmlUi.offers(st, st.by_uuid(placed)).into_iter().map(|o| o.key).collect();
    assert_eq!(offers, ["literal", "class", "interface", "enumeration", "data-type", "primitive-type", "package"]);
    archforge_ui_egui::explorer::add(h.state_mut(), "literal");
    let rename = h.state().explorer.rename.clone().expect("the new literal's name is open");
    assert_eq!(rename.text, "Literal 1");
    h.state_mut().explorer.rename.as_mut().unwrap().text = "Shipped".into();
    key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
    assert_eq!(literal_names(&h, status), ["Ordered", "Paid", "Shipped"]);
    // Delete removes a literal, one undo step, and no notice keeps it.
    let shipped = literal_named(&h, status, "Shipped");
    h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [shipped.to_string()] })).unwrap();
    archforge_ui_egui::explorer::remove_selection(h.state_mut());
    h.run();
    assert_eq!(h.state().explorer.notice, None);
    assert_eq!(literal_names(&h, status), ["Ordered", "Paid"]);
    h.state_mut().session.as_mut().unwrap().undo().unwrap();
    assert_eq!(literal_names(&h, status), ["Ordered", "Paid", "Shipped"]);
    h.state_mut().session.as_mut().unwrap().undo().unwrap();
    assert_eq!(literal_names(&h, status), ["Ordered", "Paid", "Literal 1"], "the name typed was a step of its own");
    h.state_mut().session.as_mut().unwrap().undo().unwrap();
    assert_eq!(literal_names(&h, status), ["Ordered", "Paid"], "and the add one too");
    h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [placed.to_string()] })).unwrap();
    h.run();
    // Selected, it shows its enumeration's panel.
    h.state_mut().state.show_panel(archforge_shell_api::state::Panel::Properties);
    h.run();
    let texts = painted(&h);
    for want in ["Literal · model", "Literals · 2", "Enumeration"] {
        assert!(texts.iter().any(|t| t == want), "{want} in {texts:?}");
    }
    // The search finds it as an enumeration of Commerce.
    let index = archforge_ui_egui::testing::search_index(
        archforge_ui_egui::testing::kinds(h.state()),
        archforge_ui_egui::testing::notation_uis(h.state()),
        h.state().session.as_ref().unwrap().doc(),
    );
    let hit =
        archforge_ui_egui::testing::search_hits(&index, "orderstatus").1.into_iter().find(|e| e.label == "OrderStatus").map(|e| e.detail.clone());
    assert_eq!(hit.as_deref(), Some("Enumeration · Commerce"));
    // Dragged from Models onto the diagram: its card, as one undo step; a second drag says so.
    let nodes_now = |h: &Harness<'static, Shell>| {
        let st = h.state().session.as_ref().unwrap().doc();
        st.list(st.by_uuid(h.state().canvas.active.unwrap()).unwrap(), archforge_diagram::diagram::NODE).unwrap().len()
    };
    let before = nodes_now(&h);
    let from = rect_of(&h, &format!("workspace.explorer.row.{status}")).center();
    let to = empty_spot(&h, &nodes);
    drag(&mut h, from, to);
    assert_eq!(nodes_now(&h), before + 1, "notice {:?}", h.state().canvas.notice);
    assert_eq!(h.state().session.as_ref().unwrap().undo_label(), Some("Place enumeration"));
    let from = rect_of(&h, &format!("workspace.explorer.row.{status}")).center();
    drag(&mut h, from, to);
    assert_eq!(h.state().canvas.notice.as_deref(), Some("Already on this diagram"));
    // Dragged onto another package's row, it moves there with its literals.
    let billing = {
        let s = h.state_mut().session.as_mut().unwrap();
        let out = s.execute("uml.package.add", &serde_json::json!({ "name": "Billing", "parent": commerce.to_string() })).unwrap();
        archforge_mof::Uuid::parse(out.value["id"].as_str().unwrap()).unwrap()
    };
    h.run();
    let from = rect_of(&h, &format!("workspace.explorer.row.{status}")).center();
    let to = rect_of(&h, &format!("workspace.explorer.row.{billing}")).center();
    drag(&mut h, from, to);
    let st = h.state().session.as_ref().unwrap().doc();
    let e = st.by_uuid(status).unwrap();
    assert_eq!(archforge_uml::commands::package_of(st, e), st.by_uuid(billing), "{:?}", h.state().explorer.notice);
    assert_eq!(archforge_uml::commands::literals(st, e).len(), 2);
    assert_eq!(h.state().session.as_ref().unwrap().undo_label(), Some("Move enumeration"));
}

/// The Relationship tool joins no association to or from an enumeration, saying so at the canvas's
/// top; an enumeration specializes only an enumeration, the refusals said too, and a generalization
/// dragged out of one onto empty canvas makes an enumeration; a dependency may touch it, and drawn
/// again it is not repeated, nor added to where several join the two already (AR-699); an
/// association's end dragged onto one is refused; in light and dark.
#[test]
fn the_relationship_tool_joins_no_association_to_an_enumeration_and_specializes_one_only_by_an_enumeration() {
    use uml_relate::Relate;
    const NO_ASSOCIATION: &str = "An enumeration is a data type and joins no association";
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, nodes, [status, closing]) = enumeration_sample(theme);
        let node = |h: &Harness<'static, Shell>, id: &str| rect_of(h, &format!("workspace.canvas.node.{id}")).center();
        let relate = |h: &mut Harness<'static, Shell>, kind: Relate, from: egui::Pos2, to: egui::Pos2| {
            uml_of_mut(h.state_mut()).relation = kind;
            h.state_mut().canvas.notice = None;
            let at = rect_of(h, "workspace.tools.relationship").center();
            click(h, at);
            drag(h, from, to);
        };
        let before = edge_count(&h);
        // An enumeration specializes an enumeration, and a class depends on one: a line each.
        let (from, to) = (node(&h, &closing), node(&h, &status));
        relate(&mut h, Relate::Generalization, from, to);
        assert_eq!(edge_count(&h), before + 1, "notice {:?}", h.state().canvas.notice);
        assert_eq!(h.state().session.as_ref().unwrap().undo_label(), Some("Add generalization"));
        let (from, to) = (node(&h, &nodes["OrderLine"]), node(&h, &status));
        relate(&mut h, Relate::Dependency, from, to);
        assert_eq!(edge_count(&h), before + 2, "notice {:?}", h.state().canvas.notice);
        // Drawn again, the dependency there is reused, never repeated: the diagram shows it already,
        // which is said, and nothing changes (AR-699).
        let dependencies = |h: &Harness<'static, Shell>| {
            let doc = h.state().session.as_ref().unwrap().doc();
            doc.alive().filter(|o| doc.class_of(*o).ok() == Some(archforge_uml::DEPENDENCY)).count()
        };
        let [line, order, order_status] =
            [(archforge_uml::CLASS, "OrderLine"), (archforge_uml::CLASS, "Order"), (archforge_uml::ENUMERATION, "OrderStatus")]
                .map(|(k, n)| named(&h, k, n));
        let drawn = |h: &Harness<'static, Shell>| {
            let st = h.state().session.as_ref().unwrap().doc();
            let kind = archforge_uml::matches::Kind::Dependency;
            let [dependency] = archforge_uml::matches::existing_matches(st, kind, st.by_uuid(line).unwrap(), st.by_uuid(order_status).unwrap())[..]
            else {
                panic!("one dependency from OrderLine to OrderStatus")
            };
            let edges = st.list(st.by_uuid(h.state().canvas.active.unwrap()).unwrap(), archforge_diagram::diagram::EDGE).unwrap();
            let shows = |e: &archforge_mof::Obj| st.get(*e, archforge_diagram::edge::ELEMENT).ok().and_then(|v| v.as_ref()) == Some(dependency);
            (st.uuid(dependency).unwrap(), edges.iter().filter_map(|v| v.as_ref()).find(shows).map(|e| st.uuid(e).unwrap()))
        };
        let (count, revision) = (dependencies(&h), h.state().session.as_ref().unwrap().revision());
        relate(&mut h, Relate::Dependency, from, to);
        let shown = format!("This diagram shows {} already.", drawn(&h).0);
        assert_eq!(h.state().canvas.notice.as_deref(), Some(shown.as_str()), "{theme:?}: said");
        assert_eq!((dependencies(&h), edge_count(&h), h.state().session.as_ref().unwrap().revision()), (count, before + 2, revision), "{theme:?}");
        // With no line on this diagram, as on another that shows both ends, its line is added for
        // the same dependency, one undo step that Undo takes back alone, the dependency kept.
        let (dependency, edge) = drawn(&h);
        let edge = edge.expect("the dependency's line");
        archforge_ui_egui::snapshot::run(h.state_mut(), "diagram.disconnect", serde_json::json!({ "edge": edge.to_string() })).unwrap();
        h.run();
        assert_eq!((dependencies(&h), edge_count(&h), drawn(&h)), (count, before + 1, (dependency, None)), "{theme:?}: the line alone gone");
        relate(&mut h, Relate::Dependency, from, to);
        assert_eq!(h.state().canvas.notice, None, "{theme:?}");
        let (again, added) = drawn(&h);
        assert_eq!(
            (dependencies(&h), edge_count(&h), again, added.is_some()),
            (count, before + 2, dependency, true),
            "{theme:?}: its line, no new dependency"
        );
        h.state_mut().session.as_mut().unwrap().undo().unwrap();
        h.run();
        assert_eq!((dependencies(&h), edge_count(&h)), (count, before + 1), "{theme:?}: Undo takes the line back alone");
        h.state_mut().session.as_mut().unwrap().redo().unwrap();
        h.run();
        // Two from OrderLine to OrderStatus, as a document of an earlier version may hold: drawn
        // again, neither is added to, which is said, and nothing changes.
        let s = h.state_mut().session.as_mut().unwrap();
        let twin = s.execute("uml.dependency.add", &serde_json::json!({ "client": line.to_string(), "supplier": order.to_string() })).unwrap();
        let twin = archforge_mof::Uuid::parse(twin.value["id"].as_str().unwrap()).unwrap();
        let (tables, junctions) = archforge_notations::NOTATIONS.storage();
        let old = h.state_mut().session.take().unwrap();
        let rows = archforge_mof::rows::snapshot(old.doc(), &tables, &junctions).unwrap();
        let mut n = 0u64;
        let mint = move || {
            n += 1;
            let mut r = [0u8; 10];
            r[2..].copy_from_slice(&n.to_be_bytes());
            archforge_mof::Uuid::v7(1_761_670_900_000, r)
        };
        let mut st =
            archforge_mof::rows::restore_into_empty(archforge_notations::NOTATIONS.store(Box::new(mint)).unwrap(), &rows, &tables, &junctions)
                .unwrap();
        let (d, was, now) = (st.by_uuid(twin).unwrap(), st.by_uuid(order).unwrap(), st.by_uuid(order_status).unwrap());
        st.remove_value(d, archforge_uml::dependency::SUPPLIER, &archforge_mof::Value::Ref(was)).unwrap();
        st.push(d, archforge_uml::dependency::SUPPLIER, archforge_mof::Value::Ref(now)).unwrap();
        let parallel = archforge_uml::matches::existing_matches(&st, archforge_uml::matches::Kind::Dependency, st.by_uuid(line).unwrap(), now);
        assert_eq!(parallel.len(), 2, "{theme:?}: two parallel dependencies loaded");
        let view = old.view().clone();
        h.state_mut().session = Some(archforge_engine::Session::with_view(
            archforge_notations::NOTATIONS.registry().unwrap(),
            st,
            old.counter(),
            archforge_engine::Options::default(),
            view,
        ));
        h.run();
        let (count, revision) = (dependencies(&h), h.state().session.as_ref().unwrap().revision());
        relate(&mut h, Relate::Dependency, from, to);
        let several = "Several dependencies already join these two elements in this direction, so none was added.";
        assert_eq!(h.state().canvas.notice.as_deref(), Some(several), "{theme:?}");
        assert_eq!((dependencies(&h), edge_count(&h), h.state().session.as_ref().unwrap().revision()), (count, before + 2, revision), "{theme:?}");
        // Refused, said, and nothing changes.
        let revision = h.state().session.as_ref().unwrap().revision();
        for (kind, from, to, why) in [
            (Relate::Association, nodes["OrderLine"].clone(), status.clone(), NO_ASSOCIATION),
            (Relate::Composition, nodes["Order"].clone(), status.clone(), NO_ASSOCIATION),
            (Relate::Directed, status.clone(), nodes["OrderLine"].clone(), NO_ASSOCIATION),
            (Relate::Generalization, nodes["OrderLine"].clone(), status.clone(), "Only an enumeration specializes an enumeration"),
            (Relate::Generalization, status.clone(), nodes["Order"].clone(), "An enumeration specializes only an enumeration"),
            (Relate::Realization, status.clone(), closing.clone(), "Only a class implements an interface"),
        ] {
            let (a, b) = (node(&h, &from), node(&h, &to));
            relate(&mut h, kind, a, b);
            // Said in the user's words, as the catalogue has them, not as the engine words it.
            let notice = h.state().canvas.notice.clone().expect("the refusal is said");
            assert_eq!(notice, why, "{kind:?}");
            assert_eq!((edge_count(&h), h.state().session.as_ref().unwrap().revision()), (before + 2, revision), "nothing changed: {why}");
        }
        assert!(h.state().canvas.gesture.is_none(), "no line was begun from the enumeration");
        // An association dragged out of an enumeration onto empty canvas is refused before it starts.
        let empty = egui::pos2(node(&h, &nodes["Catalog"]).x, node(&h, &status).y);
        let from = node(&h, &status);
        relate(&mut h, Relate::Association, from, empty);
        assert_eq!(h.state().canvas.notice.as_deref(), Some(NO_ASSOCIATION));
        assert_eq!(h.state().session.as_ref().unwrap().revision(), revision);
        // A generalization dragged out of one onto empty canvas makes an enumeration there, as one step.
        relate(&mut h, Relate::Generalization, from, empty);
        let rename = h.state().canvas.rename.clone().expect("its name is open for typing");
        assert_eq!(rename.text, "Enumeration 1", "notice {:?}", h.state().canvas.notice);
        let made = named(&h, archforge_uml::ENUMERATION, "Enumeration 1");
        assert_eq!(h.state().session.as_ref().unwrap().undo_label(), Some("Add enumeration"));
        assert_eq!(edge_count(&h), before + 3);
        let general = {
            let st = h.state().session.as_ref().unwrap().doc();
            let s = st.by_uuid(made).unwrap();
            archforge_uml::relations::relationships_of(st, s)
                .into_iter()
                .filter_map(|r| archforge_uml::figures::ends(st, r))
                .any(|(specific, general)| general == s && st.uuid(specific).ok().is_some_and(|u| node_of(&h, u) == status))
        };
        assert!(general, "OrderStatus specializes the new enumeration");
        h.state_mut().canvas.rename = None;
        // An association's end dragged onto an enumeration is refused and said.
        let line = {
            let d = h.state().canvas.active.unwrap().to_string();
            let s = h.state_mut().session.as_mut().unwrap();
            let st = s.doc();
            let element = |node: &str| {
                let n = st.by_uuid(archforge_mof::Uuid::parse(node).unwrap()).unwrap();
                st.uuid(st.get(n, archforge_diagram::node::ELEMENT).unwrap().as_ref().unwrap()).unwrap().to_string()
            };
            let (from, to) = (element(&nodes["OrderLine"]), element(&nodes["Product"]));
            let a =
                s.execute("uml.association.add", &serde_json::json!({ "from": from, "to": to })).unwrap().value["id"].as_str().unwrap().to_string();
            s.execute(
                "diagram.connect",
                &serde_json::json!({ "diagram": d, "element": a, "source": nodes["OrderLine"], "target": nodes["Product"] }),
            )
            .unwrap();
            s.execute("view.select", &serde_json::json!({ "ids": [a] })).unwrap();
            a
        };
        h.run();
        let revision = h.state().session.as_ref().unwrap().revision();
        let handle = rect_of(&h, "workspace.canvas.line-end.target").center();
        let to = node(&h, &status);
        drag(&mut h, handle, to);
        assert_eq!(h.state().canvas.notice.as_deref(), Some(NO_ASSOCIATION), "{line}");
        assert_eq!(h.state().session.as_ref().unwrap().revision(), revision, "nothing changed");
        // A generalization's end dragged onto a class is refused in the user's words too.
        let generalization = {
            let st = h.state().session.as_ref().unwrap().doc();
            let s = st.by_uuid(archforge_mof::Uuid::parse(&closing).unwrap()).unwrap();
            let closing_element = st.get(s, archforge_diagram::node::ELEMENT).unwrap().as_ref().unwrap();
            let g = archforge_uml::relations::relationships_of(st, closing_element)
                .into_iter()
                .find(|r| st.class_of(*r).ok() == Some(archforge_uml::GENERALIZATION))
                .unwrap();
            st.uuid(g).unwrap().to_string()
        };
        h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [generalization] })).unwrap();
        h.run();
        let handle = rect_of(&h, "workspace.canvas.line-end.target").center();
        let to = node(&h, &nodes["Order"]);
        drag(&mut h, handle, to);
        assert_eq!(h.state().canvas.notice.as_deref(), Some("An enumeration specializes only an enumeration"));
        assert_eq!(h.state().session.as_ref().unwrap().revision(), revision, "nothing changed");
    }
}

/// A line the engine refuses on a class diagram is said at the canvas's top in plain words and in
/// the chosen language, never with the command's id, the parameter it is about or the engine's
/// English: a class drawn depending on itself, in English and German. Nothing changes. In light and
/// dark (AR-431).
#[test]
fn a_relationship_refused_on_a_class_diagram_says_why_in_plain_words() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        for language in ["en", "de"] {
            let (mut h, nodes, _) = enumeration_sample(theme);
            h.state_mut().state.language = language.into();
            h.run();
            let node = archforge_mof::Uuid::parse(&nodes["OrderLine"]).unwrap();
            let d = h.state().canvas.active.unwrap();
            let d = h.state().session.as_ref().unwrap().doc().by_uuid(d).unwrap();
            let revision = h.state().session.as_ref().unwrap().revision();
            uml_of_mut(h.state_mut()).relation = uml_relate::Relate::Dependency;
            archforge_ui_egui::snapshot::cx(h.state_mut(), |cx| uml_relate::connect(cx, d, node, node));
            h.run();
            // The sentence as the catalogue words it: the kind, with a capital, in its template.
            let class = archforge_ui_egui::i18n::tr_ctx("refusal", "a class");
            let class: String = class.chars().take(1).flat_map(char::to_uppercase).chain(class.chars().skip(1)).collect();
            let said = archforge_ui_egui::i18n::fmt(&archforge_ui_egui::i18n::tr("{what} cannot depend on itself."), &[("what", &class)]);
            if language == "de" {
                assert_ne!(said, "A class cannot depend on itself.", "{theme:?}: the catalogue has it in German");
            }
            let notice = h.state().canvas.notice.clone().unwrap_or_else(|| panic!("{theme:?}, {language}: the refusal is said"));
            assert_eq!(notice, said, "{theme:?}, {language}");
            assert!(painted(&h).contains(&notice), "{theme:?}, {language}: shown at the canvas's top");
            assert_eq!(h.state().session.as_ref().unwrap().revision(), revision, "{theme:?}, {language}: nothing changed");
        }
    }
    archforge_ui_egui::i18n::set_language("en");
}

/// The classifier tool places a data type named in place, as one step, its kind kept and its face,
/// tooltip and accessible name naming it; the card says «dataType» and takes attributes and
/// operations from the canvas as a class's does: its compartments' `+`, its adorner, its menu's
/// row, ⌘↩ and ⇧⌘↩ open a draft row, and Enter writes one, one undo step; in light and dark.
#[test]
fn the_classifier_tool_places_a_data_type_named_in_place_whose_card_takes_attributes_and_operations() {
    use uml_classifier::Classifier;
    use uml_panel::MemberKind;
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, nodes) = related_sample_in(theme, egui::vec2(1600.0, 1000.0));
        // The menu lists Data type after Enumeration, named for assistive technology; the arrows
        // walk to it and Enter picks it, arming the tool.
        let at = rect_of(&h, "workspace.tools.classifier-kinds").center();
        click(&mut h, at);
        let (enumeration, data_type) = (rect_of(&h, "workspace.classifier.enumeration"), rect_of(&h, "workspace.classifier.data-type"));
        assert!(data_type.top() >= enumeration.bottom(), "Data type under Enumeration");
        {
            use egui_kittest::kittest::Queryable;
            assert!(h.query_by_label("Data type").is_some(), "the row's accessible name");
        }
        for _ in 0..3 {
            key(&mut h, egui::Key::ArrowDown, egui::Modifiers::NONE);
        }
        key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
        assert_eq!((h.state().canvas.popover, uml_of(h.state()).classifier), (None, Classifier::DataType));
        assert_eq!(h.state().canvas.tool, archforge_shell_api::canvas::Tool::Place(uml_classifier::TOOL));
        assert_eq!(classifier_hint(&h), "Data type", "the tooltip and accessible name name the kind");
        assert_eq!(*Classifier::DataType.tool_icon(), archforge_uml_ui::snapshot::icons::TOOL_DATA_TYPE, "the face shows the kind's icon");
        // A click places one in the diagram's package, named in place, as one undo step.
        let at = empty_spot(&h, &nodes);
        click(&mut h, at);
        let rename = h.state().canvas.rename.clone().expect("its name is open for typing");
        assert_eq!(rename.text, "DataType 1");
        assert_eq!(h.state().session.as_ref().unwrap().undo_label(), Some("Add data type"));
        h.state_mut().canvas.rename.as_mut().unwrap().text = "Money".into();
        key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
        let money = named(&h, archforge_uml::DATA_TYPE, "Money");
        let st = h.state().session.as_ref().unwrap().doc();
        assert_eq!(st.class_of(st.by_uuid(money).unwrap()).ok(), Some(archforge_uml::DATA_TYPE), "a DataType, not a renamed class");
        let home = archforge_uml::commands::package_of(st, st.by_uuid(money).unwrap()).map(|p| archforge_core::name_of(st, p));
        assert_eq!(home.as_deref(), Some("Commerce"), "in the diagram's package");
        assert!(painted(&h).iter().any(|t| t == "«dataType»"), "its eyebrow is its keyword, as written");
        assert_eq!(uml_of(h.state()).classifier, Classifier::DataType, "the kind stays for the window");
        // Selected, its card offers an attribute and an operation, as a class's does, and no literal.
        let node = node_of(&h, money);
        let at = rect_of(&h, &format!("workspace.canvas.node.{node}")).center();
        h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [money.to_string()] })).unwrap();
        h.run();
        let found = ids(&h);
        for id in [
            "workspace.canvas.adorn.connect",
            "workspace.canvas.adorn.add-attribute",
            "workspace.canvas.class.add-attribute",
            "workspace.canvas.class.add-operation",
        ] {
            assert!(found.iter().any(|i| i == id), "{id} in {found:?}");
        }
        for none in ["workspace.canvas.adorn.add-literal", "workspace.canvas.enumeration.add-literal"] {
            assert!(!found.iter().any(|i| i == none), "{none} on a data type's card");
        }
        // The attributes' + opens a draft row; Enter writes it as one step and opens the next.
        let at_plus = rect_of(&h, "workspace.canvas.class.add-attribute").center();
        click(&mut h, at_plus);
        let inline = uml_of(h.state()).inline.clone().expect("a draft row");
        assert_eq!((inline.edit.kind, inline.edit.target, inline.edit.class), (MemberKind::Attribute, None, money));
        h.event(egui::Event::Text("amount".into()));
        h.run();
        key(&mut h, egui::Key::Tab, egui::Modifiers::NONE);
        h.event(egui::Event::Text("Real".into()));
        h.run();
        key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
        assert!(uml_of(h.state()).inline.is_some(), "the next draft opens");
        key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
        assert_eq!(member_rows(&h, money, false), ["+ amount : Real"]);
        assert_eq!(h.state().session.as_ref().unwrap().undo_label(), Some("Add attribute"));
        assert!(painted(&h).iter().any(|t| t == "+ amount : Real"), "the card shows it");
        // ⇧⌘↩ opens an operation's draft, ⌘↩ an attribute's; the adorner and the menu's row an attribute's.
        // With Alt or a Mac's Control more neither chord opens anything.
        h.state_mut().canvas.focused = true;
        let (mac, win, shift) =
            (egui::Modifiers::MAC_CMD | egui::Modifiers::COMMAND, egui::Modifiers::CTRL | egui::Modifiers::COMMAND, egui::Modifiers::SHIFT);
        let (alt, ctrl) = (egui::Modifiers::ALT, egui::Modifiers::CTRL);
        for other in [mac | alt, mac | ctrl, win | alt, mac | shift | alt, mac | shift | ctrl, win | shift | alt] {
            key(&mut h, egui::Key::Enter, other);
            assert!(uml_of(h.state()).inline.is_none(), "{theme:?}: {other:?}");
        }
        // Each chord in each shape, over the two appearances.
        let (shape, other_shape) = if theme == ThemeKind::Light { (mac, win) } else { (win, mac) };
        key(&mut h, egui::Key::Enter, shape | shift);
        assert_eq!(uml_of(h.state()).inline.as_ref().map(|i| (i.edit.kind, i.edit.class)), Some((MemberKind::Operation, money)));
        h.event(egui::Event::Text("add".into()));
        h.run();
        key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
        key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
        assert_eq!(member_rows(&h, money, true), ["+ add()"]);
        h.state_mut().canvas.focused = true;
        key(&mut h, egui::Key::Enter, other_shape);
        assert_eq!(uml_of(h.state()).inline.as_ref().map(|i| i.edit.kind), Some(MemberKind::Attribute));
        key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
        let adorner = rect_of(&h, "workspace.canvas.adorn.add-attribute").center();
        click(&mut h, adorner);
        assert_eq!(uml_of(h.state()).inline.as_ref().map(|i| i.edit.kind), Some(MemberKind::Attribute));
        key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
        right_click(&mut h, at);
        assert!(painted(&h).iter().any(|t| t == "Add attribute"), "the menu names the attribute");
        let row = rect_of(&h, "workspace.canvas.menu.add-row").center();
        click(&mut h, row);
        assert_eq!(uml_of(h.state()).inline.as_ref().map(|i| i.edit.kind), Some(MemberKind::Attribute));
        key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
        // Each written member is one step: undone, the card is as it was.
        h.state_mut().session.as_mut().unwrap().undo().unwrap();
        h.run();
        assert_eq!(member_rows(&h, money, true), Vec::<String>::new());
        assert_eq!(member_rows(&h, money, false), ["+ amount : Real"]);
    }
}

/// Models lists data types under their package with their attributes and operations below them;
/// `+` adds one, named in place, the search finds it, and it drags onto a diagram or into another
/// package; in light and dark.
#[test]
fn data_types_are_listed_with_their_members_added_with_the_plus_found_and_dragged_onto_the_diagram() {
    use archforge_shell_api::notation_ui::NotationUi;
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, nodes) = related_sample_in(theme, egui::vec2(1600.0, 1000.0));
        let commerce = select_named(&mut h, archforge_uml::PACKAGE, "Commerce");
        let st = h.state().session.as_ref().unwrap().doc();
        let offers: Vec<&str> = uml_ui::UmlUi.offers(st, st.by_uuid(commerce)).into_iter().map(|o| o.key).collect();
        assert_eq!(offers, ["class", "interface", "enumeration", "data-type", "primitive-type", "package"]);
        archforge_ui_egui::explorer::add(h.state_mut(), "data-type");
        let rename = h.state().explorer.rename.clone().expect("named in place");
        assert_eq!(rename.text, "DataType 1");
        h.state_mut().explorer.rename.as_mut().unwrap().text = "Money".into();
        key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
        h.run();
        let money = named(&h, archforge_uml::DATA_TYPE, "Money");
        {
            let s = h.state_mut().session.as_mut().unwrap();
            s.execute("uml.attribute.add", &serde_json::json!({ "class": money.to_string(), "name": "amount", "type": "Real" })).unwrap();
            s.execute("uml.operation.add", &serde_json::json!({ "class": money.to_string(), "name": "add", "returns": money.to_string() })).unwrap();
        }
        h.state_mut().explorer.expanded.insert(money);
        h.run();
        // Its row under Commerce with the data type's icon, its members below it.
        let rows = archforge_ui_egui::testing::models_rows(
            archforge_ui_egui::testing::notation_uis(h.state()),
            h.state().session.as_ref().unwrap().doc(),
            &h.state().explorer,
        );
        let at = rows.iter().position(|r| r.label == "Money").expect("listed under Commerce");
        assert_eq!((rows[at].kind, rows[at].depth, rows[at].expandable), ("data-type", 2, true), "{:?}", tree_labels(&h));
        assert_eq!(*rows[at].icon, archforge_uml_ui::snapshot::icons::DATA_TYPE);
        let members: Vec<(&str, &str, u8, Option<&str>)> =
            rows[at + 1..at + 3].iter().map(|r| (r.label.as_str(), r.kind, r.depth, r.detail.as_deref())).collect();
        assert_eq!(members, [("amount", "attribute", 3, Some("Real")), ("add()", "operation", 3, Some("Money"))]);
        // Its name opens with F2 and renames it in place, one undo step.
        h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [money.to_string()] })).unwrap();
        h.state_mut().explorer.focused = true;
        h.run();
        key(&mut h, egui::Key::F2, egui::Modifiers::NONE);
        assert_eq!(h.state().explorer.rename.as_ref().map(|r| (r.target, r.text.as_str())), Some((money, "Money")));
        h.state_mut().explorer.rename.as_mut().unwrap().text = "Amount".into();
        h.run();
        key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
        assert_eq!(h.state().session.as_ref().unwrap().undo_label(), Some("Rename data type"));
        h.state_mut().session.as_mut().unwrap().undo().unwrap();
        h.run();
        // The search finds it as a data type of Commerce.
        let index = archforge_ui_egui::testing::search_index(
            archforge_ui_egui::testing::kinds(h.state()),
            archforge_ui_egui::testing::notation_uis(h.state()),
            h.state().session.as_ref().unwrap().doc(),
        );
        let hit = archforge_ui_egui::testing::search_hits(&index, "money").1.into_iter().find(|e| e.label == "Money").map(|e| e.detail.clone());
        assert_eq!(hit.as_deref(), Some("Data type · Commerce"));
        // Dragged from Models onto the diagram: its card, as one undo step; a second drag says so.
        let nodes_now = |h: &Harness<'static, Shell>| {
            let st = h.state().session.as_ref().unwrap().doc();
            st.list(st.by_uuid(h.state().canvas.active.unwrap()).unwrap(), archforge_diagram::diagram::NODE).unwrap().len()
        };
        let before = nodes_now(&h);
        let from = rect_of(&h, &format!("workspace.explorer.row.{money}")).center();
        let to = empty_spot(&h, &nodes);
        drag(&mut h, from, to);
        assert_eq!(nodes_now(&h), before + 1, "notice {:?}", h.state().canvas.notice);
        assert_eq!(h.state().session.as_ref().unwrap().undo_label(), Some("Place data type"));
        let from = rect_of(&h, &format!("workspace.explorer.row.{money}")).center();
        drag(&mut h, from, to);
        assert_eq!(h.state().canvas.notice.as_deref(), Some("Already on this diagram"));
        // Dragged onto another package's row, it moves there with its members.
        let billing = {
            let s = h.state_mut().session.as_mut().unwrap();
            let out = s.execute("uml.package.add", &serde_json::json!({ "name": "Billing", "parent": commerce.to_string() })).unwrap();
            archforge_mof::Uuid::parse(out.value["id"].as_str().unwrap()).unwrap()
        };
        h.run();
        let from = rect_of(&h, &format!("workspace.explorer.row.{money}")).center();
        let to = rect_of(&h, &format!("workspace.explorer.row.{billing}")).center();
        drag(&mut h, from, to);
        let st = h.state().session.as_ref().unwrap().doc();
        let t = st.by_uuid(money).unwrap();
        assert_eq!(archforge_uml::commands::package_of(st, t), st.by_uuid(billing), "{:?}", h.state().explorer.notice);
        assert_eq!(archforge_uml::members::attributes(st, t).len(), 1);
        assert_eq!(h.state().session.as_ref().unwrap().undo_label(), Some("Move data type"));
    }
}

/// The Relationship tool joins no association to or from a data type, saying so in its own words
/// beside the enumeration's; a data type specializes only a data type, the refusals said too, and a
/// generalization dragged out of one onto empty canvas makes a data type; a dependency may touch
/// it; no realization starts at one; an association's end dragged onto one is refused; in light
/// and dark.
#[test]
fn the_relationship_tool_joins_no_association_to_a_data_type_and_specializes_one_only_by_a_data_type() {
    use uml_relate::Relate;
    const NO_ASSOCIATION: &str = "A data type joins no association";
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, nodes, [money, _], [money_node, price_node]) = data_type_sample(theme);
        let node = |h: &Harness<'static, Shell>, id: &str| rect_of(h, &format!("workspace.canvas.node.{id}")).center();
        let relate = |h: &mut Harness<'static, Shell>, kind: Relate, from: egui::Pos2, to: egui::Pos2| {
            uml_of_mut(h.state_mut()).relation = kind;
            h.state_mut().canvas.notice = None;
            let at = rect_of(h, "workspace.tools.relationship").center();
            click(h, at);
            drag(h, from, to);
        };
        let before = edge_count(&h);
        // A data type specializes a data type, and a class depends on one: a line each.
        let (from, to) = (node(&h, &price_node), node(&h, &money_node));
        relate(&mut h, Relate::Generalization, from, to);
        assert_eq!(edge_count(&h), before + 1, "notice {:?}", h.state().canvas.notice);
        assert_eq!(h.state().session.as_ref().unwrap().undo_label(), Some("Add generalization"));
        let (from, to) = (node(&h, &nodes["OrderLine"]), node(&h, &money_node));
        relate(&mut h, Relate::Dependency, from, to);
        assert_eq!(edge_count(&h), before + 2, "notice {:?}", h.state().canvas.notice);
        // Refused, said in the user's words, and nothing changes.
        let revision = h.state().session.as_ref().unwrap().revision();
        for (kind, from, to, why) in [
            (Relate::Association, nodes["OrderLine"].clone(), money_node.clone(), NO_ASSOCIATION),
            (Relate::Aggregation, nodes["Order"].clone(), money_node.clone(), NO_ASSOCIATION),
            (Relate::Directed, money_node.clone(), nodes["OrderLine"].clone(), NO_ASSOCIATION),
            (Relate::Generalization, nodes["OrderLine"].clone(), money_node.clone(), "Only a data type specializes a data type"),
            (Relate::Generalization, money_node.clone(), nodes["Order"].clone(), "A data type specializes only a data type"),
            (Relate::Realization, money_node.clone(), price_node.clone(), "Only a class implements an interface"),
        ] {
            let (a, b) = (node(&h, &from), node(&h, &to));
            relate(&mut h, kind, a, b);
            let notice = h.state().canvas.notice.clone().expect("the refusal is said");
            assert_eq!(notice, why, "{kind:?}");
            assert_eq!((edge_count(&h), h.state().session.as_ref().unwrap().revision()), (before + 2, revision), "nothing changed: {why}");
        }
        assert!(h.state().canvas.gesture.is_none(), "no line was begun from the data type");
        // A generalization dragged out of one onto empty canvas makes a data type there, as one step.
        let empty = egui::pos2(node(&h, &nodes["Catalog"]).x, node(&h, &money_node).y);
        let from = node(&h, &money_node);
        relate(&mut h, Relate::Association, from, empty);
        assert_eq!(h.state().canvas.notice.as_deref(), Some(NO_ASSOCIATION), "refused before it starts");
        relate(&mut h, Relate::Generalization, from, empty);
        let rename = h.state().canvas.rename.clone().expect("its name is open for typing");
        assert_eq!(rename.text, "DataType 1", "notice {:?}", h.state().canvas.notice);
        let made = named(&h, archforge_uml::DATA_TYPE, "DataType 1");
        assert_eq!(h.state().session.as_ref().unwrap().undo_label(), Some("Add data type"));
        assert_eq!(edge_count(&h), before + 3);
        let general = {
            let st = h.state().session.as_ref().unwrap().doc();
            let g = st.by_uuid(made).unwrap();
            archforge_uml::relations::relationships_of(st, g)
                .into_iter()
                .filter_map(|r| archforge_uml::figures::ends(st, r))
                .any(|(specific, general)| general == g && st.uuid(specific).ok() == Some(money))
        };
        assert!(general, "Money specializes the new data type");
        h.state_mut().canvas.rename = None;
        // An association's end dragged onto a data type is refused and said.
        {
            let d = h.state().canvas.active.unwrap().to_string();
            let s = h.state_mut().session.as_mut().unwrap();
            let st = s.doc();
            let element = |node: &str| {
                let n = st.by_uuid(archforge_mof::Uuid::parse(node).unwrap()).unwrap();
                st.uuid(st.get(n, archforge_diagram::node::ELEMENT).unwrap().as_ref().unwrap()).unwrap().to_string()
            };
            let (from, to) = (element(&nodes["OrderLine"]), element(&nodes["Product"]));
            let a =
                s.execute("uml.association.add", &serde_json::json!({ "from": from, "to": to })).unwrap().value["id"].as_str().unwrap().to_string();
            s.execute(
                "diagram.connect",
                &serde_json::json!({ "diagram": d, "element": a, "source": nodes["OrderLine"], "target": nodes["Product"] }),
            )
            .unwrap();
            s.execute("view.select", &serde_json::json!({ "ids": [a] })).unwrap();
        }
        h.run();
        let revision = h.state().session.as_ref().unwrap().revision();
        let handle = rect_of(&h, "workspace.canvas.line-end.target").center();
        let to = node(&h, &money_node);
        drag(&mut h, handle, to);
        assert_eq!(h.state().canvas.notice.as_deref(), Some(NO_ASSOCIATION));
        assert_eq!(h.state().session.as_ref().unwrap().revision(), revision, "nothing changed");
    }
}

/// The classifier tool places a primitive type named in place, as one step, its kind kept and its
/// face, tooltip and accessible name naming it (the design's Tool · Primitive type); the card says
/// «primitive» over its name alone, a header's height, and offers no member: no adorner but
/// Connect, no compartment `+`, no menu row, and ⌘↩, ⇧⌘↩ and a double click open nothing; in light
/// and dark.
#[test]
fn the_classifier_tool_places_a_primitive_type_named_in_place_whose_card_is_its_name_alone() {
    use uml_classifier::Classifier;
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, nodes) = related_sample_in(theme, egui::vec2(1600.0, 1000.0));
        // The menu lists Primitive type after Data type, named for assistive technology; the arrows
        // walk to it and Enter picks it, arming the tool.
        let at = rect_of(&h, "workspace.tools.classifier-kinds").center();
        click(&mut h, at);
        let (data_type, primitive) = (rect_of(&h, "workspace.classifier.data-type"), rect_of(&h, "workspace.classifier.primitive-type"));
        assert!(primitive.top() >= data_type.bottom(), "Primitive type under Data type");
        {
            use egui_kittest::kittest::Queryable;
            assert!(h.query_by_label("Primitive type").is_some(), "the row's accessible name");
        }
        assert!(painted(&h).iter().any(|t| t == "Primitive type"), "the row names the kind");
        for _ in 0..4 {
            key(&mut h, egui::Key::ArrowDown, egui::Modifiers::NONE);
        }
        key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
        assert_eq!((h.state().canvas.popover, uml_of(h.state()).classifier), (None, Classifier::PrimitiveType));
        assert_eq!(h.state().canvas.tool, archforge_shell_api::canvas::Tool::Place(uml_classifier::TOOL));
        assert_eq!(classifier_hint(&h), "Primitive type", "the tooltip and accessible name name the kind");
        assert_eq!(*Classifier::PrimitiveType.tool_icon(), archforge_uml_ui::snapshot::icons::TOOL_PRIMITIVE_TYPE, "the face shows the kind's icon");
        // A click places one in the diagram's package, named in place, as one undo step.
        let at = empty_spot(&h, &nodes);
        click(&mut h, at);
        let rename = h.state().canvas.rename.clone().expect("its name is open for typing");
        assert_eq!(rename.text, "PrimitiveType 1");
        assert_eq!(h.state().session.as_ref().unwrap().undo_label(), Some("Add primitive type"));
        h.state_mut().canvas.rename.as_mut().unwrap().text = "Sku".into();
        key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
        let sku = named(&h, archforge_uml::PRIMITIVE_TYPE, "Sku");
        let st = h.state().session.as_ref().unwrap().doc();
        let home = archforge_uml::commands::package_of(st, st.by_uuid(sku).unwrap()).map(|p| archforge_core::name_of(st, p));
        assert_eq!(home.as_deref(), Some("Commerce"), "in the diagram's package");
        assert!(painted(&h).iter().any(|t| t == "«primitive»"), "its eyebrow is its keyword, as written");
        let node = node_of(&h, sku);
        assert_eq!(node_height(&h, &node), archforge_uml::PRIMITIVE_TYPE_SIZE.1, "a header's height: no compartment under it");
        assert_eq!(uml_of(h.state()).classifier, Classifier::PrimitiveType, "the kind stays for the window");
        // Selected, its card offers a line and no member.
        h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [sku.to_string()] })).unwrap();
        h.run();
        let found = ids(&h);
        assert!(found.iter().any(|i| i == "workspace.canvas.adorn.connect"), "{found:?}");
        for none in [
            "workspace.canvas.adorn.add-attribute",
            "workspace.canvas.adorn.add-literal",
            "workspace.canvas.class.add-attribute",
            "workspace.canvas.class.add-operation",
            "workspace.canvas.enumeration.add-literal",
        ] {
            assert!(!found.iter().any(|i| i == none), "{none} on a primitive type's card");
        }
        // ⌘↩, ⇧⌘↩ and a double click on the card open no draft, here or in Properties.
        for modifiers in [egui::Modifiers::COMMAND, egui::Modifiers::COMMAND | egui::Modifiers::SHIFT] {
            h.state_mut().canvas.focused = true;
            key(&mut h, egui::Key::Enter, modifiers);
            assert!(uml_of(h.state()).inline.is_none() && uml_of(h.state()).member.is_none(), "{modifiers:?} opens nothing");
        }
        let card = rect_of(&h, &format!("workspace.canvas.node.{node}")).center();
        double_click(&mut h, card + egui::vec2(0.0, 12.0));
        assert!(uml_of(h.state()).inline.is_none() && uml_of(h.state()).member.is_none(), "a double click opens nothing");
        h.state_mut().canvas.rename = None;
        // Its menu has no row to add a member.
        right_click(&mut h, card);
        assert!(ids(&h).iter().any(|i| i == "workspace.canvas.menu.remove"), "the menu is open");
        assert!(!ids(&h).iter().any(|i| i == "workspace.canvas.menu.add-row"), "no member row");
        assert!(!painted(&h).iter().any(|t| t == "Add attribute"));
    }
}

/// Models lists primitive types of the model's own under their package with no rows below them,
/// `+` adds one named in place, the search finds it, and it drags onto a diagram; the built-in
/// types stay out of Models and the search, as before. Properties shows its name, package and
/// description and no member section; Skills offers a skill on it and says a built-in type
/// carries none; in light and dark.
#[test]
fn primitive_types_are_listed_added_with_the_plus_found_and_dragged_while_the_built_in_ones_stay_out() {
    use archforge_shell_api::notation_ui::NotationUi;
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, nodes) = related_sample_in(theme, egui::vec2(1600.0, 1000.0));
        let commerce = select_named(&mut h, archforge_uml::PACKAGE, "Commerce");
        let st = h.state().session.as_ref().unwrap().doc();
        let offers: Vec<&str> = uml_ui::UmlUi.offers(st, st.by_uuid(commerce)).into_iter().map(|o| o.key).collect();
        assert_eq!(offers, ["class", "interface", "enumeration", "data-type", "primitive-type", "package"]);
        archforge_ui_egui::explorer::add(h.state_mut(), "primitive-type");
        let rename = h.state().explorer.rename.clone().expect("named in place");
        assert_eq!(rename.text, "PrimitiveType 1");
        h.state_mut().explorer.rename.as_mut().unwrap().text = "Sku".into();
        key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
        h.run();
        let sku = named(&h, archforge_uml::PRIMITIVE_TYPE, "Sku");
        // Its row under Commerce with the primitive type's icon, nothing below it.
        let types = named(&h, archforge_uml::PACKAGE, "Types");
        h.state_mut().explorer.expanded.insert(types);
        h.run();
        let rows = archforge_ui_egui::testing::models_rows(
            archforge_ui_egui::testing::notation_uis(h.state()),
            h.state().session.as_ref().unwrap().doc(),
            &h.state().explorer,
        );
        let at = rows.iter().position(|r| r.label == "Sku").expect("listed under Commerce");
        let package_depth = rows.iter().find(|r| r.label == "Commerce").map(|r| r.depth).unwrap();
        assert_eq!((rows[at].kind, rows[at].depth, rows[at].expandable), ("primitive-type", package_depth + 1, false), "{:?}", tree_labels(&h));
        assert_eq!(*rows[at].icon, archforge_uml_ui::snapshot::icons::PRIMITIVE_TYPE);
        // Types shows the model's own Date and Money, never the built-in Integer, String or Boolean.
        let t = rows.iter().position(|r| r.label == "Types").expect("Types is listed");
        let under: Vec<&str> = rows[t + 1..].iter().take_while(|r| r.depth > rows[t].depth).map(|r| r.label.as_str()).collect();
        assert_eq!(under, ["Date", "Money"], "{:?}", tree_labels(&h));
        // The search finds it as a primitive type of Commerce, and no built-in type.
        let index = archforge_ui_egui::testing::search_index(
            archforge_ui_egui::testing::kinds(h.state()),
            archforge_ui_egui::testing::notation_uis(h.state()),
            h.state().session.as_ref().unwrap().doc(),
        );
        let hit = archforge_ui_egui::testing::search_hits(&index, "sku").1.into_iter().find(|e| e.label == "Sku").map(|e| e.detail.clone());
        assert_eq!(hit.as_deref(), Some("Primitive type · Commerce"));
        assert!(archforge_ui_egui::testing::search_hits(&index, "integer").1.into_iter().all(|e| e.label != "Integer"), "no built-in type is found");
        // Dragged from Models onto the diagram: its card, a header's height, as one undo step.
        let before = edge_count(&h);
        let from = rect_of(&h, &format!("workspace.explorer.row.{sku}")).center();
        let to = empty_spot(&h, &nodes);
        drag(&mut h, from, to);
        assert_eq!(h.state().session.as_ref().unwrap().undo_label(), Some("Place primitive type"), "notice {:?}", h.state().canvas.notice);
        assert_eq!(node_height(&h, &node_of(&h, sku)), archforge_uml::PRIMITIVE_TYPE_SIZE.1);
        assert_eq!(edge_count(&h), before, "it has no lines yet");
        // Properties: its kind line, name, package and description, no member section.
        select_named(&mut h, archforge_uml::PRIMITIVE_TYPE, "Sku");
        h.state_mut().state.show_panel(archforge_shell_api::state::Panel::Properties);
        h.run();
        let texts = painted(&h);
        for want in ["Primitive type · model, in every diagram", "Primitive type"] {
            assert!(texts.iter().any(|t| t == want), "{want} in {texts:?}");
        }
        let found = ids(&h);
        for id in ["workspace.properties.name", "workspace.properties.description", "workspace.properties.class.owner"] {
            assert!(found.iter().any(|i| i == id), "{id} in {found:?}");
        }
        for none in [
            "workspace.properties.class.abstract",
            "workspace.properties.attributes.add",
            "workspace.properties.operations.add",
            "workspace.properties.literals.add",
        ] {
            assert!(!found.iter().any(|i| i == none), "{none} for a primitive type");
        }
        assert!(!texts.iter().any(|t| t.starts_with("Attributes ·") || t.starts_with("Operations ·")), "{texts:?}");
        // Skills: it carries one, and a skill for every primitive type is said so.
        h.state_mut().state.show_panel(archforge_shell_api::state::Panel::Skills);
        h.run();
        assert!(painted(&h).iter().any(|t| t == "On this primitive type · 0"));
        assert!(ids(&h).iter().any(|i| i == "workspace.skills.add"), "a primitive type carries skills");
        let skill = {
            let s = h.state_mut().session.as_mut().unwrap();
            let out = s
                .execute("skill.add", &serde_json::json!({ "subject": commerce.to_string(), "name": "Codes", "applies_to": "primitive type" }))
                .unwrap();
            archforge_mof::Uuid::parse(out.value["id"].as_str().unwrap()).unwrap()
        };
        let st = h.state().session.as_ref().unwrap().doc();
        assert_eq!(
            archforge_ui_egui::testing::skill_target_word(
                h.state().session.as_ref().unwrap().view(),
                archforge_ui_egui::testing::notation_uis(h.state()),
                st,
                st.by_uuid(skill).unwrap()
            ),
            "every primitive type"
        );
        // A built-in type, selected, carries none: the panel says so in place of +.
        let integer = {
            let st = h.state().session.as_ref().unwrap().doc();
            let t = st.alive().find(|o| archforge_uml::members::is_built_in(st, *o) && archforge_core::name_of(st, *o) == "Integer").unwrap();
            st.uuid(t).unwrap()
        };
        h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [integer.to_string()] })).unwrap();
        h.run();
        assert!(painted(&h).iter().any(|t| t == "Built-in types carry no skills"), "{:?}", painted(&h));
        assert!(!ids(&h).iter().any(|i| i == "workspace.skills.add"), "no + on a built-in type");
    }
}

/// A description typed on one element and left by clicking another, on the canvas or in Models,
/// is kept on the element it was typed on, as one undo step, and the other is left as it was. In
/// light and dark.
#[test]
fn a_description_draft_left_by_clicking_another_element_stays_on_its_own() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, nodes) = related_sample_in(theme, egui::vec2(1600.0, 1000.0));
        h.state_mut().state.show_panel(archforge_shell_api::state::Panel::Properties);
        let described = |h: &Harness<'static, Shell>, id: archforge_mof::Uuid| {
            let st = h.state().session.as_ref().unwrap().doc();
            archforge_uml::descriptions::description_of(st, st.by_uuid(id).unwrap())
        };
        // Customer, then Order on the canvas; Order, then Customer's row in Models.
        let customer = named(&h, archforge_uml::CLASS, "Customer");
        let order = named(&h, archforge_uml::CLASS, "Order");
        let packages: Vec<archforge_mof::Uuid> = {
            let st = h.state().session.as_ref().unwrap().doc();
            st.alive().filter(|o| st.class_of(*o).ok() == Some(archforge_uml::PACKAGE)).filter_map(|o| st.uuid(o).ok()).collect()
        };
        h.state_mut().explorer.expanded.extend(packages);
        h.run();
        for (from, to, text, target) in [
            (customer, order, "Someone who buys.", format!("workspace.canvas.node.{}", nodes["Order"])),
            (order, customer, "One purchase.", format!("workspace.explorer.row.{customer}")),
        ] {
            h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [from.to_string()] })).unwrap();
            h.run();
            let before = described(&h, to);
            let at = rect_of(&h, "workspace.properties.description").center();
            click(&mut h, at);
            h.event(egui::Event::Text(text.into()));
            h.run();
            let at = rect_of(&h, &target).center();
            click(&mut h, at);
            h.run();
            let s = h.state().session.as_ref().unwrap();
            assert_eq!(s.view().selection, vec![s.doc().by_uuid(to).unwrap()], "{target} selected the other element");
            assert_eq!(described(&h, from), text, "the draft stays on the element it was typed on");
            assert_eq!(described(&h, to), before, "the other element is unchanged");
            assert_eq!(s.undo_label(), Some("Describe"));
            assert!(h.state().panels.description.is_none(), "the draft is spent");
            assert!(!painted(&h).iter().any(|t| t == text), "the other's field shows its own");
        }
    }
}

/// A primitive type of the model's own named Integer is offered beside the built-in Integer, never
/// in its place: the type menu lists both, each with a muted note saying where it is from
/// ("built-in", or its package's path), in its accessible name too; each pick types the member by
/// the one picked, the built-in one in Types and never a second of it. Typed on a card, the shared
/// name is refused until one is picked from the menu. In light and dark.
#[test]
fn the_type_menu_tells_same_named_types_apart_and_never_swaps_a_built_in_type_for_the_model_s_own() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, _) = related_sample_in(theme, egui::vec2(1600.0, 1000.0));
        let commerce = named(&h, archforge_uml::PACKAGE, "Commerce");
        let own = {
            let s = h.state_mut().session.as_mut().unwrap();
            let out = s.execute("uml.primitive-type.add", &serde_json::json!({ "package": commerce.to_string(), "name": "Integer" })).unwrap();
            archforge_mof::Uuid::parse(out.value["id"].as_str().unwrap()).unwrap()
        };
        h.run();
        let customer = select_named(&mut h, archforge_uml::CLASS, "Customer");
        h.state_mut().state.show_panel(archforge_shell_api::state::Panel::Properties);
        h.run();
        let at = rect_of(&h, "workspace.properties.attributes.add").center();
        click(&mut h, at);
        h.event(egui::Event::Text("count".into()));
        h.run();
        let at = rect_of(&h, "workspace.properties.member.type").center();
        click(&mut h, at);
        // Both are offered, once each, told apart by their notes; a name no other shares has none.
        let kept = uml_of(h.state()).type_choices.clone().expect("the open menu keeps its choices");
        let both: Vec<usize> = kept.labels.iter().enumerate().filter(|(_, l)| *l == "Integer").map(|(i, _)| i).collect();
        assert_eq!(both.len(), 2, "{:?}", kept.labels);
        let (builtin, mine) = (both[0], both[1]);
        assert_eq!((kept.values[builtin].as_str(), kept.notes[builtin].as_str()), ("Integer", "built-in"));
        assert_eq!(kept.values[mine], own.to_string());
        assert!(kept.notes[mine].ends_with("Commerce") && kept.notes[mine] != "built-in", "{:?}", kept.notes[mine]);
        let real = kept.labels.iter().position(|l| l == "Real").unwrap();
        assert_eq!(kept.notes[real], "", "a name no other shares has no note");
        let texts = painted(&h);
        assert!(texts.iter().any(|t| t == "built-in") && texts.iter().any(|t| *t == kept.notes[mine]), "the notes show: {texts:?}");
        {
            use egui_kittest::kittest::Queryable;
            assert!(h.query_by_label("Integer · built-in").is_some(), "the note joins the accessible name");
            assert!(h.query_by_label(&format!("Integer · {}", kept.notes[mine])).is_some());
        }
        // The built-in one picked: the member's type is the built-in Integer in Types.
        let at = rect_of(&h, &format!("workspace.properties.member.type.{builtin}")).center();
        click(&mut h, at);
        let at = rect_of(&h, "workspace.properties.member.apply").center();
        click(&mut h, at);
        let typed = |h: &Harness<'static, Shell>, attribute: &str| {
            let st = h.state().session.as_ref().unwrap().doc();
            let a = archforge_uml::members::attributes(st, st.by_uuid(customer).unwrap())
                .into_iter()
                .find(|a| archforge_core::name_of(st, *a) == attribute)
                .unwrap();
            st.get(a, archforge_uml::typed_element::TYPE).unwrap().as_ref().unwrap()
        };
        let st = h.state().session.as_ref().unwrap().doc();
        assert!(archforge_uml::members::is_built_in(st, typed(&h, "count")), "the built-in Integer, never the model's own");
        // The model's own picked: the member's type is that one.
        select_named(&mut h, archforge_uml::CLASS, "Customer");
        let at = rect_of(&h, "workspace.properties.attributes.add").center();
        click(&mut h, at);
        h.event(egui::Event::Text("code".into()));
        h.run();
        let at = rect_of(&h, "workspace.properties.member.type").center();
        click(&mut h, at);
        let at = rect_of(&h, &format!("workspace.properties.member.type.{mine}")).center();
        click(&mut h, at);
        let at = rect_of(&h, "workspace.properties.member.apply").center();
        click(&mut h, at);
        let st = h.state().session.as_ref().unwrap().doc();
        assert_eq!(st.uuid(typed(&h, "code")).ok(), Some(own), "the model's own Integer, never the built-in one");
        let builtins = st.alive().filter(|o| archforge_uml::members::is_built_in(st, *o) && archforge_core::name_of(st, *o) == "Integer").count();
        assert_eq!(builtins, 1, "the built-in Integer once");
        // Typed on a card, the shared name is refused until one is picked from the menu.
        let catalog = named(&h, archforge_uml::CLASS, "Catalog");
        h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [catalog.to_string()] })).unwrap();
        h.run();
        let at = rect_of(&h, "workspace.canvas.class.add-attribute").center();
        click(&mut h, at);
        h.event(egui::Event::Text("size".into()));
        h.run();
        key(&mut h, egui::Key::Tab, egui::Modifiers::NONE);
        h.event(egui::Event::Text("Integer".into()));
        h.run();
        let revision = h.state().session.as_ref().unwrap().revision();
        key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
        let refused = uml_of(h.state()).inline.as_ref().and_then(|i| i.refused.clone()).expect("refused");
        assert_eq!(refused, "Several types are named Integer; pick one from the menu.");
        assert_eq!(h.state().session.as_ref().unwrap().revision(), revision, "nothing written");
        key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
    }
}

/// The Relationship tool joins no association to or from a primitive type, saying so in its own
/// words; a primitive type specializes only a primitive type, the refusals said too, and a
/// generalization dragged out of one onto empty canvas makes a primitive type, a header's height;
/// a dependency may touch it; no realization starts at one; in light and dark.
#[test]
fn the_relationship_tool_joins_no_association_to_a_primitive_type_and_specializes_one_only_by_a_primitive_type() {
    use uml_relate::Relate;
    const NO_ASSOCIATION: &str = "A primitive type joins no association";
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, nodes, [sku, _], [sku_node, code_node]) = primitive_type_sample(theme);
        let node = |h: &Harness<'static, Shell>, id: &str| rect_of(h, &format!("workspace.canvas.node.{id}")).center();
        let relate = |h: &mut Harness<'static, Shell>, kind: Relate, from: egui::Pos2, to: egui::Pos2| {
            uml_of_mut(h.state_mut()).relation = kind;
            h.state_mut().canvas.notice = None;
            let at = rect_of(h, "workspace.tools.relationship").center();
            click(h, at);
            drag(h, from, to);
        };
        let before = edge_count(&h);
        // A primitive type specializes a primitive type, and a class depends on one: a line each.
        let (from, to) = (node(&h, &sku_node), node(&h, &code_node));
        relate(&mut h, Relate::Generalization, from, to);
        assert_eq!(edge_count(&h), before + 1, "notice {:?}", h.state().canvas.notice);
        let (from, to) = (node(&h, &nodes["OrderLine"]), node(&h, &sku_node));
        relate(&mut h, Relate::Dependency, from, to);
        assert_eq!(edge_count(&h), before + 2, "notice {:?}", h.state().canvas.notice);
        // Refused, said in the user's words, and nothing changes.
        let revision = h.state().session.as_ref().unwrap().revision();
        for (kind, from, to, why) in [
            (Relate::Association, nodes["OrderLine"].clone(), sku_node.clone(), NO_ASSOCIATION),
            (Relate::Composition, nodes["Order"].clone(), sku_node.clone(), NO_ASSOCIATION),
            (Relate::Directed, sku_node.clone(), nodes["OrderLine"].clone(), NO_ASSOCIATION),
            (Relate::Generalization, nodes["OrderLine"].clone(), sku_node.clone(), "Only a primitive type specializes a primitive type"),
            (Relate::Generalization, sku_node.clone(), nodes["Order"].clone(), "A primitive type specializes only a primitive type"),
            (Relate::Realization, sku_node.clone(), code_node.clone(), "Only a class implements an interface"),
        ] {
            let (a, b) = (node(&h, &from), node(&h, &to));
            relate(&mut h, kind, a, b);
            let notice = h.state().canvas.notice.clone().expect("the refusal is said");
            assert_eq!(notice, why, "{kind:?}");
            assert_eq!((edge_count(&h), h.state().session.as_ref().unwrap().revision()), (before + 2, revision), "nothing changed: {why}");
        }
        // A generalization dragged out of one onto empty canvas makes a primitive type there, as one step.
        let empty = egui::pos2(node(&h, &nodes["Catalog"]).x, node(&h, &sku_node).y);
        let from = node(&h, &sku_node);
        relate(&mut h, Relate::Generalization, from, empty);
        let rename = h.state().canvas.rename.clone().expect("its name is open for typing");
        assert_eq!(rename.text, "PrimitiveType 1", "notice {:?}", h.state().canvas.notice);
        let made = named(&h, archforge_uml::PRIMITIVE_TYPE, "PrimitiveType 1");
        assert_eq!(h.state().session.as_ref().unwrap().undo_label(), Some("Add primitive type"));
        assert_eq!(edge_count(&h), before + 3);
        assert_eq!(node_height(&h, &node_of(&h, made)), archforge_uml::PRIMITIVE_TYPE_SIZE.1, "a header's height");
        let general = {
            let st = h.state().session.as_ref().unwrap().doc();
            let g = st.by_uuid(made).unwrap();
            archforge_uml::relations::relationships_of(st, g)
                .into_iter()
                .filter_map(|r| archforge_uml::figures::ends(st, r))
                .any(|(specific, general)| general == g && st.uuid(specific).ok() == Some(sku))
        };
        assert!(general, "Sku specializes the new primitive type");
        h.state_mut().canvas.rename = None;
    }
}

/// The Relationship menu lists Association class after Interface realization; drawn between two
/// classes it makes the line and a card above its middle as one element and one undo step, the
/// card's name open for typing and its eyebrow ASSOCIATION CLASS; at an interface it is refused and
/// said, and dropped on empty canvas it makes a new class there, named in place. In light and dark.
#[test]
fn the_relationship_tool_draws_an_association_class_with_its_card_as_one_step_and_refuses_what_is_no_class() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let mut r = reviews(theme);
        // The menu: Association class right after Interface realization, in its group.
        let at = rect_of(&r.h, "workspace.tools.relationship-kinds").center();
        click(&mut r.h, at);
        let (realization, class) = (rect_of(&r.h, "workspace.relate.realization"), rect_of(&r.h, "workspace.relate.association-class"));
        assert!(class.top() >= realization.bottom() && class.top() - realization.bottom() < 8.0, "{realization:?} then {class:?}");
        {
            use egui_kittest::kittest::Queryable;
            assert!(r.h.query_by_label("Association class: between two classes, with a card of its own").is_some(), "the row says what it draws");
        }
        click(&mut r.h, class.center());
        assert_eq!(uml_of(r.h.state()).relation, uml_relate::Relate::AssociationClass);
        let relate = |r: &mut Reviews, from: egui::Pos2, to: egui::Pos2| {
            r.h.state_mut().canvas.notice = None;
            r.h.state_mut().canvas.rename = None;
            let at = rect_of(&r.h, "workspace.tools.relationship").center();
            click(&mut r.h, at);
            drag(&mut r.h, from, to);
        };
        let before = r.shown();
        let (from, to) = (r.at("Customer"), r.at("Product"));
        relate(&mut r, from, to);
        assert_eq!(r.shown(), (before.0 + 1, before.1 + 1), "its card and its line; notice {:?}", r.h.state().canvas.notice);
        assert_eq!(r.association_classes(), ["AssociationClass 1"]);
        assert_eq!(r.h.state().session.as_ref().unwrap().undo_label(), Some("Add association class"));
        // One element selected, its card's name open for typing, the card above the line's middle.
        let (element, card) = {
            let st = r.h.state().session.as_ref().unwrap().doc();
            let ac = st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_uml::ASSOCIATION_CLASS)).unwrap();
            let (_, edges, nodes) = archforge_diagram::views_of(st, ac);
            assert_eq!((edges.len(), nodes.len()), (1, 1));
            (st.uuid(ac).unwrap(), st.uuid(nodes[0]).unwrap())
        };
        assert_eq!(r.selection(), [element.to_string()]);
        let rename = r.h.state().canvas.rename.clone().expect("its card's name is open for typing");
        assert_eq!((rename.node, rename.element, rename.text.as_str()), (card, element, "AssociationClass 1"));
        r.h.state_mut().canvas.rename = None;
        r.h.run();
        let card_rect = rect_of(&r.h, &format!("workspace.canvas.node.{card}"));
        let middle = r.line_middle();
        assert!(card_rect.bottom() < middle.y && (card_rect.center().x - middle.x).abs() < 4.0, "{card_rect:?} above {middle:?}");
        assert!(painted(&r.h).iter().any(|t| t == "ASSOCIATION CLASS"), "its eyebrow");
        let edge = {
            let st = r.h.state().session.as_ref().unwrap().doc();
            st.uuid(archforge_diagram::views_of(st, st.by_uuid(element).unwrap()).1[0]).unwrap()
        };
        assert!(ids(&r.h).contains(&format!("workspace.canvas.edge.{edge}.tie")), "the tie is drawn");
        // One step: undone, the element, its line and its card go together.
        r.h.state_mut().session.as_mut().unwrap().undo().unwrap();
        r.h.run();
        assert_eq!((r.shown(), r.association_classes().len()), (before, 0));
        r.h.state_mut().session.as_mut().unwrap().redo().unwrap();
        r.h.run();
        // Refused at an interface, at either end, said in the user's words; nothing changes.
        let revision = r.h.state().session.as_ref().unwrap().revision();
        let (from, to) = (r.at("Store"), r.at("Rated"));
        relate(&mut r, from, to);
        assert_eq!(r.h.state().canvas.notice.as_deref(), Some("An association class joins only classes"));
        relate(&mut r, to, from);
        assert_eq!(r.h.state().canvas.notice.as_deref(), Some("An association class joins only classes"));
        assert!(r.h.state().canvas.gesture.is_none(), "no line began at the interface");
        assert_eq!(r.h.state().session.as_ref().unwrap().revision(), revision, "nothing changed");
        // Dropped on empty canvas: a new class there, its name open for typing, and the association
        // class with its line and card, as one step.
        let before = r.shown();
        // Between Store and Rated, where nothing is drawn.
        let store = r.at("Store");
        let empty = egui::pos2((store.x + r.at("Rated").x) / 2.0, store.y);
        relate(&mut r, store, empty);
        let rename = r.h.state().canvas.rename.clone().expect("the new class's name is open");
        assert_eq!(rename.text, "Class 1", "notice {:?}", r.h.state().canvas.notice);
        assert_eq!(r.association_classes().len(), 2);
        assert_eq!(r.shown(), (before.0 + 2, before.1 + 1));
        r.h.state_mut().canvas.rename = None;
        r.h.state_mut().session.as_mut().unwrap().undo().unwrap();
        r.h.run();
        assert_eq!((r.shown(), r.association_classes().len()), (before, 1), "one step");
    }
}

/// An association class's card moves on its own and its line stays; its tie follows the card and,
/// when a class at its end moves, the line; a click on the line or on the card selects the one
/// element. In light and dark.
#[test]
fn an_association_class_s_card_moves_on_its_own_its_tie_follows_and_either_part_selects_it() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let mut r = reviews(theme);
        let (ac, edge, card) = r.review();
        let tie = |r: &Reviews| rect_of(&r.h, &format!("workspace.canvas.edge.{edge}.tie"));
        // Selecting the card or the line selects the association class.
        let at = rect_of(&r.h, &format!("workspace.canvas.node.{card}")).center();
        click(&mut r.h, at);
        assert_eq!(r.selection(), [ac.as_str()]);
        let empty = egui::pos2(r.line_middle().x, r.at("Store").y);
        click(&mut r.h, empty);
        assert_ne!(r.selection(), [ac.as_str()]);
        let at = r.line_middle() + egui::vec2(-60.0, 0.0);
        click(&mut r.h, at);
        assert_eq!(r.selection(), [ac.as_str()], "the line selects it too");
        // The card dragged right: it moves, the line does not, and the tie leaves from its new place.
        let line_ends = |r: &Reviews| {
            let st = r.h.state().session.as_ref().unwrap().doc();
            let e = st.by_uuid(archforge_mof::Uuid::parse(&edge).unwrap()).unwrap();
            (st.get(e, archforge_diagram::edge::SOURCE).unwrap(), st.get(e, archforge_diagram::edge::TARGET).unwrap())
        };
        let (ends, bounds, before) = (line_ends(&r), node_bounds(&r.h, &card), tie(&r));
        let from = rect_of(&r.h, &format!("workspace.canvas.node.{card}")).center();
        drag(&mut r.h, from, from + egui::vec2(120.0, 0.0));
        let moved = node_bounds(&r.h, &card);
        assert!(moved.x > bounds.x + 40.0 && moved.y == bounds.y, "{bounds:?} to {moved:?}");
        assert_eq!(line_ends(&r), ends, "the line stays");
        let after = tie(&r);
        assert!(
            (after.top() - before.top()).abs() < 1.0 && after.width() > before.width() + 10.0,
            "the tie leans to the card: {before:?} to {after:?}"
        );
        assert!((after.bottom() - r.line_middle().y).abs() < 2.0, "it still meets the line's middle");
        // Product dragged down: the line bends and its middle moves, and the tie's end with it,
        // the card staying where it was.
        let before = tie(&r);
        let from = r.at("Product");
        drag(&mut r.h, from, from + egui::vec2(0.0, 100.0));
        let after = tie(&r);
        assert_eq!(node_bounds(&r.h, &card), moved, "the card stays");
        assert!(
            (after.min - before.min).length() > 20.0 || (after.max - before.max).length() > 20.0,
            "the tie follows the line: {before:?} to {after:?}"
        );
    }
}

/// A selected association class's Properties show its name, its classes, its two ends as an
/// association's do, then its attributes and operations; members are typed on its card as on a
/// class's. In light and dark.
#[test]
fn an_association_class_shows_its_ends_and_members_in_properties_and_takes_members_on_its_card() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let mut r = reviews(theme);
        let (ac, _, card) = r.review();
        let at = rect_of(&r.h, &format!("workspace.canvas.node.{card}")).center();
        click(&mut r.h, at);
        r.h.state_mut().state.show_panel(archforge_shell_api::state::Panel::Properties);
        r.h.run();
        let found = ids(&r.h);
        for want in [
            "workspace.properties.name",
            "workspace.properties.description",
            "workspace.properties.relationship.from",
            "workspace.properties.relationship.to",
            "workspace.properties.end.0.role",
            "workspace.properties.end.1.multiplicity",
        ] {
            assert!(found.contains(&want.to_string()), "{want} in {found:?}");
        }
        let shown = painted(&r.h);
        for label in ["Association class · model, in every diagram", "End at Customer", "End at Product", "Attributes · 1", "+ stars : Integer"] {
            assert!(shown.iter().any(|t| t == label), "{label} is shown in {shown:?}");
        }
        // Its name renames it as the classifier it is, a sibling's name refused.
        let ac_id = archforge_mof::Uuid::parse(&ac).unwrap();
        let renamed = archforge_ui_egui::snapshot::run(
            r.h.state_mut(),
            "uml.association-class.rename",
            serde_json::json!({ "association_class": ac, "name": "Customer" }),
        );
        assert!(renamed.is_err_and(|e| e.contains("a class named Customer")));
        // A new attribute typed on the card through its adorner, as on a class's card.
        let at = rect_of(&r.h, "workspace.canvas.adorn.add-attribute").center();
        click(&mut r.h, at);
        assert!(uml_of(r.h.state()).inline.as_ref().is_some_and(|i| i.edit.target.is_none() && i.edit.class == ac_id));
        r.h.event(egui::Event::Text("text".into()));
        r.h.run();
        key(&mut r.h, egui::Key::Enter, egui::Modifiers::NONE);
        assert_eq!(member_rows(&r.h, ac_id, false), ["+ stars : Integer", "+ text"]);
        key(&mut r.h, egui::Key::Escape, egui::Modifiers::NONE);
        // ⇧⌘↩ opens an operation's row.
        key(&mut r.h, egui::Key::Enter, egui::Modifiers::COMMAND | egui::Modifiers::SHIFT);
        assert!(uml_of(r.h.state()).inline.as_ref().is_some_and(|i| i.edit.kind == uml_panel::MemberKind::Operation));
        r.h.event(egui::Event::Text("publish".into()));
        r.h.run();
        key(&mut r.h, egui::Key::Enter, egui::Modifiers::NONE);
        key(&mut r.h, egui::Key::Escape, egui::Modifiers::NONE);
        assert_eq!(member_rows(&r.h, ac_id, true), ["+ publish()"]);
        r.h.run();
        assert!(painted(&r.h).iter().any(|t| t == "Operations · 1"), "Properties follows");
    }
}

/// Remove from this diagram takes the association class's line and card away and keeps it in the
/// model; Delete from the model asks first, then deletes it with both. In light and dark.
#[test]
fn removing_an_association_class_from_the_diagram_takes_its_line_and_card_and_deleting_it_asks_first() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let mut r = reviews(theme);
        let (ac, _, card) = r.review();
        let before = r.shown();
        let at = rect_of(&r.h, &format!("workspace.canvas.node.{card}")).center();
        right_click(&mut r.h, at);
        let at = rect_of(&r.h, "workspace.canvas.menu.remove").center();
        click(&mut r.h, at);
        assert_eq!(r.shown(), (before.0 - 1, before.1 - 1), "its card and its line went");
        assert_eq!(r.association_classes(), ["Review"], "the model keeps it");
        r.h.state_mut().session.as_mut().unwrap().undo().unwrap();
        r.h.run();
        assert_eq!(r.shown(), before, "one step");
        // The line selected, Delete takes both from the diagram too.
        let at = r.line_middle() + egui::vec2(-60.0, 0.0);
        click(&mut r.h, at);
        assert_eq!(r.selection(), [ac.as_str()]);
        key(&mut r.h, egui::Key::Delete, egui::Modifiers::NONE);
        assert_eq!(r.shown(), (before.0 - 1, before.1 - 1));
        assert_eq!(r.association_classes(), ["Review"]);
        r.h.state_mut().session.as_mut().unwrap().undo().unwrap();
        r.h.run();
        // Delete from the model: the dialog asks, then the element goes with its line and card.
        let at = r.line_middle() + egui::vec2(-60.0, 0.0);
        click(&mut r.h, at);
        key(&mut r.h, egui::Key::Delete, egui::Modifiers::SHIFT);
        assert_eq!(r.h.state().explorer.delete.as_ref().map(|d| d.ids.clone()), Some(vec![archforge_mof::Uuid::parse(&ac).unwrap()]));
        let at = rect_of(&r.h, "workspace.explorer.delete.confirm").center();
        click(&mut r.h, at);
        assert!(r.association_classes().is_empty(), "deleted from the model");
        assert_eq!(r.shown(), (before.0 - 1, before.1 - 1));
    }
}

/// On a diagram that draws an association class's line without its card (its classes placed there
/// after it was made), Delete takes that line from the diagram and keeps the association class, its
/// card on the other diagram untouched. In light and dark.
#[test]
fn delete_on_an_association_class_line_without_its_card_takes_the_line_from_that_diagram_only() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let mut r = reviews(theme);
        let (ac, _, _) = r.review();
        let (package, customer, product) = (r.package.clone(), r.ids["Customer"].clone(), r.ids["Product"].clone());
        let s = r.h.state_mut().session.as_mut().unwrap();
        let mut run = |cmd: &str, p: serde_json::Value| s.execute(cmd, &p).unwrap().value["id"].as_str().unwrap_or_default().to_string();
        let d2 = run("diagram.add", serde_json::json!({ "kind": archforge_uml::STRUCTURE_DIAGRAM, "element": package, "name": "Reviews 2" }));
        for (class, x) in [(&customer, 80), (&product, 760)] {
            run("diagram.place", serde_json::json!({ "diagram": d2, "element": class, "x": x, "y": 520, "width": 240, "height": 136 }));
        }
        run("diagram.connect-all", serde_json::json!({ "diagram": d2 }));
        r.diagram = d2.clone();
        archforge_ui_egui::canvas::open(r.h.state_mut(), archforge_mof::Uuid::parse(&d2).unwrap());
        r.h.state_mut().canvas.fit_pending = true;
        r.h.run();
        // The nodes of the second diagram, by their class's name.
        r.nodes.clear();
        let st = r.h.state().session.as_ref().unwrap().doc();
        for n in st.list(st.by_uuid(archforge_mof::Uuid::parse(&d2).unwrap()).unwrap(), archforge_diagram::diagram::NODE).unwrap().iter() {
            let n = n.as_ref().unwrap();
            let name = archforge_core::name_of(st, st.get(n, archforge_diagram::node::ELEMENT).unwrap().as_ref().unwrap());
            let name: &'static str = if name == "Customer" { "Customer" } else { "Product" };
            r.nodes.insert(name, st.uuid(n).unwrap().to_string());
        }
        assert_eq!(r.shown(), (2, 1), "its line alone");
        let at = r.line_middle();
        click(&mut r.h, at);
        assert_eq!(r.selection(), [ac.as_str()]);
        key(&mut r.h, egui::Key::Delete, egui::Modifiers::NONE);
        assert_eq!(r.shown(), (2, 0), "the line went from this diagram");
        assert_eq!(r.association_classes(), ["Review"], "the model keeps it");
        assert_eq!(r.h.state().session.as_ref().unwrap().undo_label(), Some("Remove from this diagram"));
    }
}

/// Delete on an association class's line never deletes it from the model, even with its card on no
/// diagram at all: the line goes from this diagram, and only ⇧⌫ and its dialog delete. In light
/// and dark.
#[test]
fn delete_on_an_association_class_line_with_its_card_on_no_diagram_keeps_it_in_the_model() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let mut r = reviews(theme);
        let (ac, _, card) = r.review();
        // Its card off the diagram, then its line drawn again alone: no card anywhere.
        let d = r.diagram.clone();
        archforge_ui_egui::snapshot::run(r.h.state_mut(), "diagram.unplace", serde_json::json!({ "nodes": [card] })).unwrap();
        archforge_ui_egui::snapshot::run(r.h.state_mut(), "diagram.connect-all", serde_json::json!({ "diagram": d })).unwrap();
        r.h.run();
        let before = r.shown();
        let at = r.line_middle() + egui::vec2(-60.0, 0.0);
        click(&mut r.h, at);
        assert_eq!(r.selection(), [ac.as_str()]);
        key(&mut r.h, egui::Key::Delete, egui::Modifiers::NONE);
        assert_eq!(r.shown(), (before.0, before.1 - 1), "its line went from the diagram");
        assert_eq!(r.association_classes(), ["Review"], "the model keeps it");
        assert!(r.h.state().explorer.delete.is_none() && r.h.state().canvas.toast.is_none(), "nothing was deleted");
        assert_eq!(r.h.state().session.as_ref().unwrap().undo_label(), Some("Remove from this diagram"));
    }
}

/// An association class with its card on one diagram and its line alone on another: deleting it
/// from the first, the dialog says it disappears from both diagrams, each once. On the second, its
/// line selected as a line (an agent's `view.select` takes an edge's id) is taken off by Delete and
/// asked about by Shift+Delete, as the association class it shows. In light and dark.
#[test]
fn the_delete_dialog_counts_a_diagram_with_an_association_class_s_line_alone_and_a_selected_edge_resolves() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let mut r = reviews(theme);
        let (ac, _, card) = r.review();
        let (package, customer, product) = (r.package.clone(), r.ids["Customer"].clone(), r.ids["Product"].clone());
        let s = r.h.state_mut().session.as_mut().unwrap();
        let mut run = |cmd: &str, p: serde_json::Value| s.execute(cmd, &p).unwrap().value["id"].as_str().unwrap_or_default().to_string();
        let d2 = run("diagram.add", serde_json::json!({ "kind": archforge_uml::STRUCTURE_DIAGRAM, "element": package, "name": "Reviews 2" }));
        for (class, x) in [(&customer, 80), (&product, 760)] {
            run("diagram.place", serde_json::json!({ "diagram": d2, "element": class, "x": x, "y": 520, "width": 240, "height": 136 }));
        }
        run("diagram.connect-all", serde_json::json!({ "diagram": d2 }));
        r.h.run();
        // On the first diagram, the card selected: Shift+Delete names both diagrams.
        let at = rect_of(&r.h, &format!("workspace.canvas.node.{card}")).center();
        click(&mut r.h, at);
        key(&mut r.h, egui::Key::Delete, egui::Modifiers::SHIFT);
        let asked = r.h.state().explorer.delete.clone().expect("the dialog asks");
        assert_eq!(asked.diagrams, 2, "the card's diagram and the line's, each once");
        let at = rect_of(&r.h, "workspace.explorer.delete.cancel").center();
        click(&mut r.h, at);
        // On the second, the line's edge selected by its own id.
        archforge_ui_egui::canvas::open(r.h.state_mut(), archforge_mof::Uuid::parse(&d2).unwrap());
        r.h.run();
        let edge = {
            let st = r.h.state().session.as_ref().unwrap().doc();
            let d2 = st.by_uuid(archforge_mof::Uuid::parse(&d2).unwrap()).unwrap();
            st.uuid(st.list(d2, archforge_diagram::diagram::EDGE).unwrap()[0].as_ref().unwrap()).unwrap().to_string()
        };
        let select = |r: &mut Reviews| {
            archforge_ui_egui::snapshot::run(r.h.state_mut(), "view.select", serde_json::json!({ "ids": [edge] })).unwrap();
            r.h.state_mut().canvas.focused = true;
            r.h.run();
        };
        select(&mut r);
        key(&mut r.h, egui::Key::Delete, egui::Modifiers::SHIFT);
        let asked = r.h.state().explorer.delete.clone().expect("the dialog asks for the association class");
        assert_eq!((asked.ids, asked.diagrams), (vec![archforge_mof::Uuid::parse(&ac).unwrap()], 2));
        let at = rect_of(&r.h, "workspace.explorer.delete.cancel").center();
        click(&mut r.h, at);
        select(&mut r);
        key(&mut r.h, egui::Key::Delete, egui::Modifiers::NONE);
        let st = r.h.state().session.as_ref().unwrap().doc();
        let d2 = st.by_uuid(archforge_mof::Uuid::parse(&d2).unwrap()).unwrap();
        assert!(st.list(d2, archforge_diagram::diagram::EDGE).unwrap().is_empty(), "the line went from this diagram");
        assert_eq!(r.association_classes(), ["Review"], "the model keeps it");
    }
}

/// A class shown on two diagrams: Delete from the model says it disappears from both, asked from
/// Models and from the canvas alike (AR-187). In light and dark.
#[test]
fn deleting_a_class_shown_on_two_diagrams_says_two_from_models_and_from_the_canvas() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, nodes) = related_sample_in(theme, egui::vec2(1600.0, 1000.0));
        let first = h.state().canvas.active.unwrap();
        let customer = named(&h, archforge_uml::CLASS, "Customer");
        let package = {
            let st = h.state().session.as_ref().unwrap().doc();
            let d = st.by_uuid(first).unwrap();
            st.uuid(st.get(d, archforge_diagram::diagram::ELEMENT).unwrap().as_ref().unwrap()).unwrap().to_string()
        };
        let s = h.state_mut().session.as_mut().unwrap();
        let second = s
            .execute("diagram.add", &serde_json::json!({ "kind": archforge_uml::STRUCTURE_DIAGRAM, "element": package, "name": "Customers" }))
            .unwrap()
            .value["id"]
            .as_str()
            .unwrap()
            .to_string();
        let place = serde_json::json!({ "diagram": second, "element": customer.to_string(), "x": 80, "y": 80, "width": 240, "height": 136 });
        s.execute("diagram.place", &place).unwrap();
        h.run();
        let two = vec!["It disappears from 2 diagrams.".to_string()];
        // From Models, the class selected.
        select_named(&mut h, archforge_uml::CLASS, "Customer");
        archforge_ui_egui::explorer::remove_selection(h.state_mut());
        assert_eq!(asked_diagrams(&mut h), (2, two.clone()), "from Models");
        // From the canvas, its card selected on the first diagram.
        archforge_ui_egui::canvas::open(h.state_mut(), first);
        h.run();
        let at = rect_of(&h, &format!("workspace.canvas.node.{}", nodes["Customer"])).center();
        click(&mut h, at);
        key(&mut h, egui::Key::Delete, egui::Modifiers::SHIFT);
        assert_eq!(asked_diagrams(&mut h), (2, two), "from the canvas");
        named(&h, archforge_uml::CLASS, "Customer");
    }
}

/// A selected association class's line shows its end handles though it has a card too: its end
/// dragged onto another class moves there, and onto an interface it is refused in the user's words.
/// In light and dark.
#[test]
fn an_association_class_s_selected_line_keeps_its_end_handles_and_reconnects() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let mut r = reviews(theme);
        let (ac, _, _) = r.review();
        let at = r.line_middle() + egui::vec2(-60.0, 0.0);
        click(&mut r.h, at);
        assert_eq!(r.selection(), [ac.as_str()]);
        let found = ids(&r.h);
        for want in ["workspace.canvas.line-end.source", "workspace.canvas.line-end.target"] {
            assert!(found.contains(&want.to_string()), "{want} in {found:?}");
        }
        let ends = |r: &Reviews| {
            let st = r.h.state().session.as_ref().unwrap().doc();
            let (a, b) = archforge_uml::figures::ends(st, st.by_uuid(archforge_mof::Uuid::parse(&ac).unwrap()).unwrap()).unwrap();
            (archforge_core::name_of(st, a), archforge_core::name_of(st, b))
        };
        // Onto the interface Rated: refused and said; nothing changes.
        let revision = r.h.state().session.as_ref().unwrap().revision();
        let handle = rect_of(&r.h, "workspace.canvas.line-end.target").center();
        let to = r.at("Rated");
        drag(&mut r.h, handle, to);
        assert_eq!(r.h.state().canvas.notice.as_deref(), Some("An association class joins only classes"));
        assert_eq!(r.h.state().session.as_ref().unwrap().revision(), revision);
        // Onto the class Store: the end moves there.
        let at = r.line_middle() + egui::vec2(-60.0, 0.0);
        click(&mut r.h, at);
        let handle = rect_of(&r.h, "workspace.canvas.line-end.target").center();
        let to = r.at("Store");
        drag(&mut r.h, handle, to);
        assert_eq!(ends(&r), ("Customer".to_string(), "Store".to_string()), "notice {:?}", r.h.state().canvas.notice);
    }
}

/// A line selected by its own id on one diagram never stands for the association class's line on
/// another: with the first diagram's edge selected and the second diagram open, Delete and
/// Shift+Delete leave the second's line and the model as they are.
#[test]
fn a_line_selected_on_another_diagram_is_not_taken_for_this_one_s() {
    let mut r = reviews(ThemeKind::Light);
    let (_, edge, _) = r.review();
    let (package, customer, product) = (r.package.clone(), r.ids["Customer"].clone(), r.ids["Product"].clone());
    let s = r.h.state_mut().session.as_mut().unwrap();
    let mut run = |cmd: &str, p: serde_json::Value| s.execute(cmd, &p).unwrap().value["id"].as_str().unwrap_or_default().to_string();
    let d2 = run("diagram.add", serde_json::json!({ "kind": archforge_uml::STRUCTURE_DIAGRAM, "element": package, "name": "Reviews 2" }));
    for (class, x) in [(&customer, 80), (&product, 760)] {
        run("diagram.place", serde_json::json!({ "diagram": d2, "element": class, "x": x, "y": 520, "width": 240, "height": 136 }));
    }
    run("diagram.connect-all", serde_json::json!({ "diagram": d2 }));
    archforge_ui_egui::canvas::open(r.h.state_mut(), archforge_mof::Uuid::parse(&d2).unwrap());
    r.h.run();
    // The first diagram's line selected by its edge's id, the second diagram open.
    archforge_ui_egui::snapshot::run(r.h.state_mut(), "view.select", serde_json::json!({ "ids": [edge] })).unwrap();
    r.h.state_mut().canvas.focused = true;
    r.h.run();
    let revision = r.h.state().session.as_ref().unwrap().revision();
    key(&mut r.h, egui::Key::Delete, egui::Modifiers::NONE);
    key(&mut r.h, egui::Key::Delete, egui::Modifiers::SHIFT);
    assert!(r.h.state().explorer.delete.is_none(), "nothing is asked");
    assert_eq!(r.h.state().session.as_ref().unwrap().revision(), revision, "the second diagram's line and the model stay");
    assert_eq!(r.association_classes(), ["Review"]);
}

/// On a diagram without its card, an association class's line shows its name at its middle, as an
/// association's name sits; with its card there, the name is on the card alone. In light and dark.
#[test]
fn an_association_class_line_without_its_card_shows_its_name_at_its_middle() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let mut r = reviews(theme);
        let (_, edge, card) = r.review();
        let name_id = format!("workspace.canvas.edge.{edge}.name");
        assert!(!ids(&r.h).contains(&name_id), "with its card, the name is on the card");
        let d = r.diagram.clone();
        archforge_ui_egui::snapshot::run(r.h.state_mut(), "diagram.unplace", serde_json::json!({ "nodes": [card] })).unwrap();
        let edge = archforge_ui_egui::snapshot::run(r.h.state_mut(), "diagram.connect-all", serde_json::json!({ "diagram": d })).unwrap();
        assert_eq!(edge["added"], 1);
        r.h.run();
        let (edge, _) = {
            let st = r.h.state().session.as_ref().unwrap().doc();
            let ac = st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_uml::ASSOCIATION_CLASS)).unwrap();
            (st.uuid(archforge_diagram::views_of(st, ac).1[0]).unwrap(), ac)
        };
        let shown = rect_of(&r.h, &format!("workspace.canvas.edge.{edge}.name"));
        let middle = r.line_middle();
        assert!(shown.bottom() < middle.y && (shown.center().x - middle.x).abs() < 4.0, "{shown:?} above {middle:?}");
        assert!(painted(&r.h).iter().filter(|t| *t == "Review").count() >= 1);
    }
}

/// An attribute typed on an association class's card with one of its ends' roles is refused under
/// the row in the user's words, and a role typed in Properties with one of its attributes' names
/// is said at the panel's foot; the model keeps what it had. In light and dark.
#[test]
fn an_association_class_s_attributes_and_end_roles_keep_apart_on_the_card_and_in_properties() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let mut r = reviews(theme);
        let (ac, _, card) = r.review();
        archforge_ui_egui::snapshot::run(
            r.h.state_mut(),
            "uml.association.set-end",
            serde_json::json!({ "association": ac, "end": 0, "role": "reviewer" }),
        )
        .unwrap();
        let ac_id = archforge_mof::Uuid::parse(&ac).unwrap();
        let at = rect_of(&r.h, &format!("workspace.canvas.node.{card}")).center();
        click(&mut r.h, at);
        let at = rect_of(&r.h, "workspace.canvas.adorn.add-attribute").center();
        click(&mut r.h, at);
        r.h.event(egui::Event::Text("reviewer".into()));
        r.h.run();
        key(&mut r.h, egui::Key::Enter, egui::Modifiers::NONE);
        let refused = uml_of(r.h.state()).inline.as_ref().and_then(|i| i.refused.clone());
        assert_eq!(refused.as_deref(), Some("An end of this association class has the role reviewer; an attribute takes another name."));
        assert_eq!(member_rows(&r.h, ac_id, false), ["+ stars : Integer"], "nothing written");
        key(&mut r.h, egui::Key::Escape, egui::Modifiers::NONE);
        // In Properties, the other end's role typed as the attribute's name.
        r.h.state_mut().state.show_panel(archforge_shell_api::state::Panel::Properties);
        r.h.run();
        let at = rect_of(&r.h, "workspace.properties.end.1.role").center();
        click(&mut r.h, at);
        r.h.event(egui::Event::Text("stars".into()));
        r.h.run();
        key(&mut r.h, egui::Key::Enter, egui::Modifiers::NONE);
        assert_eq!(
            r.h.state().panels.notice.as_deref(),
            Some("This association class has an attribute named stars; an end's role takes another name.")
        );
        let st = r.h.state().session.as_ref().unwrap().doc();
        let end = archforge_uml::figures::member_ends(st, st.by_uuid(ac_id).unwrap()).unwrap()[1];
        assert_eq!(archforge_core::name_of(st, end), "", "the role was not written");
    }
}

/// A realization whose implementing class is an association class reconnects its interface end
/// onto another interface; dropped on a class it is refused in the user's words. In light and dark.
#[test]
fn a_realization_from_an_association_class_reconnects_its_interface_end() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let mut r = reviews(theme);
        let (ac, _, card) = r.review();
        let (package, d, rated, rated_node) = (r.package.clone(), r.diagram.clone(), r.ids["Rated"].clone(), r.nodes["Rated"].clone());
        let s = r.h.state_mut().session.as_mut().unwrap();
        let mut run = |cmd: &str, p: serde_json::Value| s.execute(cmd, &p).unwrap().value["id"].as_str().unwrap_or_default().to_string();
        let shown = run("uml.interface.add", serde_json::json!({ "package": package, "name": "Shown" }));
        let shown_node =
            run("diagram.place", serde_json::json!({ "diagram": d, "element": shown, "x": 1100, "y": 200, "width": 240, "height": 136 }));
        let realization = run("uml.realization.add", serde_json::json!({ "class": ac, "interface": rated }));
        run("diagram.connect", serde_json::json!({ "diagram": d, "element": realization, "source": card, "target": rated_node }));
        run("view.select", serde_json::json!({ "ids": [realization] }));
        r.h.state_mut().canvas.fit_pending = true;
        r.h.run();
        let contract = |r: &Reviews| {
            let st = r.h.state().session.as_ref().unwrap().doc();
            let o = st.by_uuid(archforge_mof::Uuid::parse(&realization).unwrap()).unwrap();
            st.uuid(archforge_uml::figures::ends(st, o).unwrap().1).unwrap().to_string()
        };
        // Onto a class: refused in the user's words, nothing changed.
        let revision = r.h.state().session.as_ref().unwrap().revision();
        let handle = rect_of(&r.h, "workspace.canvas.line-end.target").center();
        let to = r.at("Store");
        drag(&mut r.h, handle, to);
        assert_eq!(r.h.state().canvas.notice.as_deref(), Some("A class realizes only an interface"));
        assert_eq!(r.h.state().session.as_ref().unwrap().revision(), revision);
        // Onto another interface: it moves there.
        let handle = rect_of(&r.h, "workspace.canvas.line-end.target").center();
        let to = rect_of(&r.h, &format!("workspace.canvas.node.{shown_node}")).center();
        drag(&mut r.h, handle, to);
        assert_eq!(contract(&r), shown, "notice {:?}", r.h.state().canvas.notice);
    }
}

/// A selected association's ends are set in Properties (role, navigable, aggregation, each one
/// step, a refusal in the interface's words); its end dragged onto another class moves it and its
/// line; Delete removes it, said with Undo.
#[test]
fn a_selected_associations_ends_are_set_in_properties_its_end_drags_to_another_class_and_delete_removes_it() {
    let (mut h, nodes) = related_sample();
    let paid = {
        let st = h.state().session.as_ref().unwrap().doc();
        let a =
            st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_uml::ASSOCIATION) && archforge_core::name_of(st, *o) == "paid by").unwrap();
        st.uuid(a).unwrap()
    };
    h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [paid.to_string()] })).unwrap();
    h.state_mut().state.show_panel(archforge_shell_api::state::Panel::Properties);
    h.run();
    let texts = painted(&h);
    assert!(texts.iter().any(|t| t == "End at Order") && texts.iter().any(|t| t == "End at Payment"), "{texts:?}");
    // A role typed and Enter: written to the model, one step.
    let at = rect_of(&h, "workspace.properties.end.1.role").center();
    click(&mut h, at);
    h.event(egui::Event::Key { key: egui::Key::A, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::COMMAND });
    h.event(egui::Event::Text("settlement".into()));
    h.run();
    key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
    let ends = |h: &mut Harness<'static, Shell>| related(h, "Payment").into_iter().find(|r| r["name"] == "paid by").unwrap()["ends"].clone();
    assert_eq!(ends(&mut h)[1]["role"], "settlement");
    // Both ends navigable: each shows Navigable.
    assert!(ids(&h).contains(&"workspace.properties.end.0.navigable".to_string()));
    // Composite on the Payment end: the Order end allows one, so it is fine. The Order end is now
    // the whole's, where the diamond sits: it stops being navigable and shows no Navigable
    // (owner decision); the Payment end keeps its own.
    let at = rect_of(&h, "workspace.properties.end.1.aggregation").center();
    click(&mut h, at);
    let at = rect_of(&h, "workspace.properties.end.1.aggregation.2").center();
    click(&mut h, at);
    assert_eq!(ends(&mut h)[1]["aggregation"], "composite");
    assert_eq!(ends(&mut h)[0]["navigable"], false);
    let found = ids(&h);
    assert!(!found.contains(&"workspace.properties.end.0.navigable".to_string()), "{found:?}");
    assert!(found.contains(&"workspace.properties.end.1.navigable".to_string()));
    // Undo puts the plain association back, both ends navigable and both showing Navigable.
    h.state_mut().session.as_mut().unwrap().undo().unwrap();
    h.run();
    assert_eq!((ends(&mut h)[0]["navigable"].clone(), ends(&mut h)[1]["aggregation"].clone()), (serde_json::json!(true), serde_json::json!("none")));
    assert!(ids(&h).contains(&"workspace.properties.end.0.navigable".to_string()));
    h.state_mut().session.as_mut().unwrap().redo().unwrap();
    h.run();
    assert!(!ids(&h).contains(&"workspace.properties.end.0.navigable".to_string()));
    // Shared on the Order end too is refused, and said in the interface's words (AR-442).
    let at = rect_of(&h, "workspace.properties.end.0.aggregation").center();
    click(&mut h, at);
    let at = rect_of(&h, "workspace.properties.end.0.aggregation.1").center();
    click(&mut h, at);
    let said = archforge_ui_egui::i18n::tr("The other end has an aggregation already: only the part's end has one.");
    assert_eq!(h.state().panels.notice.as_ref(), Some(&said));
    assert_eq!(ends(&mut h)[0]["aggregation"], "none");
    // The line's target end, at Payment, dragged onto Product: the association and its line move.
    let handle = rect_of(&h, "workspace.canvas.line-end.target").center();
    let to = rect_of(&h, &format!("workspace.canvas.node.{}", nodes["Product"])).center();
    drag(&mut h, handle, to);
    assert!(related(&mut h, "Product").iter().any(|r| r["name"] == "paid by"), "notice {:?}", h.state().canvas.notice);
    assert!(!related(&mut h, "Payment").iter().any(|r| r["name"] == "paid by"));
    let st = h.state().session.as_ref().unwrap().doc();
    let edge = archforge_diagram::views_of(st, st.by_uuid(paid).unwrap()).1[0];
    let target = st.get(edge, archforge_diagram::edge::TARGET).unwrap().as_ref().unwrap();
    assert_eq!(st.uuid(target).unwrap().to_string(), nodes["Product"], "the line follows");
    // Delete on the selected line removes it, said with Undo.
    h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [paid.to_string()] })).unwrap();
    h.run();
    let before = edge_count(&h);
    let at = rect_of(&h, "workspace.canvas").center();
    h.event(egui::Event::PointerMoved(at));
    h.state_mut().canvas.focused = true;
    key(&mut h, egui::Key::Delete, egui::Modifiers::NONE);
    assert_eq!(edge_count(&h), before - 1);
    assert_eq!(h.state().canvas.toast.as_ref().map(|t| t.text.as_str()), Some("Association removed"));
    h.state_mut().session.as_mut().unwrap().undo().unwrap();
    assert_eq!(edge_count(&h), before);
}

/// The Models tree nests packages, lists members, and moves a class by a drag or by Properties.
#[test]
fn the_models_tree_nests_packages_lists_members_and_moves_a_class_by_drag_or_properties() {
    let mut h = workspace_harness(ThemeKind::Light, egui::vec2(1600.0, 1000.0));
    archforge_ui_egui::snapshot::cx(h.state_mut(), uml_ui::open_sample).unwrap();
    h.run();
    // A package added with Commerce selected goes inside it, its name open for typing.
    let commerce = select_named(&mut h, archforge_uml::PACKAGE, "Commerce");
    archforge_ui_egui::explorer::add(h.state_mut(), "package");
    let rename = h.state().explorer.rename.clone().expect("named in place");
    assert_eq!(rename.text, "Package 1");
    h.state_mut().explorer.rename.as_mut().unwrap().text = "Billing".into();
    key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
    h.run();
    let st = h.state().session.as_ref().unwrap().doc();
    let billing = st.alive().find(|o| archforge_core::name_of(st, *o) == "Billing").unwrap();
    assert_eq!(archforge_uml::commands::package_of(st, billing), st.by_uuid(commerce), "nested in Commerce");
    // Expand all opens the packages and the classes: members show with their types.
    let at = rect_of(&h, "workspace.explorer.fold-all").center();
    click(&mut h, at);
    let rows = tree_labels(&h);
    for want in ["Commerce", "  Billing", "  Customer", "    id : Integer", "    email : String", "    rename()"] {
        assert!(rows.iter().any(|r| r == want), "{want} in {rows:?}");
    }
    // Customer dragged onto Billing's row moves there; one undo puts it back.
    let customer = select_named(&mut h, archforge_uml::CLASS, "Customer");
    let billing_id = h.state().session.as_ref().unwrap().doc().uuid(billing).unwrap();
    let from = rect_of(&h, &format!("workspace.explorer.row.{customer}")).center();
    let to = rect_of(&h, &format!("workspace.explorer.row.{billing_id}")).center();
    drag(&mut h, from, to);
    let package_of = |h: &Harness<'static, Shell>, id: archforge_mof::Uuid| {
        let st = h.state().session.as_ref().unwrap().doc();
        archforge_uml::commands::package_of(st, st.by_uuid(id).unwrap()).map(|p| archforge_core::name_of(st, p))
    };
    assert_eq!(package_of(&h, customer).as_deref(), Some("Billing"), "notice {:?}", h.state().explorer.notice);
    h.state_mut().session.as_mut().unwrap().undo().unwrap();
    assert_eq!(package_of(&h, customer).as_deref(), Some("Commerce"));
    // Properties' Package select moves Order into Billing.
    let order = select_named(&mut h, archforge_uml::CLASS, "Order");
    h.state_mut().state.show_panel(archforge_shell_api::state::Panel::Properties);
    h.run();
    let at = rect_of(&h, "workspace.properties.class.owner").center();
    click(&mut h, at);
    let i = {
        let st = h.state().session.as_ref().unwrap().doc();
        archforge_uml::commands::all_packages(st).iter().position(|p| *p == billing).unwrap()
    };
    let at = rect_of(&h, &format!("workspace.properties.class.owner.{i}")).center();
    click(&mut h, at);
    assert_eq!(package_of(&h, order).as_deref(), Some("Billing"));
    // On the Commerce diagram Order is now an outsider, its package named.
    let st = h.state().session.as_ref().unwrap().doc();
    let d = st.by_uuid(h.state().canvas.active.unwrap()).unwrap();
    let card = archforge_uml::figures::card(st, st.by_uuid(order).unwrap(), d).unwrap();
    assert!(card.elsewhere, "an outsider on Commerce's diagram");
    assert_eq!(card.eyebrow, archforge_diagram::figure::Text::Said("Class · {package}", vec![("package", "Billing".into())]));
    // Billing removed: Order moves up to Commerce, said in the toast; undo brings Billing back.
    select_named(&mut h, archforge_uml::PACKAGE, "Billing");
    archforge_ui_egui::explorer::remove_selection(h.state_mut());
    assert!(h.state().explorer.delete.is_none(), "no dialog: nothing goes");
    assert_eq!(package_of(&h, order).as_deref(), Some("Commerce"));
    assert_eq!(h.state().canvas.toast.as_ref().map(|t| t.text.as_str()), Some("Billing removed; what it held moved to Commerce"));
    h.state_mut().session.as_mut().unwrap().undo().unwrap();
    assert_eq!(package_of(&h, order).as_deref(), Some("Billing"));
}

/// A class's menu deletes it from the model, with its card and line, after asking; undo brings them
/// back and redo deletes them again; Remove from this diagram keeps the class in the model.
#[test]
fn a_class_is_deleted_from_the_model_through_its_menu_and_comes_back_with_undo() {
    let (mut h, d, _) = read_class_diagram("paid by");
    let class_named = |h: &Harness<'static, Shell>, name: &str| {
        let st = h.state().session.as_ref().unwrap().doc();
        st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_uml::CLASS) && archforge_core::name_of(st, *o) == name)
    };
    let node_of = |h: &Harness<'static, Shell>, name: &str| {
        let st = h.state().session.as_ref().unwrap().doc();
        let c = class_named(h, name).unwrap();
        let n = archforge_diagram::views_of(st, c).2.into_iter().find(|n| st.container(*n).ok().flatten().map(|(p, _)| p) == Some(d)).unwrap();
        format!("workspace.canvas.node.{}", st.uuid(n).unwrap())
    };
    let customer = node_of(&h, "Customer");
    let lines_before = h.state().session.as_ref().unwrap().doc().list(d, archforge_diagram::diagram::EDGE).unwrap().len();
    // A right click on a class opens its menu: Add attribute, Remove from this diagram, Delete from the model.
    let at = rect_of(&h, &customer).center();
    h.event(egui::Event::PointerMoved(at));
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Secondary, pressed, modifiers: egui::Modifiers::NONE });
    }
    h.run();
    assert!(h.state().canvas.menu.is_some(), "the menu opened");
    let found = ids(&h);
    for row in ["open", "add-row", "remove", "delete"] {
        assert!(found.contains(&format!("workspace.canvas.menu.{row}")), "{row}: {found:?}");
    }
    assert!(!found.iter().any(|i| i == "workspace.canvas.menu.related" || i == "workspace.canvas.menu.add-column"), "no table rows");
    // Delete from the model asks first, naming what goes with it, then deletes.
    let at = rect_of(&h, "workspace.canvas.menu.delete").center();
    click(&mut h, at);
    let ask = h.state().explorer.delete.clone().expect("the dialog asks");
    assert_eq!(
        (ask.name.as_str(), ask.consequences.as_slice(), ask.diagrams),
        ("Customer", ["1 relationship goes with it.".to_string()].as_slice(), 1)
    );
    let at = rect_of(&h, "workspace.explorer.delete.confirm").center();
    click(&mut h, at);
    assert!(class_named(&h, "Customer").is_none(), "the class left the model");
    assert!(!ids(&h).contains(&customer), "and the diagram");
    let st = h.state().session.as_ref().unwrap().doc();
    assert_eq!(st.list(d, archforge_diagram::diagram::EDGE).unwrap().len(), lines_before - 1, "its line went with it");
    // Undo brings the class, its card and its line back; redo deletes them again.
    h.state_mut().session.as_mut().unwrap().undo().unwrap();
    h.run();
    assert!(class_named(&h, "Customer").is_some());
    assert!(ids(&h).contains(&customer));
    h.state_mut().session.as_mut().unwrap().redo().unwrap();
    h.run();
    assert!(class_named(&h, "Customer").is_none());
    // Remove from this diagram keeps the class in the model.
    let order = node_of(&h, "OrderLine");
    let at = rect_of(&h, &order).center();
    h.event(egui::Event::PointerMoved(at));
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Secondary, pressed, modifiers: egui::Modifiers::NONE });
    }
    h.run();
    let at = rect_of(&h, "workspace.canvas.menu.remove").center();
    click(&mut h, at);
    assert!(!ids(&h).contains(&order), "the card left the diagram");
    assert!(class_named(&h, "OrderLine").is_some(), "the class stays in the model");
}

/// An end moved to another class drops, in the same step, what it no longer subsets, and the
/// canvas says so in the user's words; a role edit afterwards writes. In light and dark.
#[test]
fn a_moved_end_drops_what_it_no_longer_subsets_and_says_so() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, made) = modifiers_sample(theme);
        let holds = made["holds"].clone();
        let s = h.state_mut().session.as_mut().unwrap();
        s.execute("uml.association.set-end", &serde_json::json!({ "association": holds, "end": 1, "subsets": [made["lines"]] })).unwrap();
        let st = s.doc();
        let item = st.by_uuid(archforge_mof::Uuid::parse(&made["Item"]).unwrap()).unwrap();
        let node = archforge_diagram::views_of(st, item).2.first().and_then(|n| st.uuid(*n).ok()).unwrap();
        let at = h.state().session.as_ref().unwrap().token();
        archforge_ui_egui::snapshot::cx(h.state_mut(), |cx| uml_relate::reconnect(cx, archforge_mof::Uuid::parse(&holds).unwrap(), true, node, at));
        h.run();
        assert_eq!(h.state().canvas.notice.as_deref(), Some("The moved ends no longer subset or redefine lines, so those were dropped."));
        assert!(
            painted(&h).iter().any(|t| t == "The moved ends no longer subset or redefine lines, so those were dropped."),
            "said at the canvas's top"
        );
        let st = h.state().session.as_ref().unwrap().doc();
        let end = archforge_uml::figures::member_ends(st, st.by_uuid(archforge_mof::Uuid::parse(&holds).unwrap()).unwrap()).unwrap()[1];
        assert!(archforge_uml::modifiers::subsets(st, end).is_empty());
        let roled = archforge_ui_egui::snapshot::run(
            h.state_mut(),
            "uml.association.set-end",
            serde_json::json!({ "association": holds, "end": 1, "role": "goods" }),
        );
        assert!(roled.is_ok(), "{roled:?}");
    }
}

/// A qualifier is added on the Order end in Properties with the row editor, its type from the
/// shared type selector; Apply is one undo step, the section says the end is qualified, the far
/// end's multiplicity counts per qualifier value, and the box is drawn against Order on both
/// diagrams; a second is moved up and removed, each one undo step. In light and dark.
#[test]
fn qualifiers_are_added_moved_and_removed_in_properties_and_boxed_on_the_line() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, made) = qualifiers_sample(theme);
        let a = made["contains"].clone();
        assert!(painted(&h).iter().any(|t| t == "Qualifiers · 0"), "{:?}", painted(&h));
        let at = rect_of(&h, "workspace.properties.end.0.qualifiers.add").center();
        click(&mut h, at);
        let found = ids(&h);
        for id in ["name", "type", "multiplicity", "cancel", "apply"] {
            assert!(found.iter().any(|i| *i == format!("workspace.properties.end.0.qualifier.0.{id}")), "{theme:?}: {id} in {found:?}");
        }
        assert!(!found.iter().any(|i| i == "workspace.properties.end.0.qualifier.0.remove"), "a new one has no bin");
        // The name takes the keyboard as the editor opens.
        h.event(egui::Event::Text("lineNo".into()));
        h.run();
        pick_type(&mut h, "workspace.properties.end.0.qualifier.0.type", "Integer");
        assert!(painted(&h).iter().any(|t| t == "One order and one lineNo pick 0..1 line."), "{:?}", painted(&h));
        assert_eq!(qualifier_rows(&h, &a), [Vec::<String>::new(), Vec::new()], "nothing before Apply");
        let at = rect_of(&h, "workspace.properties.end.0.qualifier.0.apply").center();
        click(&mut h, at);
        assert!(uml_of(h.state()).qualifier.is_none(), "applied, the editor closes");
        assert_eq!(qualifier_rows(&h, &a), [vec!["lineNo : Integer".to_string()], Vec::new()]);
        assert_eq!(h.state().canvas.moved_ends.len(), 1, "its end moved out to the box, for the next frame's handle");
        assert_eq!(h.state().session.as_ref().unwrap().undo_label(), Some("Add qualifier"));
        h.run();
        let shown = painted(&h);
        for said in [
            "Qualifiers · 1",
            "lineNo : Integer",
            "Qualified end: the qualifier box is drawn here, against Order.",
            "Per qualifier value",
            "Counted per qualifier value of the other end; shown next to OrderLine.",
        ] {
            assert!(shown.iter().any(|t| t == said), "{theme:?}: {said} in {shown:?}");
        }
        let edge = edge_on(&h, &a, &made["diagram"]);
        assert!(ids(&h).iter().any(|i| *i == format!("workspace.canvas.edge.{edge}.qualifiers.0")), "the box at Order");
        assert!(!ids(&h).iter().any(|i| *i == format!("workspace.canvas.edge.{edge}.qualifiers.1")), "none at OrderLine");
        // The box sits against Order's card, between it and the line.
        let st = h.state().session.as_ref().unwrap().doc();
        let order = st.by_uuid(archforge_mof::Uuid::parse(&made["Order"]).unwrap()).unwrap();
        let node = archforge_diagram::views_of(st, order).2[0];
        let card = rect_of(&h, &format!("workspace.canvas.node.{}", st.uuid(node).unwrap()));
        let boxed = rect_of(&h, &format!("workspace.canvas.edge.{edge}.qualifiers.0"));
        let touching = [boxed.top() - card.bottom(), boxed.bottom() - card.top(), boxed.left() - card.right(), boxed.right() - card.left()];
        assert!(touching.iter().any(|g| g.abs() < 1.5) && !card.shrink(2.0).intersects(boxed), "{boxed:?} against {card:?}");
        h.state_mut().session.as_mut().unwrap().undo().unwrap();
        assert_eq!(qualifier_rows(&h, &a)[0], Vec::<String>::new(), "one undo step");
        h.state_mut().session.as_mut().unwrap().redo().unwrap();
        h.run();
        // The same box on the second diagram: the content is the model's.
        archforge_ui_egui::canvas::open(h.state_mut(), archforge_mof::Uuid::parse(&made["second"]).unwrap());
        h.run();
        let edge2 = edge_on(&h, &a, &made["second"]);
        assert!(ids(&h).iter().any(|i| *i == format!("workspace.canvas.edge.{edge2}.qualifiers.0")), "the box on the second diagram");
        assert!(painted(&h).iter().any(|t| t == "lineNo : Integer"));
        h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [a] })).unwrap();
        h.run();
        // A second qualifier, moved up with the editor's arrow, then removed with its bin.
        let at = rect_of(&h, "workspace.properties.end.0.qualifiers.add").center();
        click(&mut h, at);
        h.event(egui::Event::Text("kind".into()));
        h.run();
        key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
        assert_eq!(qualifier_rows(&h, &a)[0], ["lineNo : Integer", "kind"], "Enter applies; rows in model order");
        let row = rect_of(&h, "workspace.properties.end.0.qualifier.1").left_center() + egui::vec2(20.0, 0.0);
        h.event(egui::Event::PointerMoved(row));
        h.run();
        let at = rect_of(&h, "workspace.properties.end.0.qualifier.1.edit").center();
        click(&mut h, at);
        let at = rect_of(&h, "workspace.properties.end.0.qualifier.1.up").center();
        click(&mut h, at);
        assert_eq!(qualifier_rows(&h, &a)[0], ["kind", "lineNo : Integer"]);
        assert_eq!(h.state().session.as_ref().unwrap().undo_label(), Some("Move qualifier"));
        h.run();
        let at = rect_of(&h, "workspace.properties.end.0.qualifier.0.remove").center();
        click(&mut h, at);
        assert_eq!(qualifier_rows(&h, &a)[0], ["lineNo : Integer"]);
        h.state_mut().session.as_mut().unwrap().undo().unwrap();
        assert_eq!(qualifier_rows(&h, &a)[0], ["kind", "lineNo : Integer"], "the removal is one step");
        // A row's click selects the qualifier: Properties stays the association's, the row marked.
        uml_of_mut(h.state_mut()).qualifier = None;
        h.run();
        let at = rect_of(&h, "workspace.properties.end.0.qualifier.1").left_center() + egui::vec2(20.0, 0.0);
        click(&mut h, at);
        h.run();
        let st = h.state().session.as_ref().unwrap();
        let chosen = st.view().selection.first().copied().unwrap();
        assert!(archforge_uml::qualifiers::is_qualifier(st.doc(), chosen), "the qualifier is selected");
        let shown = painted(&h);
        assert!(shown.iter().any(|t| t == "Qualifier · model") && shown.iter().any(|t| t == "Qualifiers · 2"), "{shown:?}");
    }
}

/// A selected qualifier draws its association's line selected, as its notation says the line goes
/// with it (AR-127, AR-784); with its class selected instead the line is not. In light and dark.
#[test]
fn a_selected_qualifier_draws_its_associations_line_selected() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, ids) = qualifiers_sample(theme);
        let s = h.state_mut().session.as_mut().unwrap();
        let q = s.execute("uml.qualifier.add", &serde_json::json!({ "association": ids["contains"], "end": 0, "name": "lineNo" })).unwrap();
        let q = q.value["id"].as_str().unwrap().to_string();
        let edge = {
            let st = s.doc();
            let a = st.by_uuid(archforge_mof::Uuid::parse(&ids["contains"]).unwrap()).unwrap();
            let d = st.by_uuid(archforge_mof::Uuid::parse(&ids["diagram"]).unwrap()).unwrap();
            let e = archforge_diagram::views_of(st, a).1.into_iter().find(|e| archforge_diagram::diagram_of(st, *e) == Some(d)).unwrap();
            st.uuid(e).unwrap()
        };
        for (selected, marked) in [(&ids["Order"], false), (&q, true)] {
            h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [selected] })).unwrap();
            h.run();
            let line = rect_of(&h, &format!("workspace.canvas.edge.{edge}")).expand(2.0);
            assert_eq!(accent_path_in(&h, theme, line), marked, "{theme:?}: the line drawn selected with {selected}");
        }
    }
}

/// The nested sample of the snapshot screen (D2), in light and dark: Priority nested in Order, its
/// card above Order's on the open diagram reading its own name, the ⊕ line's route between the two
/// cards, Models listing it under Order with `nested`, and its Properties naming Order as its owner.
#[test]
fn the_nested_sample_shows_its_owner_in_models_properties_and_on_the_canvas() {
    use egui_kittest::kittest::Queryable;
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, _) = related_sample_in(theme, egui::vec2(1600.0, 1000.0));
        let (priority, order) = archforge_ui_egui::snapshot::cx(h.state_mut(), uml_ui::put_nested_priority).unwrap();
        h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [priority] })).unwrap();
        h.state_mut().state.show_panel(archforge_shell_api::state::Panel::Properties);
        h.run();
        let card = |h: &Harness<'static, Shell>, id: &str| {
            let st = h.state().session.as_ref().unwrap().doc();
            let o = st.by_uuid(archforge_mof::Uuid::parse(id).unwrap()).unwrap();
            let n = archforge_diagram::views_of(st, o).2[0];
            rect_of(h, &format!("workspace.canvas.node.{}", st.uuid(n).unwrap()))
        };
        let (p, o) = (card(&h, &priority), card(&h, &order));
        assert!(archforge_shell_api::canvas::nesting_route(o, p, 7.0, &[]).is_some(), "{theme:?}: a line ties them");
        assert!(painted(&h).iter().any(|t| t == "Priority"), "its own name with Order shown");
        assert!(!painted(&h).iter().any(|t| t == "Order::Priority"));
        assert!(painted(&h).iter().any(|t| t == "nested"));
        assert!(h.query_by_label("Owner: Order").is_some());
        assert!(painted(&h).iter().any(|t| t.starts_with("Owner is a package or a classifier.")));
        assert!(!h.state().session.as_ref().unwrap().is_dirty(), "the sample is marked saved");
    }
}

/// Template parameters (design page 08, C1 and C4) are added in Properties with the row editor:
/// T as a class parameter, then Key whose Kind menu lists the supported kinds and Value and
/// Operation disabled, never picked, with Integer as its default; each Apply one undo step; the
/// signature box sits over the card's top-right corner on both diagrams; Key moved up, renamed and
/// removed with the bin. In light and dark.
#[test]
fn template_parameters_are_edited_in_properties_and_boxed_on_both_diagrams() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, made) = templates_sample(theme);
        let repo = made["Repository"].clone();
        assert!(painted(&h).iter().any(|t| t == "Template parameters · 0"), "{theme:?}: {:?}", painted(&h));
        let at = rect_of(&h, "workspace.properties.class.templateParameters.add").center();
        click(&mut h, at);
        let found = ids(&h);
        for id in ["name", "kind", "constraint", "default", "cancel", "apply"] {
            assert!(found.iter().any(|i| *i == format!("workspace.properties.class.templateParameter.0.{id}")), "{theme:?}: {id} in {found:?}");
        }
        assert!(!found.iter().any(|i| i == "workspace.properties.class.templateParameter.0.remove"), "a new one has no bin");
        h.event(egui::Event::Text("T".into()));
        h.run();
        assert!(painted(&h).iter().any(|t| t.starts_with("Arguments for T must be classes.")), "{:?}", painted(&h));
        assert!(signature_of(&h, &repo).is_empty(), "nothing before Apply");
        let at = rect_of(&h, "workspace.properties.class.templateParameter.0.apply").center();
        click(&mut h, at);
        assert!(uml_of(h.state()).template.is_none(), "applied, the editor closes");
        assert_eq!(signature_of(&h, &repo), ["T : Class"]);
        assert_eq!(h.state().session.as_ref().unwrap().undo_label(), Some("Add template parameter"));
        h.run();
        let shown = painted(&h);
        for said in ["Template parameters · 1", "T : Class", "Class · template, 1 parameter · model, in every diagram"] {
            assert!(shown.iter().any(|t| t == said), "{theme:?}: {said} in {shown:?}");
        }
        // T types Repository's own members alone: its type menus offer it, noted, and no other's do.
        let st = h.state().session.as_ref().unwrap().doc();
        let r = st.by_uuid(archforge_mof::Uuid::parse(&repo).unwrap()).unwrap();
        let inside = uml_panel::type_choices(st, Some(r));
        assert_eq!(inside.get(1).map(|(l, _)| l.as_str()), Some("T"), "nearest first: {inside:?}");
        assert_eq!(uml_panel::type_notes(st, &inside).get(1).cloned(), Some(vec!["template parameter".to_string()]));
        assert!(!uml_panel::type_choices(st, None).iter().any(|(l, _)| l == "T"), "never outside the template");
        // Key: the Kind menu offers the supported kinds; Value and Operation are named, disabled.
        let at = rect_of(&h, "workspace.properties.class.templateParameters.add").center();
        click(&mut h, at);
        h.event(egui::Event::Text("Key".into()));
        h.run();
        let kind = "workspace.properties.class.templateParameter.1.kind";
        let at = rect_of(&h, kind).center();
        click(&mut h, at);
        let shown = painted(&h);
        for said in ["PARAMETER KIND", "Class", "Interface", "Data type", "Enumeration", "Primitive type", "Value", "Operation", "not yet"] {
            assert!(shown.iter().any(|t| t == said), "{theme:?}: {said} in {shown:?}");
        }
        let at = rect_of(&h, &format!("{kind}.value")).center();
        click(&mut h, at);
        assert_eq!(uml_of(h.state()).template.as_ref().unwrap().kind, archforge_uml::templates::Kind::Class, "a kind not supported is never taken");
        assert_eq!(h.state().panels.menu, Some(uml_state::menus::TEMPLATE_KIND), "the menu stays open");
        let at = rect_of(&h, &format!("{kind}.data-type")).center();
        click(&mut h, at);
        assert_eq!(uml_of(h.state()).template.as_ref().unwrap().kind, archforge_uml::templates::Kind::DataType);
        pick_entry(&mut h, "workspace.properties.class.templateParameter.1.default", "1");
        assert_eq!(uml_of(h.state()).template.as_ref().unwrap().default, "Integer", "the built-in types first");
        let at = rect_of(&h, "workspace.properties.class.templateParameter.1.apply").center();
        click(&mut h, at);
        assert_eq!(signature_of(&h, &repo), ["T : Class", "Key : DataType = Integer"]);
        // The box over the card's top-right corner, on both diagrams: the content is the model's.
        for d in ["diagram", "second"] {
            archforge_ui_egui::canvas::open(h.state_mut(), archforge_mof::Uuid::parse(&made[d]).unwrap());
            h.run();
            let node = node_on(&h, &repo, &made[d]);
            let card = rect_of(&h, &format!("workspace.canvas.node.{node}"));
            let sig = rect_of(&h, &format!("workspace.canvas.node.{node}.signature"));
            assert!(sig.contains(card.right_top()) && sig.right() > card.right() && sig.top() < card.top(), "{d}: {sig:?} over {card:?}");
            assert!(painted(&h).iter().any(|t| t == "Key : DataType = Integer"), "{d}");
        }
        // Key moved up with the editor's arrow, renamed, then removed with its bin.
        h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [repo] })).unwrap();
        h.run();
        let row = rect_of(&h, "workspace.properties.class.templateParameter.1").left_center() + egui::vec2(20.0, 0.0);
        h.event(egui::Event::PointerMoved(row));
        h.run();
        let at = rect_of(&h, "workspace.properties.class.templateParameter.1.edit").center();
        click(&mut h, at);
        let at = rect_of(&h, "workspace.properties.class.templateParameter.1.up").center();
        click(&mut h, at);
        assert_eq!(signature_of(&h, &repo), ["Key : DataType = Integer", "T : Class"]);
        assert_eq!(h.state().session.as_ref().unwrap().undo_label(), Some("Move template parameter"));
        h.run();
        let at = rect_of(&h, "workspace.properties.class.templateParameter.0.name").center();
        click(&mut h, at);
        key(&mut h, egui::Key::A, egui::Modifiers::COMMAND);
        h.event(egui::Event::Text("Id".into()));
        h.run();
        let at = rect_of(&h, "workspace.properties.class.templateParameter.0.apply").center();
        click(&mut h, at);
        assert_eq!(signature_of(&h, &repo), ["Id : DataType = Integer", "T : Class"]);
        let at = rect_of(&h, "workspace.properties.class.templateParameter.0.edit").center();
        click(&mut h, at);
        let at = rect_of(&h, "workspace.properties.class.templateParameter.0.remove").center();
        click(&mut h, at);
        assert_eq!(signature_of(&h, &repo), ["T : Class"]);
        h.state_mut().session.as_mut().unwrap().undo().unwrap();
        assert_eq!(signature_of(&h, &repo), ["Id : DataType = Integer", "T : Class"], "the removal is one step");
    }
}

/// A template binding is drawn from OrderRepository to Repository (design page 08, C3): releasing it
/// opens the draft in Properties, which shows unfolded, and writes nothing; Apply with T still to pick is refused naming
/// it, the draft kept; Cancel, and Escape, leave the model and its history as they were; with T
/// picked, Apply adds the binding, its line and its label as one undo step and selects it, Key on
/// its default. In light and dark.
#[test]
fn a_template_binding_is_drawn_and_applied_from_its_draft() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, _, (from, to)) = binding_sample(theme);
        let d = h.state().canvas.active.unwrap();
        let undo = h.state().session.as_ref().unwrap().undo_label().map(str::to_string);
        uml_of_mut(h.state_mut()).relation = uml_relate::Relate::TemplateBinding;
        let draw = |h: &mut Harness<'static, Shell>| {
            let d = h.state().session.as_ref().unwrap().doc().by_uuid(d).unwrap();
            archforge_ui_egui::snapshot::cx(h.state_mut(), |cx| uml_relate::connect(cx, d, from, to));
            h.run();
        };
        // Another panel, folded away, gives way to the draft in Properties.
        (h.state_mut().state.panel, h.state_mut().state.panel_collapsed) = (archforge_shell_api::state::Panel::Skills, true);
        draw(&mut h);
        assert_eq!(
            (h.state().state.panel, h.state().state.panel_collapsed),
            (archforge_shell_api::state::Panel::Properties, false),
            "{theme:?}: Properties shows"
        );
        assert!(bindings_in(&h).is_empty(), "{theme:?}: nothing in the model while drawing");
        assert_eq!(h.state().session.as_ref().unwrap().undo_label().map(str::to_string), undo);
        let shown = painted(&h);
        assert!(shown.iter().any(|t| t == "New template binding") && shown.iter().any(|t| t == "Choose a class"), "{shown:?}");
        assert!(shown.iter().any(|t| t == "default"), "Key shows its default");
        let st = h.state().session.as_ref().unwrap().doc();
        assert_eq!(
            uml_bindings::draft_line(&archforge_ui_egui::testing::cx_ref(h.state()), st.by_uuid(d).unwrap()),
            Some((from, to)),
            "the draft's line is drawn"
        );
        for id in ["bound", "template", "substitution.0.argument", "substitution.1.argument", "apply", "cancel"] {
            assert!(ids(&h).iter().any(|i| *i == format!("workspace.properties.templateBinding.{id}")), "{id}");
        }
        // Apply with T to pick: refused, said, the draft kept.
        let at = rect_of(&h, "workspace.properties.templateBinding.apply").center();
        click(&mut h, at);
        assert!(painted(&h).iter().any(|t| t.contains("T needs an argument: pick a class.")), "{:?}", painted(&h));
        assert!(ids(&h).iter().any(|i| i == "workspace.properties.templateBinding.refusal"));
        assert!(uml_of(h.state()).binding.is_some() && bindings_in(&h).is_empty());
        // Cancel leaves nothing; so does Escape.
        let at = rect_of(&h, "workspace.properties.templateBinding.cancel").center();
        click(&mut h, at);
        assert!(uml_of(h.state()).binding.is_none() && bindings_in(&h).is_empty());
        draw(&mut h);
        key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
        assert!(uml_of(h.state()).binding.is_none() && bindings_in(&h).is_empty());
        assert_eq!(h.state().session.as_ref().unwrap().undo_label().map(str::to_string), undo, "{theme:?}: the history as it was");
        // T picked from its selector (Entity, Order, OrderRepository, Repository), then Apply.
        draw(&mut h);
        pick_entry(&mut h, "workspace.properties.templateBinding.substitution.0.argument", "1");
        let at = rect_of(&h, "workspace.properties.templateBinding.apply").center();
        click(&mut h, at);
        let made_now = bindings_in(&h);
        assert_eq!(made_now.len(), 1, "{theme:?}");
        let s = h.state().session.as_ref().unwrap();
        assert_eq!(s.undo_label(), Some("Add template binding"));
        assert_eq!(s.view().selection, made_now, "the binding is selected");
        let st = s.doc();
        assert_eq!(archforge_uml::figures::slot_text(st, made_now[0], "name"), "«bind» <T -> Order, Key -> Integer>");
        assert_eq!(archforge_diagram::views_of(st, made_now[0]).1.len(), 1, "its line");
        h.run();
        assert!(painted(&h).iter().any(|t| t == "«bind» <T -> Order, Key -> Integer>"), "the label on the canvas");
        assert!(ids(&h).iter().any(|i| i == "workspace.properties.templateBinding.substitution.1.argument"), "its Properties");
        h.state_mut().session.as_mut().unwrap().undo().unwrap();
        assert!(bindings_in(&h).is_empty(), "one undo step");
    }
}

/// A press on the canvas away from UML's menu of generalization lines closes it (AR-789): opened by
/// a right click on one of three selected lines, it goes with a click on another of them, though the
/// selection that click leaves still holds a generalization the menu could act on. The menu reads a
/// press outside it itself too, so this holds the outcome whichever of the two ends it. In light
/// and dark.
#[test]
fn a_press_on_the_canvas_closes_umls_line_menu() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, _) = related_sample_in(theme, egui::vec2(1600.0, 1000.0));
        let (gens, _) = archforge_ui_egui::snapshot::cx(h.state_mut(), uml_ui::put_payment_kinds).unwrap();
        h.state_mut().canvas.zoom = 0.75;
        h.state_mut().canvas.pan = egui::vec2(-470.0, -230.0);
        h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": gens })).unwrap();
        h.run();
        let edge = |h: &Harness<'static, Shell>, g: &str| {
            let st = h.state().session.as_ref().unwrap().doc();
            st.uuid(archforge_diagram::views_of(st, st.by_uuid(archforge_mof::Uuid::parse(g).unwrap()).unwrap()).1[0]).unwrap()
        };
        let at = point_on_edge(&h, edge(&h, &gens[1]));
        right_click(&mut h, at);
        assert!(uml_of(h.state()).line_menu.is_some(), "{theme:?}: the menu opened");
        let menu = ids(&h).iter().filter(|i| i.starts_with("workspace.canvas.contextMenu")).fold(egui::Rect::NOTHING, |r, i| r.union(rect_of(&h, i)));
        let away = [&gens[0], &gens[2]].into_iter().map(|g| point_on_edge(&h, edge(&h, g))).find(|p| !menu.expand(8.0).contains(*p)).unwrap();
        click(&mut h, away);
        let st = h.state().session.as_ref().unwrap().doc();
        let held = h.state().session.as_ref().unwrap().view().selection.iter().any(|o| archforge_uml::generalization_sets::is_generalization(st, *o));
        assert!(held, "{theme:?}: a generalization is still selected");
        assert!(uml_of(h.state()).line_menu.is_none(), "{theme:?}: the menu closed");
    }
}

/// A generalization set's label says its whole text after "Generalization set" while the pointer is
/// on it (AR-129), in the words UML gives the label as it draws it (AR-789). In light and dark.
#[test]
fn a_generalization_sets_label_says_its_hover_text_in_umls_words() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, _) = related_sample_in(theme, egui::vec2(1600.0, 1000.0));
        let (gens, _) = archforge_ui_egui::snapshot::cx(h.state_mut(), uml_ui::put_payment_kinds).unwrap();
        let s = h.state_mut().session.as_mut().unwrap();
        let set = s.execute("uml.generalization-set.add", &serde_json::json!({ "generalizations": gens, "name": "method", "complete": true }));
        let set = archforge_mof::Uuid::parse(set.unwrap().value["id"].as_str().unwrap()).unwrap();
        h.run();
        let labels = h.state().canvas.extra_labels.clone();
        let hover = "Generalization set method {complete, overlapping}";
        assert_eq!(labels.iter().map(|l| (l.element, l.hover.as_str())).collect::<Vec<_>>(), [(set, hover)], "{theme:?}");
        // The canvas says it while the pointer rests on the label.
        let before = painted(&h).iter().filter(|t| *t == hover).count();
        h.event(egui::Event::PointerMoved(labels[0].rect.center()));
        for _ in 0..40 {
            h.step();
        }
        assert_eq!(painted(&h).iter().filter(|t| *t == hover).count(), before + 1, "{theme:?}: its tooltip");
    }
}

/// The canvas draws UML's draft of a template binding where the drafts go (AR-789): accent dashes
/// from OrderRepository to Repository, the open arrow's tip on Repository's border; none once the
/// draft is cancelled. In light and dark.
#[test]
fn the_canvas_draws_a_template_bindings_draft_with_its_arrow_at_the_template() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, _, (from, to)) = binding_sample(theme);
        h.run();
        assert!(accent_segments(&h).is_empty(), "{theme:?}: nothing drafted yet");
        uml_of_mut(h.state_mut()).relation = uml_relate::Relate::TemplateBinding;
        let d = h.state().session.as_ref().unwrap().doc().by_uuid(h.state().canvas.active.unwrap()).unwrap();
        archforge_ui_egui::snapshot::cx(h.state_mut(), |cx| uml_relate::connect(cx, d, from, to));
        h.run();
        let segments = accent_segments(&h);
        assert!(segments.len() > 3, "{theme:?}: dashes and an arrow, {segments:?}");
        // The arrow's two strokes meet at its tip; no dash shares an end with another.
        let tips: Vec<egui::Pos2> = segments
            .iter()
            .enumerate()
            .flat_map(|(i, a)| segments.iter().skip(i + 1).flat_map(move |b| a.iter().filter(move |p| b.contains(p)).copied()))
            .collect();
        let (bound, template) = (rect_of(&h, &format!("workspace.canvas.node.{from}")), rect_of(&h, &format!("workspace.canvas.node.{to}")));
        assert!(
            !tips.is_empty() && tips.iter().all(|t| template.expand(1.0).contains(*t) && !bound.expand(1.0).contains(*t)),
            "{theme:?}: the tip {tips:?} is on Repository {template:?}, not OrderRepository {bound:?}"
        );
        uml_of_mut(h.state_mut()).binding = None;
        h.run();
        assert!(accent_segments(&h).is_empty(), "{theme:?}: dropped, nothing drawn");
    }
}

/// A card typed on is drawn grown by the new row, as its notation's draft says (AR-789): Catalog's
/// card is drawn taller than its node while a new attribute is typed on it. In light and dark.
#[test]
fn the_canvas_draws_a_card_grown_by_the_row_typed_on_it() {
    /// The tallest filled rectangle drawn from the node's top left corner, as wide: the card's
    /// body.
    fn drawn_height(h: &Harness<'static, Shell>, node: egui::Rect) -> f32 {
        /// Raises `out` to the height of each such rectangle in `s` and the shapes it holds.
        fn walk(s: &egui::Shape, node: egui::Rect, out: &mut f32) {
            match s {
                egui::Shape::Rect(r) if (r.rect.min - node.min).length() < 0.5 && (r.rect.width() - node.width()).abs() < 0.5 => {
                    *out = out.max(r.rect.height());
                }
                egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, node, out)),
                _ => {}
            }
        }
        let mut out = 0.0;
        h.output().shapes.iter().for_each(|c| walk(&c.shape, node, &mut out));
        out
    }
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, nodes) = related_sample_in(theme, egui::vec2(1600.0, 1000.0));
        let id = format!("workspace.canvas.node.{}", nodes["Catalog"]);
        let class = {
            let st = h.state().session.as_ref().unwrap().doc();
            let node = st.by_uuid(archforge_mof::Uuid::parse(&nodes["Catalog"]).unwrap()).unwrap();
            st.uuid(st.get(node, archforge_diagram::node::ELEMENT).unwrap().as_ref().unwrap()).unwrap()
        };
        h.run();
        let node = rect_of(&h, &id);
        let plain = drawn_height(&h, node);
        assert!((plain - node.height()).abs() < 0.5, "{theme:?}: drawn as its node, {plain} {node:?}");
        assert!(
            archforge_ui_egui::snapshot::cx(h.state_mut(), |cx| uml_inline::open(cx, class, uml_panel::MemberKind::Attribute, None)),
            "{theme:?}"
        );
        h.run();
        let grown = drawn_height(&h, rect_of(&h, &id));
        assert!(grown > node.height() + 1.0, "{theme:?}: grown by the row, {grown} over {}", node.height());
    }
}

/// An n-ary association's card is UML's to draw (AR-128): the canvas records it as drawn by UML's
/// kind, and its diamond, filled with the canvas's colour, has its corners at the middles of its
/// node's sides. In light and dark.
#[test]
fn umls_kind_draws_an_n_ary_associations_diamond_as_its_card() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (h, ids) = nary_sample(theme, true);
        let node = archforge_mof::Uuid::parse(&ids["diamond"]).unwrap();
        let uml = archforge_shell_api::kinds::DiagramKind::kind(&uml_ui::UmlKind);
        assert!(h.state().canvas.kind_cards.contains(&(node, uml)), "{theme:?}: {:?}", h.state().canvas.kind_cards);
        let r = rect_of(&h, &format!("workspace.canvas.node.{node}"));
        let corners = [r.left_center(), r.center_top(), r.right_center(), r.center_bottom()];
        let fill = egui::Color32::from(archforge_ui_egui::ux::palette(theme == ThemeKind::Dark).canvas);
        let drawn = h.output().shapes.iter().any(|c| {
            matches!(&c.shape, egui::Shape::Path(p) if p.closed && p.fill == fill && p.points.len() == 4
                && p.points.iter().zip(corners).all(|(a, b)| a.distance(b) < 0.5))
        });
        assert!(drawn, "{theme:?}: the diamond in {r:?}");
    }
}

/// The Relationship menu lists N-ary association after Association class, below a separator of
/// its own, its sample a line with a small diamond; a pick arms the tool with it (AR-128, A2). In
/// light and dark.
#[test]
fn the_relationship_menu_lists_n_ary_association_after_association_class_below_a_separator() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, _) = nary_sample(theme, false);
        let at = rect_of(&h, "workspace.tools.relationship-kinds").center();
        click(&mut h, at);
        let (realization, class, nary) =
            (rect_of(&h, "workspace.relate.realization"), rect_of(&h, "workspace.relate.association-class"), rect_of(&h, "workspace.relate.n-ary"));
        let step = class.top() - realization.top();
        assert!(nary.top() - class.top() > step + 8.0, "{theme:?}: a separator above N-ary association: {class:?} {nary:?}");
        assert!(painted(&h).iter().any(|t| t == "N-ary association"), "{theme:?}: named");
        let at = nary.center();
        click(&mut h, at);
        assert_eq!(uml_of(h.state()).relation, uml_relate::Relate::NAry);
        assert_eq!(h.state().canvas.tool, archforge_shell_api::canvas::Tool::Connect("relationship"));
        assert!(h.state().canvas.popover.is_none(), "a pick closes the menu");
    }
}

/// Drawn with three classes and Enter, the n-ary association is one undo step: nothing is in the
/// model while the draft grows, the hint counts its ends, Properties shows the draft; finished, the
/// association, its diamond and a leg per end are there, selected, and undo and redo take and give
/// it whole. With four, a double click on the last class finishes it the same way. In light and
/// dark.
#[test]
fn an_n_ary_association_is_drawn_class_by_class_and_finished_as_one_undo_step() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, ids) = nary_sample(theme, false);
        let before = revision(&h);
        arm_nary(&mut h);
        for n in ["customer", "product"] {
            let at = card(&h, &ids[n]);
            click(&mut h, at);
        }
        let shown = painted(&h);
        for said in [
            "N-ary association · 2 ends — click a class for the next end",
            "New n-ary association",
            "Ends so far · 2",
            "customer : Customer [*]",
            "product : Product [*]",
        ] {
            assert!(shown.iter().any(|t| t == said), "{theme:?}: {said} in {shown:?}");
        }
        assert!(ids_of(&h).iter().any(|i| i == "workspace.canvas.gestureHint"), "the hint bar");
        // Enter with two ends finishes nothing.
        key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
        assert!(naries(&h).is_empty() && revision(&h) == before, "{theme:?}: nothing before three ends");
        let at = card(&h, &ids["catalog"]);
        click(&mut h, at);
        assert_eq!(revision(&h), before, "{theme:?}: the draft is not in the model");
        key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
        let made = naries(&h);
        assert_eq!(made.len(), 1, "{theme:?}");
        assert_eq!(made[0].1, ["customer : Customer [*]", "product : Product [*]", "catalog : Catalog [*]"]);
        assert_eq!((diamonds_here(&h), legs_on(&h, None)), (1, 3), "{theme:?}: the diamond and three legs");
        assert_eq!(selection(&h), vec![made[0].0], "{theme:?}: the association selected");
        assert_eq!(h.state().canvas.tool, archforge_shell_api::canvas::Tool::Select, "the tool hands back to Select");
        assert!(uml_of(h.state()).nary.is_none());
        assert_eq!(h.state().session.as_ref().unwrap().undo_label(), Some("Add n-ary association"));
        h.state_mut().session.as_mut().unwrap().undo().unwrap();
        h.run();
        assert!(naries(&h).is_empty() && diamonds_here(&h) == 0 && legs_on(&h, None) == 0, "{theme:?}: one undo step takes it all");
        h.state_mut().session.as_mut().unwrap().redo().unwrap();
        h.run();
        assert_eq!((naries(&h).len(), diamonds_here(&h), legs_on(&h, None)), (1, 1, 3), "{theme:?}: redo gives it whole");
        // Four ends, the last by a double click.
        let order = {
            let s = h.state_mut().session.as_mut().unwrap();
            let st = s.doc();
            let o = st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_uml::CLASS) && archforge_core::name_of(st, *o) == "Order").unwrap();
            let id = st.uuid(o).unwrap().to_string();
            let placed = s
                .execute(
                    "diagram.place",
                    &serde_json::json!({ "diagram": ids["diagram"], "element": id, "x": 640, "y": 560, "width": 180, "height": 120 }),
                )
                .unwrap();
            placed.value["id"].as_str().unwrap().to_string()
        };
        h.run();
        arm_nary(&mut h);
        for n in [&ids["customer"], &ids["product"], &ids["catalog"]] {
            let at = card(&h, n);
            click(&mut h, at);
        }
        let at = card(&h, &order);
        double_click(&mut h, at);
        let made = naries(&h);
        assert_eq!(made.len(), 2, "{theme:?}: the double click finishes");
        assert!(made.iter().any(|(_, e)| e.len() == 4 && e[3] == "order : Order [*]"), "{made:?}");
        assert_eq!(h.state().session.as_ref().unwrap().undo_label(), Some("Add n-ary association"));
        h.state_mut().session.as_mut().unwrap().undo().unwrap();
        assert_eq!(naries(&h).len(), 1, "{theme:?}: one step");
    }
}

/// Backspace drops the draft's last end; Escape, another tool, a click on empty canvas and another
/// diagram each drop the draft, leaving the model as it was and no draft for the next gesture.
#[test]
fn backspace_drops_the_last_end_and_every_cancel_leaves_nothing_behind() {
    let (mut h, ids) = nary_sample(ThemeKind::Light, false);
    let before = revision(&h);
    let start = |h: &mut Harness<'static, Shell>| {
        arm_nary(h);
        for n in ["customer", "product"] {
            let at = card(h, &ids[n]);
            click(h, at);
        }
        assert!(matches!(&uml_of(h.state()).nary, Some(uml_nary::Nary::Draft { ends, .. }) if ends.len() == 2));
    };
    start(&mut h);
    key(&mut h, egui::Key::Backspace, egui::Modifiers::NONE);
    assert!(matches!(&uml_of(h.state()).nary, Some(uml_nary::Nary::Draft { ends, .. }) if ends.len() == 1), "Backspace drops one");
    assert!(painted(&h).iter().any(|t| t == "N-ary association · 1 end — click a class for the next end"), "the singular");
    // Escape.
    key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
    assert!(uml_of(h.state()).nary.is_none() && h.state().canvas.tool == archforge_shell_api::canvas::Tool::Select, "Escape cancels");
    // Another tool.
    start(&mut h);
    let at = rect_of(&h, "workspace.tools.select").center();
    click(&mut h, at);
    assert!(uml_of(h.state()).nary.is_none(), "another tool cancels");
    // Another kind of relationship.
    start(&mut h);
    let at = rect_of(&h, "workspace.tools.relationship-kinds").center();
    click(&mut h, at);
    let at = rect_of(&h, "workspace.relate.association").center();
    click(&mut h, at);
    h.run();
    assert!(uml_of(h.state()).nary.is_none(), "another kind cancels");
    // Empty canvas.
    start(&mut h);
    let empty = rect_of(&h, "workspace.canvas").left_bottom() + egui::vec2(60.0, -200.0);
    click(&mut h, empty);
    assert!(uml_of(h.state()).nary.is_none(), "a click on empty canvas cancels");
    // Another diagram, and back: no draft comes back.
    start(&mut h);
    let commerce = h.state().canvas.tabs[0];
    archforge_ui_egui::canvas::open(h.state_mut(), commerce);
    h.run();
    assert!(uml_of(h.state()).nary.is_none(), "another diagram drops it");
    archforge_ui_egui::canvas::open(h.state_mut(), archforge_mof::Uuid::parse(&ids["diagram"]).unwrap());
    h.run();
    assert!(uml_of(h.state()).nary.is_none() && !painted(&h).iter().any(|t| t == "New n-ary association"), "nothing stale");
    assert_eq!(revision(&h), before, "the model is as it was");
    assert!(naries(&h).is_empty());
}

/// Before the first end is clicked, Backspace is not the draft's: it reaches the selection's
/// removal, so a selected card leaves the diagram as it does with any other tool.
#[test]
fn backspace_before_the_first_end_removes_the_selection_as_ever() {
    let (mut h, ids) = nary_sample(ThemeKind::Light, false);
    arm_nary(&mut h);
    h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [ids["Product"]] })).unwrap();
    h.run();
    assert!(uml_of(h.state()).nary.as_ref().is_none_or(|n| matches!(n, uml_nary::Nary::Draft { ends, .. } if ends.is_empty())), "no end yet");
    let shown = |h: &Harness<'static, Shell>| {
        let st = h.state().session.as_ref().unwrap().doc();
        let d = st.by_uuid(archforge_mof::Uuid::parse(&ids["diagram"]).unwrap()).unwrap();
        let product = st.by_uuid(archforge_mof::Uuid::parse(&ids["Product"]).unwrap()).unwrap();
        archforge_uml::nary::node_in(st, d, product).is_some()
    };
    assert!(shown(&h));
    h.state_mut().canvas.focused = true;
    key(&mut h, egui::Key::Backspace, egui::Modifiers::NONE);
    assert!(!shown(&h), "the selected card left the diagram");
    assert!(naries(&h).is_empty(), "and nothing n-ary was made");
}

/// A class clicked twice gives two ends at it, told apart by their roles; a data type is refused at
/// the canvas's top and adds no end.
#[test]
fn a_class_clicked_twice_gives_two_ends_of_one_type_and_a_data_type_is_refused() {
    let (mut h, ids) = nary_sample(ThemeKind::Light, false);
    let money = {
        let s = h.state_mut().session.as_mut().unwrap();
        let st = s.doc();
        let pkg = archforge_uml::commands::package_of(st, st.by_uuid(archforge_mof::Uuid::parse(&ids["Customer"]).unwrap()).unwrap()).unwrap();
        let pkg = st.uuid(pkg).unwrap().to_string();
        let dt = s.execute("uml.data-type.add", &serde_json::json!({ "package": pkg, "name": "Amount" })).unwrap().value["id"]
            .as_str()
            .unwrap()
            .to_string();
        let n = s.execute(
            "diagram.place",
            &serde_json::json!({ "diagram": ids["diagram"], "element": dt, "x": 640, "y": 560, "width": 180, "height": 120 }),
        );
        n.unwrap().value["id"].as_str().unwrap().to_string()
    };
    h.run();
    arm_nary(&mut h);
    let at = card(&h, &money);
    click(&mut h, at);
    assert_eq!(h.state().canvas.notice.as_deref(), Some("A data type joins no association"));
    assert!(uml_of(h.state()).nary.is_none(), "no end");
    for n in ["customer", "customer", "product"] {
        let at = card(&h, &ids[n]);
        click(&mut h, at);
    }
    assert!(painted(&h).iter().any(|t| t == "customer2 : Customer [*]"), "{:?}", painted(&h));
    key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
    let made = naries(&h);
    assert_eq!(made[0].1, ["customer : Customer [*]", "customer2 : Customer [*]", "product : Product [*]"]);
    assert_eq!(legs_on(&h, None), 3, "two legs to one card");
}

/// A click on a leg and a click on the diamond both select the one association; Properties shows
/// its name, its package and its ends, and its kind line counts them. In light and dark.
#[test]
fn a_leg_and_the_diamond_both_select_the_n_ary_association() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, ids) = nary_sample(theme, true);
        let a = archforge_mof::Uuid::parse(&ids["recommends"]).unwrap();
        let leg = {
            let st = h.state().session.as_ref().unwrap().doc();
            let end = archforge_uml::nary::ends(st, st.by_uuid(a).unwrap())[0];
            st.uuid(archforge_diagram::views_of(st, end).1[0]).unwrap()
        };
        let at = point_on_edge(&h, leg);
        click(&mut h, at);
        let a_obj = h.state().session.as_ref().unwrap().doc().by_uuid(a).unwrap();
        assert_eq!(selection(&h), vec![a_obj], "{theme:?}: the leg selects the association");
        let shown = painted(&h);
        for said in
            ["recommends", "N-ary association · 3 ends · model, in every diagram", "Ends · 3", "customer : Customer [*]", "catalog : Catalog [1]"]
        {
            assert!(shown.iter().any(|t| t == said), "{theme:?}: {said} in {shown:?}");
        }
        assert!(ids_of(&h).iter().any(|i| i == "workspace.canvas.leg-end.2"), "the legs' end handles");
        let empty = rect_of(&h, "workspace.canvas").left_bottom() + egui::vec2(60.0, -200.0);
        click(&mut h, empty);
        assert_ne!(selection(&h), vec![a_obj]);
        let at = card(&h, &ids["diamond"]);
        click(&mut h, at);
        assert_eq!(selection(&h), vec![a_obj], "{theme:?}: the diamond selects it");
    }
}

/// An n-ary association's legs draw selected with it, a click on one standing for the association
/// (AR-128, AR-784), and a right click on a leg opens the association's menu as one on its diamond
/// does. In light and dark.
#[test]
fn an_n_ary_associations_legs_draw_selected_with_it_and_a_leg_opens_its_menu() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, ids) = nary_sample(theme, true);
        let leg = {
            let st = h.state().session.as_ref().unwrap().doc();
            let a = st.by_uuid(archforge_mof::Uuid::parse(&ids["recommends"]).unwrap()).unwrap();
            let end = archforge_uml::nary::ends(st, a)[0];
            st.uuid(archforge_diagram::views_of(st, end).1[0]).unwrap()
        };
        for (selected, marked) in [(&ids["customer"], false), (&ids["recommends"], true)] {
            h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [selected] })).unwrap();
            h.run();
            let line = rect_of(&h, &format!("workspace.canvas.edge.{leg}")).expand(2.0);
            assert_eq!(accent_path_in(&h, theme, line), marked, "{theme:?}: the leg drawn selected with {selected}");
        }
        let empty = rect_of(&h, "workspace.canvas").left_bottom() + egui::vec2(60.0, -200.0);
        click(&mut h, empty);
        let at = point_on_edge(&h, leg);
        right_click(&mut h, at);
        for key_ in ["add-end", "reconnect-end", "remove", "delete"] {
            assert!(ids_of(&h).iter().any(|i| *i == format!("workspace.canvas.menu.{key_}")), "{theme:?}: the leg opens {key_}");
        }
    }
}

/// A press on a right-angled leg's segment slides it and selects the n-ary association, as a
/// click on the leg does (AR-128, AR-784). In light and dark.
#[test]
fn a_right_angled_legs_segment_slid_selects_its_n_ary_association() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, ids) = nary_sample(theme, true);
        let (a, leg) = {
            let st = h.state().session.as_ref().unwrap().doc();
            let a = st.by_uuid(archforge_mof::Uuid::parse(&ids["recommends"]).unwrap()).unwrap();
            (a, archforge_diagram::views_of(st, archforge_uml::nary::ends(st, a)[0]).1[0])
        };
        let s = h.state_mut().session.as_mut().unwrap();
        let leg_id = s.doc().uuid(leg).unwrap().to_string();
        s.execute("diagram.style", &serde_json::json!({ "ids": [leg_id], "routing": "right-angled" })).unwrap();
        s.execute("view.select", &serde_json::json!({ "ids": [] })).unwrap();
        h.run();
        let route = {
            let st = h.state().session.as_ref().unwrap().doc();
            archforge_diagram::Grid::build(st, st.by_uuid(h.state().canvas.active.unwrap()).unwrap()).line(leg).unwrap().to_vec()
        };
        assert!(route.len() >= 3, "{theme:?}: a right-angled route: {route:?}");
        let view =
            archforge_shell_api::canvas::View { origin: rect_of(&h, "workspace.canvas").min, pan: h.state().canvas.pan, zoom: h.state().canvas.zoom };
        let middle = view.to_screen((route[1].0 + route[2].0) / 2.0, (route[1].1 + route[2].1) / 2.0);
        let level = (route[1].1 - route[2].1).abs() < 1e-6;
        drag(&mut h, middle, middle + if level { egui::vec2(0.0, -30.0) } else { egui::vec2(-30.0, 0.0) });
        assert_eq!(selection(&h), vec![a], "{theme:?}: the association, not its end");
    }
}

/// An n-ary association's end selected on its own, as `view.select` can select it from the command
/// line or an agent, shows no handles on its leg's line: the leg moves only its class end, by the
/// association's own leg handle (AR-128, AR-784). In light and dark.
#[test]
fn an_n_ary_end_selected_on_its_own_shows_no_line_end_handles() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, ids) = nary_sample(theme, true);
        let end = {
            let st = h.state().session.as_ref().unwrap().doc();
            let a = st.by_uuid(archforge_mof::Uuid::parse(&ids["recommends"]).unwrap()).unwrap();
            st.uuid(archforge_uml::nary::ends(st, a)[0]).unwrap().to_string()
        };
        h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [end] })).unwrap();
        h.run();
        let handles: Vec<String> = ids_of(&h).into_iter().filter(|i| i.starts_with("workspace.canvas.line-end.")).collect();
        assert!(handles.is_empty(), "{theme:?}: {handles:?}");
    }
}

/// Add end, from the Ends `+`, arms the next class click: it adds an end there with its leg, one
/// undo step; the hint says what the click does.
#[test]
fn add_end_from_the_plus_takes_the_next_class_clicked() {
    let (mut h, ids) = nary_sample(ThemeKind::Light, true);
    h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [ids["recommends"]] })).unwrap();
    h.run();
    let at = rect_of(&h, "workspace.properties.association.ends.add").center();
    click(&mut h, at);
    assert!(painted(&h).iter().any(|t| t == "Click a class for the new end"), "the hint");
    let at = card(&h, &ids["customer"]);
    click(&mut h, at);
    assert_eq!(naries(&h)[0].1.len(), 4);
    assert_eq!(naries(&h)[0].1[3], "customer2 : Customer [*]");
    assert_eq!(legs_on(&h, None), 4, "its leg drawn");
    assert!(uml_of(h.state()).nary.is_none(), "one class only");
    h.state_mut().session.as_mut().unwrap().undo().unwrap();
    assert_eq!(naries(&h)[0].1.len(), 3, "one step");
    // Escape right after the `+` cancels it, the canvas taking the keys.
    h.run();
    let at = rect_of(&h, "workspace.properties.association.ends.add").center();
    click(&mut h, at);
    assert!(uml_of(h.state()).nary.is_some(), "armed again");
    key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
    assert!(uml_of(h.state()).nary.is_none(), "Escape cancels Add end from the panel too");
    // From the diamond's menu too; Escape cancels it.
    h.run();
    let at = card(&h, &ids["diamond"]);
    right_click(&mut h, at);
    for key_ in ["add-end", "reconnect-end", "remove", "delete"] {
        assert!(ids_of(&h).iter().any(|i| *i == format!("workspace.canvas.menu.{key_}")), "{key_}");
    }
    let at = rect_of(&h, "workspace.canvas.menu.add-end").center();
    click(&mut h, at);
    assert!(uml_of(h.state()).nary.is_some(), "armed");
    key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
    assert!(uml_of(h.state()).nary.is_none(), "Escape cancels Add end");
    // Delete from the model asks first, the dialog naming the association asked from its diamond.
    h.run();
    let at = card(&h, &ids["diamond"]);
    right_click(&mut h, at);
    let at = rect_of(&h, "workspace.canvas.menu.delete").center();
    click(&mut h, at);
    let asked = h.state().explorer.delete.clone().expect("the dialog asks");
    let diamond = archforge_mof::Uuid::parse(&ids["diamond"]).unwrap();
    assert_eq!((asked.name.as_str(), asked.node), ("recommends", Some(diamond)), "{asked:?}");
    let at = rect_of(&h, "workspace.explorer.delete.cancel").center();
    click(&mut h, at);
    assert_eq!(naries(&h).len(), 1, "nothing deleted before the dialog is confirmed");
    // Remove from this diagram takes the diamond off the diagram, the association kept.
    let at = card(&h, &ids["diamond"]);
    right_click(&mut h, at);
    let at = rect_of(&h, "workspace.canvas.menu.remove").center();
    click(&mut h, at);
    assert!(!ids_of(&h).contains(&format!("workspace.canvas.node.{}", ids["diamond"])), "the diamond left the diagram");
    assert_eq!(naries(&h).len(), 1, "the association stays in the model");
}

/// A selected leg's class end drags onto another class, moving that end there with its leg; the
/// end editor's Type moves it back, one undo step with the rest.
#[test]
fn a_leg_end_drags_onto_another_class_and_the_type_select_moves_it_back() {
    let (mut h, ids) = nary_sample(ThemeKind::Light, true);
    let order = {
        let s = h.state_mut().session.as_mut().unwrap();
        let st = s.doc();
        let o = st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_uml::CLASS) && archforge_core::name_of(st, *o) == "Order").unwrap();
        let id = st.uuid(o).unwrap().to_string();
        let placed = s.execute(
            "diagram.place",
            &serde_json::json!({ "diagram": ids["diagram"], "element": id, "x": 640, "y": 560, "width": 180, "height": 120 }),
        );
        placed.unwrap().value["id"].as_str().unwrap().to_string()
    };
    h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [ids["recommends"]] })).unwrap();
    h.run();
    let handle = rect_of(&h, "workspace.canvas.leg-end.2").center();
    let to = card(&h, &order);
    drag(&mut h, handle, to);
    assert_eq!(naries(&h)[0].1[2], "catalog : Order [1]", "notice {:?}", h.state().canvas.notice);
    assert_eq!(h.state().session.as_ref().unwrap().undo_label(), Some("Reconnect association end"));
    let st = h.state().session.as_ref().unwrap().doc();
    let end = archforge_uml::nary::ends(st, st.by_uuid(archforge_mof::Uuid::parse(&ids["recommends"]).unwrap()).unwrap())[2];
    let leg = archforge_diagram::views_of(st, end).1[0];
    let target = st.get(leg, archforge_diagram::edge::TARGET).unwrap().as_ref().unwrap();
    assert_eq!(st.uuid(target).unwrap().to_string(), order, "the leg follows");
    // Back with Type.
    let at = rect_of(&h, "workspace.properties.end.2.edit").center();
    click(&mut h, at);
    let at = rect_of(&h, "workspace.properties.end.2.type").center();
    click(&mut h, at);
    let k = uml_nary::end_types(h.state().session.as_ref().unwrap().doc()).iter().position(|(_, n)| n == "Catalog").unwrap();
    let at = rect_of(&h, &format!("workspace.properties.end.2.type.{k}")).center();
    click(&mut h, at);
    let at = rect_of(&h, "workspace.properties.end.2.apply").center();
    click(&mut h, at);
    assert_eq!(naries(&h)[0].1[2], "catalog : Catalog [1]");
    assert_eq!(h.state().session.as_ref().unwrap().undo_label(), Some("Set association end"));
}

/// Delete with the association selected removes its diamond and legs from this diagram only;
/// Shift+Delete asks first and deletes it with its ends everywhere.
#[test]
fn remove_from_this_diagram_keeps_the_n_ary_association_and_delete_from_the_model_asks_first() {
    let (mut h, ids) = nary_sample(ThemeKind::Light, true);
    let a = ids["recommends"].clone();
    let at = card(&h, &ids["diamond"]);
    click(&mut h, at);
    key(&mut h, egui::Key::Delete, egui::Modifiers::NONE);
    assert_eq!((diamonds_here(&h), legs_on(&h, None)), (0, 0), "gone from the diagram");
    assert_eq!(naries(&h).len(), 1, "kept in the model");
    h.state_mut().session.as_mut().unwrap().undo().unwrap();
    h.run();
    let at = card(&h, &ids["diamond"]);
    click(&mut h, at);
    key(&mut h, egui::Key::Delete, egui::Modifiers::SHIFT);
    assert!(h.state().explorer.delete.is_some(), "asks first");
    assert_eq!(naries(&h).len(), 1);
    let at = rect_of(&h, "workspace.explorer.delete.confirm").center();
    click(&mut h, at);
    assert!(naries(&h).is_empty() && diamonds_here(&h) == 0 && legs_on(&h, None) == 0, "deleted with its ends");
    let st = h.state().session.as_ref().unwrap().doc();
    assert!(st.by_uuid(archforge_mof::Uuid::parse(&a).unwrap()).is_none_or(|o| !st.is_alive(o)));
}

/// Two diagrams stay in step: an end added on one draws its leg on the other where both show the
/// diamond and the class; a class dragged onto a diagram that shows the diamond comes with its leg.
#[test]
fn two_diagrams_show_the_same_n_ary_association_and_a_dropped_class_brings_its_leg() {
    let (mut h, ids) = nary_sample(ThemeKind::Light, true);
    let second = {
        let s = h.state_mut().session.as_mut().unwrap();
        let st = s.doc();
        let pkg = archforge_uml::commands::package_of(st, st.by_uuid(archforge_mof::Uuid::parse(&ids["Customer"]).unwrap()).unwrap()).unwrap();
        let pkg = st.uuid(pkg).unwrap().to_string();
        let d = s
            .execute("diagram.add", &serde_json::json!({ "kind": archforge_uml::STRUCTURE_DIAGRAM, "element": pkg, "name": "Second" }))
            .unwrap()
            .value["id"]
            .as_str()
            .unwrap()
            .to_string();
        for (c, x) in [("Customer", 80), ("Product", 500), ("Catalog", 900)] {
            s.execute("diagram.place", &serde_json::json!({ "diagram": d, "element": ids[c], "x": x, "y": 80, "width": 200, "height": 120 }))
                .unwrap();
        }
        s.execute("diagram.place", &serde_json::json!({ "diagram": d, "element": ids["recommends"], "x": 500, "y": 400, "width": 44, "height": 28 }))
            .unwrap();
        s.execute("diagram.connect-all", &serde_json::json!({ "diagram": d })).unwrap();
        d
    };
    assert_eq!(legs_on(&h, Some(&second)), 3, "connect-all draws the legs");
    h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [ids["recommends"]] })).unwrap();
    h.run();
    let at = rect_of(&h, "workspace.properties.association.ends.add").center();
    click(&mut h, at);
    let at = card(&h, &ids["product"]);
    click(&mut h, at);
    assert_eq!((legs_on(&h, None), legs_on(&h, Some(&second))), (4, 4), "the new leg on both diagrams");
    // Catalog off this diagram and dropped back: its leg comes with it.
    h.state_mut().session.as_mut().unwrap().execute("diagram.unplace", &serde_json::json!({ "nodes": [ids["catalog"]] })).unwrap();
    h.run();
    assert_eq!(legs_on(&h, None), 3);
    let d = h.state().session.as_ref().unwrap().doc().by_uuid(archforge_mof::Uuid::parse(&ids["diagram"]).unwrap()).unwrap();
    archforge_ui_egui::snapshot::cx(h.state_mut(), |cx| {
        archforge_shell_api::kinds::DiagramKind::drop(&uml_ui::UmlKind, cx, d, archforge_mof::Uuid::parse(&ids["Catalog"]).unwrap(), (460.0, 640.0))
    });
    h.run();
    assert_eq!(legs_on(&h, None), 4, "the dropped class brings its leg");
}

/// Stereotypes applied (AR-135, design page 08, F3 and F4), in light and dark: Order's Properties
/// have a Stereotypes section whose `+` opens the picker, Entity to apply, Id, Reference and Tracked
/// disabled with why; a refused row picks nothing; typing filters and Enter applies, one undo step,
/// `«Entity»` after Order's row in Models and in its card's eyebrow. Each property has a control
/// fitting its type: cached a select showing its default with the `default` tag, table a field; a
/// value typed and entered is set in one step, a select's pick too, and its Default row clears it; a
/// value that does not read stays in its field with the reason under it, nothing written. Show
/// stereotype values on this diagram lists the values on the card; the bin takes the stereotype off.
#[test]
fn stereotypes_are_applied_and_their_values_edited_in_properties() {
    use egui_kittest::kittest::Queryable;
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, nodes) = related_sample_in(theme, egui::vec2(1600.0, 1100.0));
        let (_, _, entity) = archforge_ui_egui::snapshot::cx(h.state_mut(), |cx| uml_profiles::put_profiles(cx, false)).unwrap();
        let (logging, rows_p) = {
            let s = h.state_mut().session.as_mut().unwrap();
            let logging = s.execute("uml.profile.add", &serde_json::json!({ "name": "Logging" })).unwrap().value["id"].as_str().unwrap().to_string();
            s.execute("uml.stereotype.add", &serde_json::json!({ "profile": logging, "name": "Tracked", "extends": ["Class"] })).unwrap();
            let rows = s.execute("uml.attribute.add", &serde_json::json!({ "class": entity, "name": "rows", "type": "Integer" })).unwrap();
            (logging, rows.value["id"].as_str().unwrap().to_string())
        };
        let _ = logging;
        let order = select_named(&mut h, archforge_uml::CLASS, "Order");
        h.state_mut().state.show_panel(archforge_shell_api::state::Panel::Properties);
        h.run();
        assert!(painted_has(&h, "Stereotypes · 0"), "{theme:?}: {:?}", painted(&h));
        assert!(painted_has(&h, "None applied yet. + offers the stereotypes of the profiles applied to Commerce or a package around it."));
        // F4: the picker.
        let at = rect_of(&h, uml_stereotypes::ADD_ID).center();
        click(&mut h, at);
        assert_eq!(h.state().panels.menu, Some(uml_state::menus::STEREOTYPE_PICK));
        assert!(painted_has(&h, "APPLY TO ORDER"), "{theme:?}: {:?}", painted(&h));
        assert!(painted_has(&h, "properties only") && painted_has(&h, "associations only") && painted_has(&h, "Logging not applied"));
        let id_row = {
            let st = h.state().session.as_ref().unwrap().doc();
            let s = st.alive().find(|o| archforge_uml::profiles::is_stereotype(st, *o) && archforge_core::name_of(st, *o) == "Id").unwrap();
            format!("{}.{}", uml_stereotypes::ADD_ID, st.uuid(s).unwrap())
        };
        assert!(h.query_by_label("Id").is_some(), "a refused row keeps its accessible name");
        let before = h.state().session.as_ref().unwrap().revision();
        let at = rect_of(&h, &id_row).center();
        click(&mut h, at);
        assert_eq!(h.state().session.as_ref().unwrap().revision(), before, "a refused row picks nothing");
        // Typing filters; Enter applies the first that applies.
        h.event(egui::Event::Text("ent".into()));
        h.run();
        assert!(!ids(&h).contains(&id_row), "{theme:?}: filtered out");
        assert!(painted_has(&h, "Filter: ent"));
        key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
        assert_eq!(h.state().session.as_ref().unwrap().undo_label(), Some("Apply stereotype"));
        assert_eq!(h.state().panels.menu, None);
        h.run();
        assert!(painted_has(&h, "Stereotypes · 1") && painted_has(&h, "«Entity»") && painted_has(&h, "Persistence"));
        assert!(painted_has(&h, "default"), "{theme:?}: cached shows its default with the tag");
        assert!(h.query_by_label("cached: false (default)").is_some(), "the select's accessible name says the default");
        // F3: a value typed in its field, one step.
        let (table, cached) = {
            let st = h.state().session.as_ref().unwrap().doc();
            let props = archforge_uml::profiles::properties(st, st.by_uuid(archforge_mof::Uuid::parse(&entity).unwrap()).unwrap());
            (st.uuid(props[0]).unwrap(), st.uuid(props[1]).unwrap())
        };
        let field = |p: archforge_mof::Uuid| format!("{}.{entity}.value.{p}", uml_stereotypes::APPLIED_ID);
        type_into(&mut h, &field(table), "orders");
        key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
        assert_eq!(h.state().session.as_ref().unwrap().undo_label(), Some("Set stereotype value"));
        let shown = |h: &Harness<'static, Shell>, p: archforge_mof::Uuid| {
            let st = h.state().session.as_ref().unwrap().doc();
            let o = st.by_uuid(order).unwrap();
            let i = archforge_uml::applications::application(st, o, st.by_uuid(archforge_mof::Uuid::parse(&entity).unwrap()).unwrap()).unwrap();
            archforge_uml::applications::shown(st, i, st.by_uuid(p).unwrap())
        };
        assert_eq!(shown(&h, table).as_deref(), Some("\"orders\""));
        // A select's pick sets it, its Default row clears it.
        let at = rect_of(&h, &field(cached)).center();
        click(&mut h, at);
        let at = rect_of(&h, &format!("{}.1", field(cached))).center();
        click(&mut h, at);
        assert_eq!(shown(&h, cached).as_deref(), Some("true"));
        let at = rect_of(&h, &field(cached)).center();
        click(&mut h, at);
        assert!(painted_has(&h, "Default (false)"));
        let at = rect_of(&h, &format!("{}.0", field(cached))).center();
        click(&mut h, at);
        assert_eq!(shown(&h, cached), None, "{theme:?}: back to its default, unset");
        // A value that does not read stays in its field with the reason; nothing is written.
        let rows_p = archforge_mof::Uuid::parse(&rows_p).unwrap();
        let before = h.state().session.as_ref().unwrap().revision();
        type_into(&mut h, &field(rows_p), "many");
        key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
        assert_eq!(h.state().session.as_ref().unwrap().revision(), before, "{theme:?}: nothing written");
        assert!(painted_has(&h, "many is no value of rows. A whole number, such as 42 or -7."), "{theme:?}: {:?}", painted(&h));
        assert!(ids(&h).contains(&format!("{}.refusal", field(rows_p))));
        // Models and the card say «Entity».
        assert!(painted_has(&h, "«Entity»"));
        let card = {
            let st = h.state().session.as_ref().unwrap().doc();
            let o = st.by_uuid(order).unwrap();
            let d = st.by_uuid(h.state().canvas.active.unwrap()).unwrap();
            archforge_uml::figures::card(st, o, d).unwrap()
        };
        assert_eq!(card.eyebrow, archforge_diagram::figure::Text::Plain("«Entity»".into()));
        assert_eq!(card.compartments.len(), 2, "no values while the diagram hides them");
        // Show stereotype values on this diagram: Order's card lists table, and grows to show it.
        uml_of_mut(h.state_mut()).stereotypes.value = None;
        let height = |h: &Harness<'static, Shell>| {
            let st = h.state().session.as_ref().unwrap().doc();
            archforge_diagram::grid::bounds_of(st, st.by_uuid(archforge_mof::Uuid::parse(&nodes["Order"]).unwrap()).unwrap()).height
        };
        let low = height(&h);
        h.run();
        let at = rect_of(&h, uml_stereotypes::SHOW_ID).center();
        click(&mut h, at);
        assert_eq!(h.state().session.as_ref().unwrap().undo_label(), Some("Show stereotype values"));
        assert!(height(&h) > low, "{theme:?}: the card grew to show its values");
        h.run();
        assert!(painted_has(&h, "table = \"orders\""), "{theme:?}: {:?}", painted(&h));
        // The bin takes it off; the stereotype stays.
        let at = rect_of(&h, &format!("{}.{entity}.remove", uml_stereotypes::APPLIED_ID)).center();
        click(&mut h, at);
        assert_eq!(h.state().session.as_ref().unwrap().undo_label(), Some("Unapply stereotype"));
        h.run();
        assert!(painted_has(&h, "Stereotypes · 0"));
    }
}

/// A return by zoom shows the diagram as it was left when that view is sound (AR-668): Top, left
/// at a view of its own by opening Sales, comes back at exactly that view, at the least zoom, 10 %,
/// too. A view in which Sales' card covers most of the canvas, a saved zoom outside the zoom range,
/// or a pan that is not finite is no view to restore: the return lands at 100 % with Sales' card
/// centred instead.
#[test]
fn a_return_by_zoom_restores_the_view_it_was_left_with_while_that_view_is_sound() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let sound = (egui::vec2(-50.0, 30.0), 0.6);
        for (case, left, saved) in [
            ("as left", sound, None),
            ("as left at the least zoom", (egui::vec2(100.0, 100.0), archforge_shell_api::canvas::ZOOM_RANGE.0), None),
            ("the card covering the canvas", (egui::vec2(-590.0, -470.0), 3.0), None),
            ("zoom beyond the range", sound, Some((egui::vec2(-50.0, 30.0), archforge_shell_api::canvas::ZOOM_RANGE.1 * 2.0))),
            ("pan not finite", sound, Some((egui::vec2(f32::NAN, 30.0), 0.6))),
        ] {
            let mut h = workspace_harness(theme, egui::vec2(1600.0, 1000.0));
            let (top, sales, card, sales_d) = top_with_sales(&mut h, (480, 300), &[]);
            archforge_ui_egui::canvas::open(h.state_mut(), top);
            h.run();
            (h.state_mut().canvas.pan, h.state_mut().canvas.zoom) = left;
            h.run();
            archforge_ui_egui::snapshot::cx(h.state_mut(), |cx| uml_package_nav::enter(cx, sales, 0.0));
            h.run();
            if let Some(view) = saved {
                let mut route = uml_nav_route::Route::default();
                let history = h.state().session.as_ref().unwrap().history();
                route.push(uml_nav_route::Step { source: top, destination: sales_d, source_name: "Top".into(), view: Some(view), history });
                uml_of_mut(h.state_mut()).nav.routes.insert(sales_d, route);
            }
            rest(&mut h);
            assert_eq!(zoom_out_gesture(&mut h, 40, true), vec![sales_d, top], "{theme:?} {case}");
            if case.starts_with("as left") {
                assert_eq!((h.state().canvas.pan, h.state().canvas.zoom), left, "{theme:?} {case}: Top as it was left");
            } else {
                assert!(
                    centred_at_landing(&h, card),
                    "{theme:?} {case}: Sales centred at 100 %, {:?}",
                    (h.state().canvas.pan, h.state().canvas.zoom)
                );
            }
        }
    }
}

/// A return by zoom measures the view it would restore on the canvas as it is at the return
/// (AR-668): Top, left by ⌘↓-style entry into Sales at a view that shows Sales' card in part,
/// covering less than [`uml_package_nav::ENTER_COVER`] of the canvas, comes back at 100 % with
/// the card centred once the window has shrunk so far that the card would cover more at that view,
/// or, near the canvas's right edge, would be out of view entirely.
#[test]
fn a_return_by_zoom_measures_the_saved_view_on_the_window_it_returns_to() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        for (case, zoom) in [("covering", 1.6), ("hidden", 0.6)] {
            let mut h = workspace_harness(theme, egui::vec2(1600.0, 1000.0));
            let (top, sales, card, sales_d) = top_with_sales(&mut h, (480, 300), &[]);
            archforge_ui_egui::canvas::open(h.state_mut(), top);
            h.run();
            // Sales' card at the canvas's top left, or with only its left 100 points in view at the right.
            let width = rect_of(&h, "workspace.canvas").width();
            let left = if case == "covering" { 20.0 } else { width - 100.0 };
            let view = (egui::vec2(left - 200.0 * zoom, 20.0 - 160.0 * zoom), zoom);
            // The share of the canvas Sales' card covers at `view`.
            let share = |h: &Harness<'static, Shell>| {
                let canvas = rect_of(h, "workspace.canvas");
                let shown = egui::Rect::from_min_size(canvas.min + view.0 + egui::vec2(200.0, 160.0) * zoom, egui::vec2(480.0, 300.0) * zoom);
                let covered = shown.intersect(canvas);
                covered.width().max(0.0) * covered.height().max(0.0) / (canvas.width() * canvas.height())
            };
            (h.state_mut().canvas.pan, h.state_mut().canvas.zoom) = view;
            h.run();
            let before = share(&h);
            assert!(before > 0.0 && before < uml_package_nav::ENTER_COVER, "{theme:?} {case}: Sales covers {before} when left");
            archforge_ui_egui::snapshot::cx(h.state_mut(), |cx| uml_package_nav::enter(cx, sales, 0.0));
            h.run();
            h.set_size(egui::vec2(1000.0, 700.0));
            h.run();
            rest(&mut h);
            assert_eq!(zoom_out_gesture(&mut h, 40, true), vec![sales_d, top], "{theme:?} {case}");
            let landed = (h.state().canvas.pan, h.state().canvas.zoom);
            assert!(centred_at_landing(&h, card), "{theme:?} {case}: Sales centred at 100 %, {landed:?}");
            let after = share(&h);
            let changed = if case == "covering" { after >= uml_package_nav::ENTER_COVER } else { after == 0.0 };
            assert!(changed, "{theme:?} {case}: Sales would cover {after} at the view left now");
        }
    }
}

/// A view a zoom entry was made at shows the package covering most of the canvas, so a return
/// there would let the next zoom-in tick enter again (AR-668, delegated UX decision): zooming in
/// on Sales' card until Sales opens, then out again, lands Top at 100 % with the card centred, and
/// one tick of zooming in on the card then stays on Top. So it does when the window grew while
/// Sales was open, though the card then covers less of the larger canvas at the entry's view.
#[test]
fn a_return_to_the_view_a_zoom_entry_left_centres_the_package_and_the_next_tick_stays() {
    for (theme, grown) in [(ThemeKind::Light, false), (ThemeKind::Dark, false), (ThemeKind::Light, true), (ThemeKind::Dark, true)] {
        let mut h = workspace_harness(theme, egui::vec2(1600.0, 1000.0));
        let (top, _, card, sales_d) = top_with_sales(&mut h, (480, 300), &[]);
        archforge_ui_egui::canvas::open(h.state_mut(), top);
        h.run();
        rest(&mut h);
        h.event(egui::Event::PointerMoved(rect_of(&h, &format!("workspace.canvas.node.{card}")).center()));
        for _ in 0..40 {
            if h.state().canvas.active == Some(sales_d) {
                break;
            }
            h.event(egui::Event::Zoom(1.25));
            h.step();
        }
        assert_eq!(h.state().canvas.active, Some(sales_d), "{theme:?}: zooming in entered Sales");
        if grown {
            h.set_size(egui::vec2(2600.0, 1700.0));
        }
        h.run();
        rest(&mut h);
        assert_eq!(zoom_out_gesture(&mut h, 40, true), vec![sales_d, top], "{theme:?} grown {grown}");
        assert!(centred_at_landing(&h, card), "{theme:?}: Sales centred at 100 %, {:?}", (h.state().canvas.pan, h.state().canvas.zoom));
        rest(&mut h);
        h.event(egui::Event::PointerMoved(rect_of(&h, &format!("workspace.canvas.node.{card}")).center()));
        h.event(egui::Event::Zoom(1.25));
        h.step();
        h.run();
        assert_eq!(h.state().canvas.active, Some(top), "{theme:?}: one tick in does not enter Sales again");
    }
}

/// A return by zoom measures the view it would restore as zooming in does, by every package card
/// (AR-668): Top, left by ⌘↓-style entry into Sales at a view where Archive's card covers most of
/// the canvas and Sales' card shows in part, lands at 100 % with Sales' card centred instead, and
/// one tick of zooming in on Archive then stays on Top.
#[test]
fn a_return_by_zoom_does_not_restore_a_view_another_package_covers() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let mut h = workspace_harness(theme, egui::vec2(1600.0, 1000.0));
        let (top, sales, card, sales_d) = top_with_sales(&mut h, (240, 148), &[]);
        let id = |v: serde_json::Value| archforge_mof::Uuid::parse(v["id"].as_str().unwrap()).unwrap();
        let s = h.state_mut().session.as_mut().unwrap();
        let archive = id(s.execute("uml.package.add", &serde_json::json!({ "name": "Archive" })).unwrap().value);
        let p = serde_json::json!({ "diagram": top.to_string(), "element": archive.to_string(), "x": 460, "y": 160, "width": 480, "height": 300 });
        let archive_card = id(s.execute("diagram.place", &p).unwrap().value);
        let archive_d = own_diagram(s, archive);
        archforge_ui_egui::canvas::open(h.state_mut(), top);
        h.run();
        // Archive's card fills the canvas from 120 points in, Sales' card's right 60 points in view.
        let zoom = 3.0;
        (h.state_mut().canvas.pan, h.state_mut().canvas.zoom) = (egui::vec2(120.0 - 460.0 * zoom, 40.0 - 160.0 * zoom), zoom);
        h.run();
        let c = &h.state().canvas;
        let canvas = rect_of(&h, "workspace.canvas");
        let share = |n| {
            let covered = rect_of(&h, &format!("workspace.canvas.node.{n}")).intersect(canvas);
            covered.width().max(0.0) * covered.height().max(0.0) / (canvas.width() * canvas.height())
        };
        let (archive_share, sales_share) = (share(archive_card), share(card));
        assert!(archive_share >= uml_package_nav::ENTER_COVER, "{theme:?}: Archive covers {archive_share} at {:?}", (c.pan, c.zoom));
        assert!(sales_share > 0.0, "{theme:?}: Sales' card partly in view, {sales_share}");
        archforge_ui_egui::snapshot::cx(h.state_mut(), |cx| uml_package_nav::enter(cx, sales, 0.0));
        h.run();
        rest(&mut h);
        assert_eq!(zoom_out_gesture(&mut h, 40, true), vec![sales_d, top], "{theme:?}");
        assert!(centred_at_landing(&h, card), "{theme:?}: Sales centred at 100 %, {:?}", (h.state().canvas.pan, h.state().canvas.zoom));
        rest(&mut h);
        h.event(egui::Event::PointerMoved(rect_of(&h, &format!("workspace.canvas.node.{archive_card}")).center()));
        h.event(egui::Event::Zoom(1.25));
        h.step();
        h.run();
        assert_eq!(h.state().canvas.active, Some(top), "{theme:?}: one tick in does not enter Archive, {archive_d}");
    }
}

/// A return to a diagram that no longer shows the package left fits that diagram's content, though
/// its saved view was sound (AR-668): Top, left at 60 % by entering Sales, loses Sales' card; the
/// return shows both classes far apart in view, below 100 %. Emptied entirely, Top shows at 100 %
/// from its top left, as an empty diagram fits. So does Top as the owning parent's default when
/// Sales' diagram was opened directly, with no route back. Classes wider apart than ten canvases
/// fit only down to the least zoom, 10 %, the middle between them in the canvas's centre.
#[test]
fn a_return_to_a_diagram_without_the_packages_card_fits_its_content() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        for (far, routed) in [(&[0, 4000][..], true), (&[], true), (&[0, 4000], false), (&[], false), (&[0, 400_000], false)] {
            let emptied = far.is_empty();
            let mut h = workspace_harness(theme, egui::vec2(1600.0, 1000.0));
            let (top, sales, card, sales_d) = top_with_sales(&mut h, (240, 148), far);
            archforge_ui_egui::canvas::open(h.state_mut(), top);
            h.run();
            (h.state_mut().canvas.pan, h.state_mut().canvas.zoom) = (egui::vec2(-50.0, 30.0), 0.6);
            h.run();
            if routed {
                archforge_ui_egui::snapshot::cx(h.state_mut(), |cx| uml_package_nav::enter(cx, sales, 0.0));
            } else {
                archforge_ui_egui::canvas::open(h.state_mut(), sales_d);
                assert!(uml_of(h.state()).nav.routes.get(&sales_d).is_none_or(|r| r == &uml_nav_route::Route::default()), "{theme:?}: no route");
            }
            h.run();
            h.state_mut().session.as_mut().unwrap().execute("diagram.unplace", &serde_json::json!({ "nodes": [card.to_string()] })).unwrap();
            h.run();
            rest(&mut h);
            assert_eq!(zoom_out_gesture(&mut h, 40, true), vec![sales_d, top], "{theme:?} emptied {emptied} routed {routed}");
            let c = &h.state().canvas;
            if emptied {
                assert_eq!((c.pan, c.zoom), (egui::Vec2::ZERO, 1.0), "{theme:?} routed {routed}: the empty diagram from its top left");
            } else if far[1] > 4000 {
                assert_eq!(c.zoom, archforge_shell_api::canvas::ZOOM_RANGE.0, "{theme:?}: too wide to fit, at the least zoom");
                // The classes span x 0 to 400 200 and y 0 to 120: their middle at the centre.
                let middle = c.pan + egui::vec2(200_100.0, 60.0) * c.zoom;
                assert!((middle - c.size / 2.0).length() < 1.0, "{theme:?}: the middle shown, {middle:?} in {:?}", c.size);
            } else {
                let seen = [0.0, 4200.0].iter().all(|x| (0.0..=c.size.x).contains(&(c.pan.x + x * c.zoom)));
                assert!(seen && c.zoom < 1.0, "{theme:?} routed {routed}: both classes fitted in view, {:?}", (c.pan, c.zoom));
            }
        }
    }
}

/// A return by zoom rejects a saved view at exactly the entry threshold, where zooming in would try
/// to enter (AR-668): Top, left by Open-style entry into Sales at a view where Sales' card covers
/// exactly [`uml_package_nav::ENTER_COVER`] of the canvas, comes back at 100 % with the card
/// centred. The card spans the canvas's height and its right 60 % in whole points, so the share is
/// exactly the threshold.
#[test]
fn a_return_by_zoom_rejects_a_view_at_exactly_the_entry_threshold() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        // A window one point wider than usual makes the canvas 845 points wide, a multiple of 5.
        let mut h = workspace_harness(theme, egui::vec2(1601.0, 1000.0));
        let (top, sales, card, sales_d) = top_with_sales(&mut h, (3000, 3000), &[]);
        archforge_ui_egui::canvas::open(h.state_mut(), top);
        h.run();
        let canvas = rect_of(&h, "workspace.canvas");
        assert_eq!(canvas.width() % 5.0, 0.0, "{theme:?}: canvas {canvas:?}");
        // At 100 %, the card's left edge 40 % in, its top above the canvas.
        let view = (egui::vec2(canvas.width() * 0.4 - 200.0, -400.0), 1.0);
        (h.state_mut().canvas.pan, h.state_mut().canvas.zoom) = view;
        h.run();
        let covered = rect_of(&h, &format!("workspace.canvas.node.{card}")).intersect(canvas);
        let share = covered.width() * covered.height() / (canvas.width() * canvas.height());
        assert_eq!(share, uml_package_nav::ENTER_COVER, "{theme:?}: Sales covers exactly the threshold, {covered:?}");
        archforge_ui_egui::snapshot::cx(h.state_mut(), |cx| uml_package_nav::enter(cx, sales, 0.0));
        h.run();
        rest(&mut h);
        assert_eq!(zoom_out_gesture(&mut h, 40, true), vec![sales_d, top], "{theme:?}");
        assert!(centred_at_landing(&h, card), "{theme:?}: Sales centred at 100 %, {:?}", (h.state().canvas.pan, h.state().canvas.zoom));
    }
}

/// Each return restores the view of its own visit (AR-668): A → B → A → B by Open, A left first at
/// one view and then at another, B at a third; zooming out goes back B → A → B → A, landing on A's
/// second view, then B's, then A's first.
#[test]
fn returns_restore_the_view_each_visit_was_left_with() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let mut h = workspace_harness(theme, egui::vec2(1600.0, 1000.0));
        let id = |v: serde_json::Value| archforge_mof::Uuid::parse(v["id"].as_str().unwrap()).unwrap();
        // A is Commerce's diagram and shows Archive; B is Archive's and shows Commerce.
        let (commerce, archive, a, b) = {
            let s = h.state_mut().session.as_mut().unwrap();
            let commerce = id(s.execute("uml.package.add", &serde_json::json!({ "name": "Commerce" })).unwrap().value);
            let archive = id(s.execute("uml.package.add", &serde_json::json!({ "name": "Archive" })).unwrap().value);
            let (a, b) = (own_diagram(s, commerce), own_diagram(s, archive));
            for (on, what) in [(a, archive), (b, commerce)] {
                let p =
                    serde_json::json!({ "diagram": on.to_string(), "element": what.to_string(), "x": 200, "y": 160, "width": 240, "height": 148 });
                s.execute("diagram.place", &p).unwrap();
            }
            (commerce, archive, a, b)
        };
        let views = [(egui::vec2(40.0, 30.0), 0.8), (egui::vec2(60.0, 20.0), 0.9), (egui::vec2(-20.0, 50.0), 0.7)];
        archforge_ui_egui::canvas::open(h.state_mut(), a);
        h.run();
        for (package, view) in [archive, commerce, archive].into_iter().zip(views) {
            (h.state_mut().canvas.pan, h.state_mut().canvas.zoom) = view;
            h.run();
            archforge_ui_egui::snapshot::cx(h.state_mut(), |cx| uml_package_nav::enter(cx, package, 0.0));
            h.run();
            rest(&mut h);
        }
        assert_eq!(h.state().canvas.active, Some(b), "{theme:?}");
        for (from, to, view) in [(b, a, views[2]), (a, b, views[1]), (b, a, views[0])] {
            assert_eq!(zoom_out_gesture(&mut h, 40, true), vec![from, to], "{theme:?}");
            assert_eq!((h.state().canvas.pan, h.state().canvas.zoom), view, "{theme:?}: {to} as that visit left it");
            rest(&mut h);
        }
    }
}

/// A return that skips a step to a removed diagram lands on one with no card of the package left,
/// and fits it rather than restoring the view it was left with (AR-668): Top → Sales → Archive,
/// Sales' diagram then removed; zooming out of Archive opens Top with both its far classes in view.
#[test]
fn a_return_past_a_removed_diagram_fits_the_one_it_lands_on() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let mut h = workspace_harness(theme, egui::vec2(1600.0, 1000.0));
        let (top, sales, _, sales_d) = top_with_sales(&mut h, (240, 148), &[0, 4000]);
        let id = |v: serde_json::Value| archforge_mof::Uuid::parse(v["id"].as_str().unwrap()).unwrap();
        let s = h.state_mut().session.as_mut().unwrap();
        let archive = id(s.execute("uml.package.add", &serde_json::json!({ "name": "Archive", "parent": sales.to_string() })).unwrap().value);
        let p =
            serde_json::json!({ "diagram": sales_d.to_string(), "element": archive.to_string(), "x": 200, "y": 160, "width": 240, "height": 148 });
        s.execute("diagram.place", &p).unwrap();
        let archive_d = own_diagram(s, archive);
        archforge_ui_egui::canvas::open(h.state_mut(), top);
        h.run();
        let left = (egui::vec2(-50.0, 30.0), 0.6);
        (h.state_mut().canvas.pan, h.state_mut().canvas.zoom) = left;
        h.run();
        archforge_ui_egui::snapshot::cx(h.state_mut(), |cx| uml_package_nav::enter(cx, sales, 0.0));
        h.run();
        archforge_ui_egui::snapshot::cx(h.state_mut(), |cx| uml_package_nav::enter(cx, archive, 0.0));
        h.run();
        assert_eq!(h.state().canvas.active, Some(archive_d), "{theme:?}: Archive open");
        // Sales' diagram is its default: removing it makes Sales a new, empty one.
        let s = h.state_mut().session.as_mut().unwrap();
        let at = s.token();
        let p = serde_json::json!({ "diagram": sales_d.to_string(), "create_replacement": { "name": "Sales" }, "expected_revision": { "history": at.history, "revision": at.revision } });
        s.execute("diagram.remove", &p).unwrap();
        h.run();
        rest(&mut h);
        assert_eq!(zoom_out_gesture(&mut h, 40, true), vec![archive_d, top], "{theme:?}: past Sales' removed diagram");
        let c = &h.state().canvas;
        let seen = [0.0, 4200.0].iter().all(|x| (0.0..=c.size.x).contains(&(c.pan.x + x * c.zoom)));
        assert!(seen && (c.pan, c.zoom) != left, "{theme:?}: Top fitted, not as left, {:?}", (c.pan, c.zoom));
    }
}

/// A return restores the view an entry by Open or ⌘↓ left (AR-668): Top, at a view of its own with
/// Sales selected and in view, entered through Sales' Open button or ⌘↓, comes back on zooming out
/// at exactly that pan and zoom.
#[test]
fn a_return_restores_the_view_an_entry_by_open_or_command_down_left() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        for control in ["Open", "⌘↓"] {
            let mut h = workspace_harness(theme, egui::vec2(1600.0, 1000.0));
            let (top, sales, card, sales_d) = top_with_sales(&mut h, (480, 300), &[]);
            archforge_ui_egui::canvas::open(h.state_mut(), top);
            h.run();
            let left = (egui::vec2(-50.0, 30.0), 0.6);
            (h.state_mut().canvas.pan, h.state_mut().canvas.zoom) = left;
            h.run();
            rest(&mut h);
            h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [sales.to_string()] })).unwrap();
            h.state_mut().canvas.focused = true;
            h.run();
            if control == "Open" {
                let at = rect_of(&h, &format!("workspace.canvas.node.{card}.enter")).center();
                click(&mut h, at);
            } else {
                key(&mut h, egui::Key::ArrowDown, egui::Modifiers::MAC_CMD | egui::Modifiers::COMMAND);
            }
            assert_eq!(h.state().canvas.active, Some(sales_d), "{theme:?}: {control} entered Sales");
            h.run();
            rest(&mut h);
            assert_eq!(zoom_out_gesture(&mut h, 40, true), vec![sales_d, top], "{theme:?} {control}");
            assert_eq!((h.state().canvas.pan, h.state().canvas.zoom), left, "{theme:?}: Top as {control} left it");
        }
    }
}

/// A return that lands at the least zoom goes back one more step on the next zoom-out once the
/// switch has rested, with no zoom-in first (AR-668): Top → Sales → Archive, Archive's card then
/// removed from Sales' diagram, whose classes lie too far apart to fit above 10 %; zooming out of
/// Archive lands on Sales fitted at 10 %, where a zoom-out straight away does nothing, and after
/// the rest one fresh zoom-out returns to Top.
#[test]
fn a_return_landing_at_the_least_zoom_goes_back_on_the_next_zoom_out() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let mut h = workspace_harness(theme, egui::vec2(1600.0, 1000.0));
        let (top, sales, _, sales_d) = top_with_sales(&mut h, (240, 148), &[]);
        let id = |v: serde_json::Value| archforge_mof::Uuid::parse(v["id"].as_str().unwrap()).unwrap();
        let s = h.state_mut().session.as_mut().unwrap();
        let archive = id(s.execute("uml.package.add", &serde_json::json!({ "name": "Archive", "parent": sales.to_string() })).unwrap().value);
        let p =
            serde_json::json!({ "diagram": sales_d.to_string(), "element": archive.to_string(), "x": 200, "y": 160, "width": 240, "height": 148 });
        let archive_card = id(s.execute("diagram.place", &p).unwrap().value);
        for (i, x) in [0, 400_000].iter().enumerate() {
            let c = id(s.execute("uml.class.add", &serde_json::json!({ "package": sales.to_string(), "name": format!("Wide{i}") })).unwrap().value);
            let p = serde_json::json!({ "diagram": sales_d.to_string(), "element": c.to_string(), "x": x, "y": 0, "width": 200, "height": 120 });
            s.execute("diagram.place", &p).unwrap();
        }
        let archive_d = own_diagram(s, archive);
        archforge_ui_egui::canvas::open(h.state_mut(), top);
        h.run();
        archforge_ui_egui::snapshot::cx(h.state_mut(), |cx| uml_package_nav::enter(cx, sales, 0.0));
        h.run();
        archforge_ui_egui::snapshot::cx(h.state_mut(), |cx| uml_package_nav::enter(cx, archive, 0.0));
        h.run();
        assert_eq!(h.state().canvas.active, Some(archive_d), "{theme:?}: Archive open");
        let p = serde_json::json!({ "nodes": [archive_card.to_string()] });
        h.state_mut().session.as_mut().unwrap().execute("diagram.unplace", &p).unwrap();
        h.run();
        rest(&mut h);
        assert_eq!(zoom_out_gesture(&mut h, 40, true), vec![archive_d, sales_d], "{theme:?}: back to Sales");
        assert_eq!(h.state().canvas.zoom, archforge_shell_api::canvas::ZOOM_RANGE.0, "{theme:?}: Sales too wide, fitted at 10 %");
        assert_eq!(zoom_out_gesture(&mut h, 1, true), vec![sales_d], "{theme:?}: a zoom-out straight after the switch does nothing");
        rest(&mut h);
        assert_eq!(zoom_out_gesture(&mut h, 1, true), vec![sales_d, top], "{theme:?}: one fresh zoom-out back to Top");
    }
}

/// A line, a note's link or a label a diagram command refuses by its rules is said in the chosen
/// language: a line between two classes with no relationship, of a class, of an attribute, which
/// lacks an end, between boxes that do not show its ends, and one shown already; a note's link from
/// a class's box, to itself, to another note and twice, and one onto another diagram said from the
/// reason alone; and a label on a note's link or shown already. In light and dark (AR-515).
#[test]
fn a_line_link_or_label_refused_is_said_in_the_chosen_language() {
    use serde_json::json;
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, nodes) = related_sample_in(theme, egui::vec2(1600.0, 1000.0));
        h.state_mut().state.language = "de".into();
        h.run();
        let run = |h: &mut Harness<'static, Shell>, cmd: &str, p: serde_json::Value| archforge_ui_egui::snapshot::run(h.state_mut(), cmd, p);
        let refused = |h: &mut Harness<'static, Shell>, cmd: &str, p: serde_json::Value| run(h, cmd, p).err().unwrap_or_default();
        let id = |h: &mut Harness<'static, Shell>, cmd: &str, p: serde_json::Value| run(h, cmd, p).unwrap()["id"].as_str().unwrap().to_string();
        let d = h.state().canvas.active.unwrap().to_string();
        let order = named(&h, archforge_uml::CLASS, "Order").to_string();
        let st = h.state().session.as_ref().unwrap().doc();
        let g = st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_uml::GENERALIZATION)).unwrap();
        let (specific, general) = archforge_uml::figures::ends(st, g).unwrap();
        let (specific, general) = (&nodes[&archforge_core::name_of(st, specific)], &nodes[&archforge_core::name_of(st, general)]);
        let g = st.uuid(g).unwrap().to_string();
        let edges = st.list(st.by_uuid(h.state().canvas.active.unwrap()).unwrap(), archforge_diagram::diagram::EDGE).unwrap();
        let shows = |e: &archforge_mof::Obj| st.get(*e, archforge_diagram::edge::ELEMENT).ok().and_then(|v| v.as_ref());
        let is_association = |e: &archforge_mof::Obj| shows(e).and_then(|el| st.class_of(el).ok()) == Some(archforge_uml::ASSOCIATION);
        let association = st.uuid(edges.iter().filter_map(archforge_mof::Value::as_ref).find(is_association).unwrap()).unwrap().to_string();
        let connect = |element: &str, source: &str, target: &str| json!({ "diagram": d, "element": element, "source": source, "target": target });
        let between = connect("", &nodes["Order"], &nodes["Customer"]);
        let unnamed = "Eine Linie zwischen zwei Elementen zeigt eine Beziehung. Geben Sie die Beziehung an.";
        assert_eq!(refused(&mut h, "diagram.connect", between), unnamed, "{theme:?}: the language is German");
        let of_class = connect(&order, &nodes["Order"], &nodes["Customer"]);
        assert_eq!(refused(&mut h, "diagram.connect", of_class), "Ein Diagramm dieser Art zeigt Order nicht.", "{theme:?}");
        let status = id(&mut h, "uml.attribute.add", json!({ "class": order, "name": "status" }));
        let lacks = "Diesem Element fehlt ein Ende, daher zeigt keine Linie es.";
        assert_eq!(refused(&mut h, "diagram.connect", connect(&status, &nodes["Order"], &nodes["Customer"])), lacks, "{theme:?}");
        let first = "Der erste Kasten zeigt nicht das erste Ende der Beziehung.";
        assert_eq!(refused(&mut h, "diagram.connect", connect(&g, general, general)), first, "{theme:?}");
        let second = "Der zweite Kasten zeigt nicht das zweite Ende der Beziehung.";
        assert_eq!(refused(&mut h, "diagram.connect", connect(&g, specific, specific)), second, "{theme:?}");
        let shown = format!("Dieses Diagramm zeigt {g} bereits.");
        assert_eq!(refused(&mut h, "diagram.connect", connect(&g, specific, general)), shown, "{theme:?}");
        // A note's links.
        let note = id(&mut h, "diagram.note", json!({ "diagram": d, "text": "Hinweis", "x": 0, "y": 0 }));
        let other = id(&mut h, "diagram.note", json!({ "diagram": d, "text": "Zweiter", "x": 0, "y": 200 }));
        let link = |note: &str, target: &str| json!({ "diagram": d, "note": note, "target": target });
        let not_note = "Dieser Kasten zeigt Order. Nur eine Notiz wird mit einem Kasten oder einer Linie verbunden.";
        assert_eq!(refused(&mut h, "diagram.link-note", link(&nodes["Order"], &note)), not_note, "{theme:?}");
        assert_eq!(refused(&mut h, "diagram.link-note", link(&note, &note)), "Eine Notiz wird nicht mit sich selbst verbunden.", "{theme:?}");
        let unlinkable = "Eine Notiz wird mit einem Kasten verbunden, der ein Element zeigt, oder mit der Linie einer Beziehung.";
        assert_eq!(refused(&mut h, "diagram.link-note", link(&note, &other)), unlinkable, "{theme:?}");
        let anchor = id(&mut h, "diagram.link-note", link(&note, &nodes["Order"]));
        assert_eq!(refused(&mut h, "diagram.link-note", link(&note, &nodes["Order"])), "Die Notiz ist bereits damit verbunden.", "{theme:?}");
        let elsewhere = archforge_shell_api::panels::said(SCREENS.notation_uis(), &archforge_engine::Reason::TargetOtherDiagram);
        assert_eq!(elsewhere.as_deref(), Some("Womit die Notiz verbunden wird, liegt in einem anderen Diagramm."), "{theme:?}");
        // Labels.
        let label = |edge: &str| json!({ "edge": edge, "kind": "from_end" });
        let unlabelled = "Die Verbindung einer Notiz zeigt kein Element und erhält daher keine Beschriftung.";
        assert_eq!(refused(&mut h, "diagram.label", label(&anchor)), unlabelled, "{theme:?}");
        id(&mut h, "diagram.label", label(&association));
        assert_eq!(refused(&mut h, "diagram.label", label(&association)), "Diese Linie hat den Beschriftungstext from_end bereits.", "{theme:?}");
    }
    archforge_ui_egui::i18n::set_language("en");
}
