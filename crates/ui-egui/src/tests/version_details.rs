use egui_kittest::kittest::{NodeT, Queryable};

use super::*;

const VIEW: &str = "workspace.version-details.view";

/// Three versions, newest first, each naming the one before it; the last is the first version.
fn history() -> Vec<crate::VersionEntry> {
    let mut newest = version_row("c3", Some("Payment methods split out"), (Some("Dejan"), false), "2026-11-07T16:15:00Z");
    let mut middle = version_row("c2", Some("First classes"), (None, false), "2026-11-07T08:20:00Z");
    let first = version_row("c1", None, (None, false), "2026-11-06T15:05:00Z");
    (newest.previous, middle.previous) = (Some("c2".into()), Some("c1".into()));
    vec![newest, middle, first]
}

/// Runs `test` over a listed history, once in each appearance.
fn in_both(test: impl Fn(&mut Harness<'static, Shell>, &SavingHost)) {
    in_both_appearances(|theme| {
        let (mut h, host) = versions_listed(theme, history());
        host.version_reads.lock().unwrap().clear();
        host.reads.lock().unwrap().clear();
        test(&mut h, &host);
    });
}

/// Clicks a control.
fn tap(h: &mut Harness<'static, Shell>, id: &str) {
    let at = rect_of(h, id).center();
    click(h, at);
}

/// Opens the details of the version `row`.
fn opened(h: &mut Harness<'static, Shell>, row: &str) {
    tap(h, &format!("workspace.versions.row.{row}"));
    h.run_steps(1);
}

/// The pointer on View shows `words`.
fn said_on_hover(h: &mut Harness<'static, Shell>, words: &str) {
    let at = rect_of(h, VIEW).center();
    h.event(egui::Event::PointerMoved(at));
    h.run_steps(4);
    assert!(painted_has(h, words), "{:?}", painted(h));
}

/// Answers the read of `commit`.
fn answer(host: &SavingHost, commit: &str, rows: Result<std::sync::Arc<archforge_mof::rows::Rows>, String>) {
    let mut reads = host.reads.lock().unwrap();
    let at = reads.iter().position(|(c, _)| c == commit).unwrap_or_else(|| panic!("no read of {commit}"));
    reads.remove(at).1.send(rows).unwrap();
}

/// What the host has been asked to read and has not answered yet.
fn asked(host: &SavingHost) -> Vec<String> {
    host.reads.lock().unwrap().iter().map(|(c, _)| c.clone()).collect()
}

/// Frames until `done`; a bound only against a hang.
fn frames_until(h: &mut Harness<'static, Shell>, done: impl Fn(&Harness<'static, Shell>) -> bool) {
    let until = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while !done(h) {
        assert!(std::time::Instant::now() < until, "still waiting");
        h.run_steps(1);
    }
    h.run_steps(1);
}

/// Frames until the comparison on another thread has answered.
fn settled(h: &mut Harness<'static, Shell>) {
    frames_until(h, |h| !has_id(h, "workspace.version-details.loading"));
}

/// Frames until the working changes list has its answer: an edit's answer moves the rows below it.
fn list_still(h: &mut Harness<'static, Shell>) {
    h.run_steps(2);
    frames_until(h, |h| h.state().versions.comparing.is_none());
}

/// What a version is, when and by whom; "first version" only for the first, nothing invented.
#[test]
fn details_say_what_a_version_is() {
    const FIRST: &str = "First version: there is nothing earlier to compare it with.";
    in_both(|h, _| {
        assert!(!has_id(h, VIEW), "no version is chosen yet");
        opened(h, "c3");
        assert_eq!(h.state().versions.selected.as_deref(), Some("c3"));
        let meta = "7 Nov 17:15 · Dejan";
        let times = |h: &Harness<'static, Shell>, t: &str| painted(h).iter().filter(|p| *p == t).count();
        assert_eq!(times(h, "Payment methods split out"), 2, "the row and the title: {:?}", painted(h));
        assert_eq!(times(h, meta), 2, "the date and the author, in the row and under the title");
        assert!(!painted_has(h, FIRST), "it has a version before it");
        opened(h, "c1");
        assert!(painted_has(h, FIRST), "{:?}", painted(h));
        assert_eq!(times(h, "No description"), 2, "said for no description, and no author made up");
    });
}

/// View closes the details and shows the version; while one shows, its own View only closes them.
#[test]
fn view_shows_the_version_from_its_details() {
    in_both(|h, host| {
        opened(h, "c2");
        host.reads.lock().unwrap().clear();
        tap(h, VIEW);
        assert!(has_id(h, "workspace.tabs.add") && !has_id(h, VIEW), "the canvas, where a refusal is said");
        assert_eq!(host.reads.lock().unwrap().iter().map(|(c, _)| c.as_str()).collect::<Vec<_>>(), ["c2"]);
        let rows = rows_now(h);
        answer_read(h, host, Ok(rows));
        assert_eq!(h.state().viewing(), Some("c2"));
        assert!(painted(h).iter().any(|t| t.starts_with("Viewing the version of 7 Nov")), "named by its date: {:?}", painted(h));
        // The panel reads its list again for the session shown.
        for read in host.version_reads.lock().unwrap().drain(..) {
            let _ = read.send(Ok(crate::VersionsPage { versions: history(), more: false }));
        }
        h.run_steps(3);
        opened(h, "c2");
        host.reads.lock().unwrap().clear();
        tap(h, VIEW);
        assert!(!has_id(h, VIEW) && host.reads.lock().unwrap().is_empty(), "nothing read again");
        assert_eq!(h.state().viewing(), Some("c2"));
    });
}

/// View is off while a hold runs: it starts nothing and the details stay.
#[test]
fn view_waits_for_a_change_under_way() {
    in_both(|h, host| {
        opened(h, "c2");
        h.state_mut().view_version("c9").unwrap();
        h.run_steps(2);
        let asked = host.reads.lock().unwrap().len();
        tap(h, VIEW);
        assert_eq!(host.reads.lock().unwrap().len(), asked, "nothing started");
        assert_eq!(h.state().version_in_view(), Some("c9"));
        assert!(has_id(h, VIEW), "and they are not closed for nothing");
        said_on_hover(h, "Wait for the change under way to finish.");
    });
}

/// With an edit under way View is off and says so on hover, rather than refusing on a hidden canvas.
#[test]
fn view_says_why_an_edit_keeps_it_off() {
    in_both(|h, host| {
        opened(h, "c2");
        h.state_mut().session.as_mut().unwrap().begin_interaction("er.schema.add").unwrap();
        h.run_steps(2);
        let asked = host.reads.lock().unwrap().len();
        tap(h, VIEW);
        assert_eq!(host.reads.lock().unwrap().len(), asked, "nothing started");
        assert!(has_id(h, VIEW), "the details stay");
        said_on_hover(h, "Finish or cancel your edit first.");
    });
}

/// The project's name closes the details; the rail gives the centre back and brings them again;
/// opening a diagram or another session ends them.
#[test]
fn details_give_way_to_the_canvas() {
    in_both(|h, _| {
        opened(h, "c3");
        tap(h, "workspace.version-details.close");
        assert!(!has_id(h, VIEW) && has_id(h, "workspace.tabs.add"));
        opened(h, "c3");
        tap(h, "workspace.explorers.models");
        assert!(!has_id(h, VIEW) && has_id(h, "workspace.tabs.add"), "the Models explorer has the canvas");
        tap(h, "workspace.explorers.source-control");
        assert!(has_id(h, VIEW), "still open");
        h.state_mut().canvas.notice = Some("Refused by a panel".into());
        h.run_steps(1);
        assert!(painted_has(h, "Refused by a panel"), "a notice for the canvas these cover");
        crate::canvas::open(h.state_mut(), archforge_mof::Uuid::v7(0, [0; 10]));
        assert!(h.state().versions.details.is_none(), "what was opened would be hidden");
        opened(h, "c3");
        h.state_mut().session_replaced();
        h.run_steps(2);
        assert!(h.state().versions.details.is_none(), "about the session left");
    });
}

/// The cards say what changed from the version before to this one: the model's elements first,
/// and the first version reads nothing. The later version lacks what was added, so only the
/// details say Removed, and the id of the diagram sorts before the schema's.
#[test]
fn details_list_what_changed_since_the_version_before() {
    in_both(|h, host| {
        add_schema(h, "first");
        let without = rows_now(h);
        let s = h.state_mut().session.as_mut().unwrap();
        let schema = s.doc().alive().find(|o| s.doc().class_of(*o).ok() == Some(archforge_er::SCHEMA)).unwrap();
        let schema = s.doc().uuid(schema).unwrap().to_string();
        s.execute("diagram.add", &serde_json::json!({ "kind": "er", "element": schema, "name": "Shop schema" })).unwrap();
        add_schema(h, "shop");
        let with = rows_now(h);
        list_still(h);
        host.reads.lock().unwrap().clear();
        opened(h, "c3");
        assert_eq!(asked(host), ["c2", "c3"], "the version before and this one");
        assert!(has_id(h, "workspace.version-details.loading") && !painted_has(h, "0 changed items"), "nothing half-read");
        answer(host, "c2", Ok(with));
        answer(host, "c3", Ok(without));
        settled(h);
        assert!(painted_has(h, "3 changed items"), "the schema, the diagram and its root: {:?}", painted(h));
        let gone: Vec<_> = h.query_all_by_label_contains(", Removed").map(|n| n.accesskit_node().label().unwrap_or_default().to_string()).collect();
        assert!(gone.len() == 3 && gone[0].starts_with("shop,") && gone[1].starts_with("Shop schema,"), "drawn in this order: {gone:?}");
        opened(h, "c1");
        assert!(painted_has(h, "First version: there is nothing earlier to compare it with."), "{:?}", painted(h));
        assert!(!has_id(h, "workspace.version-details.loading") && asked(host).is_empty(), "nothing read for it");
    });
}

/// A comparison on its way wakes the window, once input has stopped asking for frames, and the
/// window rests when the answer has landed. Only the model's root differs from a version with
/// nothing in it, and that is no change of its own.
#[test]
fn a_comparison_on_its_way_wakes_the_window_and_the_root_is_no_change() {
    in_both(|h, host| {
        let rows = rows_now(h);
        let nothing = std::sync::Arc::new(archforge_mof::rows::Rows {
            tables: vec![Vec::new(); rows.tables.len()],
            junctions: vec![Vec::new(); rows.junctions.len()],
        });
        opened(h, "c3");
        h.run_steps(4);
        assert!(h.ctx.has_requested_repaint(), "waiting on another thread");
        answer(host, "c2", Ok(nothing));
        answer(host, "c3", Ok(rows));
        settled(h);
        assert!(painted_has(h, "0 changed items"), "{:?}", painted(h));
        h.run_steps(4);
        assert!(!h.ctx.has_requested_repaint(), "at rest once it has landed");
    });
}

/// A failed comparison says so and is not "no changes"; Retry reads again and shows the answer,
/// so a read the host refused for having too many under way can be asked for again.
#[test]
fn a_failed_comparison_is_said_and_retried() {
    in_both(|h, host| {
        let rows = rows_now(h);
        opened(h, "c3");
        answer(host, "c2", Err("Too many".into()));
        answer(host, "c3", Err("Too many".into()));
        settled(h);
        assert!(has_id(h, "workspace.version-details.failed") && painted_has(h, "The changes couldn't be compared."), "{:?}", painted(h));
        assert!(!painted(h).iter().any(|t| t.ends_with("changed items")), "no count for what is not known");
        tap(h, "workspace.version-details.retry");
        h.run_steps(1);
        assert_eq!(asked(host), ["c2", "c3"], "both models again");
        assert!(has_id(h, "workspace.version-details.loading") && !has_id(h, "workspace.version-details.failed"));
        answer(host, "c2", Ok(rows.clone()));
        answer(host, "c3", Ok(rows));
        settled(h);
        assert!(painted_has(h, "0 changed items") && !has_id(h, "workspace.version-details.failed"), "{:?}", painted(h));
    });
}

/// A read that ends without an answer is a failed comparison, not one that waits for ever.
#[test]
fn a_read_that_stops_is_a_failed_comparison() {
    in_both(|h, host| {
        opened(h, "c3");
        host.reads.lock().unwrap().clear();
        settled(h);
        assert!(has_id(h, "workspace.version-details.failed"), "{:?}", painted(h));
    });
}

/// An answer for details left cannot land on those shown now.
#[test]
fn an_answer_for_details_left_is_dropped() {
    in_both(|h, host| {
        let rows = rows_now(h);
        opened(h, "c3");
        let older = host.reads.lock().unwrap().drain(..).map(|(_, tx)| tx).collect::<Vec<_>>();
        opened(h, "c2");
        older.iter().for_each(|tx| tx.send(Ok(rows.clone())).unwrap());
        h.run_steps(2);
        assert!(has_id(h, "workspace.version-details.loading") && asked(host) == ["c1", "c2"], "still c2's reads: {:?}", painted(h));
    });
}
