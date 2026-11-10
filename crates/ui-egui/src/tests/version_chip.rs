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
    if h.state().version_list.steps.reading.is_some() {
        assert!(h.output().viewport_output[&egui::ViewportId::ROOT].repaint_delay.as_secs() < 1, "a read waited for wakes the window");
    }
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
        assert_eq!(*host.version_limits.lock().unwrap().last().unwrap(), 50, "the design's page");
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
        assert!(h.state().version_list.steps.reading.is_none(), "what the list read serves the steps round it");
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
            assert!(h.state().version_list.steps.reading.is_none(), "no read left waiting");
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
            key(&mut h, egui::Key::H, MOVE[1]);
            assert!(!ids(&h).iter().any(|i| i == "workspace.header.version.row.1"), "the list, the same read, shows none of them");
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

/// The versions `from..to`, newest first, each with a date of its own and a name to filter by.
fn page(from: usize, to: usize, more: bool) -> crate::VersionsPage {
    let entry = |n: usize| crate::VersionEntry {
        hash: format!("v{n}"),
        date: format!("2026-11-{:02}T14:{:02}:00Z", 28 - n / 60, 59 - n % 60),
        description: Some(format!("Step {n}")),
        ..Default::default()
    };
    crate::VersionsPage { versions: (from..to).map(entry).collect(), more }
}

/// Answers the reads waiting, the History panel's among them when a version has just shown.
fn answer(h: &mut Harness<'static, Shell>, host: &SavingHost, reply: Result<crate::VersionsPage, String>) {
    let waiting = host.version_reads.lock().unwrap().drain(..).collect::<Vec<_>>();
    assert!(!waiting.is_empty(), "a read is waiting");
    waiting.into_iter().for_each(|read| drop(read.send(reply.clone())));
    h.run_steps(3);
}

/// Opens the list and answers its read with `first`.
fn open_with(h: &mut Harness<'static, Shell>, host: &SavingHost, first: Result<crate::VersionsPage, String>) {
    key(h, egui::Key::H, MOVE[1]);
    answer(h, host, first);
}

/// Turns the wheel over the list to its end.
fn wheel_to_end(h: &mut Harness<'static, Shell>) {
    h.event(egui::Event::PointerMoved(rect_of(h, "workspace.header.version.filter").center() + egui::vec2(0.0, 60.0)));
    h.event(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: egui::vec2(0.0, -4000.0),
        modifiers: egui::Modifiers::NONE,
        phase: egui::TouchPhase::Move,
        source: egui::MouseWheelSource::Wheel,
    });
    h.run_steps(8);
}

/// A page's worth reads at once; the wheel scrolls the list alone and the page after comes when the
/// end shows, one read at a time and the window awake for it; at the end of the history none follows.
#[test]
fn the_list_reads_on_as_its_end_comes_into_view() {
    in_both(|mut h, host| {
        open_with(&mut h, &host, Ok(page(0, 50, true)));
        assert_eq!(host.version_asks.lock().unwrap().last().unwrap().as_deref(), None);
        assert!(host.version_reads.lock().unwrap().is_empty(), "nothing more while the end is out of view");
        assert!(!ids(&h).iter().any(|i| i == "workspace.header.version.row.20"), "the rows scroll");
        let pan = h.state().canvas.pan;
        wheel_to_end(&mut h);
        assert!(ids(&h).iter().any(|i| i == "workspace.header.version.row.50"), "the wheel reached the last row");
        assert_eq!(h.state().canvas.pan, pan, "the wheel moved the list, not the canvas under it");
        assert_eq!(host.version_asks.lock().unwrap().last().unwrap().as_deref(), Some("v49"), "the page after what is read");
        assert_eq!(*host.version_limits.lock().unwrap().last().unwrap(), 50);
        assert_eq!(host.version_reads.lock().unwrap().len(), 1, "one read at a time");
        answer(&mut h, &host, Ok(page(50, 52, false)));
        wheel_to_end(&mut h);
        assert!(ids(&h).iter().any(|i| i == "workspace.header.version.row.52"), "the next page is in the list");
        assert!(host.version_reads.lock().unwrap().is_empty(), "the history has ended");
    });
}

/// The arrows bring the highlight into view and read on past the end of what shows.
#[test]
fn the_arrows_scroll_the_list() {
    in_both(|mut h, host| {
        open_with(&mut h, &host, Ok(page(0, 50, true)));
        for _ in 0..49 {
            key(&mut h, egui::Key::ArrowDown, MOVE[0]);
        }
        h.run_steps(2);
        assert!(ids(&h).iter().any(|i| i == "workspace.header.version.row.49"), "the highlight is in view");
        assert_eq!(host.version_reads.lock().unwrap().len(), 1, "the end came into view");
        key(&mut h, egui::Key::Enter, MOVE[0]);
        assert_eq!(host.reads.lock().unwrap()[0].0, "v48", "row 0 is Current work");
    });
}

/// A read that fails says which, keeps what was read, is not told as an empty list, asks nothing
/// until Retry, and Retry reads what failed.
#[test]
fn a_failed_read_shows_its_row_and_retries() {
    in_both(|mut h, host| {
        open_with(&mut h, &host, Err("no".into()));
        assert!(painted_has(&h, "The versions couldn't be read.") && list_shows(&h), "with current work still there");
        h.run_steps(4);
        assert!(host.version_reads.lock().unwrap().is_empty(), "no read again by itself");
        let at = rect_of(&h, "workspace.header.version.retry").center();
        click(&mut h, at);
        assert_eq!(host.version_asks.lock().unwrap().last().unwrap().as_deref(), None, "the newest again");
        answer(&mut h, &host, Ok(page(0, 3, true)));
        assert_eq!(host.version_asks.lock().unwrap().last().unwrap().as_deref(), Some("v2"), "three rows leave the end in view");
        answer(&mut h, &host, Err("no".into()));
        assert!(painted_has(&h, "The next versions couldn't be read.") && painted_has(&h, "Step 1"), "what was read stays");
        h.event(egui::Event::Text("zzz".into()));
        h.run_steps(3);
        assert!(
            painted_has(&h, "The next versions couldn't be read.") && !painted_has(&h, "Nothing matches the filter"),
            "a failure is not an empty result"
        );
        let at = rect_of(&h, "workspace.header.version.retry").center();
        click(&mut h, at);
        assert_eq!(host.version_asks.lock().unwrap().last().unwrap().as_deref(), Some("v2"), "that page only");
    });
}

/// Typing narrows by what a row says, even with the keyboard taken off the field; nothing to pick
/// is said once all is read; Enter then does nothing; closing lets the field and the keyboard go.
#[test]
fn the_filter_narrows_the_list() {
    in_both(|mut h, host| {
        open_with(&mut h, &host, Ok(page(0, 3, false)));
        key(&mut h, egui::Key::ArrowDown, MOVE[0]);
        let field = crate::widgets::egui_id("workspace.header.version.filter");
        assert!(h.ctx.memory(|m| m.has_focus(field)), "it takes the keyboard");
        h.ctx.memory_mut(|m| m.surrender_focus(field));
        h.run_steps(2);
        h.event(egui::Event::Text("STEP".into()));
        h.run_steps(3);
        assert!(
            painted_has(&h, "Step 2") && !ids(&h).iter().any(|i| i == "workspace.header.version.row.2"),
            "current work and the latest say no such thing"
        );
        key(&mut h, egui::Key::Enter, MOVE[0]);
        assert_eq!(host.reads.lock().unwrap()[0].0, "v1", "the highlight starts again at the first row left");
        h.state_mut().back_to_current_work();
        h.run_steps(3);
        key(&mut h, egui::Key::H, MOVE[1]);
        assert!(painted_has(&h, "Same as current work"), "a new opening starts unfiltered, from what is read");
        assert!(host.version_reads.lock().unwrap().is_empty(), "nothing is read again");
        h.event(egui::Event::Text("zzz".into()));
        h.run_steps(3);
        assert!(painted_has(&h, "Nothing matches the filter"));
        key(&mut h, egui::Key::Enter, MOVE[0]);
        assert!(h.state().viewing().is_none() && h.query_by_label("Filter versions…").is_some(), "no row, no pick, and the list stays");
        key(&mut h, egui::Key::Escape, MOVE[0]);
        assert!(!ids(&h).iter().any(|i| i == "workspace.header.version.filter") && h.ctx.memory(|m| m.focused()).is_none());
    });
}

/// A filter that holds nothing yet reads on to the end: an empty page ends it, however it answers.
#[test]
fn a_filter_reads_on_until_the_history_ends() {
    in_both(|mut h, host| {
        open_with(&mut h, &host, Ok(page(0, 60, true)));
        h.event(egui::Event::Text("zzz".into()));
        h.run_steps(3);
        assert!(painted_has(&h, "Loading") && !painted_has(&h, "Nothing matches the filter"), "more may match");
        answer(&mut h, &host, Ok(crate::VersionsPage { versions: Vec::new(), more: true }));
        h.run_steps(3);
        assert!(host.version_reads.lock().unwrap().is_empty() && painted_has(&h, "Nothing matches the filter"));
    });
}

/// A chip whose read failed offers to read again rather than saying there is nothing older.
#[test]
fn a_failed_read_does_not_read_as_the_end_of_the_history() {
    use egui_kittest::kittest::NodeT;
    in_both(|mut h, host| {
        h.state_mut().view_version("h9").unwrap();
        let rows = rows_now(&h);
        answer_read(&mut h, &host, Ok(rows));
        h.run_steps(2);
        answer(&mut h, &host, Err("no".into()));
        assert!(!h.get_by_label("Older version").accesskit_node().is_disabled(), "not told as the end");
        let at = rect_of(&h, "workspace.header.version.older").center();
        click(&mut h, at);
        h.run_steps(2);
        assert_eq!(host.version_reads.lock().unwrap().len(), 1, "it reads again");
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

/// What a step is refused with while an edit is under way.
const EDITING: &str = "Finish or cancel your edit first.";

/// Asks for ⌘[ with an edit under way, which refuses it; the edit is then called off.
fn refused_step(h: &mut Harness<'static, Shell>, host: &SavingHost) {
    h.state_mut().session.as_mut().unwrap().begin_interaction("er.schema.add").unwrap();
    press(h, egui::Key::OpenBracket);
    answer_versions(host);
    h.run_steps(3);
    assert!(host.reads.lock().unwrap().is_empty() && h.state().viewing().is_none(), "the step did not go");
    h.state_mut().session.as_mut().unwrap().cancel().unwrap();
    h.run_steps(2);
}

/// With no diagram open a refused step says why in the empty centre, and the step that then goes
/// through takes the reason away, here and in the view set aside for coming back to.
#[test]
fn a_refused_step_says_why_with_no_diagram_open() {
    in_both(|mut h, host| {
        assert_eq!(centre(&h), (vec![], None), "nothing is open");
        refused_step(&mut h, &host);
        assert_eq!(painted_times(&h, EDITING), 1, "{:?}", painted(&h));
        press(&mut h, egui::Key::OpenBracket);
        assert_eq!(painted_times(&h, EDITING), 0, "the step went through, so the reason is over");
        shown(&mut h, &host, "h2");
        h.state_mut().back_to_current_work();
        h.run_steps(2);
        assert_eq!(painted_times(&h, EDITING), 0, "nor does it come back with current work");
    });
}

/// A skill's tab in the centre says why as well, and a diagram says it once.
#[test]
fn a_refused_step_says_why_over_a_skill_and_once_over_a_diagram() {
    in_both(|mut h, host| {
        let diagram = add_diagram(&mut h, "Alpha");
        let root = crate::panels::run(h.state_mut(), "er.schema.add", serde_json::json!({ "name": "Sales" })).unwrap()["id"].clone();
        let skill = crate::panels::run(h.state_mut(), "skill.add", serde_json::json!({ "subject": root, "name": "Naming", "body": "text" })).unwrap();
        show_diagram(&mut h, skill["id"].as_str().and_then(archforge_mof::Uuid::parse).unwrap());
        refused_step(&mut h, &host);
        assert_eq!(painted_times(&h, EDITING), 1, "{:?}", painted(&h));
        show_diagram(&mut h, diagram);
        assert_eq!(painted_times(&h, EDITING), 0, "opening something else ends it");
        refused_step(&mut h, &host);
        assert_eq!(painted_times(&h, EDITING), 1, "{:?}", painted(&h));
    });
}

/// Over a diagram the words stay below its path bar.
#[test]
fn a_notice_sits_below_the_path_bar() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, _, _) = nested_orders(theme);
        h.state_mut().canvas.notice = Some("Refused".into());
        h.run();
        let words = text_boxes(&h).into_iter().find(|(t, _)| t == "Refused").map(|(_, r)| r).unwrap();
        assert!(words.top() >= rect_of(&h, "workspace.canvas.path").bottom(), "{words:?}");
    }
}

/// What was said about a diagram goes with the last tab, closed or its diagram removed, rather
/// than staying on in the empty centre.
#[test]
fn a_notice_goes_with_the_last_diagram() {
    const SAID: &str = "About that diagram";
    in_both(|mut h, _host| {
        let said = |h: &mut Harness<'static, Shell>, name: &str| {
            let id = add_diagram(h, name);
            show_diagram(h, id);
            h.state_mut().canvas.notice = Some(SAID.into());
            h.run_steps(2);
            assert_eq!(painted_times(h, SAID), 1, "{:?}", painted(h));
            id
        };
        let closed = said(&mut h, "Alpha");
        crate::canvas::close_tab(h.state_mut(), closed);
        h.run_steps(2);
        assert_eq!((centre(&h), painted_times(&h, SAID)), ((vec![], None), 0), "the tab closed");
        let removed = said(&mut h, "Beta");
        crate::panels::run(h.state_mut(), "diagram.remove", serde_json::json!({ "diagram": removed.to_string() })).unwrap();
        h.run_steps(3);
        assert_eq!((centre(&h), painted_times(&h, SAID)), ((vec![], None), 0), "the diagram removed");
    });
}

type Way = (&'static str, fn(&mut Shell) -> Result<(), crate::hold::HoldRefused>);

/// The ways of leaving current work, begun on `c1` or the branch `x`.
fn ways() -> [Way; 5] {
    [
        ("view", |s| s.view_version("c1")),
        ("step", |s| s.move_to_version("c1")),
        ("restore", |s| s.restore_version("c1", "d")),
        ("switch", |s| s.switch_branch("x".to_string())),
        ("replace", |s| s.begin_replacement(std::sync::mpsc::channel().1, None)),
    ]
}

/// That nothing has begun: no hold, no read, no restore job and no switch.
#[track_caller]
fn nothing_began(h: &Harness<'static, Shell>, host: &SavingHost, what: &str) {
    assert!(!h.state().is_held() && h.state().ask.is_none(), "{what}");
    assert!(host.reads.lock().unwrap().is_empty() && host.restores.lock().unwrap().is_empty() && host.switches.lock().unwrap().is_empty(), "{what}");
}

const ID: archforge_mof::Uuid = archforge_mof::Uuid([7; 16]);

/// An edit left open between frames: an n-ary end armed on the canvas.
fn draw(s: &mut Shell) {
    s.canvas.nary = Some(crate::uml_nary::Nary::AddEnd { diagram: ID, association: ID });
}

/// Every way of leaving is refused for an open editor, drawing, drag or dialog, with nothing begun.
#[test]
fn an_open_editor_stops_every_way_of_leaving() {
    use crate::{
        checks::CheckEdit,
        editing::{Gesture, NodeRename},
        explorer::Rename,
        hold::HoldRefused::Editing,
        panels::NameEdit,
    };
    let open: [(&str, &dyn Fn(&mut Shell)); 8] = [
        ("panel name", &|s| s.panels.name = Some(NameEdit { target: ID, text: "x".into() })),
        ("panel card", &|s| {
            s.panels.check = Some(CheckEdit { target: None, entity: ID, name: "x".into(), expression: "1".into(), refused: None, opened: false })
        }),
        ("canvas name", &|s| s.canvas.rename = Some(NodeRename { node: ID, entity: ID, text: "x".into(), opened: false, refused: None })),
        ("drag", &|s| s.canvas.gesture = Some(Gesture::Marquee { from: egui::Pos2::ZERO })),
        ("drawing", &draw),
        ("tree drag", &|s| s.explorer.dragging = Some(ID)),
        ("tree name", &|s| s.explorer.rename = Some(Rename { target: ID, text: "x".into(), fresh: false, refused: None, opened: false })),
        ("dialog", &crate::settings::open),
    ];
    for (what, opens) in open {
        in_both(|mut h, host| {
            opens(h.state_mut());
            for (way, go) in ways() {
                assert_eq!(go(h.state_mut()), Err(Editing), "{what} {way}");
                nothing_began(&h, &host, &format!("{what} {way}"));
            }
        });
    }
}

/// A refusal leaves the version in view with its date, says why, and keeps the list open for the
/// row to be tried again; a step that then goes clears the reason.
#[test]
fn a_refusal_keeps_the_version_and_the_list_and_says_why() {
    let words = Some(EDITING);
    in_both(|mut h, host| {
        let rows = rows_now(&h);
        crate::version_chip::view(h.state_mut(), "c1".to_string(), "8 Nov 14:02".to_string());
        h.run_steps(3);
        answer_read(&mut h, &host, Ok(rows));
        crate::settings::open(h.state_mut());
        for (way, go) in ways() {
            assert!(go(h.state_mut()).is_err() && h.state().viewing() == Some("c1"), "{way} leaves the version alone");
        }
        crate::version_chip::view(h.state_mut(), "c2".to_string(), "9 Nov 09:00".to_string());
        assert_eq!(h.state().canvas.notice.as_deref(), words, "said");
        assert!(crate::version_chip::read_only_words(h.state()).contains("8 Nov 14:02"), "not the date refused");
        h.state_mut().settings = None;
        assert!(h.state_mut().back_to_current_work());
        opened_list(&mut h, &host);
        let before = host.reads.lock().unwrap().len();
        h.state_mut().session.as_mut().unwrap().begin_interaction("er.schema.add").unwrap();
        let at = rect_of(&h, "workspace.header.version.row.1").center();
        click(&mut h, at);
        assert!(list_shows(&h) && h.state().canvas.notice.as_deref() == words, "the list stays");
        assert!(!h.state().is_held() && h.state().viewing().is_none() && host.reads.lock().unwrap().len() == before, "the row began nothing");
        h.state_mut().session.as_mut().unwrap().cancel().unwrap();
        assert!(crate::version_chip::view(h.state_mut(), "h1".to_string(), "x".to_string()));
        assert_eq!(h.state().canvas.notice, None, "the step that goes clears the reason");
    });
}

/// A description being typed is written before leaving; one the model refuses stops it, kept and said.
#[test]
fn a_refused_description_stops_leaving_and_a_good_one_goes_first() {
    use crate::{hold::HoldRefused::Description, panels::DescriptionDraft};
    for (way, go) in ways() {
        in_both(|mut h, host| {
            let schema = crate::panels::run(h.state_mut(), "er.schema.add", serde_json::json!({ "name": "shop" })).unwrap()["id"].clone();
            let table =
                crate::panels::run(h.state_mut(), "er.entity.add", serde_json::json!({ "schema": schema, "name": "customer" })).unwrap()["id"]
                    .clone();
            let target = archforge_mof::Uuid::parse(table.as_str().unwrap()).unwrap();
            h.state_mut().panels.description = Some(DescriptionDraft { target, text: "a\u{7}b".into(), refused: false });
            assert_eq!(go(h.state_mut()), Err(Description), "{way}");
            nothing_began(&h, &host, way);
            assert!(h.state().panels.description.as_ref().is_some_and(|d| d.refused), "{way}: the draft stays");
            crate::version_chip::view(h.state_mut(), "c1".to_string(), "x".to_string());
            assert!(h.state().canvas.notice.is_some() && h.state().canvas.notice == h.state().panels.notice, "{way}: the model's refusal is said");
            h.state_mut().panels.description = Some(DescriptionDraft { target, text: "A customer".into(), refused: false });
            assert_eq!(go(h.state_mut()), Ok(()), "{way}");
            assert!(h.state().is_held() && h.state().panels.description.is_none(), "{way}: written, then held");
        });
    }
}

/// Adds a diagram named `name` of a schema of its own, and gives its id.
fn add_diagram(h: &mut Harness<'static, Shell>, name: &str) -> archforge_mof::Uuid {
    let schema = crate::panels::run(h.state_mut(), "er.schema.add", serde_json::json!({ "name": name })).unwrap()["id"].clone();
    let made = crate::panels::run(h.state_mut(), "diagram.add", serde_json::json!({ "kind": "er", "element": schema, "name": name })).unwrap();
    made["id"].as_str().and_then(archforge_mof::Uuid::parse).unwrap()
}

/// Opens `id` in the centre and lets it draw.
fn show_diagram(h: &mut Harness<'static, Shell>, id: archforge_mof::Uuid) {
    crate::canvas::open(h.state_mut(), id);
    h.run_steps(3);
}

/// Shows the version `commit`, which reads as `rows`.
fn enter(h: &mut Harness<'static, Shell>, host: &SavingHost, rows: std::sync::Arc<archforge_mof::rows::Rows>, commit: &str) {
    start_viewing(h, commit);
    answer_read(h, host, Ok(rows));
    assert_eq!(h.state().viewing(), Some(commit));
}

/// What the centre shows: the tabs and the active one.
fn centre(h: &Harness<'static, Shell>) -> (Vec<archforge_mof::Uuid>, Option<archforge_mof::Uuid>) {
    (h.state().canvas.tabs.clone(), h.state().canvas.active)
}

/// A version that has the diagram showing opens on it, in a tab of its own and fitted, and going
/// back brings the tabs, the selection, the pan and the zoom as they were.
#[test]
fn a_version_opens_on_the_diagram_showing_and_going_back_restores_the_work() {
    in_both(|mut h, host| {
        let (a, b) = (add_diagram(&mut h, "Alpha"), add_diagram(&mut h, "Beta"));
        let earlier = rows_now(&h);
        show_diagram(&mut h, a);
        show_diagram(&mut h, b);
        h.state_mut().canvas.pan = egui::vec2(123.0, 45.0);
        h.state_mut().canvas.zoom = 1.75;
        h.run_steps(2);
        let (before, selected) = (centre(&h), crate::panels::selected_id(h.state()));
        enter(&mut h, &host, earlier, "c1");
        assert_eq!(centre(&h), (vec![b], Some(b)));
        assert!(painted_has(&h, "Beta") && !h.state().canvas.fit_pending, "drawn and fitted: {:?}", painted(&h));
        assert!(h.state().canvas.pan != egui::vec2(123.0, 45.0) || h.state().canvas.zoom != 1.75, "the work's view stays with it");
        assert!(h.state_mut().back_to_current_work());
        h.run_steps(3);
        assert_eq!((centre(&h), crate::panels::selected_id(h.state())), (before, selected));
        assert_eq!((h.state().canvas.pan, h.state().canvas.zoom), (egui::vec2(123.0, 45.0), 1.75));
    });
}

/// Entering opens what shows when the version arrives: a tab changed while it is read counts.
#[test]
fn a_version_opens_on_what_shows_when_it_arrives() {
    in_both(|mut h, host| {
        let (a, b) = (add_diagram(&mut h, "Alpha"), add_diagram(&mut h, "Beta"));
        let rows = rows_now(&h);
        show_diagram(&mut h, a);
        start_viewing(&mut h, "c1");
        show_diagram(&mut h, b);
        answer_read(&mut h, &host, Ok(rows));
        assert_eq!(centre(&h), (vec![b], Some(b)));
    });
}

/// Where the version lacks what shows, or what shows is no diagram, its first diagram opens: the
/// first in the list, not the latest made. With no diagram at all the canvas stays empty.
#[test]
fn a_version_without_what_shows_opens_its_first_diagram_or_nothing() {
    in_both(|mut h, host| {
        let none = rows_now(&h);
        let first = add_diagram(&mut h, "Alpha");
        add_diagram(&mut h, "Beta");
        let two = rows_now(&h);
        let gone = add_diagram(&mut h, "Gamma");
        show_diagram(&mut h, gone);
        enter(&mut h, &host, two.clone(), "c1");
        assert_eq!(centre(&h), (vec![first], Some(first)), "a stale id opens nothing");
        h.state_mut().back_to_current_work();
        h.run_steps(2);
        enter(&mut h, &host, none, "c2");
        assert_eq!(centre(&h), (vec![], None), "the empty state");
        assert_eq!(h.state().canvas.notice, None);
        h.state_mut().back_to_current_work();
        let root = crate::panels::run(h.state_mut(), "er.schema.add", serde_json::json!({ "name": "Sales" })).unwrap()["id"].clone();
        let skill = crate::panels::run(h.state_mut(), "skill.add", serde_json::json!({ "subject": root, "name": "Naming", "body": "text" })).unwrap();
        show_diagram(&mut h, skill["id"].as_str().and_then(archforge_mof::Uuid::parse).unwrap());
        let with_skill = rows_now(&h);
        enter(&mut h, &host, with_skill, "c3");
        assert_eq!(h.state().canvas.active, Some(first), "a skill's tab is no diagram");
    });
}

/// The step keys, the viewing chip's steps and the list each stay on the diagram in view in the
/// version left, whatever current work has open.
#[test]
fn every_way_between_versions_stays_on_the_diagram_in_view() {
    in_both(|mut h, host| {
        let (a, b) = (add_diagram(&mut h, "Alpha"), add_diagram(&mut h, "Beta"));
        show_diagram(&mut h, a);
        press(&mut h, egui::Key::OpenBracket);
        answer_versions(&host);
        h.run_steps(3);
        shown(&mut h, &host, "h3");
        assert_eq!(centre(&h), (vec![a], Some(a)));
        show_diagram(&mut h, b);
        press(&mut h, egui::Key::OpenBracket);
        shown(&mut h, &host, "h2");
        assert_eq!(centre(&h), (vec![b], Some(b)), "the step key");
        let at = rect_of(&h, "workspace.header.version.older").center();
        click(&mut h, at);
        h.run_steps(2);
        shown(&mut h, &host, "h1");
        assert_eq!(centre(&h), (vec![b], Some(b)), "the chip's step");
        opened_list(&mut h, &host);
        key(&mut h, egui::Key::ArrowDown, MOVE[0]);
        key(&mut h, egui::Key::Enter, MOVE[0]);
        h.run_steps(3);
        shown(&mut h, &host, "h3");
        assert_eq!(centre(&h), (vec![b], Some(b)), "the list");
    });
}

/// A version with nothing in view carries nothing: the next opens its first diagram, not the one
/// current work has open. A read dropped on the way carries nothing into the next either.
#[test]
fn nothing_is_carried_past_an_empty_view_or_a_dropped_read() {
    in_both(|mut h, host| {
        let none = rows_now(&h);
        let (a, b) = (add_diagram(&mut h, "Alpha"), add_diagram(&mut h, "Beta"));
        let two = rows_now(&h);
        show_diagram(&mut h, b);
        enter(&mut h, &host, none, "c1");
        assert_eq!(centre(&h), (vec![], None));
        h.state_mut().move_to_version("c2").unwrap();
        h.run_steps(3);
        answer_read(&mut h, &host, Ok(two.clone()));
        assert_eq!((h.state().viewing(), centre(&h)), (Some("c2"), (vec![a], Some(a))), "the version's first, not the work's");
        h.state_mut().move_to_version("c3").unwrap();
        h.run_steps(3);
        assert!(h.state_mut().back_to_current_work(), "the read is dropped");
        host.reads.lock().unwrap().clear();
        assert_eq!(centre(&h).1, Some(b), "current work's own");
        enter(&mut h, &host, two, "c4");
        assert_eq!(centre(&h), (vec![b], Some(b)), "what the dropped read carried is gone");
    });
}

/// The window at `width`, and the chip's rectangle there.
fn chip_at(h: &mut Harness<'static, Shell>, width: f32) -> egui::Rect {
    h.set_size(egui::vec2(width, 860.0));
    h.run_steps(3);
    rect_of(h, "workspace.header.version")
}

/// The icon drawn centred on `centre`: its picture's rectangle, padding included.
fn icon_at(h: &Harness<'static, Shell>, centre: egui::Pos2) -> Option<egui::Rect> {
    /// The bounds of the first textured mesh under `s` centred on `centre`.
    fn walk(s: &egui::Shape, centre: egui::Pos2) -> Option<egui::Rect> {
        match s {
            egui::Shape::Mesh(m) if m.texture_id != egui::TextureId::default() && m.calc_bounds().center().distance(centre) < 1.0 => {
                Some(m.calc_bounds())
            }
            egui::Shape::Vec(v) => v.iter().find_map(|s| walk(s, centre)),
            _ => None,
        }
    }
    h.output().shapes.iter().find_map(|c| walk(&c.shape, centre))
}

/// The size the first text reading `text` is painted at.
fn text_size(h: &Harness<'static, Shell>, text: &str) -> Option<f32> {
    /// The first font size under `s` of a text reading `text`.
    fn walk(s: &egui::Shape, text: &str) -> Option<f32> {
        match s {
            egui::Shape::Text(t) if t.galley.text() == text => t.galley.job.sections.first().map(|s| s.format.font_id.size),
            egui::Shape::Vec(v) => v.iter().find_map(|s| walk(s, text)),
            _ => None,
        }
    }
    h.output().shapes.iter().find_map(|c| walk(&c.shape, text))
}

/// Whether the versions' clock is drawn at `centre`: its 14-point picture, not the plain clock's 17.
fn versions_clock_at(h: &Harness<'static, Shell>, centre: egui::Pos2) -> bool {
    icon_at(h, centre).is_some_and(|r| r.width() < 17.0 + 2.0)
}

/// Whether the unsaved dot is drawn on the chip's icon.
fn icon_dot(h: &Harness<'static, Shell>, chip: egui::Rect) -> bool {
    let p = crate::ux::palette(h.state().state.theme == ThemeKind::Dark);
    /// Whether a 3-point circle of `fill` is drawn at `at` under `s`.
    fn walk(s: &egui::Shape, at: egui::Pos2, fill: egui::Color32) -> bool {
        match s {
            egui::Shape::Circle(c) => c.center.distance(at) < 0.5 && c.fill == fill && (c.radius - 3.0).abs() < 0.01,
            egui::Shape::Vec(v) => v.iter().any(|s| walk(s, at, fill)),
            _ => false,
        }
    }
    h.output().shapes.iter().any(|c| walk(&c.shape, chip.min + egui::vec2(24.0, 8.0), p.amber))
}

/// The label goes before search does: with room for search but not the whole chip, one icon beside
/// it; without room for search, the icon against Help and no search. The tabs leave it room.
#[test]
fn the_chip_narrows_to_an_icon_before_search_hides() {
    in_both(|mut h, _host| {
        h.state_mut().tabs[0].name = "A project with a rather long name indeed".to_string();
        let wide = chip_at(&mut h, 1600.0);
        assert_eq!(wide.size(), egui::vec2(135.0, 32.0));
        let beside = chip_at(&mut h, 900.0);
        assert_eq!(beside.size(), egui::vec2(32.0, 32.0), "the label went");
        assert_eq!(rect_of(&h, "workspace.header.search").left() - beside.right(), 12.0, "search is still there, beside it");
        let icon = chip_at(&mut h, 640.0);
        assert_eq!(icon.size(), egui::vec2(32.0, 32.0));
        let help = rect_of(&h, "workspace.header.help");
        assert_eq!(help.left() - icon.right(), 12.0, "against Help");
        assert!(!ids(&h).iter().any(|i| i == "workspace.header.search"), "search hid");
        let tab = rect_of(&h, "workspace.header.project.0");
        assert!(tab.right() <= icon.left(), "the tab {tab:?} keeps clear of the chip {icon:?}");
    });
}

/// At 640 the viewing chip is the icon, untinted, with a dot for work unsaved and no steps; its
/// tooltip and accessible name say what it views, and a click opens the list.
#[test]
fn the_narrow_viewing_chip_names_the_version_it_shows() {
    in_both(|mut h, host| {
        let p = crate::ux::palette(h.state().state.theme == ThemeKind::Dark);
        press(&mut h, egui::Key::OpenBracket);
        answer_versions(&host);
        h.run_steps(3);
        shown(&mut h, &host, "h2");
        let icon = chip_at(&mut h, 640.0);
        assert_eq!(icon.size(), egui::vec2(32.0, 32.0));
        assert!(rect_shapes(&h).iter().any(|(r, fill, _)| *r == icon && *fill == p.surface), "untinted");
        assert!(!rect_shapes(&h).iter().any(|(r, fill, _)| *r == icon && *fill == p.accent_tint));
        assert!(versions_clock_at(&h, icon.center()), "the clock with its counter-clockwise arrow");
        assert!(!icon_dot(&h, icon), "nothing is unsaved");
        let band = rect_shapes(&h).into_iter().find(|(r, fill, _)| *fill == p.accent_tint && r.height() == 44.0).unwrap().0;
        assert!(versions_clock_at(&h, egui::pos2(band.left() + 20.0, band.center().y)), "the banner's clock is the same one");
        assert!(!ids(&h).iter().any(|i| i == "workspace.header.version.back" || i == "workspace.header.version.older"), "one icon, no steps");
        let read_only = |h: &Harness<'static, Shell>| {
            painted(h).iter().filter(|t| t.starts_with("Viewing the version of 8 Nov") && t.ends_with("· Read only")).count()
        };
        let on_the_chip = h.query_all_by_label_contains("Viewing the version of 8 Nov").any(|n| icon.contains(n.rect().center()));
        assert!(on_the_chip, "its accessible name says what it views");
        let before = read_only(&h);
        h.event(egui::Event::PointerMoved(icon.center()));
        for _ in 0..90 {
            h.step();
        }
        assert_eq!(read_only(&h), before + 1, "its tooltip says what it views");
        click(&mut h, icon.center());
        h.run_steps(2);
        assert!(list_shows(&h), "it opens the list");
    });
}

/// Work left unsaved while a version shows puts the dot on the narrow chip, as on current work.
#[test]
fn the_narrow_chip_shows_a_dot_for_unsaved_work() {
    in_both(|mut h, host| {
        let icon = chip_at(&mut h, 640.0);
        assert!(!icon_dot(&h, icon) && h.query_by_label("Current work").is_some(), "nothing unsaved");
        add_schema(&mut h, "shop");
        h.run_steps(2);
        assert!(icon_dot(&h, icon) && h.query_by_label("Current work · unsaved changes").is_some(), "work unsaved");
        h.state_mut().view_version("h1").unwrap();
        h.run_steps(3);
        let rows = rows_now(&h);
        answer_read(&mut h, &host, Ok(rows));
        assert_eq!(h.state().viewing(), Some("h1"));
        h.run_steps(2);
        assert!(icon_dot(&h, icon), "the work still unsaved shows while a version does");
    });
}

/// The whole chip at the design's measures: the clock with its arrow and the label at 12 points.
#[test]
fn the_chip_has_the_designs_clock_and_label_size() {
    in_both(|mut h, _host| {
        let chip = chip_at(&mut h, 1600.0);
        assert_eq!(text_size(&h, "Current work"), Some(12.0));
        assert!(versions_clock_at(&h, egui::pos2(chip.left() + 18.0, chip.center().y)), "the clock with its arrow, as the design has it");
    });
}
