use serde_json::json;

use super::*;

/// A sample project with a conversation, viewing its only version, its diagram open.
fn viewing() -> (Harness<'static, Shell>, (crate::chat_fixture::Rig, (String, archforge_mof::Uuid)), std::sync::Arc<SavingHost>) {
    let (mut h, rig, host, _) = chat_and_a_version(ThemeKind::Light);
    let store = h.state().session.as_ref().unwrap().doc();
    let diagram = store.alive().find(|o| store.class_of(*o).ok() == Some(archforge_diagram::DIAGRAM)).unwrap();
    let id = store.uuid(diagram).unwrap().to_string();
    let made = crate::panels::run(h.state_mut(), "diagram.note", json!({ "diagram": id, "x": 900, "y": 40, "text": "A note" })).unwrap();
    let note = made["id"].as_str().unwrap().to_string();
    let root = crate::chat_fixture::element(h.state(), archforge_uml::CLASS, "Customer").unwrap().to_string();
    let skill = json!({ "subject": root, "name": "Naming", "body": "text" });
    let skill = crate::panels::run(h.state_mut(), "skill.add", skill).unwrap()["id"].as_str().and_then(archforge_mof::Uuid::parse).unwrap();
    let earlier = rows_now(&h);
    start_viewing(&mut h, "c1");
    answer_read(&mut h, &host, Ok(earlier));
    assert_eq!(h.state().viewing(), Some("c1"));
    let store = h.state().session.as_ref().unwrap().doc();
    let diagram = store.alive().find(|o| store.class_of(*o).ok() == Some(archforge_diagram::DIAGRAM)).unwrap();
    let id = store.uuid(diagram).unwrap();
    crate::canvas::open(h.state_mut(), id);
    h.state_mut().state.panel = crate::state::Panel::Properties;
    h.run_steps(3);
    (h, (rig, (note, skill)), host)
}

/// The first control whose id starts with `prefix`, its centre.
fn centre_of(h: &Harness<'static, Shell>, prefix: &str) -> egui::Pos2 {
    let id = ids(h).into_iter().find(|i| i.starts_with(prefix)).unwrap_or_else(|| panic!("no {prefix}"));
    rect_of(h, &id).center()
}

/// The small squares of a node's corner handles, drawn in the accent.
fn corner_squares(h: &Harness<'static, Shell>) -> usize {
    let accent = crate::ux::palette(h.state().state.theme == ThemeKind::Dark).accent;
    rect_shapes(h).iter().filter(|(r, fill, stroke)| *stroke == accent && *fill != accent && r.size() == egui::vec2(6.0, 6.0)).count()
}

/// A selected node's corners, which resize, are not drawn while a version shows; its ring is.
#[test]
fn a_selected_node_has_no_corners_while_a_version_shows() {
    in_both(viewing, |mut h, _rig, _host| {
        let node = centre_of(&h, "workspace.canvas.node.");
        let (rect, accent) = (
            rect_of(&h, ids(&h).iter().find(|i| i.starts_with("workspace.canvas.node.")).unwrap()),
            crate::ux::palette(h.state().state.theme == ThemeKind::Dark).accent,
        );
        click(&mut h, node);
        h.run_steps(2);
        assert!(rect_shapes(&h).iter().any(|(r, _, stroke)| *stroke == accent && r.contains_rect(rect.shrink(1.0))), "the ring shows it is selected");
        assert_eq!(corner_squares(&h), 0, "no corners while a version shows");
        h.state_mut().back_to_current_work();
        h.run_steps(3);
        let node = centre_of(&h, "workspace.canvas.node.");
        click(&mut h, node);
        h.run_steps(2);
        assert_eq!(corner_squares(&h), 4, "current work has its corners");
    });
}

/// That nothing changed, nothing was refused, and nothing that edits has opened.
#[track_caller]
fn still_only_looking(h: &Harness<'static, Shell>, rows: &archforge_mof::rows::Rows, what: &str) {
    let s = h.state();
    assert_eq!(rows_now(h).as_ref(), rows, "{what} changed the version");
    assert_eq!(s.viewing(), Some("c1"), "{what}");
    assert_eq!((&s.canvas.notice, &s.save_error), (&None, &None), "{what} met a refusal");
    assert!(
        !s.explorer.menu_open
            && s.explorer.delete.is_none()
            && s.explorer.context.is_none()
            && s.explorer.rename.is_none()
            && s.canvas.menu.is_none()
            && s.canvas.inline.is_none(),
        "{what} opened an editor"
    );
    assert_eq!(s.canvas.tool, crate::canvas::Tool::Select, "{what} armed a tool");
}

/// Every way of editing is off while a version shows: the keys, the tools, the explorer's and
/// diagrams' actions, the canvas's gestures and Properties.
#[test]
fn every_way_of_editing_is_off_while_a_version_shows() {
    use egui::{Key, Modifiers as M};
    in_both(viewing, |mut h, (_rig, (note, skill)), _host| {
        let node = centre_of(&h, "workspace.canvas.node.");
        click(&mut h, node);
        h.event(egui::Event::PointerMoved(node));
        h.run_steps(4);
        let drawn = |h: &Harness<'static, Shell>, what: &str| ids(h).iter().any(|i| i.contains(what));
        assert!(["canvas.adorn.", "class.add-"].iter().all(|w| !drawn(&h, w)), "no handle that edits a card");
        let store = h.state().session.as_ref().unwrap().doc();
        let line = store.alive().find(|o| store.class_of(*o).ok() == Some(archforge_uml::ASSOCIATION)).unwrap();
        let edge = store.uuid(line).unwrap().to_string();
        crate::panels::run(h.state_mut(), "view.select", json!({ "ids": [edge] })).unwrap();
        h.run_steps(2);
        assert!(!drawn(&h, "canvas.line-end."), "no handle that moves a line's end");
        crate::panels::run(h.state_mut(), "view.select", json!({ "ids": [note] })).unwrap();
        h.run_steps(2);
        assert!(!drawn(&h, "canvas.adorn."), "no handle that links a note");
        click(&mut h, node);
        h.run_steps(2);
        let rows = rows_now(&h);
        let shift_command = M::COMMAND.plus(M::SHIFT);
        for (k, m) in [
            (Key::Delete, M::NONE),
            (Key::Backspace, M::NONE),
            (Key::Delete, M::SHIFT),
            (Key::Z, M::COMMAND),
            (Key::Z, shift_command),
            (Key::Y, M::COMMAND),
            (Key::Enter, M::COMMAND),
            (Key::ArrowRight, M::NONE),
            (Key::F2, M::NONE),
        ] {
            key(&mut h, k, m);
            still_only_looking(&h, &rows, &format!("{k:?} with {m:?}"));
        }
        let tools: Vec<String> = ids(&h).into_iter().filter(|i| i.starts_with("workspace.tools.")).collect();
        for id in tools.iter().filter(|i| !["workspace.tools.select", "workspace.tools.pan"].contains(&i.as_str())) {
            let at = rect_of(&h, id).center();
            click(&mut h, at);
            still_only_looking(&h, &rows, id);
        }
        for prefix in ["workspace.tabs.add", "workspace.explorer.add", "workspace.properties.name"] {
            if ids(&h).iter().any(|i| i.starts_with(prefix)) {
                let (at, focus) = (centre_of(&h, prefix), h.ctx.memory(|m| m.focused()));
                click(&mut h, at);
                assert_eq!(h.ctx.memory(|m| m.focused()), focus, "{prefix} takes no typing");
                still_only_looking(&h, &rows, prefix);
            }
        }
        let row = centre_of(&h, "workspace.explorer.row.");
        click(&mut h, row);
        for k in [Key::F2, Key::Delete] {
            key(&mut h, k, M::NONE);
            still_only_looking(&h, &rows, "a key on the explorer's row");
        }
        right_click(&mut h, row);
        still_only_looking(&h, &rows, "the explorer's menu");
        drag(&mut h, row, node);
        h.run_steps(2);
        still_only_looking(&h, &rows, "dragging a row onto the canvas");
        h.state_mut().state.explorer = crate::state::Explorer::Diagrams;
        h.run_steps(2);
        for prefix in ["workspace.diagrams.add", "workspace.diagrams.group."] {
            let id = ids(&h).into_iter().find(|i| i.starts_with(prefix) && i.ends_with(".add") || i == prefix).unwrap();
            let at = rect_of(&h, &id).center();
            click(&mut h, at);
            assert!(h.state().diagrams.new.is_none(), "{prefix}");
        }
        drag(&mut h, node, node + egui::vec2(80.0, 60.0));
        still_only_looking(&h, &rows, "dragging a card");
        double_click(&mut h, node);
        still_only_looking(&h, &rows, "double-clicking a card");
        right_click(&mut h, node);
        still_only_looking(&h, &rows, "the card's menu");
        crate::skill_tab::open(h.state_mut(), skill);
        h.run_steps(3);
        let (at, focus) = (rect_of(&h, "workspace.skill.text").left_top() + egui::vec2(20.0, 12.0), h.ctx.memory(|m| m.focused()));
        click(&mut h, at);
        assert_eq!(h.ctx.memory(|m| m.focused()), focus, "a skill's text takes no typing");
    });
}

/// The banner goes back, the toolbar's looking tools stay on and the status bar says so.
#[test]
fn the_banner_says_what_shows_and_goes_back() {
    in_both(viewing, |mut h, _rig, _host| {
        assert_eq!(painted(&h).iter().filter(|t| t.contains("Viewing an earlier version · Read only")).count(), 2, "the banner and the status bar");
        assert!(painted(&h).iter().any(|t| t == "Read only"), "the badge");
        assert_eq!(crate::home::off_words(h.state()), "Read only while viewing a version");
        let at = rect_of(&h, "workspace.tools.pan").center();
        click(&mut h, at);
        assert_eq!(h.state().canvas.tool, crate::canvas::Tool::Pan);
        h.state_mut().state.panel = crate::state::Panel::Agent;
        h.run_steps(3);
        let at = rect_of(&h, "workspace.chat.allow-changes").center();
        click(&mut h, at);
        assert!(h.state().chat_panel.chat.as_ref().unwrap().allow_changes, "Chat is not read only");
        assert!(painted_has(&h, "Back to current work to chat"), "the composer says how to come back");
        let (at, focus) = (rect_of(&h, "workspace.chat.composer").center(), h.ctx.memory(|m| m.focused()));
        click(&mut h, at);
        assert_eq!(h.ctx.memory(|m| m.focused()), focus, "the composer takes no typing");
        let at = rect_of(&h, "workspace.banner.back").center();
        click(&mut h, at);
        h.run_steps(2);
        assert_eq!(h.state().viewing(), None);
        assert!(!painted_has(&h, "Viewing an earlier version"));
    });
}

/// Undoing a turn from its card is offered on current work and not while a version shows.
#[test]
fn a_turns_undo_is_off_while_a_version_shows() {
    use crate::chat::{ChatEvent, TurnEnd};
    use crate::chat_fixture as fx;
    let chat = || {
        let (h, rig, host, earlier) = chat_and_a_version(ThemeKind::Light);
        (h, (rig, host), earlier)
    };
    in_both(chat, |mut h, (rig, host), earlier| {
        let undo = |h: &Harness<'static, Shell>| ids(h).iter().any(|i| i == "workspace.chat.turn.1.undo");
        let at = rect_of(&h, "workspace.chat.allow-changes").center();
        click(&mut h, at);
        ask_in_chat(&mut h, "Describe a class");
        let id = fx::element(h.state(), archforge_uml::CLASS, "Customer").unwrap();
        let params = serde_json::json!({ "command": "element.describe", "params": { "element": id.to_string(), "text": "A buyer" } });
        fx::call(h.state_mut(), "engine.execute", params).unwrap();
        chat_event(&mut h, &rig, ChatEvent::TurnEnded(TurnEnd::Completed));
        assert!(undo(&h), "offered on current work");
        start_viewing(&mut h, "c1");
        answer_read(&mut h, &host, Ok(earlier));
        assert!(!undo(&h), "not while a version shows");
    });
}

/// Only what changes nothing stays on the canvas; every other action goes.
#[test]
fn only_looking_stays_on_the_canvas() {
    use crate::editing::{Action, Gesture, only_looks};
    let (id, at) = (archforge_mof::Uuid([1; 16]), egui::Pos2::ZERO);
    assert!(
        [Action::Nothing, Action::Select(vec![id]), Action::Toggle(id), Action::Commit, Action::Start(Gesture::Marquee { from: at }, None)]
            .iter()
            .all(only_looks)
    );
    let move_it = Action::Start(Gesture::Move { nodes: vec![id], from: at }, None);
    let edits = [
        move_it,
        Action::Place("x", 0.0, 0.0),
        Action::Rename(id, id),
        Action::Open(id, Some(0)),
        Action::PlaceNote(0.0, 0.0),
        Action::EditNote(id),
        Action::ResetEnd(id, 0),
        Action::ResetRoute(id),
        Action::NaryEnd(id, false),
        Action::NaryAddEnd(id),
        Action::NaryMenu(at, id),
        Action::LineMenu(at, None),
    ];
    assert!(!edits.iter().any(only_looks));
}
