use egui_kittest::kittest::Queryable;

use super::*;
use crate::actions::{self, VERSIONS};

const MOVE: [egui::Modifiers; 2] = [egui::Modifiers::NONE, egui::Modifiers::COMMAND.plus(egui::Modifiers::SHIFT)];

/// Runs `test` on an open project, once in each appearance.
fn in_both(test: impl Fn(Harness<'static, Shell>, std::sync::Arc<SavingHost>)) {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, host) = saving_harness(Some(commerce()), None);
        h.state_mut().state.theme = theme;
        host.version_reads.lock().unwrap().clear();
        test(h, host);
    }
}

/// Answers every read of versions waiting with three, newest first.
fn answer_versions(host: &SavingHost) {
    let v = |n: usize, description: Option<&str>, by_agent| crate::VersionEntry {
        hash: format!("h{n}"),
        date: format!("2026-11-08T14:{n:02}:00Z"),
        description: description.map(str::to_string),
        by_agent,
        ..Default::default()
    };
    let versions = vec![v(3, Some("Add status"), false), v(2, None, true), v(1, None, false)];
    host.version_reads
        .lock()
        .unwrap()
        .drain(..)
        .for_each(|read| drop(read.send(Ok(crate::VersionsPage { versions: versions.clone(), more: false }))));
}

/// Opens the list, from the chip's date while viewing, and answers its read with three versions, newest first.
fn opened_list(h: &mut Harness<'static, Shell>, host: &SavingHost) {
    if h.state().viewing().is_some() {
        let at = rect_of(h, "workspace.header.version").center();
        click(h, at);
    } else {
        key(h, egui::Key::H, MOVE[1]);
    }
    assert!(h.output().viewport_output[&egui::ViewportId::ROOT].repaint_delay.as_secs() < 1, "a read waited for wakes the window");
    answer_versions(host);
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
        let at = rect_of(&h, "workspace.header.version").center();
        for open in [true, false] {
            click(&mut h, at);
            assert_eq!(list_shows(&h), open);
        }
        assert_eq!(*host.version_limits.lock().unwrap().last().unwrap(), 7, "the newest versions, each one in reach");
        host.version_reads.lock().unwrap().remove(0);
        opened_list(&mut h, &host);
        assert!(["No unsaved changes", "Same as current work", "Saved by an agent"].iter().all(|w| painted_has(&h, w)));
        assert!(!painted_has(&h, "Add status"));
        add_schema(&mut h, "shop");
        assert!(painted_has(&h, "Unsaved changes") && painted_has(&h, "Add status") && h.query_by_label("Current work · unsaved changes").is_some());
    });
}

/// The keys move, view and close; a hold or the project going closes the list; a time reads local.
#[test]
fn the_keys_move_view_and_close() {
    in_both(|mut h, host| {
        opened_list(&mut h, &host);
        key(&mut h, egui::Key::H, egui::Modifiers::COMMAND);
        assert!(host.version_reads.lock().unwrap().is_empty(), "the shift is part of the keys");
        for arrow in [egui::Key::ArrowDown; 5].into_iter().chain([egui::Key::ArrowUp, egui::Key::ArrowUp, egui::Key::Enter]) {
            key(&mut h, arrow, MOVE[0]);
        }
        assert_eq!(host.reads.lock().unwrap()[0].0, "h3", "the row under the highlight, the last one being as far as it goes");
        let rows = rows_now(&h);
        answer_read(&mut h, &host, Ok(rows));
        assert!(h.query_by_label_contains("Viewing the version of 8 Nov").is_some());
        assert!(painted(&h).iter().any(|t| t.starts_with("Viewing the version of 8 Nov") && t.ends_with("· Read only")), "the banner names it");
        assert!(h.state().version_list.steps.reading.is_some(), "a version shown reads what lies round it");
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
        h.state_mut().save();
        host.saves.lock().unwrap().pop().unwrap().send(Err(crate::SaveError::Failed("no".into()))).unwrap();
        h.run_steps(3);
        assert!(!list_shows(&h), "a finished save closes it, even a failed one");
        assert_eq!(crate::version_chip::when("2026-11-08T23:30:00Z", crate::chat_words::Clock { now: 0, offset: Some(3_600) }), "9 Nov 00:30");
    });
}

/// The keys and the action wait while a hold or any dialog is up.
#[test]
fn the_list_cannot_open_behind_a_hold_or_a_dialog() {
    use crate::chat_delete::Ask;
    use crate::diagrams::{NewDiagram, Step};
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
            block(h.state_mut());
            assert!(!actions::available(h.state(), VERSIONS), "all of them");
            actions::run(h.state_mut(), VERSIONS);
            assert!(host.version_reads.lock().unwrap().is_empty(), "nothing is read");
        });
    }
}

/// However the project goes, or is held, the list closes with no read left waiting and stays closed.
#[test]
fn the_list_closes_on_every_way_the_project_goes() {
    let other = || crate::Opened { path: "/other".into(), ..Default::default() };
    let routes: [&dyn Fn(&mut Shell); 4] = [&|s| s.begin_hold().unwrap(), &Shell::go_home, &Shell::close_project, &|s| s.open_project(other())];
    for route in routes {
        in_both(|mut h, host| {
            key(&mut h, egui::Key::H, MOVE[1]);
            route(h.state_mut());
            h.run_steps(2);
            assert!(h.state().version_list.reading.is_none(), "no read left waiting");
            host.closings.lock().unwrap().drain(..).for_each(|c| drop(c.send(Ok(()))));
            h.state_mut().open_project(commerce());
            h.run_steps(4);
            assert!(!list_shows(&h), "it does not come back");
        });
    }
}

/// Presses a step's keys and lets the window take them.
fn press(h: &mut Harness<'static, Shell>, k: egui::Key) {
    key(h, k, egui::Modifiers::COMMAND);
    h.run_steps(2);
}

/// Shows the version the window has just asked for.
fn shown(h: &mut Harness<'static, Shell>, host: &SavingHost, hash: &str) {
    assert_eq!(host.reads.lock().unwrap().first().map(|r| r.0.clone()).as_deref(), Some(hash));
    let rows = rows_now(h);
    answer_read(h, host, Ok(rows));
    assert_eq!(h.state().viewing(), Some(hash));
    h.run_steps(2);
}

/// From current work a step reads the 50 newest and goes to the first that differs; then it moves
/// one at a time and stops at the ends; ⇧⌘H goes back. With unsaved work it goes to the latest.
#[test]
fn the_keys_step_through_the_versions() {
    in_both(|mut h, host| {
        press(&mut h, egui::Key::OpenBracket);
        assert_eq!(host.version_limits.lock().unwrap().last(), Some(&50));
        answer_versions(&host);
        h.run_steps(3);
        shown(&mut h, &host, "h2");
        opened_list(&mut h, &host);
        key(&mut h, egui::Key::Escape, MOVE[0]);
        press(&mut h, egui::Key::OpenBracket);
        assert!(h.state().version_list.steps.reading.is_none(), "what was read serves the next step, the list closed or not");
        shown(&mut h, &host, "h1");
        press(&mut h, egui::Key::OpenBracket);
        assert!(host.reads.lock().unwrap().is_empty(), "the oldest has nothing before it");
        press(&mut h, egui::Key::CloseBracket);
        shown(&mut h, &host, "h2");
        press(&mut h, egui::Key::CloseBracket);
        shown(&mut h, &host, "h3");
        press(&mut h, egui::Key::CloseBracket);
        assert!(host.reads.lock().unwrap().is_empty(), "the newest has nothing after it");
        key(&mut h, egui::Key::H, MOVE[1]);
        assert_eq!(h.state().viewing(), None, "⇧⌘H goes back");
        press(&mut h, egui::Key::CloseBracket);
        assert!(host.reads.lock().unwrap().is_empty(), "nothing is newer than current work");
        add_schema(&mut h, "shop");
        press(&mut h, egui::Key::OpenBracket);
        shown(&mut h, &host, "h3");
    });
}

/// A second press while the read is on its way is ignored, the window keeps waking, and a hold or
/// another project drops the step; a finished save means reading again.
#[test]
fn a_step_waiting_for_its_read() {
    let routes: [&dyn Fn(&mut Shell); 2] = [&|s| s.begin_hold().unwrap(), &|s| s.project.as_mut().unwrap().path = "/another".into()];
    for route in routes {
        in_both(|mut h, host| {
            press(&mut h, egui::Key::OpenBracket);
            press(&mut h, egui::Key::OpenBracket);
            assert_eq!(host.version_reads.lock().unwrap().len(), 1, "the second press is ignored");
            assert!(h.output().viewport_output[&egui::ViewportId::ROOT].repaint_delay.as_secs() < 1, "it wakes while waiting");
            route(h.state_mut());
            h.run_steps(2);
            assert!(h.state().version_list.steps.reading.is_none(), "the step is dropped");
            let _ = host.version_reads.lock().unwrap().remove(0).send(Ok(Default::default()));
            h.run_steps(2);
            assert!(host.reads.lock().unwrap().is_empty());
        });
    }
    in_both(|mut h, host| {
        press(&mut h, egui::Key::OpenBracket);
        answer_versions(&host);
        h.run_steps(3);
        shown(&mut h, &host, "h2");
        h.state_mut().back_to_current_work();
        h.state_mut().save();
        host.saves.lock().unwrap().pop().unwrap().send(Ok(true)).unwrap();
        h.run_steps(3);
        press(&mut h, egui::Key::OpenBracket);
        assert!(h.state().version_list.steps.reading.is_some(), "a finished save means reading again");
    });
}

/// A restore or a branch switch that has started means reading the versions again, whether it
/// finished or failed after saving the work: no step goes through what was read before it.
#[test]
fn a_restore_or_a_branch_switch_means_reading_the_versions_again() {
    use crate::replace::JobFailure::WorkSaved;
    let jobs: [fn(&mut Harness<'static, Shell>, &SavingHost) -> RestoreEnds; 2] = [
        |h, host| {
            let rows = rows_now(h);
            restore_under_way(h, host, rows)
        },
        |h, host| {
            let (rows, ends) = (rows_now(h), switch_asked(h, host));
            switch_checked(h, &ends, rows);
            ends.moved
        },
    ];
    for (job, failed) in jobs.into_iter().flat_map(|job| [(job, false), (job, true)]) {
        in_both(|mut h, host| {
            press(&mut h, egui::Key::OpenBracket);
            answer_versions(&host);
            h.run_steps(3);
            shown(&mut h, &host, "h2");
            job(&mut h, &host).0.send(if failed { Err(WorkSaved("no".into())) } else { Ok(()) }).unwrap();
            h.run_steps(8);
            assert!(!h.state().is_held() && h.state().viewing().is_none(), "over, and back on the work");
            press(&mut h, egui::Key::OpenBracket);
            assert!(host.reads.lock().unwrap().is_empty(), "no step through what was read before");
            assert!(h.state().version_list.steps.reading.is_some(), "the versions are read again");
        });
    }
}

/// The viewing chip steps, opens the list from its date and goes back; at an end a step is muted,
/// does nothing and reads as unavailable.
#[test]
fn the_viewing_chip_steps_and_goes_back() {
    use egui_kittest::kittest::NodeT;
    in_both(|mut h, host| {
        press(&mut h, egui::Key::OpenBracket);
        answer_versions(&host);
        h.run_steps(3);
        shown(&mut h, &host, "h2");
        let at = rect_of(&h, "workspace.header.version.older").center();
        h.state_mut().begin_hold().unwrap();
        h.run_steps(2);
        click(&mut h, at);
        assert!(h.state().canvas.notice.is_none() && host.reads.lock().unwrap().is_empty(), "a hold keeps the steps off");
        h.state_mut().end_hold();
        h.run_steps(2);
        click(&mut h, at);
        h.run_steps(2);
        shown(&mut h, &host, "h1");
        assert!(h.get_by_label("Older version").accesskit_node().is_disabled(), "the end reads as unavailable");
        assert!(!h.get_by_label("Newer version").accesskit_node().is_disabled());
        let at = rect_of(&h, "workspace.header.version.older").center();
        click(&mut h, at);
        assert!(host.reads.lock().unwrap().is_empty(), "an end does nothing");
        let at = rect_of(&h, "workspace.header.version").center();
        click(&mut h, at);
        h.run_steps(2);
        assert!(list_shows(&h), "the date opens the list");
        key(&mut h, egui::Key::Escape, MOVE[0]);
        let at = rect_of(&h, "workspace.header.version.back").center();
        click(&mut h, at);
        assert_eq!(h.state().viewing(), None);
    });
}

/// A version that cannot be shown says why at the top of the canvas.
#[test]
fn a_refused_view_says_why() {
    in_both(|mut h, _host| {
        h.state_mut().begin_hold().unwrap();
        crate::version_chip::view(h.state_mut(), "h1".to_string(), "x".to_string());
        assert_eq!(h.state().canvas.notice.as_deref(), Some("Wait for the change under way to finish."));
    });
}
