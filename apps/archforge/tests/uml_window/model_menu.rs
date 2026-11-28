//! The Model menu on a class diagram: each of UML's rows is off where its tool cannot be used, and
//! the longest menu fits the lowest window.

use archforge_shell_api::canvas::Tool;
use archforge_shell_api::state::Explorer;
use archforge_ui_egui::actions::{available, run};
use archforge_ui_egui::testing::why_model_menu_off as why_off;

use super::*;

const BOTH: [ThemeKind; 2] = [ThemeKind::Light, ThemeKind::Dark];

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

/// A version's details to open over the canvas.
fn open_details(s: &mut Shell) {
    s.services.projects = Some(std::sync::Arc::new(FakeHost::default()));
    archforge_ui_egui::testing::open_version_details(s, version_row("c1", None, (None, false), "2026-11-06T15:05:00Z"));
}

/// Each thing that keeps the tools from the canvas turns every row off, stops a row already chosen,
/// and is named; details and a changed element cover the canvas only under Versions.
#[test]
fn every_row_is_off_where_its_tool_cannot_be_used() {
    type Setup = fn(&mut Shell);
    let (busy, show) = (Some("The project is busy"), Some("Show a diagram first"));
    let states: [(&str, Setup, Option<&str>); 8] = [
        ("held", |s| s.begin_hold().unwrap(), busy),
        ("read-only", |s| s.session.as_mut().unwrap().set_read_only(true).unwrap(), busy),
        ("no project shown", |s| s.active = None, Some("Open or go back to a project first")),
        ("no diagram", |s| s.canvas.active = None, show),
        ("a changed element", |s| archforge_ui_egui::testing::open_change(s, found(1)), show),
        ("a version's details", open_details, show),
        ("a dialog", archforge_ui_egui::support::open, None),
        ("a question", |s| s.ask = Some(archforge_ui_egui::close_ask::Ask::Unsaved { path: "/shop.archforge".into() }), None),
    ];
    for ((name, setup, why), theme) in states.into_iter().flat_map(|state| BOTH.map(|theme| (state, theme))) {
        let mut h = uml_sized(theme, BIG);
        let s = h.state_mut();
        s.state.explorer = Explorer::SourceControl;
        assert!(available(s, "model.note") && available(s, "model.class"), "{name}: on before");
        setup(s);
        for id in ["model.note", "model.class", "model.relate.dependency"] {
            assert!(!available(s, id), "{name}: {id}");
            run(s, id);
            assert_eq!(s.canvas.tool, Tool::Select, "{name}: {id}");
        }
        assert!(why.is_none_or(|why| why_off(s) == why), "{name}: {}", why_off(s));
        if name.starts_with("a ") && why.is_some() {
            s.state.explorer = Explorer::Models;
            assert!(available(s, "model.note"), "{name}: drawn only under Versions");
        }
    }
}

/// In the lowest window the class diagram's whole menu still fits, each row a line of text high.
#[test]
fn the_longest_menu_fits_the_lowest_window() {
    for theme in BOTH {
        let mut h = uml_sized(theme, egui::vec2(640.0, 400.0));
        click_menu(&mut h, "strip.menu.model");
        let last = rect_of(&h, "strip.menu.row.model.link");
        assert!(last.bottom() <= 400.0 && last.height() >= 17.0, "{theme:?}: {last:?}");
    }
}
