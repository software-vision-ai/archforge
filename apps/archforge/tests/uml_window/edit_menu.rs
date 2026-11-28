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
