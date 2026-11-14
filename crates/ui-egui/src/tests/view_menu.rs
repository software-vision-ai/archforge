//! The View menu: its rows in the design's order, Find… and its keys, the zooms and Whole diagram
//! as the minimap's controls do them, and Minimap and the rows of the explorers and panels, which
//! show and hide theirs and are ticked while it shows.

use egui::{Key, Modifiers};

use super::*;
use crate::actions::ticked;
use crate::actions::{FIND, available, run};
use crate::menu_bar::why_off;
use crate::state::{Explorer, Panel};
use crate::view_menu::{CHAT, DIAGRAMS, MINIMAP, MODELS, PROPERTIES, VIEW_MENU, WHOLE, ZOOM_IN, ZOOM_OUT};

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

/// Minimap shows and hides the minimap, and each explorer's or panel's row shows it, or folds it
/// away while it shows, as its rail icon does; another's row changes to that one. Each is ticked
/// only while its own shows. Without a diagram, or under a version's details, the canvas's rows are
/// off, and on Home only Find… is on and nothing is ticked.
#[test]
fn the_rows_show_and_hide_the_minimap_explorers_and_panels() {
    in_both_appearances(|theme| {
        let mut h = view_harness(theme);
        assert_eq!((available(h.state(), MINIMAP), ticked(h.state(), MINIMAP)), (true, Some(true)), "{theme:?}: showing");
        run(h.state_mut(), MINIMAP);
        assert_eq!((h.state().canvas.minimap_open, ticked(h.state(), MINIMAP)), (false, Some(false)), "{theme:?}: hidden");
        run(h.state_mut(), MINIMAP);
        assert_eq!((h.state().canvas.minimap_open, ticked(h.state(), MINIMAP)), (true, Some(true)), "{theme:?}: shown again");
        // A version's details over the canvas leave its rows nothing to act on.
        h.state_mut().services.projects = Some(std::sync::Arc::new(SavingHost::default()));
        h.state_mut().state.show_explorer(Explorer::SourceControl);
        crate::version_details::open(h.state_mut(), version_row("c1", None, (None, false), "2026-11-08T15:30:00Z"));
        assert!(!available(h.state(), ZOOM_IN) && !available(h.state(), WHOLE) && !available(h.state(), MINIMAP), "{theme:?}: covered");
        assert_eq!((why_off(h.state(), ZOOM_OUT), ticked(h.state(), MINIMAP)), ("Show a diagram first".to_string(), Some(false)));
        h.state_mut().versions.details = None;
        h.state_mut().state.show_explorer(Explorer::Models);
        let explorers = |h: &Harness<'static, Shell>| [DIAGRAMS, MODELS].map(|id| ticked(h.state(), id));
        assert_eq!(explorers(&h), [Some(false), Some(true)], "{theme:?}");
        assert!(available(h.state(), MODELS) && available(h.state(), DIAGRAMS), "{theme:?}: a row showing can hide");
        run(h.state_mut(), DIAGRAMS);
        assert_eq!((h.state().state.explorer, explorers(&h)), (Explorer::Diagrams, [Some(true), Some(false)]), "{theme:?}");
        run(h.state_mut(), DIAGRAMS);
        assert_eq!((h.state().state.explorer_collapsed, explorers(&h)), (true, [Some(false); 2]), "{theme:?}: its own row folds it");
        run(h.state_mut(), DIAGRAMS);
        assert_eq!((h.state().state.explorer_collapsed, explorers(&h)), (false, [Some(true), Some(false)]), "{theme:?}: and opens it again");
        h.state_mut().state.show_panel(Panel::Properties);
        assert_eq!([CHAT, PROPERTIES].map(|id| ticked(h.state(), id)), [Some(false), Some(true)], "{theme:?}");
        run(h.state_mut(), CHAT);
        h.run();
        assert_eq!((h.state().state.panel, ticked(h.state(), CHAT)), (Panel::Agent, Some(true)), "{theme:?}");
        assert!(has_id(&h, "workspace.panels.agent"), "{theme:?}");
        run(h.state_mut(), CHAT);
        assert_eq!((h.state().state.panel_collapsed, ticked(h.state(), CHAT)), (true, Some(false)), "{theme:?}: folded");
        assert_eq!([FIND, ZOOM_IN, WHOLE].map(|id| ticked(h.state(), id)), [None; 3], "{theme:?}: rows that only act have no tick");
        h.state_mut().canvas.active = None;
        assert!(!available(h.state(), ZOOM_IN) && !available(h.state(), MINIMAP), "{theme:?}");
        assert_eq!(why_off(h.state(), WHOLE), "Show a diagram first");
        h.state_mut().go_home();
        h.run();
        let on: Vec<&str> = VIEW_MENU.concat().into_iter().filter(|id| available(h.state(), id)).collect();
        assert_eq!(on, [FIND], "{theme:?}: on Home");
        assert!(VIEW_MENU.concat().into_iter().all(|id| ticked(h.state(), id) != Some(true)), "{theme:?}: nothing shows on Home");
        assert_eq!(why_off(h.state(), DIAGRAMS), "Open or go back to a project first");
    });
}

/// A dialog over the window turns the rows off but leaves their ticks: the explorer, the panel and
/// the minimap still show behind it. Closed again, the rows are on and still ticked.
#[test]
fn a_dialog_turns_the_rows_off_and_leaves_their_ticks() {
    in_both_appearances(|theme| {
        let mut h = view_harness(theme);
        h.state_mut().state.show_explorer(Explorer::Models);
        h.state_mut().state.show_panel(Panel::Properties);
        h.run();
        let rows = [MINIMAP, MODELS, PROPERTIES];
        let state = |h: &Harness<'static, Shell>| rows.map(|id| (available(h.state(), id), ticked(h.state(), id)));
        assert_eq!(state(&h), [(true, Some(true)); 3], "{theme:?}: with no dialog");
        crate::support::open(h.state_mut());
        h.run();
        assert!(has_id(&h, "workspace.minimap") && has_id(&h, "workspace.panels.properties"), "{theme:?}: still drawn behind About");
        assert_eq!(state(&h), [(false, Some(true)); 3], "{theme:?}: off, and ticked");
        h.state_mut().about = None;
        h.run();
        assert_eq!(state(&h), [(true, Some(true)); 3], "{theme:?}: on again");
    });
}

/// In the bar's menu a row that shows and hides is a check box to a screen reader, checked while
/// its thing shows, and wears a tick only then; choosing it hides the thing and unticks the row. A
/// row that only acts stays a plain menu item with no checked state.
#[test]
fn a_row_with_a_tick_is_a_check_box_checked_while_its_thing_shows() {
    use egui::accesskit::{Role, Toggled};
    use egui_kittest::kittest::{NodeT, Queryable};
    in_both_appearances(|theme| {
        let mut h = view_harness(theme);
        h.state_mut().state.show_panel(Panel::Properties);
        let accent = crate::ux::palette(theme.is_dark()).accent;
        // The row alone: a panel's name is its rail icon's and its header's too.
        let said = |h: &Harness<'static, Shell>, name: &str| {
            let rows: Vec<_> = h.get_all_by_label(name).map(|n| n.accesskit_node().data().clone()).filter(|d| d.role() != Role::Unknown).collect();
            assert_eq!(rows.len(), 1, "{name}");
            (rows[0].role(), rows[0].toggled())
        };
        let drawn = |h: &Harness<'static, Shell>, id: &str| {
            let row = rect_of(h, &format!("strip.menu.row.{id}"));
            icon_boxes(h, accent).iter().filter(|icon| row.contains_rect(**icon)).count()
        };
        click_menu(&mut h, "strip.menu.view");
        assert_eq!(said(&h, "Minimap"), (Role::MenuItemCheckBox, Some(Toggled::True)), "{theme:?}");
        assert_eq!(said(&h, "Chat"), (Role::MenuItemCheckBox, Some(Toggled::False)), "{theme:?}: Properties shows");
        assert_eq!(said(&h, "Zoom in"), (Role::MenuItem, None), "{theme:?}: a row that only acts");
        assert_eq!([MINIMAP, CHAT, ZOOM_IN].map(|id| drawn(&h, id)), [1, 0, 0], "{theme:?}: a tick on the row that shows, alone");
        click_menu(&mut h, "strip.menu.row.view.minimap");
        h.run();
        assert!(!h.state().canvas.minimap_open, "{theme:?}: chosen while ticked, it hides");
        click_menu(&mut h, "strip.menu.view");
        assert_eq!((said(&h, "Minimap"), drawn(&h, MINIMAP)), ((Role::MenuItemCheckBox, Some(Toggled::False)), 0), "{theme:?}");
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

/// On a canvas too narrow for it, a folded minimap can't be shown: the row is off and unticked,
/// says why, and leaves it folded.
#[test]
fn minimap_waits_for_a_wide_enough_canvas() {
    in_both_appearances(|theme| {
        let mut h = view_harness(theme);
        h.state_mut().canvas.minimap_open = false;
        h.set_size(egui::vec2(1000.0, 900.0));
        h.run();
        assert!(!available(h.state(), MINIMAP), "{theme:?}");
        assert_eq!(why_off(h.state(), MINIMAP), "Make the canvas larger to show the minimap");
        run(h.state_mut(), MINIMAP);
        assert!(!h.state().canvas.minimap_open, "{theme:?}: still folded");
        h.set_size(egui::vec2(1600.0, 900.0));
        h.run();
        assert!(available(h.state(), MINIMAP), "{theme:?}: wide enough again");
    });
}

/// On a canvas wide enough but too short for the minimap's box, Minimap is off and says why, and
/// an open minimap gives way to its button rather than to nothing: not shown, so not ticked.
#[test]
fn minimap_waits_for_a_tall_enough_canvas() {
    in_both_appearances(|theme| {
        let mut h = view_harness(theme);
        h.state_mut().canvas.minimap_open = false;
        h.set_size(egui::vec2(2000.0, 330.0));
        h.run();
        let size = h.state().canvas.size;
        assert!(size.x > 1200.0 && size.y < 170.0, "{theme:?}: wide but short: {size:?}");
        assert!(!available(h.state(), MINIMAP), "{theme:?}");
        assert_eq!(why_off(h.state(), MINIMAP), "Make the canvas larger to show the minimap");
        run(h.state_mut(), MINIMAP);
        assert!(!h.state().canvas.minimap_open, "{theme:?}: still folded");
        h.state_mut().canvas.minimap_open = true;
        h.run();
        assert!(!has_id(&h, "workspace.minimap") && has_id(&h, "workspace.minimap.toggle"), "{theme:?}: its button stands in");
        assert_eq!((available(h.state(), MINIMAP), ticked(h.state(), MINIMAP)), (false, Some(false)), "{theme:?}: open, with no room to show");
    });
}
