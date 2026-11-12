use egui_kittest::kittest::{NodeT, Queryable};

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
        key(&mut h, egui::Key::ArrowDown, MOVE[0]);
        key(&mut h, egui::Key::Enter, MOVE[0]);
        assert_eq!(host.reads.lock().unwrap()[0].0, "h2", "from a version, to another");
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
        // Unsaved work, which Save makes a version of (with none it asks for nothing).
        h.state_mut().session.as_mut().unwrap().mark_unsaved();
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

/// That `hash`, read before, shows with no second read.
#[track_caller]
fn shown_kept(h: &mut Harness<'static, Shell>, host: &SavingHost, hash: &str) {
    h.run_steps(2);
    assert!(host.reads.lock().unwrap().is_empty(), "no second read");
    assert_eq!(h.state().viewing(), Some(hash));
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
        shown_kept(&mut h, &host, "h2");
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
        h.run_steps(4);
        shown_kept(&mut h, &host, "h3");
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
        // Unsaved work, which Save makes a version of (with none it asks for nothing).
        h.state_mut().session.as_mut().unwrap().mark_unsaved();
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

/// A version that cannot be shown says why.
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
        ("step", |s| s.move_to_version("c1", None)),
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

/// Leaving closes a colour picker as Cancel does: a refusal keeps it, and one opened under the hold
/// does not outlast the replacement.
#[test]
fn leaving_closes_a_colour_picker_and_a_refusal_keeps_it() {
    use crate::appearance::{Group, Picker};
    let picker = || Some(Picker::new(Group::Fill, egui::Color32::RED));
    in_both(|mut h, _| {
        let earlier = rows_now(&h);
        h.state_mut().panels.picker = picker();
        crate::settings::open(h.state_mut());
        assert_eq!(h.state_mut().clear_to_leave(None).err(), Some(crate::hold::HoldRefused::Editing));
        assert!(h.state().panels.picker.is_some(), "a refusal leaves it open");
        h.state_mut().settings = None;
        assert!(h.state_mut().clear_to_leave(None).is_ok() && h.state().panels.picker.is_none(), "leaving closes it");
        let read = start_replacing(&mut h, None);
        h.state_mut().panels.picker = picker();
        read.send(Ok(earlier)).unwrap();
        h.run_steps(4);
        assert!(!h.state().is_held() && h.state().panels.picker.is_none(), "the replacement closes it");
    });
}

/// An association's ends stop leaving only while holding something not yet written: the panel
/// fills them in for any association, and an open fold holds nothing.
#[test]
fn an_end_typed_on_the_selected_association_stops_leaving() {
    use crate::{
        hold::HoldRefused::Editing,
        uml_relate::{EndModifiers, EndsEdit},
    };
    /// An end's modifiers as a plain end has them.
    fn modifiers() -> EndModifiers {
        EndModifiers {
            is_static: false,
            default: String::new(),
            derived: false,
            read_only: false,
            ordered: false,
            unique: true,
            subsets: vec![],
            redefines: vec![],
        }
    }
    /// The ends as the panel fills them in with nothing typed.
    fn untouched() -> EndsEdit {
        let (none, written) = ([String::new(), String::new()], ["1".to_string(), "*".to_string()]);
        EndsEdit {
            association: ID,
            role: none.clone(),
            multiplicity: written.clone(),
            written_role: none,
            written,
            refused: [None, None],
            role_tried: [None, None],
            modifiers: [modifiers(), modifiers()],
            written_modifiers: [modifiers(), modifiers()],
            modifier_refused: [None, None],
            modifiers_open: [false, false],
        }
    }
    type Change = fn(&mut EndsEdit);
    let typed: [(&str, Change); 3] = [
        ("role", |e| e.role[0] = "x".into()),
        ("multiplicity", |e| e.multiplicity[1] = "2..3".into()),
        ("modifiers", |e| e.modifiers[1].derived = true),
    ];
    in_both(|mut h, _| {
        let s = h.state_mut();
        s.panels.ends = Some(untouched());
        assert!(s.clear_to_leave(None).is_ok(), "the panel filled them in and nothing was typed");
        s.panels.ends.as_mut().unwrap().modifiers_open = [true, true];
        assert!(s.clear_to_leave(None).is_ok(), "an open fold holds nothing");
        for (what, change) in typed {
            s.panels.ends = Some(untouched());
            change(s.panels.ends.as_mut().unwrap());
            assert_eq!(s.clear_to_leave(None).err(), Some(Editing), "{what}");
        }
    });
}

/// A click off an association ends its draft: what reads is written though its field never saw the
/// click, the rest goes as Cancel, and a replacement without the association has nothing to lose.
#[test]
fn a_moved_selection_ends_an_associations_draft_before_a_replacement() {
    in_both(|mut h, _| {
        let earlier = rows_now(&h);
        crate::uml_ui::open_sample(h.state_mut()).unwrap();
        h.state_mut().state.show_panel(crate::state::Panel::Properties);
        h.run();
        let away = rect_of(&h, "workspace.canvas").left_top() + egui::vec2(30.0, 30.0);
        for (field, typed) in [("role", "payer"), ("multiplicity", "2..5")] {
            select_named(&mut h, archforge_uml::ASSOCIATION, "paid by");
            let before = revision(&h);
            retype(&mut h, &format!("workspace.properties.end.1.{field}"), typed);
            assert_eq!(revision(&h), before, "{field}: still selected, it is only typed");
            h.state_mut().panels.ends.as_mut().unwrap().modifiers[1].derived = true;
            click(&mut h, away);
            assert!(h.state().panels.ends.is_none(), "{field}");
        }
        let paid = select_named(&mut h, archforge_uml::ASSOCIATION, "paid by");
        let st = h.state().session.as_ref().unwrap().doc();
        let end = archforge_uml::figures::member_ends(st, st.by_uuid(paid).unwrap()).unwrap()[1];
        let model = (archforge_core::name_of(st, end), archforge_uml::figures::multiplicity(st, end), archforge_uml::modifiers::is_derived(st, end));
        assert_eq!(model, ("payer".to_string(), "2..5".to_string(), false), "what read is written, Modifiers not Applied are not");
        let before = revision(&h);
        let ends = h.state_mut().panels.ends.as_mut().unwrap();
        (ends.multiplicity[0], ends.refused[0]) = ("3".into(), Some("refused".into()));
        ends.multiplicity[1] = "5..2".into();
        assert_eq!(h.state_mut().clear_to_leave(None).err(), Some(crate::hold::HoldRefused::Editing), "selected, it holds work");
        click(&mut h, away);
        assert!(h.state().panels.ends.is_none() && revision(&h) == before, "a refused or unreadable value is dropped, not written");
        let read = start_replacing(&mut h, None);
        read.send(Ok(earlier)).unwrap();
        h.run_steps(4);
        assert!(!h.state().is_held() && h.state().session.as_ref().unwrap().doc().by_uuid(paid).is_none(), "replaced without it");
        assert!(h.state().panels.ends.is_none() && h.state().panels.notice.is_none());
    });
}

/// A role the model refused goes as Cancel when the selection moves on, even once nothing would
/// refuse it any more; what was tried is forgotten once the model's role changes.
#[test]
fn a_refused_role_is_not_written_when_the_selection_moves_on() {
    const ROLE: &str = "workspace.properties.end.1.role";
    /// What end `i` of `a` is called in the model.
    fn role(h: &Harness<'static, Shell>, a: archforge_mof::Uuid, i: usize) -> String {
        let st = h.state().session.as_ref().unwrap().doc();
        archforge_core::name_of(st, archforge_uml::figures::member_ends(st, st.by_uuid(a).unwrap()).unwrap()[i])
    }
    in_both(|mut h, _| {
        crate::uml_ui::open_sample(h.state_mut()).unwrap();
        h.state_mut().state.show_panel(crate::state::Panel::Properties);
        h.run();
        let paid = select_named(&mut h, archforge_uml::ASSOCIATION, "paid by");
        let name_end = |h: &mut Harness<'static, Shell>, end: usize, name: &str| {
            let params = serde_json::json!({ "association": paid.to_string(), "end": end, "role": name });
            h.state_mut().session.as_mut().unwrap().execute("uml.association.set-end", &params).unwrap();
            h.run();
        };
        name_end(&mut h, 0, "taken");
        retype(&mut h, ROLE, "taken");
        key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
        assert_eq!(role(&h, paid, 1), "payment", "refused: the other end has that role");
        name_end(&mut h, 0, "buyer");
        let away = rect_of(&h, "workspace.canvas").left_top() + egui::vec2(30.0, 30.0);
        click(&mut h, away);
        assert!(h.state().panels.ends.is_none() && role(&h, paid, 1) == "payment", "never written, though nothing refuses it now");
        select_named(&mut h, archforge_uml::ASSOCIATION, "paid by");
        retype(&mut h, ROLE, "payer");
        key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
        h.run();
        assert_eq!(role(&h, paid, 1), "payer");
        name_end(&mut h, 1, "other");
        retype(&mut h, ROLE, "payer");
        click(&mut h, away);
        assert_eq!(role(&h, paid, 1), "payer", "the model moved on, so this typing is new");
    });
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

/// A Chat turn running on a project a version can be read from.
fn turn_running(theme: ThemeKind) -> (Harness<'static, Shell>, crate::chat_fixture::Rig, std::sync::Arc<SavingHost>) {
    let (mut h, rig, host, _) = chat_and_a_version(theme);
    ask_in_chat(&mut h, "A long question");
    (h, rig, host)
}

/// That the turn is left running, not stopped, with no question up.
#[track_caller]
fn turn_left_alone(h: &Harness<'static, Shell>, rig: &crate::chat_fixture::Rig, what: &str) {
    assert_eq!(
        (h.state().ask.clone(), chat_calls(rig).2, chat_turns(h).last().unwrap().state.clone()),
        (None, 0, crate::chat_panel::TurnState::Running),
        "{what}"
    );
}

/// A Chat turn running asks before each way that can be replayed: Keep working leaves everything
/// as it was, Stop and go on runs what was asked for and the hold stops the turn; a switch asks the
/// host for its check once Chat has settled, so it has neither a read nor a restore here. Keys go to
/// the question, not the composer behind it.
#[test]
fn a_chat_turn_running_asks_first_and_the_answer_decides() {
    use crate::{close_ask::Ask, hold::HoldRefused::ChatRunning, leaving::Leaving};
    let cases: [(Way, Leaving, usize, usize, usize); 3] = [
        (ways()[0], Leaving::View { commit: "c1".into(), label: None }, 1, 0, 0),
        (ways()[2], Leaving::Restore { commit: "c1".into(), saved_at: "d".into() }, 1, 1, 0),
        (ways()[3], Leaving::Switch("x".into()), 0, 0, 0),
    ];
    in_both_appearances(|theme| {
        for ((way, go), then, reads, restores, switches) in cases.clone() {
            let (mut h, rig, host) = turn_running(theme);
            let at = rect_of(&h, "workspace.chat.composer").center();
            click(&mut h, at);
            h.event(egui::Event::Text("y".into()));
            h.run();
            assert_eq!(go(h.state_mut()), Err(ChatRunning), "{way}");
            let (path, generation) = (h.state().project.as_ref().unwrap().path.clone(), h.state().session_generation());
            assert_eq!(h.state().ask, Some(Ask::StopChat { path, generation, then: then.clone() }), "{way}");
            h.run();
            assert!(painted_has(&h, "Chat is still changing the model. Stop it and go on?") && painted_has(&h, "Its reply so far is kept."), "{way}");
            h.event(egui::Event::Text("x".into()));
            h.run();
            assert_eq!(h.state().chat_panel.draft, "y", "{way}: the composer behind the question takes no keys");
            let at = rect_of(&h, "close-ask.keep").center();
            click(&mut h, at);
            turn_left_alone(&h, &rig, way);
            nothing_began(&h, &host, way);
            assert_eq!(go(h.state_mut()), Err(ChatRunning), "{way}: asked again");
            h.run();
            let at = rect_of(&h, "close-ask.stop-chat").center();
            click(&mut h, at);
            assert!(h.state().ask.is_none() && h.state().is_held() && chat_calls(&rig).2 == 1, "{way}: the turn stops under the hold");
            assert_eq!(
                (host.reads.lock().unwrap().len(), host.restores.lock().unwrap().len(), host.switches.lock().unwrap().len()),
                (reads, restores, switches),
                "{way} runs its own operation"
            );
        }
    });
}

/// `begin_replacement` holds a receiver and a job that cannot wait for an answer: it never asks,
/// and the hold stops the turn.
#[test]
fn a_replacement_settles_a_running_turn_without_asking() {
    in_both_appearances(|theme| {
        let (mut h, rig, _host) = turn_running(theme);
        assert_eq!(h.state_mut().begin_replacement(std::sync::mpsc::channel().1, None), Ok(()));
        h.run();
        assert!(h.state().ask.is_none() && h.state().is_held() && chat_calls(&rig).2 == 1);
    });
}

/// The answer replays through the guards: an editor opened meanwhile refuses it where it is seen,
/// nothing begins, the turn runs on, and the next try asks again.
#[test]
fn stop_and_go_on_goes_through_the_guards_again() {
    use crate::hold::HoldRefused::ChatRunning;
    in_both_appearances(|theme| {
        let (mut h, rig, host) = turn_running(theme);
        assert!(!crate::version_chip::view(h.state_mut(), "c1".to_string(), "x".to_string()), "asked, not yet on its way");
        h.run();
        crate::settings::open(h.state_mut());
        let at = rect_of(&h, "close-ask.stop-chat").center();
        click(&mut h, at);
        assert_eq!(h.state().canvas.notice.as_deref(), Some(EDITING));
        turn_left_alone(&h, &rig, "refused");
        nothing_began(&h, &host, "refused");
        h.state_mut().settings = None;
        assert_eq!(h.state_mut().view_version("c1"), Err(ChatRunning), "the answer does not carry over to the next try");
    });
}

/// The chip's date goes with the question: Keep working leaves what the chip says, and Stop and go
/// on says the date of the version it was asked about.
#[test]
fn the_question_carries_the_date_the_chip_will_say() {
    in_both_appearances(|theme| {
        let (mut h, _rig, _host) = turn_running(theme);
        crate::version_chip::view(h.state_mut(), "c1".to_string(), "8 Nov 14:02".to_string());
        assert_eq!(h.state().canvas.notice, None, "the question says it");
        h.run();
        let at = rect_of(&h, "close-ask.keep").center();
        click(&mut h, at);
        assert_eq!(h.state().version_list.shown, None, "kept working: nothing remembered");
        crate::version_chip::view(h.state_mut(), "c1".to_string(), "8 Nov 14:02".to_string());
        h.run();
        let at = rect_of(&h, "close-ask.stop-chat").center();
        click(&mut h, at);
        assert_eq!(h.state().version_list.shown, Some(("c1".to_string(), "8 Nov 14:02".to_string())));
    });
}

/// A session replaced while the question is up takes it along: nothing is replayed over what shows now.
#[test]
fn a_session_replaced_takes_the_question_along() {
    in_both_appearances(|theme| {
        let (mut h, rig, host) = turn_running(theme);
        assert!(!crate::version_chip::view(h.state_mut(), "c1".to_string(), "x".to_string()));
        h.state_mut().session_replaced();
        h.run();
        turn_left_alone(&h, &rig, "gone");
        nothing_began(&h, &host, "gone");
    });
}

/// A text already refused stops the leaving anyway, so Chat isn't asked about.
#[test]
fn a_refused_description_stops_leaving_before_the_question() {
    in_both_appearances(|theme| {
        let (mut h, rig, _host) = turn_running(theme);
        let target = crate::chat_fixture::element(h.state(), archforge_uml::CLASS, "Customer").unwrap();
        h.state_mut().panels.description = Some(crate::panels::DescriptionDraft { target, text: "a\u{7}b".into(), refused: true });
        assert_eq!(h.state_mut().view_version("c1"), Err(crate::hold::HoldRefused::Description));
        turn_left_alone(&h, &rig, "no question");
    });
}

/// Where the notice stands, the toast's card or the caption.
const NOTICE: &str = "workspace.canvas.notice";

/// Opens a diagram with its minimap folded, as frame V2.4 has it, and has a step refused over it;
/// gives the canvas.
fn refused_over_a_diagram(h: &mut Harness<'static, Shell>, host: &SavingHost) -> egui::Rect {
    let id = add_diagram(h, "Alpha");
    show_diagram(h, id);
    h.state_mut().canvas.minimap_open = false;
    refused_step(h, host);
    rect_of(h, "workspace.canvas")
}

/// The centres of the amber dots the last frame painted within `area`.
fn amber_dots(h: &Harness<'static, Shell>, area: egui::Rect) -> Vec<egui::Pos2> {
    /// Adds the centre of every circle filled `amber` under `s` to `out`.
    fn walk(s: &egui::Shape, amber: egui::Color32, out: &mut Vec<egui::Pos2>) {
        match s {
            egui::Shape::Circle(c) if c.fill == amber => out.push(c.center),
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, amber, out)),
            _ => {}
        }
    }
    let amber = crate::ux::palette(h.state().state.theme == ThemeKind::Dark).amber;
    let mut out = Vec::new();
    h.output().shapes.iter().for_each(|c| walk(&c.shape, amber, &mut out));
    out.retain(|at| area.contains(*at));
    out
}

/// The colour the last frame painted `words` in.
fn ink_of(h: &Harness<'static, Shell>, words: &str) -> Option<egui::Color32> {
    /// The colour of the first text under `s` that reads `words`, if any does.
    fn walk(s: &egui::Shape, words: &str) -> Option<egui::Color32> {
        match s {
            egui::Shape::Text(t) if t.galley.text() == words => Some(t.galley.job.sections[0].format.color),
            egui::Shape::Vec(v) => v.iter().find_map(|s| walk(s, words)),
            _ => None,
        }
    }
    h.output().shapes.iter().find_map(|c| walk(&c.shape, words))
}

/// Where the last frame painted `words`.
fn box_of(h: &Harness<'static, Shell>, words: &str) -> egui::Rect {
    text_boxes(h).into_iter().find(|(t, _)| t == words).map(|(_, r)| r).unwrap_or_else(|| panic!("{:?}", painted(h)))
}

/// A refused step is frame V2.4's toast: the card centred over the toolbar, the surface colour,
/// the amber dot, the words in the text colour and read out as a status.
#[test]
fn a_refused_step_is_the_toast_over_the_toolbar() {
    in_both(|mut h, host| {
        let canvas = refused_over_a_diagram(&mut h, &host);
        let (card, p) = (rect_of(&h, NOTICE), crate::ux::palette(h.state().state.theme == ThemeKind::Dark));
        assert_eq!(card.size(), egui::vec2(440.0, 40.0));
        assert_eq!((card.center().x, card.bottom()), (canvas.center().x, canvas.bottom() - 80.0));
        assert!(card.bottom() < rect_of(&h, "workspace.tools.arrange").top(), "above the toolbar");
        assert!(rect_shapes(&h).iter().any(|(r, fill, _)| *r == card && *fill == p.surface), "the card");
        assert_eq!(amber_dots(&h, card), vec![egui::pos2(card.left() + 17.0, card.center().y)]);
        let words = box_of(&h, EDITING);
        assert_eq!((words.left(), ink_of(&h, EDITING)), (card.left() + 30.0, Some(p.text)));
        assert!(card.contains_rect(words), "{words:?} in {card:?}");
        let status = h.get_by_label(EDITING);
        assert_eq!(status.accesskit_node().role(), egui::accesskit::Role::Status);
        assert_eq!(status.accesskit_node().live(), egui::accesskit::Live::Polite);
    });
}

/// The toast stands at one place whatever the centre holds: nothing, a skill or a diagram.
#[test]
fn the_toast_stands_at_one_place_whatever_the_centre_holds() {
    in_both(|mut h, host| {
        refused_step(&mut h, &host);
        let empty = rect_of(&h, NOTICE);
        let root = crate::panels::run(h.state_mut(), "er.schema.add", serde_json::json!({ "name": "Sales" })).unwrap()["id"].clone();
        let skill = crate::panels::run(h.state_mut(), "skill.add", serde_json::json!({ "subject": root, "name": "Naming", "body": "text" })).unwrap();
        show_diagram(&mut h, skill["id"].as_str().and_then(archforge_mof::Uuid::parse).unwrap());
        refused_step(&mut h, &host);
        let over_skill = rect_of(&h, NOTICE);
        refused_over_a_diagram(&mut h, &host);
        assert_eq!((empty, over_skill), (rect_of(&h, NOTICE), rect_of(&h, NOTICE)));
    });
}

/// A long reason wraps inside the card, which grows upwards from the same foot, and is read whole.
#[test]
fn a_long_refused_step_wraps_inside_the_toast() {
    let long = "This reason takes several rows to say in the toast over the toolbar. ".repeat(4);
    in_both(|mut h, host| {
        let canvas = refused_over_a_diagram(&mut h, &host);
        h.state_mut().canvas.say_refused_step(Some(long.clone()));
        h.run_steps(2);
        let (card, words) = (rect_of(&h, NOTICE), box_of(&h, &long));
        assert!(words.height() > 2.0 * crate::ux::types::SMALL.line && card.contains_rect(words), "{words:?} in {card:?}");
        assert_eq!((card.width(), card.bottom()), (440.0, canvas.bottom() - 80.0));
        let status = h.get_by_label(&long);
        assert_eq!(status.accesskit_node().role(), egui::accesskit::Role::Status);
        assert_eq!(amber_dots(&h, card), vec![egui::pos2(card.left() + 17.0, words.top() + crate::ux::types::SMALL.line / 2.0)], "on the first row");
        h.set_size(egui::vec2(h.ctx.content_rect().width(), 340.0));
        h.run_steps(2);
        assert_eq!(rect_of(&h, NOTICE).top(), rect_of(&h, "workspace.canvas").top() + 8.0, "kept inside a short canvas");
    });
}

/// In a narrower window the toast keeps clear of the minimap and inside the canvas.
#[test]
fn the_toast_keeps_clear_of_the_minimap() {
    in_both(|mut h, host| {
        let canvas = refused_over_a_diagram(&mut h, &host);
        h.state_mut().canvas.minimap_open = true;
        let width = h.ctx.content_rect().width() - canvas.width() + 700.0;
        h.set_size(egui::vec2(width, 860.0));
        h.run_steps(2);
        let (canvas, card) = (rect_of(&h, "workspace.canvas"), rect_of(&h, NOTICE));
        let minimap = crate::canvas::minimap_box(h.state(), canvas).expect("the minimap shows");
        assert!(card.right() <= minimap.left() - 16.0 && card.left() >= canvas.left() + 16.0, "{card:?} {minimap:?} {canvas:?}");
    });
}

/// A toast with Undo at the same place, on the canvas or for a deleted conversation, stays
/// readable: the refused step stands above it.
#[test]
fn the_toast_stands_above_a_toast_with_undo() {
    in_both(|mut h, host| {
        let canvas = refused_over_a_diagram(&mut h, &host);
        let alone = rect_of(&h, NOTICE);
        let undo_label = h.state().session.as_ref().unwrap().undo_label().map(str::to_string);
        h.state_mut().canvas.toast = Some(crate::connect::Toast { text: "Removed".into(), until: u64::MAX, undo_label });
        h.run_steps(2);
        assert!(rect_of(&h, NOTICE).bottom() < rect_of(&h, "workspace.toast").top(), "over the canvas's toast");
        h.state_mut().canvas.toast = None;
        h.run_steps(2);
        assert_eq!(rect_of(&h, NOTICE), alone, "and back down when it has gone");
        let shown = h.state().project.as_ref().unwrap().path.clone();
        h.state_mut().chats.deletions.push(crate::chat_delete::Deletion {
            project: std::path::PathBuf::from("/another"),
            chat: archforge_mof::Uuid([9; 16]),
            key: "k".into(),
            origin: "Chat 1".into(),
            title: "A chat".into(),
            stage: crate::chat_delete::Stage::Deleted { before: "c1".into() },
            toast: true,
            until: None,
        });
        h.run_steps(2);
        assert_eq!(rect_of(&h, NOTICE), alone, "another project's toast is not up here");
        h.state_mut().chats.deletions[0].project = shown;
        h.run_steps(2);
        assert!(rect_of(&h, NOTICE).bottom() < rect_of(&h, "workspace.chat.toast").top(), "over the deleted conversation's toast");
        assert!(rect_of(&h, NOTICE).top() >= canvas.top());
        // A deletion still under way with its toast closed keeps its row, and raises nothing.
        let (_answer, rx) = std::sync::mpsc::channel();
        let d = &mut h.state_mut().chats.deletions[0];
        (d.toast, d.stage) = (false, crate::chat_delete::Stage::Deleting { rx });
        h.run_steps(2);
        assert_eq!(rect_of(&h, NOTICE), alone, "its toast has gone");
    });
}

/// Stop and go on, refused once the question has gone, says why in the toast too.
#[test]
fn a_way_out_refused_after_the_question_is_the_toast() {
    in_both(|mut h, _| {
        h.state_mut().session.as_mut().unwrap().begin_interaction("er.schema.add").unwrap();
        h.state_mut().go_on_leaving(crate::leaving::Leaving::View { commit: "c1".into(), label: None });
        h.run_steps(2);
        assert_eq!((rect_of(&h, NOTICE).width(), painted_times(&h, EDITING)), (440.0, 1));
    });
}

/// Anything else said after a refused step is the caption it always was, at the top left, and
/// the same goes once the notice has ended: the toast is only ever the refusal it was raised for.
#[test]
fn what_is_said_after_a_refused_step_stays_the_caption() {
    const SAID: &str = "Already on this diagram";
    in_both(|mut h, host| {
        let canvas = refused_over_a_diagram(&mut h, &host);
        let p = crate::ux::palette(h.state().state.theme == ThemeKind::Dark);
        h.state_mut().canvas.notice = Some(SAID.into());
        h.run_steps(2);
        let caption = rect_of(&h, NOTICE);
        assert_eq!((caption.left(), ink_of(&h, SAID)), (canvas.left() + 16.0, Some(p.amber)));
        assert!(caption.bottom() < canvas.center().y && amber_dots(&h, canvas).is_empty(), "{caption:?}");
        h.state_mut().canvas.notice = None;
        h.run_steps(2);
        assert!(!has_id(&h, NOTICE) && amber_dots(&h, canvas).is_empty(), "nothing is left of it");
    });
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
        shown_kept(&mut h, &host, "h3");
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
        h.state_mut().move_to_version("c2", None).unwrap();
        h.run_steps(3);
        answer_read(&mut h, &host, Ok(two.clone()));
        assert_eq!((h.state().viewing(), centre(&h)), (Some("c2"), (vec![a], Some(a))), "the version's first, not the work's");
        h.state_mut().move_to_version("c3", None).unwrap();
        h.run_steps(3);
        assert!(h.state_mut().back_to_current_work(), "the read is dropped");
        host.reads.lock().unwrap().clear();
        assert_eq!(centre(&h).1, Some(b), "current work's own");
        enter(&mut h, &host, two, "c4");
        assert_eq!(centre(&h), (vec![b], Some(b)), "what the dropped read carried is gone");
    });
}

/// Viewing `v`, of 8 Nov, which holds neither schema; gives the rows of `b`, which holds the schema
/// `b`, and of `c`, which holds `b` and `c`.
fn stepping(h: &mut Harness<'static, Shell>, host: &SavingHost) -> [std::sync::Arc<archforge_mof::rows::Rows>; 2] {
    let earlier = rows_now(h);
    add_schema(h, "b");
    let b = rows_now(h);
    add_schema(h, "c");
    let c = rows_now(h);
    crate::version_chip::view(h.state_mut(), "v".into(), "8 Nov 14:02".into());
    h.run_steps(3);
    answer_read(h, host, Ok(earlier));
    [b, c]
}

/// Steps to `commit` as the chip does, its date being `9 Nov <commit>`.
fn step_to(h: &mut Harness<'static, Shell>, commit: &str) {
    assert!(crate::version_chip::view(h.state_mut(), commit.into(), format!("9 Nov {commit}")));
    h.run_steps(3);
}

/// That `commit` shows, holding the schemas `b` and `c` or not.
#[track_caller]
fn showing(h: &Harness<'static, Shell>, commit: &str, schemas: (bool, bool)) {
    let names = schema_names(h);
    assert_eq!((h.state().viewing(), (names.contains(&"b".to_string()), names.contains(&"c".to_string()))), (Some(commit), schemas));
}

/// That what the chip and the banner say is still the version of 8 Nov.
#[track_caller]
fn dated_as_before(h: &Harness<'static, Shell>) {
    assert!(crate::version_chip::read_only_words(h.state()).contains("8 Nov 14:02"));
}

/// A step takes no hold, wakes the window, and leaves the version, its date and its view until the
/// next arrives; what was set aside stays through it.
#[test]
fn a_step_takes_no_hold_and_the_version_stays_until_the_next_arrives() {
    in_both(|mut h, host| {
        let [b, _] = stepping(&mut h, &host);
        answer_versions(&host);
        h.run_steps(3);
        step_to(&mut h, "b");
        assert!(!h.state().is_held(), "no hold");
        assert!(h.output().viewport_output[&egui::ViewportId::ROOT].repaint_delay.as_secs() < 1, "it wakes while waiting");
        showing(&h, "v", (false, false));
        dated_as_before(&h);
        answer_read(&mut h, &host, Ok(b));
        showing(&h, "b", (true, false));
        assert!(!h.state().is_held() && h.query_by_label("Viewing the version of 9 Nov b").is_some(), "the date comes with it");
        assert!(h.state_mut().back_to_current_work() && schema_names(&h).contains(&"c".to_string()), "the work comes back");
    });
}

/// One read runs at a time, an answer overtaken never shows, and the newest asked for does, its
/// read saved when it is the one already under way.
#[test]
fn an_answer_overtaken_never_shows_and_the_newest_asked_for_does() {
    in_both(|mut h, host| {
        let [b, c] = stepping(&mut h, &host);
        step_to(&mut h, "b");
        step_to(&mut h, "c");
        assert_eq!(host.reads.lock().unwrap().len(), 1, "one read at a time");
        answer_read(&mut h, &host, Ok(b.clone()));
        showing(&h, "v", (false, false));
        assert_eq!(host.reads.lock().unwrap()[0].0, "c", "the newest is read next");
        answer_read(&mut h, &host, Ok(c));
        showing(&h, "c", (true, true));
        assert!(host.reads.lock().unwrap().is_empty(), "no read more");
    });
}

/// An answer overtaken goes into the cache unseen, so that stepping to it later reads nothing,
/// and shows what it read, on the same one read path whether it is a step or an entry.
#[test]
fn an_answer_overtaken_is_kept_and_a_version_read_before_needs_no_second_read() {
    in_both(|mut h, host| {
        let [b, c] = stepping(&mut h, &host);
        step_to(&mut h, "b");
        step_to(&mut h, "c");
        answer_read(&mut h, &host, Ok(b));
        showing(&h, "v", (false, false));
        assert!(h.state().loads.kept_commits().contains(&"b"), "kept under its own hash");
        answer_read(&mut h, &host, Ok(c));
        showing(&h, "c", (true, true));
        step_to(&mut h, "b");
        shown_kept(&mut h, &host, "b");
        showing(&h, "b", (true, false));
        step_to(&mut h, "x");
        step_to(&mut h, "c");
        showing(&h, "c", (true, true));
        host.reads.lock().unwrap().clear();
        assert!(h.state_mut().back_to_current_work());
        h.state_mut().view_version("c").unwrap();
        assert_eq!(h.state().stepping_from(), None, "an entry is no step asked for");
        h.run_steps(4);
        assert!(host.reads.lock().unwrap().is_empty() && !h.state().is_held(), "an entry takes it from the cache too");
        showing(&h, "c", (true, true));
    });
}

/// A restore or a branch switch empties the cache as it starts, and the read then under way
/// neither fills it nor counts for a later step.
#[test]
fn a_restore_or_a_switch_empties_the_cache_and_the_read_under_way_stays_out() {
    let routes: [fn(&mut Shell); 2] = [|s| s.restore_version("v", "8 Nov").unwrap(), |s| s.switch_branch("other".into()).unwrap()];
    for route in routes {
        in_both(|mut h, host| {
            let [b, _] = stepping(&mut h, &host);
            step_to(&mut h, "b");
            assert_eq!(h.state().loads.kept_commits(), ["v"], "what the entry read is kept");
            route(h.state_mut());
            assert!(h.state().loads.kept_commits().is_empty(), "as it starts");
            answer_read(&mut h, &host, Ok(b));
            assert!(h.state().loads.kept_commits().is_empty(), "the read under way stays out");
        });
    }
}

/// An answer read for one project is not used for the same hash wanted of another: it is read again.
#[test]
fn an_answer_for_another_project_is_not_used_for_the_same_hash() {
    in_both(|mut h, host| {
        let [b, _] = stepping(&mut h, &host);
        step_to(&mut h, "b");
        h.state_mut().project.as_mut().unwrap().path = "/another".into();
        step_to(&mut h, "b");
        answer_read(&mut h, &host, Ok(b.clone()));
        showing(&h, "v", (false, false));
        assert_eq!(host.reads.lock().unwrap()[0].0, "b", "read for the other project");
        answer_read(&mut h, &host, Ok(b));
        showing(&h, "b", (true, false));
    });
}

/// A read from before the cache was emptied is not used even for the version it read: it is read
/// again, and only then shows.
#[test]
fn a_read_from_before_the_cache_was_emptied_is_read_again() {
    in_both(|mut h, host| {
        let [b, _] = stepping(&mut h, &host);
        step_to(&mut h, "b");
        h.state_mut().loads.clear();
        answer_read(&mut h, &host, Ok(b.clone()));
        showing(&h, "v", (false, false));
        assert_eq!(host.reads.lock().unwrap()[0].0, "b", "read afresh");
        answer_read(&mut h, &host, Ok(b));
        showing(&h, "b", (true, false));
    });
}

/// A step that fails, or whose read is lost, stays on the version in view and says why; a read
/// overtaken says nothing however it ends.
#[test]
fn a_failed_step_stays_on_the_version_in_view_and_says_why() {
    in_both(|mut h, host| {
        let _ = stepping(&mut h, &host);
        step_to(&mut h, "b");
        answer_read(&mut h, &host, Err("not readable".to_string().into()));
        showing(&h, "v", (false, false));
        dated_as_before(&h);
        assert_eq!(h.state().canvas.notice.as_deref(), Some("not readable"));
        h.run_steps(1);
        assert_eq!(rect_of(&h, NOTICE).size(), egui::vec2(440.0, 40.0), "said in the toast");
        assert!(!h.state().loads.waiting() && !h.state().is_held());
        step_to(&mut h, "b");
        assert_eq!(h.state().canvas.notice, None, "the step that goes clears the reason");
        host.reads.lock().unwrap().remove(0);
        h.run_steps(3);
        showing(&h, "v", (false, false));
        assert!(h.state().canvas.notice.as_deref().is_some_and(|n| n.contains("without an answer")), "the read was lost");
    });
}

/// Stepping back to the version in view calls off the one asked for: its answer does not show.
#[test]
fn stepping_back_to_the_version_in_view_calls_off_the_one_asked_for() {
    in_both(|mut h, host| {
        let [b, _] = stepping(&mut h, &host);
        step_to(&mut h, "b");
        step_to(&mut h, "v");
        assert_eq!(h.state().stepping_from(), Some("v"));
        answer_read(&mut h, &host, Ok(b));
        showing(&h, "v", (false, false));
        dated_as_before(&h);
        assert!(!h.state().loads.waiting() && host.reads.lock().unwrap().is_empty());
    });
}

/// The keys go from the version asked for, so two presses go two versions, and pressing back to
/// the one in view calls the step off.
#[test]
fn the_keys_step_from_the_version_asked_for() {
    in_both(|mut h, host| {
        add_schema(&mut h, "shop");
        press(&mut h, egui::Key::OpenBracket);
        answer_versions(&host);
        h.run_steps(3);
        shown(&mut h, &host, "h3");
        press(&mut h, egui::Key::OpenBracket);
        press(&mut h, egui::Key::OpenBracket);
        assert_eq!(h.state().stepping_from(), Some("h1"));
        press(&mut h, egui::Key::CloseBracket);
        press(&mut h, egui::Key::CloseBracket);
        assert_eq!(h.state().stepping_from(), Some("h3"), "back to the one in view");
        let rows = rows_now(&h);
        answer_read(&mut h, &host, Ok(rows));
        assert_eq!(h.state().viewing(), Some("h3"), "called off");
    });
}

/// A hold, going back or the project going clears the step asked for, and its answer changes nothing.
#[test]
fn a_hold_or_going_back_clears_the_step_asked_for() {
    let routes: [fn(&mut Shell); 3] = [|s| s.begin_hold().unwrap(), |s| assert!(s.back_to_current_work()), Shell::clear_workspace];
    for route in routes {
        in_both(|mut h, host| {
            let [b, _] = stepping(&mut h, &host);
            step_to(&mut h, "b");
            route(h.state_mut());
            let viewing = h.state().viewing().map(str::to_string);
            assert_eq!(h.state().stepping_from(), viewing.as_deref());
            answer_read(&mut h, &host, Ok(b));
            assert_eq!(h.state().viewing().map(str::to_string), viewing, "nothing landed");
            assert!(!h.state().loads.waiting() && host.reads.lock().unwrap().is_empty());
        });
    }
}

/// A step between versions leaves no current work: a Chat turn is neither asked about nor stopped.
#[test]
fn a_step_leaves_a_chat_turn_alone() {
    in_both_appearances(|theme| {
        let (mut h, rig, host) = turn_running(theme);
        let preview = h.state().build(&rows_now(&h)).unwrap();
        h.state_mut().show_version(preview, "v".into());
        assert_eq!(h.state_mut().move_to_version("b", None), Ok(()));
        turn_left_alone(&h, &rig, "a step");
        assert!(!h.state().is_held() && host.reads.lock().unwrap().len() == 1);
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

/// A failure that may pass, the host's limit among them, marks no row; one that lasts marks its
/// row, of the project shown only, and a newer Archforge's has no reason. Rows that cannot be built
/// mark theirs too. Picking any of them stays on the version in view and says why; showing
/// another version moves the session and keeps what is known.
#[test]
fn only_a_lasting_failure_marks_the_row() {
    use crate::VersionReadError::{Failed, Shape, Unavailable};
    let reason = |h: &Harness<'static, Shell>, hash: &str| h.state().versions.unavailable.get(hash).cloned();
    in_both(|mut h, host| {
        let [b, _] = stepping(&mut h, &host);
        step_to(&mut h, "x");
        answer_read(&mut h, &host, Err(Failed("Too many".into())));
        assert_eq!((reason(&h, "x"), h.state().canvas.notice.as_deref()), (None, Some("Too many")), "it may pass");
        step_to(&mut h, "x");
        answer_read(&mut h, &host, Err(Unavailable));
        assert_eq!(reason(&h, "x"), Some(Some(Unavailable.to_string())));
        assert_ne!(Shape.to_string(), Unavailable.to_string(), "two reasons");
        assert_eq!(h.state().canvas.notice, Some(Unavailable.to_string()), "says why");
        showing(&h, "v", (false, false));
        step_to(&mut h, "x");
        assert_eq!(host.reads.lock().unwrap().remove(0).0, "x", "a marked version can still be picked");
        h.run_steps(3);
        assert!(h.state().canvas.notice.is_some(), "and that pick says why");
        step_to(&mut h, "n");
        answer_read(&mut h, &host, Err(Shape));
        assert_eq!((reason(&h, "n"), h.state().canvas.notice.clone()), (Some(None), Some(Shape.to_string())));
        step_to(&mut h, "unbuilt");
        answer_read(&mut h, &host, Ok(Default::default()));
        showing(&h, "v", (false, false));
        assert!(reason(&h, "unbuilt").is_some_and(|why| why.is_some()), "rows that cannot be built never will be");
        step_to(&mut h, "b");
        answer_read(&mut h, &host, Ok(b));
        h.run_steps(2);
        showing(&h, "b", (true, false));
        assert!(reason(&h, "x").is_some(), "showing a version keeps what is known of the others");
        step_to(&mut h, "other");
        h.state_mut().project.as_mut().unwrap().path = "/elsewhere".into();
        h.run_steps(2);
        answer_read(&mut h, &host, Err(Unavailable));
        assert_eq!(reason(&h, "other"), None, "another project's rows are not these");
    });
}

/// Entering a version that cannot be read or built marks its row and leaves the work as it was.
#[test]
fn an_entry_that_fails_for_good_marks_the_row() {
    use crate::VersionReadError::Unavailable;
    in_both(|mut h, host| {
        crate::version_chip::view(h.state_mut(), "gone".into(), "8 Nov".into());
        h.run_steps(3);
        answer_read(&mut h, &host, Err(Unavailable));
        assert_eq!(h.state().viewing(), None);
        crate::version_chip::view(h.state_mut(), "unbuilt".into(), "8 Nov".into());
        h.run_steps(3);
        answer_read(&mut h, &host, Ok(Default::default()));
        h.run_steps(3);
        assert_eq!(h.state().viewing(), None);
        assert!(["gone", "unbuilt"].iter().all(|c| h.state().versions.unavailable[*c].is_some()));
    });
}

/// A host that gives no ids says nothing of a version's rows: entering or stepping still fails
/// and says why, and no row is marked.
#[test]
fn a_host_without_ids_marks_no_row() {
    in_both(|mut h, host| {
        let _ = stepping(&mut h, &host);
        h.state_mut().services.ids = None;
        step_to(&mut h, "x");
        answer_read(&mut h, &host, Ok(Default::default()));
        assert!(h.state().canvas.notice.is_some(), "the step says why");
        h.state_mut().back_to_current_work();
        h.run_steps(2);
        crate::version_chip::view(h.state_mut(), "y".into(), "8 Nov".into());
        h.run_steps(3);
        answer_read(&mut h, &host, Ok(Default::default()));
        h.run_steps(3);
        assert_eq!(h.state().viewing(), None);
        assert!(h.state().versions.unavailable.is_empty());
    });
}

/// What is known of a version ends with the workspace and with a restore or a branch switch, as
/// it holds of a branch; entering a version touches neither.
#[test]
fn a_reason_ends_when_the_branch_may_differ() {
    let enders: [fn(&mut Shell); 3] = [
        |s| s.restore_version("c1", "2026-11-08T14:02:11Z").unwrap(),
        |s| s.switch_branch("elsewhere".to_string()).unwrap(),
        |s| s.clear_workspace(),
    ];
    for end in enders {
        in_both(|mut h, _| {
            h.state_mut().versions.unavailable.insert("a".into(), None);
            end(h.state_mut());
            assert!(h.state().versions.unavailable.is_empty());
        });
    }
    in_both(|mut h, _| {
        h.state_mut().versions.unavailable.insert("a".into(), None);
        h.state_mut().view_version("b").unwrap();
        assert_eq!(h.state().versions.unavailable.len(), 1, "an entry is no change of branch");
    });
}

/// What a step past `from` to `to` says, of two of [`answer_versions`]'s dates.
fn skip_words(h: &Harness<'static, Shell>, from: usize, to: usize) -> String {
    let clock = crate::chat_panel::clock(h.state());
    let date = |n: usize| crate::version_chip::when(&format!("2026-11-08T14:{n:02}:00Z"), clock);
    crate::i18n::fmt(&crate::i18n::tr("{from} can't be shown. Skipped to {to}."), &[("from", &date(from)), ("to", &date(to))])
}

/// Marks the version `hash` as one that can't be shown.
fn mark(h: &mut Harness<'static, Shell>, hash: &str) {
    h.state_mut().versions.unavailable.insert(hash.into(), None);
}

/// Unsaved work, so that going older from current work starts at the newest version.
fn unsaved_then_older(h: &mut Harness<'static, Shell>, host: &SavingHost) {
    add_schema(h, "shop");
    press(h, egui::Key::OpenBracket);
    answer_versions(host);
    h.run_steps(3);
}

/// That the toast says `words`, once.
#[track_caller]
fn toast_says(h: &Harness<'static, Shell>, words: &str) {
    assert_eq!(h.state().canvas.notice.as_deref(), Some(words));
    assert_eq!(h.state().canvas.refused_step.as_deref(), Some(words), "as the toast");
    assert_eq!(painted_times(h, words), 1, "{:?}", painted(h));
}

/// A step passes what can't be shown, either way and from current work, and says so only once it
/// has arrived; with nothing else that way it goes nowhere, whichever end is unavailable.
#[test]
fn stepping_skips_what_cannot_be_shown_and_says_so_on_arrival() {
    in_both(|mut h, host| {
        mark(&mut h, "h3");
        unsaved_then_older(&mut h, &host);
        assert_eq!(h.state().canvas.notice, None, "not before it has arrived");
        shown(&mut h, &host, "h2");
        toast_says(&h, &skip_words(&h, 3, 2));
        press(&mut h, egui::Key::CloseBracket);
        assert_eq!((h.state().viewing(), host.reads.lock().unwrap().len()), (Some("h2"), 0), "the newest can't be shown");
        assert!(h.get_by_label("Newer version").accesskit_node().is_disabled(), "and the chip says there is nowhere to go");
        h.state_mut().versions.unavailable.clear();
        press(&mut h, egui::Key::CloseBracket);
        assert_eq!(h.state().canvas.notice, None, "the step that goes ends what was said");
        shown(&mut h, &host, "h3");
        assert_eq!(h.state().canvas.notice, None, "nothing was passed");
        mark(&mut h, "h2");
        press(&mut h, egui::Key::OpenBracket);
        assert_eq!(h.state().canvas.notice, None, "not before it has arrived");
        shown(&mut h, &host, "h1");
        toast_says(&h, &skip_words(&h, 2, 1));
        press(&mut h, egui::Key::CloseBracket);
        shown_kept(&mut h, &host, "h3");
        toast_says(&h, &skip_words(&h, 2, 3));
        mark(&mut h, "h1");
        press(&mut h, egui::Key::OpenBracket);
        assert_eq!((h.state().viewing(), host.reads.lock().unwrap().len()), (Some("h3"), 0), "the oldest can't be shown either");
        assert!(h.get_by_label("Older version").accesskit_node().is_disabled());
    });
}

/// Passing several, the words name the first; and all passed is nowhere to go.
#[test]
fn a_skip_past_several_names_the_first() {
    in_both(|mut h, host| {
        ["h3", "h2"].iter().for_each(|hash| mark(&mut h, hash));
        unsaved_then_older(&mut h, &host);
        shown(&mut h, &host, "h1");
        toast_says(&h, &skip_words(&h, 3, 1));
    });
    in_both(|mut h, host| {
        ["h3", "h2", "h1"].iter().for_each(|hash| mark(&mut h, hash));
        unsaved_then_older(&mut h, &host);
        assert_eq!((h.state().viewing(), host.reads.lock().unwrap().len()), (None, 0), "nothing to show");
    });
}

/// What a step passed is said of that step alone: a read that fails or a person going back ends
/// it, and the version showing later by another way says nothing of it.
#[test]
fn a_skip_that_never_arrives_is_not_said() {
    for gone_back in [false, true] {
        in_both(|mut h, host| {
            mark(&mut h, "h3");
            unsaved_then_older(&mut h, &host);
            if gone_back {
                h.state_mut().back_to_current_work();
            } else {
                answer_read(&mut h, &host, Err("old".to_string().into()));
            }
            h.run_steps(2);
            assert!(h.state().loads.skipped.is_none(), "over with the step");
            crate::version_chip::view(h.state_mut(), "h2".into(), "8 Nov".into());
            h.run_steps(3);
            shown(&mut h, &host, "h2");
            assert_eq!(h.state().canvas.notice, None, "said of no later arrival");
        });
    }
}

/// Viewing `v1`, the last of the two versions read with older pages to read; the History panel's
/// read answered too.
fn at_the_end_of_what_is_read(h: &mut Harness<'static, Shell>, host: &SavingHost) {
    press(h, egui::Key::OpenBracket);
    answer(h, host, Ok(page(0, 2, true)));
    shown(h, host, "v1");
    answer(h, host, Ok(page(0, 2, true)));
}

/// The `after` of the last read of versions asked for.
fn last_asked(host: &SavingHost) -> Option<String> {
    host.version_asks.lock().unwrap().last().unwrap().clone()
}

/// A step past what is read reads the next page and goes on, one step and one read however many
/// presses; a step newer than the newest reads nothing, older pages or not; ‹ is live until the
/// history has really ended.
#[test]
fn a_step_past_what_is_read_reads_on() {
    use egui::Key::{CloseBracket, OpenBracket};
    in_both(|mut h, host| {
        at_the_end_of_what_is_read(&mut h, &host);
        assert!(!h.get_by_label("Older version").accesskit_node().is_disabled(), "older pages exist");
        press(&mut h, CloseBracket);
        shown(&mut h, &host, "v0");
        answer(&mut h, &host, Ok(page(0, 2, true)));
        press(&mut h, CloseBracket);
        assert!(host.version_reads.lock().unwrap().is_empty(), "nothing is newer than the newest");
        press(&mut h, OpenBracket);
        shown_kept(&mut h, &host, "v1");
        answer(&mut h, &host, Ok(page(0, 2, true)));
        press(&mut h, OpenBracket);
        assert_eq!(last_asked(&host).as_deref(), Some("v1"), "the page after what is read");
        press(&mut h, OpenBracket);
        assert_eq!(host.version_reads.lock().unwrap().len(), 1, "one read, however many presses");
        assert!(host.reads.lock().unwrap().is_empty(), "nothing to go to yet");
        answer(&mut h, &host, Ok(page(2, 4, false)));
        shown(&mut h, &host, "v2");
        answer(&mut h, &host, Ok(page(0, 4, false)));
        press(&mut h, OpenBracket);
        shown(&mut h, &host, "v3");
        answer(&mut h, &host, Ok(page(0, 4, false)));
        assert!(h.get_by_label("Older version").accesskit_node().is_disabled(), "the history has ended");
        press(&mut h, OpenBracket);
        assert!(host.version_reads.lock().unwrap().is_empty() && host.reads.lock().unwrap().is_empty(), "an end does nothing");
    });
}

/// From a version further down than what is read, as History can reach, a step either way reads
/// until it finds where it stands.
#[test]
fn a_step_from_a_version_not_read_yet_reads_until_it_is_found() {
    for (key, to) in [(egui::Key::CloseBracket, "v2"), (egui::Key::OpenBracket, "v4")] {
        in_both(|mut h, host| {
            h.state_mut().view_version("v3").unwrap();
            let rows = rows_now(&h);
            answer_read(&mut h, &host, Ok(rows));
            h.run_steps(2);
            answer(&mut h, &host, Ok(page(0, 2, true)));
            press(&mut h, key);
            assert_eq!(last_asked(&host).as_deref(), Some("v1"));
            answer(&mut h, &host, Ok(page(2, 5, false)));
            shown(&mut h, &host, to);
        });
    }
}

/// Before the first read has answered the end isn't known: ‹ is live, and a press is the step taken
/// when the read that is already on its way answers.
#[test]
fn a_press_during_the_first_read_waits_for_it() {
    in_both(|mut h, host| {
        h.state_mut().view_version("v1").unwrap();
        let rows = rows_now(&h);
        answer_read(&mut h, &host, Ok(rows));
        h.run_steps(2);
        assert!(!h.get_by_label("Older version").accesskit_node().is_disabled(), "not told as the end before it is known");
        let asked = host.version_asks.lock().unwrap().len();
        press(&mut h, egui::Key::OpenBracket);
        assert_eq!(host.version_asks.lock().unwrap().len(), asked, "the read on its way serves it");
        answer(&mut h, &host, Ok(page(0, 3, false)));
        shown(&mut h, &host, "v2");
    });
}

/// Reading on passes whole pages of what can't be shown and names the first it passed; an empty
/// page ends it, whatever it says.
#[test]
fn a_step_reads_on_past_pages_that_cannot_be_shown() {
    in_both(|mut h, host| {
        at_the_end_of_what_is_read(&mut h, &host);
        ["v2", "v3"].iter().for_each(|hash| mark(&mut h, hash));
        press(&mut h, egui::Key::OpenBracket);
        answer(&mut h, &host, Ok(page(2, 4, true)));
        assert_eq!(last_asked(&host).as_deref(), Some("v3"), "nothing to show yet, so on again");
        assert!(host.reads.lock().unwrap().is_empty());
        answer(&mut h, &host, Ok(page(4, 5, false)));
        shown(&mut h, &host, "v4");
        let clock = crate::chat_panel::clock(h.state());
        let date = |n: usize| crate::version_chip::when(&page(n, n + 1, false).versions[0].date, clock);
        toast_says(&h, &crate::i18n::fmt(&crate::i18n::tr("{from} can't be shown. Skipped to {to}."), &[("from", &date(2)), ("to", &date(4))]));
    });
    in_both(|mut h, host| {
        at_the_end_of_what_is_read(&mut h, &host);
        press(&mut h, egui::Key::OpenBracket);
        answer(&mut h, &host, Ok(crate::VersionsPage { versions: Vec::new(), more: true }));
        assert!(host.version_reads.lock().unwrap().is_empty() && host.reads.lock().unwrap().is_empty());
        assert!(h.get_by_label("Older version").accesskit_node().is_disabled(), "the history has ended");
    });
}

/// A step whose read fails says so and stays; it isn't told as the end, the next press reads that
/// page again and what was said ends as it starts. Words said by others are left alone.
#[test]
fn a_step_whose_read_fails_says_so_and_the_next_reads_again() {
    use crate::version_chip::failure_words;
    in_both(|mut h, host| {
        at_the_end_of_what_is_read(&mut h, &host);
        h.state_mut().canvas.say_refused_step(Some("Said by another".into()));
        press(&mut h, egui::Key::OpenBracket);
        assert_eq!(h.state().canvas.notice.as_deref(), Some("Said by another"), "a read ends only its own words");
        answer(&mut h, &host, Err("no".into()));
        toast_says(&h, &failure_words(false));
        assert_eq!(h.state().viewing(), Some("v1"));
        assert!(!h.get_by_label("Older version").accesskit_node().is_disabled(), "not told as the end");
        press(&mut h, egui::Key::OpenBracket);
        assert_eq!(h.state().canvas.notice, None, "reading again ends it");
        assert_eq!(last_asked(&host).as_deref(), Some("v1"), "that page again");
        answer(&mut h, &host, Ok(page(2, 3, false)));
        shown(&mut h, &host, "v2");
    });
}

/// With nothing read, the words are of the newest versions; a read that then finds none ends them,
/// and so does a save, which makes what was read old.
#[test]
fn a_first_read_that_fails_says_so_and_its_words_end() {
    use crate::version_chip::failure_words;
    in_both(|mut h, host| {
        press(&mut h, egui::Key::OpenBracket);
        answer(&mut h, &host, Err("no".into()));
        toast_says(&h, &failure_words(true));
        assert_eq!(h.state().viewing(), None);
        press(&mut h, egui::Key::OpenBracket);
        answer(&mut h, &host, Ok(crate::VersionsPage::default()));
        assert_eq!(h.state().canvas.notice, None, "the read that followed found nothing to step to");
        press(&mut h, egui::Key::OpenBracket);
        answer(&mut h, &host, Err("no".into()));
        toast_says(&h, &failure_words(true));
        // Unsaved work, which Save makes a version of (with none it asks for nothing).
        h.state_mut().session.as_mut().unwrap().mark_unsaved();
        h.state_mut().save();
        host.saves.lock().unwrap().pop().unwrap().send(Ok(true)).unwrap();
        h.run_steps(3);
        assert_eq!(h.state().canvas.notice, None, "a finished save ends it");
    });
}

/// What covers the canvas says the failure too, as the toast there would go unseen.
#[test]
fn a_failed_step_is_said_over_a_versions_details() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let entry = version_row("a", None, (None, false), "2026-11-08T15:30:00Z");
        let (mut h, host) = versions_listed(theme, vec![entry.clone()]);
        crate::version_details::open(h.state_mut(), entry);
        h.run_steps(2);
        press(&mut h, egui::Key::OpenBracket);
        answer(&mut h, &host, Err("no".into()));
        assert!(has_id(&h, "workspace.version-details.view"), "the details are still there");
        assert_eq!(painted_times(&h, &crate::version_chip::failure_words(true)), 1, "{:?}", painted(&h));
    }
}

/// A step that waited is over once the person has moved, by going back or by a step the other way:
/// its read then moves nothing and says nothing, whichever way it ends.
#[test]
fn a_step_waiting_ends_when_the_person_moves() {
    for reply in [Ok(page(2, 3, false)), Err("no".to_string())] {
        in_both(|mut h, host| {
            at_the_end_of_what_is_read(&mut h, &host);
            press(&mut h, egui::Key::OpenBracket);
            h.state_mut().back_to_current_work();
            h.run_steps(2);
            answer(&mut h, &host, reply.clone());
            assert!(h.state().viewing().is_none() && !h.state().is_held(), "it stays on current work, entering nothing");
            assert_eq!(h.state().canvas.notice, None);
        });
        in_both(|mut h, host| {
            at_the_end_of_what_is_read(&mut h, &host);
            press(&mut h, egui::Key::OpenBracket);
            press(&mut h, egui::Key::CloseBracket);
            answer(&mut h, &host, reply.clone());
            assert_eq!(h.state().canvas.notice, None);
            shown(&mut h, &host, "v0");
            assert!(host.reads.lock().unwrap().is_empty(), "only the step taken since");
        });
    }
}

/// A step that waited stays over when the person comes back to where they asked it from: the read
/// then takes them nowhere without another press.
#[test]
fn a_step_waiting_stays_over_after_leaving_and_coming_back() {
    in_both(|mut h, host| {
        at_the_end_of_what_is_read(&mut h, &host);
        press(&mut h, egui::Key::OpenBracket);
        press(&mut h, egui::Key::CloseBracket);
        assert_eq!(h.state().loads.wanted_commit(), Some("v0"), "it left");
        press(&mut h, egui::Key::OpenBracket);
        assert_eq!((h.state().viewing(), h.state().loads.wanted_commit()), (Some("v1"), None), "and came back");
        answer(&mut h, &host, Ok(page(2, 3, false)));
        assert_eq!((h.state().viewing(), h.state().loads.wanted_commit()), (Some("v1"), None), "nothing further was asked for");
    });
}

/// Words said since the failure, which leave the toast's own field behind, outlast a new read.
#[test]
fn a_new_read_leaves_what_was_said_since_a_failed_step() {
    in_both(|mut h, host| {
        at_the_end_of_what_is_read(&mut h, &host);
        press(&mut h, egui::Key::OpenBracket);
        answer(&mut h, &host, Err("no".into()));
        toast_says(&h, &crate::version_chip::failure_words(false));
        h.state_mut().canvas.notice = Some("Said since".into());
        press(&mut h, egui::Key::OpenBracket);
        assert_eq!(last_asked(&host).as_deref(), Some("v1"), "a new read began");
        assert_eq!(h.state().canvas.notice.as_deref(), Some("Said since"));
    });
}

/// The failed-load banner and its Retry, by the ids `versions` gives them.
const LOAD_FAILED: &str = "workspace.versions.load.failed";
const LOAD_RETRY: &str = "workspace.versions.load.retry";

/// History open and listing a version, the read of `b` failed in a way that may pass, from `a`
/// when `stepped` and otherwise from current work.
fn load_failed(theme: ThemeKind, stepped: bool) -> (Harness<'static, Shell>, std::sync::Arc<SavingHost>) {
    let (mut h, host) = versions_listed(theme, vec![version_row("a", None, (None, false), "2026-11-08T15:30:00Z")]);
    if stepped {
        crate::version_chip::view(h.state_mut(), "a".into(), "8 Nov".into());
        h.run_steps(3);
        let rows = rows_now(&h);
        answer_read(&mut h, &host, Ok(rows));
        h.run_steps(3);
        host.reads.lock().unwrap().clear();
    }
    assert!(crate::version_chip::view(h.state_mut(), "b".into(), "9 Nov b".into()));
    h.run_steps(3);
    answer_read(&mut h, &host, Err(crate::VersionReadError::Failed("Too many".into())));
    h.run_steps(3);
    assert!(has_id(&h, LOAD_FAILED) && painted(&h).iter().any(|t| t == "This version couldn't be read."), "{theme:?}: {:?}", painted(&h));
    (h, host)
}

/// The banner's Retry reads the same version again and then says the date it was asked for with;
/// failing again brings the banner back.
#[test]
fn the_load_banner_retries_the_same_version() {
    in_both_appearances(|theme| {
        for stepped in [false, true] {
            let (mut h, host) = load_failed(theme, stepped);
            click_id(&mut h, LOAD_RETRY);
            assert_eq!(host.reads.lock().unwrap().first().map(|r| r.0.clone()).as_deref(), Some("b"), "{theme:?}: the same version");
            assert!(!has_id(&h, LOAD_FAILED) && h.state().versions.load_failed.is_none(), "{theme:?}: gone once it reads");
            answer_read(&mut h, &host, Err("again".to_string().into()));
            h.run_steps(3);
            assert!(has_id(&h, LOAD_FAILED), "{theme:?}: back when it fails again");
            click_id(&mut h, LOAD_RETRY);
            let rows = rows_now(&h);
            answer_read(&mut h, &host, Ok(rows));
            h.run_steps(3);
            assert_eq!(h.state().viewing(), Some("b"), "{theme:?}");
            assert!(crate::version_chip::read_only_words(h.state()).contains("9 Nov b"), "{theme:?}: its date");
        }
        // A version asked for with no date does not take the date of one asked for before.
        let (mut h, host) = load_failed(theme, false);
        h.state_mut().view_version("c").unwrap();
        h.run_steps(3);
        answer_read(&mut h, &host, Err("gone".to_string().into()));
        assert_eq!(h.state().versions.load_failed.as_ref().map(|f| f.label.clone()), Some(None), "{theme:?}");
    });
}

/// Retry is a move too: a step waiting for a read is over, even when the retry fails and leaves the
/// person where they asked the step from.
#[test]
fn a_retry_ends_a_step_waiting_for_a_read() {
    in_both_appearances(|theme| {
        let (mut h, host) = load_failed(theme, true);
        let page = |hash: &str| crate::VersionsPage { versions: vec![version_row(hash, None, (None, false), "2026-11-08T15:30:00Z")], more: true };
        answer(&mut h, &host, Ok(page("a")));
        press(&mut h, egui::Key::OpenBracket);
        assert_eq!(last_asked(&host).as_deref(), Some("a"), "{theme:?}: the step reads on");
        click_id(&mut h, LOAD_RETRY);
        answer_read(&mut h, &host, Err("again".to_string().into()));
        assert_eq!((h.state().viewing(), h.state().loads.wanted_commit()), (Some("a"), None), "{theme:?}: back where the step was asked");
        answer(&mut h, &host, Ok(page("c")));
        assert!(h.state().loads.wanted_commit().is_none() && host.reads.lock().unwrap().is_empty(), "{theme:?}: no step taken");
    });
}

/// A refused Retry leaves the banner up and says why as a step refused.
#[test]
fn a_refused_retry_leaves_the_banner() {
    in_both_appearances(|theme| {
        let (mut h, host) = load_failed(theme, false);
        h.state_mut().session.as_mut().unwrap().begin_interaction("er.schema.add").unwrap();
        click_id(&mut h, LOAD_RETRY);
        assert!(has_id(&h, LOAD_FAILED) && host.reads.lock().unwrap().is_empty(), "{theme:?}: refused");
        assert_eq!(h.state().canvas.notice.as_deref(), Some(EDITING), "{theme:?}: says why");
        assert_eq!(h.state().canvas.refused_step.as_deref(), Some(EDITING), "{theme:?}: as a step refused");
        h.state_mut().session.as_mut().unwrap().cancel().unwrap();
        click_id(&mut h, LOAD_RETRY);
        assert!(!has_id(&h, LOAD_FAILED) && h.state().canvas.notice.is_none(), "{theme:?}: goes once it may");
    });
}

/// Only a failure that may pass, of the project shown and for the version asked for, puts it up.
#[test]
fn only_a_failure_that_may_pass_offers_retry() {
    use crate::VersionReadError::{Failed, Shape, Unavailable};
    in_both(|mut h, host| {
        let _ = stepping(&mut h, &host);
        for lasting in [Shape, Unavailable] {
            step_to(&mut h, "b");
            answer_read(&mut h, &host, Err(lasting));
            assert!(h.state().versions.load_failed.is_none());
        }
        step_to(&mut h, "x");
        step_to(&mut h, "y");
        answer_read(&mut h, &host, Err(Failed("overtaken".into())));
        assert!(h.state().versions.load_failed.is_none(), "not the version asked for now");
        answer_read(&mut h, &host, Err(Failed("may pass".into())));
        let failed = h.state().versions.load_failed.clone().expect("the version asked for");
        assert_eq!((failed.commit.as_str(), failed.label.as_deref(), failed.why.as_str()), ("y", Some("9 Nov y"), "may pass"));
        step_to(&mut h, "z");
        h.state_mut().project.as_mut().unwrap().path = "/elsewhere".into();
        h.run_steps(2);
        answer_read(&mut h, &host, Err(Failed("late".into())));
        assert!(h.state().versions.load_failed.is_none(), "another project's");
    });
}

/// The banner ends with a step elsewhere or to what shows, going back, another entry, the project
/// or branch going, and only then: ⇧⌘H with nothing in view moves nothing.
#[test]
fn the_load_banner_ends_on_every_path() {
    let enders: [(bool, fn(&mut Shell)); 7] = [
        (false, |s| _ = crate::version_chip::view(s, "c".into(), "9 Nov c".into())),
        (false, |s| s.restore_version("c1", "2026-11-08T14:02:11Z").unwrap()),
        (false, |s| s.switch_branch("elsewhere".to_string()).unwrap()),
        (false, |s| s.clear_workspace()),
        (true, |s| _ = crate::version_chip::view(s, "c".into(), "9 Nov c".into())),
        (true, |s| _ = crate::version_chip::view(s, "a".into(), "8 Nov".into())),
        (true, |s| _ = s.back_to_current_work()),
    ];
    in_both_appearances(|theme| {
        for (stepped, end) in enders {
            let (mut h, _host) = load_failed(theme, stepped);
            end(h.state_mut());
            assert!(h.state().versions.load_failed.is_none(), "{theme:?}");
        }
        let (mut h, _host) = load_failed(theme, true);
        h.state_mut().project.as_mut().unwrap().path = "/elsewhere".into();
        h.run_steps(3);
        assert!(h.state().versions.load_failed.is_none(), "{theme:?}: the project changed");
        let (mut h, _host) = load_failed(theme, false);
        assert!(!h.state_mut().back_to_current_work(), "{theme:?}: nothing in view");
        h.run_steps(2);
        assert!(has_id(&h, LOAD_FAILED), "{theme:?}: the banner stays");
    });
}
