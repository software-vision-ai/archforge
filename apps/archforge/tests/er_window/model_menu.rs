//! The Model menu's window tests: the rows each notation's kind answers.

use archforge_ui_egui::actions::available;
use archforge_ui_egui::model_menu::sections as model_sections;

use super::*;

const BOTH: [ThemeKind; 2] = [ThemeKind::Light, ThemeKind::Dark];

const SHARED: [&str; 2] = ["model.note", "model.link"];

const ER_ROWS: [&str; 2] = ["model.entity", "model.relationship"];

const BIG: egui::Vec2 = egui::vec2(1600.0, 1000.0);

/// A project showing an empty class diagram, with the in-window bar.
fn uml_sized(theme: ThemeKind, size: egui::Vec2) -> Harness<'static, Shell> {
    let mut h = workspace_harness(theme, size);
    h.state_mut().in_window_menu = true;
    let s = h.state_mut().session.as_mut().unwrap();
    let root = s.execute("uml.model.root", &serde_json::json!({})).unwrap().value["id"].as_str().unwrap().to_string();
    let d =
        s.execute("diagram.add", &serde_json::json!({ "kind": archforge_uml::STRUCTURE_DIAGRAM, "element": root, "name": "Shop" })).unwrap().value;
    archforge_ui_egui::canvas::open(h.state_mut(), archforge_mof::Uuid::parse(d["id"].as_str().unwrap()).unwrap());
    h.run();
    h
}

/// A project showing an ER diagram, with the in-window bar.
fn er(theme: ThemeKind) -> Harness<'static, Shell> {
    let (mut h, ..) = diagram_harness();
    (h.state_mut().state.theme, h.state_mut().in_window_menu) = (theme, true);
    h.run();
    h
}

/// Opens the Model menu and gives the actions of the rows it draws.
fn drawn_rows(h: &mut Harness<'static, Shell>) -> Vec<String> {
    click_menu(h, "strip.menu.model");
    menu_ids(h).iter().filter_map(|i| i.strip_prefix("strip.menu.row.")).map(String::from).collect()
}

/// Each diagram's own kinds and the shared rows are listed, and only those; a skill's tab has no canvas.
#[test]
fn the_menu_lists_only_what_the_open_diagram_offers() {
    for theme in BOTH {
        let mut h = uml_sized(theme, BIG);
        assert_eq!(model_sections(h.state()).iter().map(Vec::len).collect::<Vec<_>>(), [6, 6, 2, 2, 2], "{theme:?}: the tools' own sections");
        assert_eq!(drawn_rows(&mut h), model_sections(h.state()).concat(), "{theme:?}");
        let root = h.state().session.as_ref().map(|s| s.doc().uuid(archforge_uml::commands::root_package(s.doc()).unwrap()).unwrap()).unwrap();
        let skill = serde_json::json!({ "subject": root.to_string(), "name": "Naming", "body": "text" });
        let skill = archforge_ui_egui::snapshot::run(h.state_mut(), "skill.add", skill).unwrap();
        archforge_ui_egui::canvas::open(h.state_mut(), archforge_mof::Uuid::parse(skill["id"].as_str().unwrap()).unwrap());
        assert_eq!(model_sections(h.state()), [SHARED], "{theme:?}: a skill");
        assert!(!available(h.state(), "model.note"), "{theme:?}: a skill");
        let mut h = er(theme);
        assert_eq!(model_sections(h.state()), [ER_ROWS, SHARED], "{theme:?}");
        assert_eq!(drawn_rows(&mut h), [ER_ROWS, SHARED].concat(), "{theme:?}");
        assert_eq!(model_sections(menu_harness(theme, true).state()), [SHARED], "{theme:?}: no diagram");
    }
}
