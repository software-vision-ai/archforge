//! The Edit menu over UML's elements: Select All takes a generalization set and Delete on a set's
//! label removes the set and keeps its lines.

use egui::{Key, Modifiers};

use super::*;

/// The selection's ids, sorted.
fn selection(h: &Harness<'static, Shell>) -> Vec<String> {
    let s = h.state().session.as_ref().unwrap();
    let mut ids: Vec<String> = s.view().selection.iter().map(|o| s.doc().uuid(*o).unwrap().to_string()).collect();
    ids.sort();
    ids
}

/// Delete after a click on a generalization set's label takes the set from the model and keeps its
/// generalizations and every box (AR-129), both while no node holds the label and after the label
/// was moved, which nests its node in a classifier's: the keys' boxes are the diagram's own nodes,
/// so a selected set never counts among them. With a box selected beside the set, the box goes off
/// the diagram and the set stays.
#[test]
fn delete_on_a_set_label_removes_the_set_and_keeps_its_lines() {
    use archforge_uml::generalization_sets as sets;
    let object = |h: &Harness<'static, Shell>, id: &str| {
        let st = h.state().session.as_ref().unwrap().doc();
        st.by_uuid(archforge_mof::Uuid::parse(id).unwrap()).filter(|o| st.is_alive(*o))
    };
    let boxes = |h: &Harness<'static, Shell>| {
        let s = h.state().session.as_ref().unwrap();
        s.doc().list(s.view().open_diagram.unwrap(), archforge_diagram::diagram::NODE).unwrap().len()
    };
    for (theme, moved, with_box) in [
        (ThemeKind::Light, false, false),
        (ThemeKind::Dark, false, false),
        (ThemeKind::Light, true, false),
        (ThemeKind::Dark, true, false),
        (ThemeKind::Light, false, true),
        (ThemeKind::Dark, true, true),
    ] {
        let (mut h, _) = related_sample_in(theme, egui::vec2(1600.0, 1000.0));
        let (gens, payment) = archforge_ui_egui::snapshot::cx(h.state_mut(), uml_ui::put_payment_kinds).unwrap();
        (h.state_mut().canvas.zoom, h.state_mut().canvas.pan) = (0.75, egui::vec2(-470.0, -230.0));
        let d = h.state().canvas.active.unwrap().to_string();
        let s = h.state_mut().session.as_mut().unwrap();
        let set = s.execute("uml.generalization-set.add", &serde_json::json!({ "generalizations": gens, "name": "method" })).unwrap().value["id"]
            .as_str()
            .unwrap()
            .to_string();
        s.execute("view.select", &serde_json::json!({ "ids": [] })).unwrap();
        h.run();
        let label = format!("workspace.canvas.generalizationSet.{set}");
        if moved {
            let at = rect_of(&h, &label).center();
            drag(&mut h, at, at + egui::vec2(40.0, -20.0));
            h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [] })).unwrap();
            h.run();
        }
        let st = h.state().session.as_ref().unwrap().doc();
        let node = sets::label_node(st, st.by_uuid(archforge_mof::Uuid::parse(&d).unwrap()).unwrap(), object(&h, &set).unwrap());
        assert_eq!(node.is_some(), moved, "{theme:?}: a node holds the label only once it was moved");
        let before = boxes(&h);
        let at = rect_of(&h, &label).center();
        click(&mut h, at);
        assert_eq!(selection(&h), std::slice::from_ref(&set), "{theme:?} moved {moved}: the click on the label selects the set alone");
        if with_box {
            h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [payment, set] })).unwrap();
            h.run();
        }
        key(&mut h, Key::Delete, Modifiers::NONE);
        assert!(gens.iter().all(|g| object(&h, g).is_some()), "{theme:?} moved {moved}: the generalizations stay");
        assert!(object(&h, &payment).is_some(), "{theme:?} moved {moved}: no class left the model");
        if with_box {
            assert!(object(&h, &set).is_some(), "{theme:?} moved {moved}: beside a box the set stays in the model");
            assert_eq!(boxes(&h), before - 1, "{theme:?} moved {moved}: and the box is off the diagram");
        } else {
            assert!(object(&h, &set).is_none(), "{theme:?} moved {moved}: the set is gone from the model");
            assert_eq!(boxes(&h), before, "{theme:?} moved {moved}: every box is still on the diagram");
        }
    }
}

/// Select all takes a generalization set with the boxes and lines, once, both while its label sits
/// where the lines put it, no node holding it, and after the label was moved, which nests its node
/// in a classifier's.
#[test]
fn select_all_takes_a_generalization_set_whether_its_label_was_moved_or_not() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, _) = related_sample_in(theme, egui::vec2(1600.0, 1000.0));
        let (gens, _) = archforge_ui_egui::snapshot::cx(h.state_mut(), uml_ui::put_payment_kinds).unwrap();
        let d = h.state().canvas.active.unwrap().to_string();
        let s = h.state_mut().session.as_mut().unwrap();
        let set = s.execute("uml.generalization-set.add", &serde_json::json!({ "generalizations": gens, "name": "method" })).unwrap().value["id"]
            .as_str()
            .unwrap()
            .to_string();
        h.state_mut().canvas.focused = true;
        h.run();
        let label = h.state().canvas.extra_labels.clone();
        assert_eq!(label.iter().map(|l| (l.element.to_string(), l.node)).collect::<Vec<_>>(), [(set.clone(), None)], "{theme:?}: drawn, no node");
        key(&mut h, Key::A, Modifiers::COMMAND);
        let unmoved = selection(&h);
        assert_eq!(unmoved.iter().filter(|id| **id == set).count(), 1, "{theme:?}: the set of an unmoved label");
        assert!(gens.iter().all(|g| unmoved.contains(g)), "{theme:?}: its lines beside it");
        let s = h.state_mut().session.as_mut().unwrap();
        s.execute("view.select", &serde_json::json!({ "ids": [] })).unwrap();
        s.execute("uml.generalization-set.place", &serde_json::json!({ "diagram": d, "set": set, "x": 40, "y": 40 })).unwrap();
        h.run();
        assert!(h.state().canvas.extra_labels.iter().all(|l| l.node.is_some()), "{theme:?}: a node holds the label now");
        key(&mut h, Key::A, Modifiers::COMMAND);
        assert_eq!(selection(&h), unmoved, "{theme:?}: the same selection with the label moved");
    }
}

/// Whether `text` was copied in the last frame.
fn copied(h: &Harness<'static, Shell>, text: &str) -> bool {
    h.output().platform_output.commands.contains(&egui::OutputCommand::CopyText(text.into()))
}

/// Opens Edit and chooses the row `at` by the pointer, each press and release a frame of its own;
/// whether the frames after it copied `text`.
fn through_edit(h: &mut Harness<'static, Shell>, at: &str, text: &str) -> bool {
    for id in ["strip.menu.edit", at] {
        h.run();
        let at = rect_of(h, id).center();
        h.event(egui::Event::PointerMoved(at));
        h.run();
        (0..2).for_each(|n| {
            h.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: n == 0, modifiers: Modifiers::NONE })
        });
    }
    (0..4).any(|_| (h.step(), copied(h, text)).1)
}

/// An editor on the canvas, its text `typed` "Draft", stays open through the in-window Edit menu:
/// Select all, Copy, Cut and Paste act on its text, `pasted` replacing it. File, opened from Edit,
/// finds it committed.
fn stays_open(h: &mut Harness<'static, Shell>, typed: fn(&Shell) -> Option<String>, pasted: &str, theme: ThemeKind) {
    let row = |r: &str| format!("strip.menu.row.edit.{r}");
    through_edit(h, &row("select-all"), "");
    assert!(through_edit(h, &row("copy"), "Draft") && typed(h.state()).as_deref() == Some("Draft"), "{theme:?}: Copy copied all of it");
    assert!(through_edit(h, &row("cut"), "Draft") && typed(h.state()).as_deref() == Some(""), "{theme:?}: Cut took it");
    through_edit(h, &row("paste"), "");
    h.event(egui::Event::Paste(pasted.into()));
    h.run();
    assert_eq!(typed(h.state()).as_deref(), Some(pasted), "{theme:?}: pasted");
    for id in ["strip.menu.edit", "strip.menu.file"] {
        let at = rect_of(h, id).center();
        click(h, at);
    }
    assert_eq!(
        (archforge_ui_egui::testing::open_menu(h.state()), typed(h.state())),
        (Some("file"), None),
        "{theme:?}: File, from Edit, found it committed"
    );
    key(h, Key::Escape, Modifiers::NONE);
}

/// A class's member typed in place, in its name field and in its type field, stays open through the
/// in-window Edit menu, whose press is a click away from the member's row; another menu commits it.
#[test]
fn the_member_editor_stays_open_through_the_in_window_edit_menu() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let mut h = workspace_harness(theme, egui::vec2(1600.0, 1000.0));
        archforge_ui_egui::snapshot::cx(h.state_mut(), uml_ui::open_sample).unwrap();
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
            h.event(egui::Event::Text(if ty { "rating" } else { "Draft" }.into()));
            h.run();
            if ty {
                key(&mut h, Key::Tab, Modifiers::NONE);
                h.event(egui::Event::Text("Draft".into()));
                h.run();
                stays_open(&mut h, |s| uml_of(s).inline.as_ref().map(|i| i.ty.clone()), "Real", theme);
            } else {
                stays_open(&mut h, |s| uml_of(s).inline.as_ref().map(|i| i.edit.name.clone()), "Later", theme);
            }
        }
        let rows = member_rows(&h, catalog, false);
        assert!(rows.iter().any(|r| r == "+ Later") && rows.iter().any(|r| r == "+ rating : Real"), "{theme:?}: {rows:?}");
    }
}

/// Delete with a generalization set selected beside a box deletes nothing from the model, though
/// the selection holds the set: the set stays.
#[test]
fn delete_after_select_all_deletes_no_set_from_the_model() {
    let alive = |h: &Harness<'static, Shell>, id: &str| {
        let st = h.state().session.as_ref().unwrap().doc();
        st.by_uuid(archforge_mof::Uuid::parse(id).unwrap()).is_some_and(|o| st.is_alive(o))
    };
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, _) = related_sample_in(theme, egui::vec2(1600.0, 1000.0));
        let (gens, payment) = archforge_ui_egui::snapshot::cx(h.state_mut(), uml_ui::put_payment_kinds).unwrap();
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
