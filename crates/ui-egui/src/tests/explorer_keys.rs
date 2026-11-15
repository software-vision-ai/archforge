//! The explorer trees and the keyboard (AR-478). A row of Diagrams or Models, and a group's `+`, a
//! row's arrow and its drag handle, take the keyboard: Tab and Shift+Tab land on each in turn and
//! the tree brings it whole into view, and Page Up, Page Down, Home and End move the keyboard
//! through the rows. While Models holds the keys itself, the same keys move its selection, and the
//! tree follows it.

use egui::{Key, Modifiers};

use super::*;

/// Whether the control `id` has the keyboard, listed or not.
fn holds(h: &Harness<'static, Shell>, id: &str) -> bool {
    h.ctx.memory(|m| m.has_focus(crate::widgets::egui_id(id)))
}

/// Where a tree shows the rows it brings into view: from `top`, where its first row is drawn
/// unscrolled, to the column's foot less 17 points, the 8 the tree keeps clear under its last row's
/// pitch of 34 and the 9 of that pitch under the row's 25 points below its line.
fn within(h: &Harness<'static, Shell>, top: f32) -> (f32, f32) {
    (top, rect_of(h, "workspace.explorer.resize").bottom() - 17.0)
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

/// The last of `rows`, from the first, drawn whole between `bounds`.
fn last_whole(h: &Harness<'static, Shell>, rows: &[String], bounds: (f32, f32)) -> usize {
    (0..rows.len()).take_while(|n| whole(h, &rows[*n], bounds)).last().unwrap()
}

/// From the first of `order` Tab, and then Shift+Tab from its last, lands on each of the rest in
/// turn, none skipped; each is drawn whole between `bounds`, or the row `shown` names for it is.
fn walk(h: &mut Harness<'static, Shell>, order: &[String], bounds: (f32, f32), shown: impl Fn(&str) -> Option<String>) {
    assert!(!has_id(h, order.last().unwrap()), "the tree goes on below the fold");
    for back in [false, true] {
        let order: Vec<&String> = if back { order.iter().rev().collect() } else { order.iter().collect() };
        focus(h, order[0]);
        let mut landed = vec![order[0].clone()];
        while landed.len() < order.len() {
            key(h, Key::Tab, if back { Modifiers::SHIFT } else { Modifiers::NONE });
            let on = order.iter().find(|id| holds(h, id));
            landed.push(on.map_or_else(|| format!("{:?}", h.ctx.memory(|m| m.focused())), |id| (*id).clone()));
            if let Some(row) = on.and_then(|id| shown(id)) {
                assert!(whole(h, &row, bounds), "{row} in view, after {:?}", &landed[..landed.len() - 1]);
            }
        }
        assert_eq!(landed.iter().collect::<Vec<_>>(), order, "every control in turn, back {back}");
    }
}

/// As the wheel leaves the row `rows[k]`, whose control `from` has the keyboard, cut by the tree's
/// top (`up`) or its foot, Shift+Tab or Tab goes on to `to` past that edge, its row brought whole
/// into view: the row past an edge is laid out.
fn past_the_edge(h: &mut Harness<'static, Shell>, rows: &[String], (k, from): (usize, &str), (to, up): (&str, bool), bounds: (f32, f32)) {
    focus(h, from);
    let top = rect_of(h, &rows[k]).top();
    let by = if up { top - (bounds.0 - 20.0) } else { top - (bounds.1 + 7.0) };
    let s = h.state_mut();
    let tree = if s.state.explorer == crate::state::Explorer::Diagrams { &mut s.diagrams.scroll } else { &mut s.explorer.scroll };
    tree.offset += by;
    h.run();
    assert!(holds(h, from) && has_id(h, &rows[k]) && !whole(h, &rows[k], bounds), "{from} cut by the edge, keeping the keyboard");
    key(h, Key::Tab, if up { Modifiers::SHIFT } else { Modifiers::NONE });
    let next = if up { k - 1 } else { k + 1 };
    assert!(holds(h, to) && whole(h, &rows[next], bounds), "on to {to}, in view");
}

/// From the first of `rows`, Page Down twice moves what `on` says moved (the keyboard, or the
/// selection) by what shows, End to the last row, Page Up twice back by what shows and Home to the
/// first, each brought whole into view between `bounds`.
fn pages(h: &mut Harness<'static, Shell>, rows: &[String], bounds: (f32, f32), on: impl Fn(&Harness<'static, Shell>, &str) -> bool) {
    let shown = last_whole(h, rows, bounds);
    let press = |h: &mut Harness<'static, Shell>, key_: Key, to: usize| {
        key(h, key_, Modifiers::NONE);
        assert!(on(h, &rows[to]) && whole(h, &rows[to], bounds), "{key_:?} to row {to}, in {bounds:?}");
    };
    press(h, Key::PageDown, shown);
    press(h, Key::PageDown, 2 * shown);
    press(h, Key::End, rows.len() - 1);
    let first = (0..rows.len()).find(|n| whole(h, &rows[*n], bounds)).unwrap();
    assert!(first > 30, "End scrolled the tree to its end");
    press(h, Key::PageUp, first);
    press(h, Key::PageUp, first - shown);
    press(h, Key::Home, 0);
}

/// Sixty diagrams, each of a schema of its own, in Diagrams at the design's window size; gives the
/// controls of the tree that take the keyboard, in its order: each row, and after a group's its `+`.
fn many_diagrams(theme: ThemeKind) -> (Harness<'static, Shell>, Vec<String>) {
    let mut h = workspace_harness(theme, egui::vec2(1600.0, 1000.0));
    for n in 0..60 {
        let schema = crate::panels::run(h.state_mut(), "er.schema.add", serde_json::json!({ "name": format!("s{n:02}") })).unwrap()["id"].clone();
        let made = serde_json::json!({ "kind": "er", "element": schema, "name": format!("d{n:02}") });
        crate::panels::run(h.state_mut(), "diagram.add", made).unwrap();
    }
    h.state_mut().state.show_explorer(crate::state::Explorer::Diagrams);
    h.run();
    let shell = h.state();
    let tree = crate::diagrams::rows(shell.session.as_ref().unwrap().doc(), &shell.diagrams, shell.kinds);
    let add = |row: &crate::diagrams::Row| match row.kind {
        crate::diagrams::Kind::Group(kind) if shell.kinds.iter().any(|k| k.kind() == kind) => Some(crate::diagrams::add_id(kind)),
        _ => None,
    };
    let controls = tree.rows.iter().flat_map(|row| std::iter::once(crate::diagrams::row_id(row)).chain(add(row))).collect();
    (h, controls)
}

/// From the filter field, Tab lands on every row of Diagrams and every group's `+` in turn, and
/// Shift+Tab from the last row back to the filter field, each brought whole into view; Tab from
/// the last row in view moves the keyboard as the rows are laid out, and that frame asks for the
/// next, which brings the row under the fold into view. Tab goes on past a row the wheel left cut by
/// an edge. The page keys move the keyboard; not with the control key, as Windows and Linux send it.
#[test]
fn diagrams_follows_the_keyboard() {
    in_both_appearances(|theme| {
        let (mut h, controls) = many_diagrams(theme);
        let bounds = within(&h, rect_of(&h, &controls[0]).top());
        let order = std::iter::once("workspace.diagrams.filter".to_string()).chain(controls.iter().cloned()).collect::<Vec<_>>();
        walk(&mut h, &order, bounds, |id| (!id.ends_with(".filter")).then(|| id.to_string()));
        assert_eq!(h.state().diagrams.scroll.offset, 0.0, "{theme:?}: Shift+Tab took the tree back to its top");
        let rows = controls.into_iter().filter(|id| !id.ends_with(".add")).collect::<Vec<_>>();
        let shown = last_whole(&h, &rows, bounds);
        focus(&mut h, &rows[shown]);
        h.key_down(Key::Tab);
        h.step();
        h.key_up(Key::Tab);
        h.step();
        let asked = h.ctx.repaint_causes().iter().any(|c| c.file.ends_with("explorer.rs"));
        h.run();
        assert!(asked && holds(&h, &rows[shown + 1]) && whole(&h, &rows[shown + 1], bounds), "{theme:?}: {:?}", h.ctx.repaint_causes());
        past_the_edge(&mut h, &rows, (30, &rows[30]), (&rows[29], true), bounds);
        past_the_edge(&mut h, &rows, (31, &rows[31]), (&rows[32], false), bounds);
        focus(&mut h, &rows[0]);
        pages(&mut h, &rows, bounds, holds);
        key(&mut h, Key::PageDown, Modifiers::CTRL.plus(Modifiers::COMMAND));
        assert!(holds(&h, &rows[0]), "{theme:?}: not with the control key");
        // In the smallest window, over the open diagram's footer, not even a row shows whole: a
        // page key still moves a row.
        h.set_size(egui::vec2(1600.0, 400.0));
        crate::canvas::open(h.state_mut(), rows[10].rsplit('.').next().and_then(archforge_mof::Uuid::parse).unwrap());
        focus(&mut h, &rows[10]);
        assert!(!whole(&h, &rows[10], (0.0, rect_of(&h, "workspace.explorer.resize").bottom() - 128.0)), "{theme:?}: no row shows whole");
        key(&mut h, Key::PageDown, Modifiers::NONE);
        assert!(holds(&h, &rows[11]), "{theme:?}: Page Down moved a row");
        key(&mut h, Key::PageUp, Modifiers::NONE);
        assert!(holds(&h, &rows[10]), "{theme:?}: Page Up moved a row");
    });
}

/// Sixty tables in a schema, open in Models at the design's window size, nothing selected; gives
/// the rows' ids in the tree's order.
fn many_tables(theme: ThemeKind) -> (Harness<'static, Shell>, Vec<String>) {
    let mut h = workspace_harness(theme, egui::vec2(1600.0, 1000.0));
    let schema = crate::panels::run(h.state_mut(), "er.schema.add", serde_json::json!({ "name": "Shop" })).unwrap()["id"].clone();
    for n in 0..60 {
        crate::panels::run(h.state_mut(), "er.entity.add", serde_json::json!({ "schema": schema, "name": format!("t{n:02}") })).unwrap();
    }
    h.state_mut().explorer.expanded.insert(schema.as_str().and_then(archforge_mof::Uuid::parse).unwrap());
    // The last table added is selected, and the tree followed it: nothing selected, at the top.
    crate::panels::run(h.state_mut(), "view.select", serde_json::json!({ "ids": [] })).unwrap();
    h.state_mut().explorer.scroll = Default::default();
    h.state_mut().state.show_explorer(crate::state::Explorer::Models);
    h.run();
    let shell = h.state();
    let tree = crate::explorer::rows(shell.notation_uis, shell.session.as_ref().unwrap().doc(), &shell.explorer);
    let rows = tree.iter().map(|row| format!("workspace.explorer.row.{}", row.id)).collect::<Vec<_>>();
    assert_eq!(rows.len(), 61, "{theme:?}: {rows:?}");
    (h, rows)
}

/// Whether the row `id` of Models is the selected element's.
fn selected(h: &Harness<'static, Shell>, id: &str) -> bool {
    let s = h.state().session.as_ref().unwrap();
    s.view().selection.first().and_then(|o| s.doc().uuid(*o).ok()).is_some_and(|u| id == format!("workspace.explorer.row.{u}"))
}

/// From the filter field, Tab lands on the tree, then on every row of Models with its arrow or drag
/// handle in turn, and Shift+Tab from the last back to the filter field, each row brought whole into
/// view, past a row the wheel left cut by an edge too; the page keys move the keyboard. While the tree holds the keys, after a click on a row,
/// they move the selection instead, and the arrows still move it a row; not with the control key.
#[test]
fn models_follows_the_keyboard() {
    in_both_appearances(|theme| {
        let (mut h, rows) = many_tables(theme);
        let bounds = within(&h, rect_of(&h, &rows[0]).top());
        let head = ["workspace.explorer.filter".to_string(), "workspace.explorer.tree".into(), rows[0].clone(), format!("{}.fold", rows[0])];
        let order = head.into_iter().chain(rows[1..].iter().flat_map(|t| [format!("{t}.drag"), t.clone()])).collect::<Vec<_>>();
        walk(&mut h, &order, bounds, |id| {
            let row = id.trim_end_matches(".fold").trim_end_matches(".drag");
            row.starts_with("workspace.explorer.row.").then(|| row.to_string())
        });
        assert_eq!(h.state().explorer.scroll.offset, 0.0, "{theme:?}: Shift+Tab took the tree back to its top");
        past_the_edge(&mut h, &rows, (30, &format!("{}.drag", rows[30])), (&rows[29], true), bounds);
        past_the_edge(&mut h, &rows, (31, &rows[31]), (&format!("{}.drag", rows[32]), false), bounds);
        focus(&mut h, &rows[0]);
        pages(&mut h, &rows, bounds, holds);
        h.ctx.memory_mut(|m| m.surrender_focus(crate::widgets::egui_id(&rows[0])));
        let at = rect_of(&h, &rows[0]).center();
        click(&mut h, at);
        assert!(h.state().explorer.focused && selected(&h, &rows[0]), "{theme:?}: the tree has the keys");
        pages(&mut h, &rows, bounds, selected);
        key(&mut h, Key::End, Modifiers::CTRL.plus(Modifiers::COMMAND));
        key(&mut h, Key::ArrowDown, Modifiers::NONE);
        assert!(selected(&h, &rows[1]) && h.state().explorer.scroll.offset == 0.0, "{theme:?}: not with the control key");
    });
}
