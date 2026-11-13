//! The Model menu: its rows are what the open diagram's tools make, each arms that tool and no
//! other, and all of them answer to the one rule of when a tool can be picked.

use egui::Key::{ArrowDown, ArrowRight, Enter, F10};

use super::*;
use crate::actions::{available, label, run};
use crate::canvas::Tool;
use crate::model_menu::{sections as model_sections, why_off};
use crate::state::Explorer;
use crate::uml_classifier::{Classifier, KINDS, TOOL};
use crate::uml_relate::Relate;

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
    let d = s.execute("diagram.add", &serde_json::json!({ "kind": "uml.class", "element": root, "name": "Shop" })).unwrap().value;
    crate::canvas::open(h.state_mut(), archforge_mof::Uuid::parse(d["id"].as_str().unwrap()).unwrap());
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
        let skill = crate::panels::run(h.state_mut(), "skill.add", skill).unwrap();
        crate::canvas::open(h.state_mut(), archforge_mof::Uuid::parse(skill["id"].as_str().unwrap()).unwrap());
        assert_eq!(model_sections(h.state()), [SHARED], "{theme:?}: a skill");
        assert!(!available(h.state(), "model.note"), "{theme:?}: a skill");
        let mut h = er(theme);
        assert_eq!(model_sections(h.state()), [ER_ROWS, SHARED], "{theme:?}");
        assert_eq!(drawn_rows(&mut h), [ER_ROWS, SHARED].concat(), "{theme:?}");
        assert_eq!(model_sections(menu_harness(theme, true).state()), [SHARED], "{theme:?}: no diagram");
    }
}

/// Runs `id` from the kinds given, with a popover open, and gives the canvas's tool and kinds after.
fn armed_by(h: &mut Harness<'static, Shell>, id: &str, classifier: Classifier, relate: Relate) -> (Tool, Classifier, Relate) {
    let s = h.state_mut();
    (s.canvas.tool, s.canvas.uml_classifier, s.canvas.uml_relation) = (Tool::Select, classifier, relate);
    (s.canvas.popover, s.canvas.menu_focus) = (Some(KINDS), Some(0));
    run(s, id);
    assert!(s.canvas.tool == Tool::Select || (s.canvas.popover, s.canvas.menu_focus) == (None, None), "{id} closes the toolbar's popover");
    (s.canvas.tool, s.canvas.uml_classifier, s.canvas.uml_relation)
}

/// Every kind of the tools' own lists has its row, which arms its tool with that kind, leaves the
/// other tool's kind as it was and carries the list's name for it.
#[test]
fn every_row_arms_its_tool() {
    for theme in BOTH {
        let mut h = uml_sized(theme, BIG);
        let (connect, none) = (Tool::Connect("relationship"), Classifier::AssociationClass);
        for kind in Classifier::ALL {
            let id = format!("model.{}", kind.key());
            assert_eq!(armed_by(&mut h, &id, none, Relate::Dependency), (Tool::Place(TOOL), kind, Relate::Dependency), "{id}");
            assert_eq!(label(h.state(), &id), kind.word());
        }
        for (kind, other) in Relate::ALL.into_iter().zip(Relate::ALL.into_iter().cycle().skip(1)) {
            let id = format!("model.relate.{}", kind.key());
            assert_eq!(armed_by(&mut h, &id, none, other), (connect, none, kind), "{id}");
            assert_eq!(label(h.state(), &id), kind.word());
        }
        let shared = [("model.note", Tool::Note, "Note"), ("model.link", Tool::Link, "Link note")];
        for (id, tool, name) in shared.into_iter().chain([("model.package", Tool::Place("package"), "Package")]) {
            assert_eq!(armed_by(&mut h, id, none, Relate::Dependency), (tool, none, Relate::Dependency), "{id}");
            assert_eq!(label(h.state(), id), name);
        }
        assert_eq!(
            armed_by(&mut h, "model.entity", none, Relate::Dependency).0,
            Tool::Select,
            "another notation's row, chosen before the diagram changed"
        );
        let mut h = er(theme);
        let own = [("model.entity", Tool::Place("entity"), "Entity"), ("model.relationship", connect, "Relationship")];
        for (id, tool, name) in shared.into_iter().chain(own) {
            assert_eq!(armed_by(&mut h, id, none, Relate::Dependency).0, tool, "{id}");
            assert_eq!(label(h.state(), id), name);
        }
        assert_eq!(armed_by(&mut h, "model.class", none, Relate::Dependency).0, Tool::Select, "another notation's row");
    }
}

/// A version's details to open over the canvas.
fn open_details(s: &mut Shell) {
    s.services.projects = Some(std::sync::Arc::new(crate::actions::tests::CountingHost::default()));
    crate::version_details::open(s, version_row("c1", None, (None, false), "2026-11-06T15:05:00Z"));
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
        ("a changed element", |s| crate::change_view::open(s, found(1)), show),
        ("a version's details", open_details, show),
        ("a dialog", crate::support::open, None),
        ("a question", |s| s.ask = Some(crate::close_ask::Ask::Unsaved { path: "/shop.archforge".into() }), None),
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

/// Chosen from the bar, a row arms the tool and the next click on the canvas makes the element
/// through the tool's own command; a row off takes no click; the keys reach the rows too.
#[test]
fn a_chosen_row_makes_its_element_as_the_tool_does() {
    for theme in BOTH {
        let mut h = uml_sized(theme, BIG);
        click_menu(&mut h, "strip.menu.model");
        click_menu(&mut h, "strip.menu.row.model.interface");
        assert_eq!((h.state().menu_bar.open, h.state().canvas.tool), (None, Tool::Place(TOOL)), "{theme:?}");
        let centre = rect_of(&h, "workspace.canvas").center();
        click(&mut h, centre);
        assert_eq!(h.state().session.as_ref().unwrap().undo_label(), Some("Add interface"), "{theme:?}");
        let mut h = menu_harness(theme, true);
        click_menu(&mut h, "strip.menu.model");
        click_menu(&mut h, "strip.menu.row.model.note");
        assert_eq!((h.state().menu_bar.open, h.state().canvas.tool), (Some("model"), Tool::Select), "{theme:?}: off, it stays open");
        let mut h = er(theme);
        [F10, ArrowRight, ArrowRight, ArrowRight, ArrowDown].into_iter().for_each(|key| menu_press(&mut h, key));
        assert_eq!(menu_at(&h), (Some("model"), Some("model.relationship")), "{theme:?}");
        menu_press(&mut h, Enter);
        assert_eq!(h.state().canvas.tool, Tool::Connect("relationship"), "{theme:?}");
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
