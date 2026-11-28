//! The Model menu's window tests: the rows each notation's kind answers, the tools they arm and the
//! elements they make.

use archforge_shell_api::canvas::Tool;
use archforge_ui_egui::actions::{available, label, run};
use archforge_ui_egui::model_menu::sections as model_sections;
use egui::Key::{ArrowDown, ArrowRight, Enter, F10};
use uml_classifier::{Classifier, KINDS, TOOL};
use uml_relate::Relate;

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

/// A key pressed and let go, with no modifier.
fn menu_press(h: &mut Harness<'static, Shell>, key: egui::Key) {
    h.event(egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE });
    h.event(egui::Event::Key { key, physical_key: None, pressed: false, repeat: false, modifiers: egui::Modifiers::NONE });
    h.run();
}

/// The menu open and the row the keys are on.
fn menu_at(h: &Harness<'static, Shell>) -> (Option<&'static str>, Option<&'static str>) {
    (archforge_ui_egui::testing::open_menu(h.state()), archforge_ui_egui::testing::menu_row(h.state()))
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

/// Runs `id` from the kinds given, with a popover open, and gives the canvas's tool and kinds after.
fn armed_by(h: &mut Harness<'static, Shell>, id: &str, classifier: Classifier, relate: Relate) -> (Tool, Classifier, Relate) {
    let s = h.state_mut();
    s.canvas.tool = Tool::Select;
    let uml = uml_of_mut(s);
    (uml.classifier, uml.relation) = (classifier, relate);
    (s.canvas.popover, s.canvas.menu_focus) = (Some(KINDS), Some(0));
    run(s, id);
    assert!(s.canvas.tool == Tool::Select || (s.canvas.popover, s.canvas.menu_focus) == (None, None), "{id} closes the toolbar's popover");
    (s.canvas.tool, uml_of(s).classifier, uml_of(s).relation)
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

/// Chosen from the bar, a row arms the tool and the next click on the canvas makes the element
/// through the tool's own command; a row off takes no click; the keys reach the rows too.
#[test]
fn a_chosen_row_makes_its_element_as_the_tool_does() {
    for theme in BOTH {
        let mut h = uml_sized(theme, BIG);
        click_menu(&mut h, "strip.menu.model");
        click_menu(&mut h, "strip.menu.row.model.interface");
        assert_eq!((archforge_ui_egui::testing::open_menu(h.state()), h.state().canvas.tool), (None, Tool::Place(TOOL)), "{theme:?}");
        let centre = rect_of(&h, "workspace.canvas").center();
        click(&mut h, centre);
        assert_eq!(h.state().session.as_ref().unwrap().undo_label(), Some("Add interface"), "{theme:?}");
        let mut h = menu_harness(theme, true);
        click_menu(&mut h, "strip.menu.model");
        click_menu(&mut h, "strip.menu.row.model.note");
        assert_eq!(
            (archforge_ui_egui::testing::open_menu(h.state()), h.state().canvas.tool),
            (Some("model"), Tool::Select),
            "{theme:?}: off, it stays open"
        );
        let mut h = er(theme);
        [F10, ArrowRight, ArrowRight, ArrowRight, ArrowDown].into_iter().for_each(|key| menu_press(&mut h, key));
        assert_eq!(menu_at(&h), (Some("model"), Some("model.relationship")), "{theme:?}");
        menu_press(&mut h, Enter);
        assert_eq!(h.state().canvas.tool, Tool::Connect("relationship"), "{theme:?}");
    }
}

/// The ids of `rows`, by section.
fn ids_of(rows: &[&[archforge_shell_api::kinds::ModelRow]]) -> Vec<Vec<&'static str>> {
    rows.iter().map(|section| section.iter().map(|row| row.key).collect()).collect()
}

/// Each notation's kind answers its own rows as data, by section in the menu's order: each row's
/// id, its word and the tool it arms, with the choice of the tool's popover it makes first where
/// the tool has one. The table of actions holds every kind's rows in the kinds' order, then the
/// shared ones, each id once, none with keys or among the table's own actions, which alone Home's
/// search lists.
#[test]
fn each_kind_answers_its_own_rows() {
    use archforge_shell_api::kinds::DiagramKind;
    assert_eq!(ids_of(er_kind::ErKind.menu_rows()), [ER_ROWS.to_vec()]);
    let classifiers = ["model.class", "model.interface", "model.enumeration", "model.data-type", "model.primitive-type", "model.package"];
    let relate = [
        vec![
            "model.relate.association",
            "model.relate.directed",
            "model.relate.aggregation",
            "model.relate.composition",
            "model.relate.generalization",
            "model.relate.dependency",
        ],
        vec!["model.relate.realization", "model.relate.association-class"],
        vec!["model.relate.n-ary", "model.relate.template-binding"],
    ];
    let uml: Vec<Vec<&str>> = std::iter::once(classifiers.to_vec()).chain(relate).collect();
    assert_eq!(ids_of(uml_ui::UmlKind.menu_rows()), uml);
    let all: Vec<&str> = archforge_ui_egui::testing::model_rows(SCREENS.kinds()).into_iter().map(|(_, row)| row.key).collect();
    assert_eq!(all, [ER_ROWS.to_vec(), uml.concat(), SHARED.to_vec()].concat());
    assert!(all.iter().enumerate().all(|(n, id)| !all[n + 1..].contains(id)), "{all:?}");
    assert!(
        all.iter().all(|id| archforge_ui_egui::actions::keys(id).is_none() && !archforge_ui_egui::actions::ACTIONS.iter().any(|a| a.id == *id)),
        "{all:?}"
    );
    assert_eq!(
        archforge_ui_egui::testing::model_menus(SCREENS.kinds()),
        [[ER_ROWS.to_vec(), SHARED.to_vec()].concat(), [uml.concat(), SHARED.to_vec()].concat()]
    );
    // The data itself: each row's word, the tool it arms, whether it makes a choice first, and the
    // kind it is of.
    let data: Vec<(&str, String, Tool, bool, Option<&str>)> = archforge_ui_egui::testing::model_rows(SCREENS.kinds())
        .into_iter()
        .map(|(kind, row)| (row.key, (row.word)(), row.tool, row.choice.is_some(), kind))
        .collect();
    let (er, structure) = (Some(archforge_er::DIAGRAM_KIND), Some(archforge_uml::STRUCTURE_DIAGRAM));
    let mut wanted = vec![
        ("model.entity", "Entity".to_string(), Tool::Place("entity"), false, er),
        ("model.relationship", "Relationship".to_string(), Tool::Connect("relationship"), false, er),
    ];
    wanted.extend(classifiers.into_iter().zip(Classifier::ALL).map(|(id, k)| (id, k.word(), Tool::Place(TOOL), true, structure)));
    wanted.push(("model.package", "Package".to_string(), Tool::Place("package"), false, structure));
    wanted.extend(
        uml[1..].concat().into_iter().zip(Relate::ALL).map(|(id, k)| (id, k.word().to_string(), Tool::Connect("relationship"), true, structure)),
    );
    wanted.extend([("model.note", "Note".to_string(), Tool::Note, false, None), ("model.link", "Link note".to_string(), Tool::Link, false, None)]);
    assert_eq!(data, wanted);
}

/// The menu's entries are made from the rows' data, whichever kind answers them: on a diagram of
/// its kind, each row's label is its word and choosing it arms the tool it names, closing the
/// toolbar's popover.
#[test]
fn the_entries_are_made_from_the_rows_data() {
    use archforge_shell_api::kinds::DiagramKind;
    for theme in BOTH {
        let kinds: [(Harness<'static, Shell>, &dyn DiagramKind); 2] = [(er(theme), &er_kind::ErKind), (uml_sized(theme, BIG), &uml_ui::UmlKind)];
        for (mut h, kind) in kinds {
            let s = h.state_mut();
            for (_, row) in archforge_ui_egui::testing::model_rows(&[kind]) {
                (s.canvas.tool, s.canvas.popover, s.canvas.menu_focus) = (Tool::Select, Some(KINDS), Some(0));
                assert_eq!(label(s, row.key), (row.word)(), "{theme:?}: {}", row.key);
                assert!(available(s, row.key), "{theme:?}: {}", row.key);
                run(s, row.key);
                assert_eq!((s.canvas.tool, s.canvas.popover, s.canvas.menu_focus), (row.tool, None, None), "{theme:?}: {}", row.key);
            }
        }
    }
}
