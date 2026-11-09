use egui_kittest::kittest::Queryable;

use super::*;
use crate::actions::{self, VERSIONS};

const MOVE: [egui::Modifiers; 2] = [egui::Modifiers::NONE, egui::Modifiers::COMMAND.plus(egui::Modifiers::SHIFT)];

/// Runs `test` on an open project, once in each appearance.
fn in_both(test: impl Fn(Harness<'static, Shell>, std::sync::Arc<SavingHost>)) {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, host) = saving_harness(Some(commerce()), None);
        h.state_mut().state.theme = theme;
        test(h, host);
    }
}

/// Opens the list with the keys and answers its read with three versions, newest first.
fn opened_list(h: &mut Harness<'static, Shell>, host: &SavingHost) {
    key(h, egui::Key::H, MOVE[1]);
    assert!(h.output().viewport_output[&egui::ViewportId::ROOT].repaint_delay.as_secs() < 1, "a read waited for wakes the window");
    let v = |n: usize, description: Option<&str>, by_agent| crate::VersionEntry {
        hash: format!("h{n}"),
        date: format!("2026-11-08T14:{n:02}:00Z"),
        description: description.map(str::to_string),
        by_agent,
        ..Default::default()
    };
    let versions = vec![v(3, Some("Add status"), false), v(2, None, true), v(1, None, false)];
    host.pages.lock().unwrap().remove(0).2.send(Ok(crate::VersionsPage { versions, more: false })).unwrap();
    h.run_steps(3);
}

/// Whether the list is drawn.
fn list_shows(h: &Harness<'static, Shell>) -> bool {
    ids(h).iter().any(|i| i == "workspace.header.version.row.0")
}

/// The chip says what shows, a click toggles the list, and its latest version reads as current work.
#[test]
fn the_chip_names_what_shows_and_the_list_says_what_each_version_is() {
    in_both(|mut h, host| {
        assert!(h.query_by_label("Current work").is_some());
        for open in [true, false] {
            let at = rect_of(&h, "workspace.header.version").center();
            click(&mut h, at);
            assert_eq!(list_shows(&h), open);
        }
        let (after, limit, _) = host.pages.lock().unwrap().remove(0);
        assert_eq!((after, limit), (None, 7), "the newest versions, each one in reach");
        opened_list(&mut h, &host);
        assert!(
            ["No unsaved changes", "Same as current work", "Saved by an agent"].iter().all(|w| painted_has(&h, w)) && !painted_has(&h, "Add status")
        );
        add_schema(&mut h, "shop");
        h.run_steps(2);
        assert!(painted_has(&h, "Unsaved changes") && painted_has(&h, "Add status"));
        assert!(h.query_by_label("Current work · unsaved changes").is_some());
    });
}

/// The keys move, view and close; a hold or the project going closes the list; a time reads local.
#[test]
fn the_keys_move_view_and_close() {
    in_both(|mut h, host| {
        opened_list(&mut h, &host);
        key(&mut h, egui::Key::H, egui::Modifiers::COMMAND);
        assert!(host.pages.lock().unwrap().is_empty(), "the shift is part of the keys");
        for arrow in [egui::Key::ArrowDown; 5].into_iter().chain([egui::Key::ArrowUp, egui::Key::ArrowUp, egui::Key::Enter]) {
            key(&mut h, arrow, MOVE[0]);
        }
        assert_eq!(host.reads.lock().unwrap()[0].0, "h3", "the row under the highlight, the last one being as far as it goes");
        let rows = rows_now(&h);
        answer_read(&mut h, &host, Ok(rows));
        assert_eq!(h.state().viewing(), Some("h3"));
        assert!(h.query_by_label_contains("Viewing the version of 8 Nov").is_some());
        opened_list(&mut h, &host);
        key(&mut h, egui::Key::Escape, MOVE[0]);
        assert!(!list_shows(&h) && h.state().viewing() == Some("h3"), "Esc never leaves the version");
        opened_list(&mut h, &host);
        key(&mut h, egui::Key::ArrowDown, MOVE[0]);
        key(&mut h, egui::Key::Enter, MOVE[0]);
        assert_eq!(host.reads.lock().unwrap()[0].0, "h3", "from a version, to another");
        h.state_mut().back_to_current_work();
        opened_list(&mut h, &host);
        click(&mut h, egui::pos2(20.0, 600.0));
        assert!(!list_shows(&h), "a click elsewhere closes it");
        opened_list(&mut h, &host);
        key(&mut h, egui::Key::Enter, MOVE[0]);
        assert_eq!(h.state().viewing(), None);
        h.state_mut().view_version("h9").unwrap();
        h.run_steps(3);
        assert!(h.query_by_label("Viewing a version").is_some(), "no other version's date");
        h.state_mut().back_to_current_work();
        opened_list(&mut h, &host);
        h.state_mut().begin_hold().unwrap();
        h.run_steps(2);
        assert!(!list_shows(&h), "a hold closes it");
        h.state_mut().end_hold();
        opened_list(&mut h, &host);
        h.state_mut().project.as_mut().unwrap().path = "/another".into();
        h.run_steps(2);
        assert!(!list_shows(&h));
        assert_eq!(crate::version_chip::when("2026-11-08T23:30:00Z", crate::chat_words::Clock { now: 0, offset: Some(3_600) }), "9 Nov 00:30");
    });
}

/// The keys and the action wait while a hold or any dialog is up.
#[test]
fn the_list_cannot_open_behind_a_hold_or_a_dialog() {
    use crate::{
        chat_delete::Ask,
        diagrams::{NewDiagram, Step},
    };
    let blocks: [&dyn Fn(&mut Shell); 10] = [
        &|s| s.begin_hold().unwrap(),
        &|s| s.session = None,
        &|s| s.ask = Some(crate::close_ask::Ask::Unsaved { path: "/p".into() }),
        &crate::support::open,
        &Shell::open_generate,
        &crate::new_project::open,
        &crate::settings::open,
        &|s| s.explorer.delete = Some(Default::default()),
        &|s| {
            s.diagrams.new = Some(NewDiagram { step: Step::Kind, name: "".into(), kind: "", schema: None, menu: None, refused: None, opened: false })
        },
        &|s| s.chat_panel.ask = Some(Ask { chat: archforge_mof::Uuid([0; 16]), row: 0, summary: "".into(), running: false, focused: false }),
    ];
    for block in blocks {
        in_both(|mut h, host| {
            assert!(actions::available(h.state(), VERSIONS));
            block(h.state_mut());
            assert!(!actions::available(h.state(), VERSIONS), "all of them");
            actions::run(h.state_mut(), VERSIONS);
            assert!(host.pages.lock().unwrap().is_empty(), "nothing is read");
        });
    }
}
