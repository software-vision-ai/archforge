//! The lists of the Versions panel and the keyboard (AR-474): Page Up, Page Down, Home and End move
//! the keyboard through a list while one of its rows has it, and a row, or another control of a
//! list, that Tab comes to is brought whole into view; a text field keeps its own keys.

use egui::{Key, Modifiers};

use super::*;

/// Where a list shows its rows: from `top`, where its first row is drawn unscrolled, to the top of
/// the section under it less the 14 points between them, or to the column's foot.
fn within(h: &Harness<'static, Shell>, top: f32, under: Option<&str>) -> (f32, f32) {
    (top, under.map_or_else(|| rect_of(h, "workspace.explorer.resize").bottom(), |id| rect_of(h, id).top() - 14.0))
}

/// Whether the control `id` has the keyboard, listed or not.
fn holds(h: &Harness<'static, Shell>, id: &str) -> bool {
    h.ctx.memory(|m| m.has_focus(crate::widgets::egui_id(id)))
}

/// Whether the control `id` is listed and drawn whole between `bounds`.
fn whole(h: &Harness<'static, Shell>, id: &str, (top, bottom): (f32, f32)) -> bool {
    has_id(h, id) && rect_of(h, id).top() >= top && rect_of(h, id).bottom() <= bottom
}

/// Puts the keyboard on the control `id`, as a screen reader may.
fn focus(h: &mut Harness<'static, Shell>, id: &str) {
    h.ctx.memory_mut(|m| m.request_focus(crate::widgets::egui_id(id)));
    h.run();
}

/// The panel on `main` with twenty more branches read, two versions listed.
fn many_branches(theme: ThemeKind) -> Harness<'static, Shell> {
    let (mut h, host) = versions_panel(theme);
    *host.branch_now.lock().unwrap() = Some("main".to_string());
    let branch =
        |name: String, current| crate::BranchEntry { shown: name.clone(), name, current, unsaved: false, latest: None, latest_version: None };
    let listed = std::iter::once(branch("main".into(), true)).chain((0..20).map(|i| branch(format!("b{i:02}"), false))).collect();
    *host.branch_list.lock().unwrap() = Some(Ok(listed));
    versions_answer(&mut h, &host, Ok(two_versions()));
    h
}

/// The panel with thirty working changes, the filter over them.
fn many_changes(theme: ThemeKind) -> Harness<'static, Shell> {
    let (mut h, _host) = changes_panel(theme);
    (0..30).for_each(|i| add_schema(&mut h, &format!("s{i:02}")));
    compared(&mut h);
    assert!(has_id(&h, "workspace.versions.filter"), "{:?}", ids(&h));
    h
}

/// The ids of the thirty working changes' rows, in the list's order: all of the model.
fn change_rows(h: &Harness<'static, Shell>) -> Vec<String> {
    let rows = h.state().versions.changes.iter().map(|d| format!("workspace.versions.change.{}.{}", d.id, d.class.0)).collect::<Vec<_>>();
    assert_eq!(rows.len(), 30, "{rows:?}");
    rows
}

/// Presses `key` and asserts that the keyboard went to `to`, drawn whole between `bounds`.
fn press_to(h: &mut Harness<'static, Shell>, key_: Key, to: &str, bounds: (f32, f32)) {
    key(h, key_, Modifiers::NONE);
    assert!(holds(h, to), "{key_:?} to {to}");
    assert!(whole(h, to, bounds), "{key_:?}: {to} at {:?} in {bounds:?}", rect_of(h, to));
}

/// In each list, Page Down and Page Up move the keyboard by what shows, End and Home to the last
/// row and the first, each brought whole into view; not with the control key, as Windows and Linux
/// send it, nor in the frame of the click that opens the menu bar, nor while a row's menu is open,
/// and not from the filter field, which keeps its keys.
#[test]
fn the_page_keys_move_the_keyboard_through_each_list() {
    let row = |n: usize| format!("workspace.versions.row.v{n}");
    in_both_appearances(|theme| {
        let (mut h, _host) = long_history(theme);
        let bounds = within(&h, rect_of(&h, &row(0)).top(), None);
        focus(&mut h, &row(0));
        let last_whole = (0..50).take_while(|n| whole(&h, &row(*n), bounds)).last().unwrap();
        assert!(!has_id(&h, &row(last_whole + 2)), "{theme:?}: the history goes on below the fold");
        press_to(&mut h, Key::PageDown, &row(last_whole), bounds);
        press_to(&mut h, Key::PageDown, &row(2 * last_whole), bounds);
        press_to(&mut h, Key::End, &row(49), bounds);
        let first_whole = (0..50).find(|n| whole(&h, &row(*n), bounds)).unwrap();
        assert!(first_whole > 30, "{theme:?}: End scrolled the list to its end");
        press_to(&mut h, Key::PageUp, &row(first_whole), bounds);
        press_to(&mut h, Key::PageUp, &row(first_whole - last_whole), bounds);
        press_to(&mut h, Key::Home, &row(0), bounds);
        assert_eq!(rect_of(&h, &row(0)).top(), bounds.0, "{theme:?}: the list is back at its top");
        key(&mut h, Key::PageDown, Modifiers::CTRL.plus(Modifiers::COMMAND));
        assert!(holds(&h, &row(0)), "{theme:?}: not with the control key");
        // Nor in the frame of the click that opens the in-window menu bar (AR-518).
        h.state_mut().in_window_menu = true;
        h.run();
        open_file_with(&mut h, Key::End, true);
        assert!(!holds(&h, &row(49)) && whole(&h, &row(0), bounds), "{theme:?}: with the click that opens the menu bar");

        let mut h = many_changes(theme);
        let rows = change_rows(&h);
        let bounds = within(&h, rect_of(&h, &rows[0]).top() - 24.0, Some("workspace.versions.history"));
        focus(&mut h, &rows[0]);
        press_to(&mut h, Key::End, &rows[29], bounds);
        press_to(&mut h, Key::Home, &rows[0], bounds);
        // The filter keeps Home and End: what is typed after them lands at its ends.
        focus(&mut h, "workspace.versions.filter");
        h.event(egui::Event::Text("s1".into()));
        for (key_, typed, reads) in [(Key::Home, "x", "xs1"), (Key::End, "y", "xs1y"), (Key::PageUp, "", "xs1y")] {
            key(&mut h, key_, Modifiers::NONE);
            h.event(egui::Event::Text(typed.into()));
            h.run();
            assert_eq!(h.state().versions.filter, reads, "{theme:?}: {key_:?} in the filter");
            assert!(holds(&h, "workspace.versions.filter"), "{theme:?}: {key_:?} left the keyboard in the filter");
        }

        let mut h = many_branches(theme);
        let branch = |name: &str| format!("workspace.versions.branch.row.{name}");
        let bounds = within(&h, rect_of(&h, &branch("main")).top(), Some("workspace.versions.working"));
        focus(&mut h, &branch("b00"));
        press_to(&mut h, Key::End, &branch("b19"), bounds);
        press_to(&mut h, Key::Home, &branch("main"), bounds);
        click_id(&mut h, "workspace.versions.branch.more.b00");
        focus(&mut h, &branch("b00"));
        key(&mut h, Key::End, Modifiers::NONE);
        assert!(h.state().panels.menu.is_some() && holds(&h, &branch("b00")), "{theme:?}: a row's open menu keeps the keys");
    });
}

/// In a window so short that Working changes shows less than a page of a group's heading and two
/// rows, Page Down from the group's first row and Page Up from the row under it each move a row.
#[test]
fn a_page_key_moves_at_least_a_row_in_a_short_list() {
    in_both_appearances(|theme| {
        let (mut h, _host) = changes_panel(theme);
        (0..5).for_each(|i| add_schema(&mut h, &format!("s{i}")));
        compared(&mut h);
        let rows = h.state().versions.changes.iter().map(|d| format!("workspace.versions.change.{}.{}", d.id, d.class.0)).collect::<Vec<_>>();
        // The list's page is its height less 8 and a row: under the 70 points to the second row.
        let short = |h: &Harness<'static, Shell>| {
            let (top, bottom) = within(h, rect_of(h, &rows[0]).top() - 24.0, Some("workspace.versions.history"));
            bottom - top - 8.0 - 44.0 < 70.0
        };
        for height in (400..900).step_by(4) {
            h.set_size(egui::vec2(1600.0, height as f32));
            h.run_steps(2);
            if has_id(&h, &rows[0]) && short(&h) {
                break;
            }
        }
        assert!(has_id(&h, &rows[0]) && short(&h), "{theme:?}: a short list shows");
        focus(&mut h, &rows[0]);
        key(&mut h, Key::PageDown, Modifiers::NONE);
        assert!(holds(&h, &rows[1]), "{theme:?}: Page Down from the group's first row");
        key(&mut h, Key::PageUp, Modifiers::NONE);
        assert!(holds(&h, &rows[0]), "{theme:?}: Page Up from the row under it");
    });
}

/// Tab and Shift+Tab land on every part of a list in turn, none skipped, and bring each whole into
/// view: the versions and Show more under them, the working changes, and the branches and their
/// menu buttons.
#[test]
fn tab_brings_each_part_of_a_list_whole_into_view() {
    // From the first of `order` Tab, or Shift+Tab from its last when `back`, lands on each of the
    // rest in turn, drawn whole between `bounds`.
    let walk = |h: &mut Harness<'static, Shell>, order: &[String], back: bool, bounds: (f32, f32)| {
        let order: Vec<&String> = if back { order.iter().rev().collect() } else { order.iter().collect() };
        focus(h, order[0]);
        let mut landed = vec![order[0].clone()];
        while landed.len() < order.len() {
            key(h, Key::Tab, if back { Modifiers::SHIFT } else { Modifiers::NONE });
            let on = order.iter().find(|id| holds(h, id));
            landed.push(on.map_or_else(|| format!("{:?}", h.ctx.memory(|m| m.focused())), |id| (*id).clone()));
            if let Some(on) = on {
                assert!(whole(h, on, bounds), "{on} in view, after {:?}", &landed[..landed.len() - 1]);
            }
        }
        assert_eq!(landed.iter().collect::<Vec<_>>(), order, "every part in turn");
    };
    in_both_appearances(|theme| {
        let (mut h, _host) = long_history(theme);
        let bounds = within(&h, rect_of(&h, "workspace.versions.row.v0").top(), None);
        let history = (0..50).map(|n| format!("workspace.versions.row.v{n}")).chain(["workspace.versions.more".to_string()]).collect::<Vec<_>>();
        walk(&mut h, &history, false, bounds);
        walk(&mut h, &history, true, bounds);

        let mut h = many_changes(theme);
        let rows = change_rows(&h);
        let bounds = within(&h, rect_of(&h, &rows[0]).top() - 24.0, Some("workspace.versions.history"));
        walk(&mut h, &rows, false, bounds);
        walk(&mut h, &rows, true, bounds);

        let mut h = many_branches(theme);
        let main = "workspace.versions.branch.row.main";
        let bounds = within(&h, rect_of(&h, main).top(), Some("workspace.versions.working"));
        let names = (0..20).map(|i| format!("b{i:02}"));
        let rows = names.flat_map(|n| ["row", "more"].map(|part| format!("workspace.versions.branch.{part}.{n}")));
        let branches = std::iter::once(main.to_string()).chain(rows).collect::<Vec<_>>();
        walk(&mut h, &branches, false, bounds);
        walk(&mut h, &branches, true, bounds);
    });
}

/// The keyboard moved to a row cut by the list's foot, as a screen reader moves it, brings it into
/// view, in a frame the move asks for; the wheel then moves the list away from it, and a click on a
/// row cut by the list's top opens it and leaves the list where it is.
#[test]
fn a_list_follows_the_keyboard_and_asks_for_its_frame() {
    in_both_appearances(|theme| {
        let mut h = many_changes(theme);
        let rows = change_rows(&h);
        let bounds = within(&h, rect_of(&h, &rows[0]).top() - 24.0, Some("workspace.versions.history"));
        let cut = rows.iter().find(|id| has_id(&h, id) && !whole(&h, id, bounds)).unwrap().clone();
        focus(&mut h, &rows[0]);
        h.ctx.memory_mut(|m| m.request_focus(crate::widgets::egui_id(&cut)));
        h.step();
        // The frame the move came in asked for the next at once, and the list followed in it.
        h.step();
        let asked = h.ctx.repaint_causes().iter().any(|c| c.file.ends_with("versions.rs"));
        assert!(asked && holds(&h, &cut) && whole(&h, &cut, bounds), "{theme:?}: {:?}", h.ctx.repaint_causes());
        let over = rect_of(&h, &rows[0]).center();
        h.event(egui::Event::PointerMoved(over));
        // A notch a frame, as a hand turns it, while the row keeps the keyboard.
        for _ in 0..60 {
            let (unit, delta, phase) = (egui::MouseWheelUnit::Point, egui::vec2(0.0, -30.0), egui::TouchPhase::Move);
            h.event(egui::Event::MouseWheel { unit, delta, modifiers: Modifiers::NONE, phase, source: egui::MouseWheelSource::Wheel });
            h.step();
        }
        h.run();
        assert!(!has_id(&h, &cut) && has_id(&h, &rows[29]), "{theme:?}: the wheel took the list to its end");
        let first = rows.iter().find(|id| has_id(&h, id)).unwrap().clone();
        let at = rect_of(&h, &first);
        assert!(at.top() < bounds.0, "{theme:?}: the first row shown is cut by the list's top");
        click(&mut h, egui::pos2(at.center().x, at.bottom() - 4.0));
        assert!(h.state().versions.element.is_some() && rect_of(&h, &first) == at, "{theme:?}: a click opens it and moves nothing");
    });
}
