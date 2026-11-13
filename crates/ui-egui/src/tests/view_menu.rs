//! The View menu: its rows in the design's order, Find… and its keys, the zooms and Whole diagram
//! as the minimap's controls do them, Show minimap, and the rows that open an explorer or a panel.

use egui::{Key, Modifiers};

use super::*;
use crate::actions::{FIND, available, run};
use crate::menu_bar::why_off;
use crate::state::{Explorer, Panel};
use crate::view_menu::{CHAT, DIAGRAMS, MINIMAP, MODELS, VIEW_MENU, WHOLE, ZOOM_IN, ZOOM_OUT};

/// The diagram harness in `theme`, with the bar drawn in the window.
fn view_harness(theme: ThemeKind) -> Harness<'static, Shell> {
    let (mut h, ..) = diagram_harness();
    h.state_mut().state.theme = theme;
    h.state_mut().in_window_menu = true;
    h.run();
    h
}

/// The point of the diagram at the canvas's centre.
fn at_centre(h: &Harness<'static, Shell>) -> egui::Vec2 {
    let c = &h.state().canvas;
    (c.size / 2.0 - c.pan) / c.zoom
}

/// The menu holds the design's rows; Find… opens the search from the menu and from ⌘K, and is off
/// while a field has the keyboard; the zooms keep the centre, Whole diagram fits as the minimap does.
#[test]
fn the_rows_find_zoom_and_fit() {
    in_both_appearances(|theme| {
        let mut h = view_harness(theme);
        click_menu(&mut h, "strip.menu.view");
        let drawn: Vec<String> = menu_ids(&h).iter().filter_map(|i| i.strip_prefix("strip.menu.row.")).map(String::from).collect();
        assert_eq!(drawn, VIEW_MENU.concat(), "{theme:?}: the design's order");
        click_menu(&mut h, "strip.menu.row.view.find");
        h.run();
        assert!(h.state().home_search.open, "{theme:?}: chosen, Find… opens the search");
        assert!(!available(h.state(), FIND), "{theme:?}: off while its field has the keyboard");
        assert_eq!(why_off(h.state(), FIND), "Leave the text field first");
        key(&mut h, Key::Escape, Modifiers::NONE);
        h.run();
        assert!(!h.state().home_search.open && available(h.state(), FIND), "{theme:?}");
        key(&mut h, Key::K, Modifiers::COMMAND);
        assert!(h.state().home_search.open, "{theme:?}: ⌘K is Find…'s");
        key(&mut h, Key::Escape, Modifiers::NONE);
        let (zoom, centre) = (h.state().canvas.zoom, at_centre(&h));
        run(h.state_mut(), ZOOM_IN);
        assert_eq!(h.state().canvas.zoom, zoom * crate::canvas::ZOOM_STEP, "{theme:?}");
        assert!((at_centre(&h) - centre).length() < 0.01, "{theme:?}: about the centre");
        run(h.state_mut(), ZOOM_OUT);
        assert!((h.state().canvas.zoom - zoom).abs() < 1e-5, "{theme:?}");
        (h.state_mut().canvas.zoom, h.state_mut().canvas.pan) = (3.0, egui::vec2(-4000.0, 900.0));
        run(h.state_mut(), WHOLE);
        h.run();
        let fitted = (h.state().canvas.zoom, h.state().canvas.pan);
        assert_ne!(fitted.0, 3.0, "{theme:?}: fitted");
        (h.state_mut().canvas.zoom, h.state_mut().canvas.pan) = (3.0, egui::vec2(-4000.0, 900.0));
        let fit = rect_of(&h, "workspace.minimap.fit").center();
        click(&mut h, fit);
        assert_eq!((h.state().canvas.zoom, h.state().canvas.pan), fitted, "{theme:?}: as the minimap's Whole diagram");
    });
}

/// Show minimap is on only while it is folded; a row opens its explorer or panel, and is off while
/// that shows. Without a diagram, or under a version's details, the canvas's rows are off, and on
/// Home only Find… is on.
#[test]
fn the_rows_show_the_minimap_explorers_and_panels() {
    in_both_appearances(|theme| {
        let mut h = view_harness(theme);
        assert!(!available(h.state(), MINIMAP), "{theme:?}");
        assert_eq!(why_off(h.state(), MINIMAP), "The minimap is showing");
        h.state_mut().canvas.size = egui::vec2(400.0, 300.0);
        assert_eq!(why_off(h.state(), MINIMAP), "Widen the canvas to show the minimap");
        h.run();
        h.state_mut().canvas.minimap_open = false;
        run(h.state_mut(), MINIMAP);
        assert!(h.state().canvas.minimap_open, "{theme:?}");
        // A version's details over the canvas leave its rows nothing to act on.
        h.state_mut().services.projects = Some(std::sync::Arc::new(SavingHost::default()));
        h.state_mut().state.show_explorer(Explorer::SourceControl);
        crate::version_details::open(h.state_mut(), version_row("c1", None, (None, false), "2026-11-08T15:30:00Z"));
        assert!(!available(h.state(), ZOOM_IN) && !available(h.state(), WHOLE), "{theme:?}: covered");
        assert_eq!(why_off(h.state(), ZOOM_OUT), "Show a diagram first");
        h.state_mut().versions.details = None;
        h.state_mut().state.show_explorer(Explorer::Models);
        assert!(!available(h.state(), MODELS) && available(h.state(), DIAGRAMS), "{theme:?}");
        assert_eq!(why_off(h.state(), MODELS), "Already showing");
        run(h.state_mut(), DIAGRAMS);
        assert_eq!(h.state().state.explorer, Explorer::Diagrams, "{theme:?}");
        h.state_mut().state.explorer_collapsed = true;
        run(h.state_mut(), DIAGRAMS);
        assert!(!h.state().state.explorer_collapsed, "{theme:?}: a folded explorer opens again");
        h.state_mut().state.show_panel(Panel::Properties);
        run(h.state_mut(), CHAT);
        h.run();
        assert_eq!(h.state().state.panel, Panel::Agent, "{theme:?}");
        h.state_mut().canvas.active = None;
        assert!(!available(h.state(), ZOOM_IN) && !available(h.state(), MINIMAP), "{theme:?}");
        assert_eq!(why_off(h.state(), WHOLE), "Show a diagram first");
        h.state_mut().go_home();
        h.run();
        let on: Vec<&str> = VIEW_MENU.concat().into_iter().filter(|id| available(h.state(), id)).collect();
        assert_eq!(on, [FIND], "{theme:?}: on Home");
        assert_eq!(why_off(h.state(), DIAGRAMS), "Open or go back to a project first");
    });
}

/// A bar with no room for the search's field keeps Find… off and says why, and closes a list that
/// was open, its keys with it; with room again, ⌘K opens it.
#[test]
fn find_waits_for_room_for_the_search() {
    in_both_appearances(|theme| {
        let mut h = view_harness(theme);
        h.set_size(egui::vec2(640.0, 900.0));
        h.run();
        assert!(!has_id(&h, "workspace.header.search"), "{theme:?}: no room for the field");
        assert!(!available(h.state(), FIND), "{theme:?}");
        assert_eq!(why_off(h.state(), FIND), "Widen the window to search");
        key(&mut h, Key::K, Modifiers::COMMAND);
        assert!(!h.state().home_search.open, "{theme:?}: ⌘K waits too");
        h.set_size(egui::vec2(1600.0, 900.0));
        h.run();
        key(&mut h, Key::K, Modifiers::COMMAND);
        assert!(h.state().home_search.open, "{theme:?}: with room again");
        h.set_size(egui::vec2(640.0, 900.0));
        h.run();
        assert!(!h.state().home_search.open, "{theme:?}: the list closes with the field");
    });
}

/// On a canvas too narrow for it, a folded minimap can't be shown: the row is off, says why, and
/// leaves it folded.
#[test]
fn show_minimap_waits_for_a_wide_enough_canvas() {
    in_both_appearances(|theme| {
        let mut h = view_harness(theme);
        h.state_mut().canvas.minimap_open = false;
        h.set_size(egui::vec2(1000.0, 900.0));
        h.run();
        assert!(!available(h.state(), MINIMAP), "{theme:?}");
        assert_eq!(why_off(h.state(), MINIMAP), "Widen the canvas to show the minimap");
        run(h.state_mut(), MINIMAP);
        assert!(!h.state().canvas.minimap_open, "{theme:?}: still folded");
        h.set_size(egui::vec2(1600.0, 900.0));
        h.run();
        assert!(available(h.state(), MINIMAP), "{theme:?}: wide enough again");
    });
}
