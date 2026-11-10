use std::sync::Arc;

use archforge_mof::rows::Rows;
use archforge_mof::{Uuid, Value};
use egui_kittest::kittest::{NodeT, Queryable};

use super::*;
use crate::versions::Compared;
use archforge_notations::differences::{Area, CardField, ChangeKind, Difference, ElementCard, ElementSides};

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
    in_both_listing(history(), test);
}

/// Runs `test` over `listed`, once in each appearance.
fn in_both_listing(listed: Vec<crate::VersionEntry>, test: impl Fn(&mut Harness<'static, Shell>, &SavingHost)) {
    in_both_appearances(|theme| {
        let (mut h, host) = versions_listed(theme, listed.clone());
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

/// View refused for an editor still open keeps the details and says why over them.
#[test]
fn a_refused_view_keeps_the_details_and_says_why() {
    in_both(|h, host| {
        opened(h, "c2");
        host.reads.lock().unwrap().clear();
        h.state_mut().explorer.dragging = Some(archforge_mof::Uuid([7; 16]));
        tap(h, VIEW);
        assert!(h.state().versions.details.is_some(), "the details stay");
        assert_eq!(h.state().canvas.notice.as_deref(), Some("Finish or cancel your edit first."));
        assert!(host.reads.lock().unwrap().is_empty() && !h.state().is_held());
    });
}

/// The pages that cover the canvas keep a refused step as the caption under their way back, where
/// the person pressed: no toast's dot, and no toast's width, on the details, Compare or a change.
#[test]
fn a_refused_step_stays_the_caption_on_the_pages_over_the_canvas() {
    const WORDS: &str = "Finish or cancel your edit first.";
    let says = |h: &mut Harness<'static, Shell>, page: &str| {
        h.state_mut().canvas.say_refused_step(Some(WORDS.to_string()));
        h.run_steps(2);
        let notice = rect_of(h, "workspace.canvas.notice");
        let p = crate::ux::palette(h.state().state.theme == ThemeKind::Dark);
        assert!(notice.width() < 440.0 && notice.height() == crate::ux::types::CAPTION.line, "{page}: {notice:?}");
        assert!(notice.top() < h.ctx.content_rect().center().y, "{page}: near the top, {notice:?}");
        assert!(!rect_shapes(h).iter().any(|(r, fill, _)| *fill == p.surface && r.contains_rect(notice) && r.width() == 440.0), "{page}");
    };
    in_both(|h, _| {
        opened(h, "c2");
        says(h, "details");
        tap(h, COMPARE);
        h.run_steps(1);
        says(h, "Compare");
    });
    in_both(|h, host| {
        element_open(h, host);
        says(h, "change");
    });
}

/// A long reason is wrapped, its room kept above the title, and read whole as a status, over the
/// details and on the canvas alike.
#[test]
fn a_long_notice_is_wrapped_and_read_whole() {
    const NOTICE: &str = "workspace.canvas.notice";
    let long = "This reason takes several rows to say in the centre of the window. ".repeat(5);
    in_both(|h, _| {
        let said = |h: &mut Harness<'static, Shell>, long: &str| {
            h.state_mut().canvas.notice = Some(long.to_string());
            h.run_steps(2);
            let status = h.get_by_label(long);
            let status = status.accesskit_node();
            assert_eq!((status.role(), status.live()), (egui::accesskit::Role::Status, egui::accesskit::Live::Polite), "read whole");
            let height = rect_of(h, NOTICE).height();
            assert!(height > 2.0 * crate::ux::types::CAPTION.line, "wrapped: {height}");
            height
        };
        opened(h, "c2");
        let before = rect_of(h, VIEW).top();
        let height = said(h, &long);
        let moved = rect_of(h, VIEW).top() - before;
        assert!((moved - (height - crate::ux::types::CAPTION.line)).abs() < 0.5, "room kept: {moved} for {height}");
        h.state_mut().versions.details = None;
        said(h, &long);
    });
}

/// View with a Chat turn running keeps the details while the question waits and when its answer is
/// refused; Stop and go on closes them once it goes ahead.
#[test]
fn the_details_stay_until_chats_question_is_answered() {
    in_both_appearances(|theme| {
        let (mut h, _rig, _host, _) = chat_and_a_version(theme);
        h.state_mut().state.explorer = crate::state::Explorer::SourceControl;
        crate::version_details::open(h.state_mut(), history().remove(1));
        ask_in_chat(&mut h, "A long question");
        h.run_steps(2);
        tap(&mut h, VIEW);
        assert!(h.state().versions.details.is_some() && h.state().ask.is_some(), "asked, not yet on its way");
        crate::settings::open(h.state_mut());
        tap(&mut h, "close-ask.stop-chat");
        assert!(h.state().versions.details.is_some() && !h.state().is_held(), "refused now: they stay to say why");
        assert_eq!(h.state().canvas.notice.as_deref(), Some("Finish or cancel your edit first."));
        h.state_mut().settings = None;
        tap(&mut h, VIEW);
        tap(&mut h, "close-ask.stop-chat");
        assert!(h.state().versions.details.is_none() && h.state().is_held());
        assert_eq!(h.state().version_being_read(), Some("c2"), "this version is the one it reads");
    });
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
        assert_eq!(times(h, "History"), 2, "the section's header and the breadcrumb");
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
        h.state_mut().canvas.notice = Some("Refused earlier".into());
        tap(h, VIEW);
        assert!(!has_id(h, VIEW) && host.reads.lock().unwrap().is_empty(), "nothing read again");
        assert_eq!(h.state().viewing(), Some("c2"));
        assert_eq!(h.state().canvas.notice, None, "the reason from an earlier try goes too");
    });
}

/// View on the version already in view closes the details and calls off a step on its way.
#[test]
fn view_on_the_version_in_view_calls_off_a_step() {
    in_both(|h, host| {
        let preview = h.state().build(&rows_now(h)).unwrap();
        h.state_mut().show_version(preview, "c2".into());
        h.run_steps(3);
        host.version_reads.lock().unwrap().drain(..).for_each(|read| drop(read.send(Ok(crate::VersionsPage { versions: history(), more: false }))));
        h.run_steps(3);
        opened(h, "c2");
        h.state_mut().move_to_version("c1", None).unwrap();
        tap(h, VIEW);
        assert_eq!((h.state().viewing(), h.state().stepping_from()), (Some("c2"), Some("c2")));
        assert!(h.state().versions.details.is_none());
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

/// The rows without and with a schema added, once the list has stopped moving and nothing is
/// left to read for it: the answer about the edit would move the rows under a click.
fn before_and_after(h: &mut Harness<'static, Shell>, host: &SavingHost) -> (Arc<Rows>, Arc<Rows>) {
    let before = rows_now(h);
    add_schema(h, "shop");
    let after = rows_now(h);
    list_still(h);
    host.reads.lock().unwrap().clear();
    (before, after)
}

/// Frames until the host has been asked for the next comparison's reads.
fn next_reads(h: &mut Harness<'static, Shell>, host: &SavingHost) {
    frames_until(h, |_| !asked(host).is_empty());
}

/// Past the host's limit of reads under way: rows clicked while a comparison runs read no more
/// models. The answer for a row left is not shown, and the row clicked last is compared next.
#[test]
fn quick_clicks_run_one_comparison_at_a_time() {
    in_both(|h, host| {
        let (before, after) = before_and_after(h, host);
        opened(h, "c3");
        for row in ["c2", "c3", "c2", "c3", "c2"] {
            opened(h, row);
        }
        assert_eq!(asked(host), ["c2", "c3"], "only the first comparison's two reads");
        answer(host, "c2", Ok(before.clone()));
        answer(host, "c3", Ok(after));
        next_reads(h, host);
        assert_eq!(asked(host), ["c1", "c2"], "for the row shown now");
        assert!(has_id(h, "workspace.version-details.loading") && !painted_has(h, "1 changed item"), "c3's answer stays out: {:?}", painted(h));
        answer(host, "c1", Ok(before.clone()));
        answer(host, "c2", Ok(before));
        settled(h);
        assert!(painted_has(h, "0 changed items"), "{:?}", painted(h));
    });
}

/// Back on the row whose comparison is running, that one's answer is used and nothing is read again.
#[test]
fn a_row_clicked_again_uses_the_comparison_running() {
    in_both(|h, host| {
        let (before, after) = before_and_after(h, host);
        opened(h, "c3");
        opened(h, "c2");
        opened(h, "c3");
        assert_eq!(asked(host), ["c2", "c3"], "still the first");
        answer(host, "c2", Ok(before));
        answer(host, "c3", Ok(after));
        settled(h);
        assert!(painted_has(h, "1 changed item") && asked(host).is_empty(), "{:?}", painted(h));
    });
}

/// An answer lands only on the two versions it compared: neither on another version with the same
/// one before it, nor on the same version with another one before it.
#[test]
fn an_answer_lands_only_on_the_two_versions_it_compared() {
    in_both(|h, host| {
        let (before, after) = before_and_after(h, host);
        for (hash, previous, reads) in [("c4", "c2", ["c2", "c4"]), ("c3", "c1", ["c1", "c3"])] {
            opened(h, "c3");
            crate::version_details::open(
                h.state_mut(),
                crate::VersionEntry { hash: hash.into(), previous: Some(previous.into()), ..history().remove(0) },
            );
            h.run_steps(1);
            answer(host, "c2", Ok(before.clone()));
            answer(host, "c3", Ok(after.clone()));
            next_reads(h, host);
            assert_eq!(asked(host), reads, "for the pair shown");
            assert!(has_id(h, "workspace.version-details.loading") && !painted_has(h, "1 changed item"), "{:?}", painted(h));
            answer(host, previous, Ok(before.clone()));
            answer(host, hash, Ok(before.clone()));
            settled(h);
            assert!(painted_has(h, "0 changed items"), "{:?}", painted(h));
        }
    });
}

/// What `leave` puts in the session's place: the comparison from before neither lands on the same
/// row shown again nor holds up that row's own reads.
fn a_new_session_starts_its_own_comparison(leave: impl Fn(&mut Harness<'static, Shell>, &SavingHost, Arc<Rows>)) {
    in_both(|h, host| {
        let (before, after) = before_and_after(h, host);
        opened(h, "c3");
        let left: Vec<_> = host.reads.lock().unwrap().drain(..).map(|(_, tx)| tx).collect();
        leave(h, host, before.clone());
        h.run_steps(2);
        assert!(h.state().versions.details.is_none(), "about the session left");
        versions_answer(h, host, Ok(crate::VersionsPage { versions: history(), more: false }));
        answer_read(h, host, Ok(before.clone()));
        list_still(h);
        opened(h, "c3");
        assert_eq!(asked(host), ["c2", "c3"], "its own reads, without waiting for those left");
        left.iter().for_each(|tx| drop(tx.send(Ok(after.clone()))));
        h.run_steps(2);
        assert!(has_id(h, "workspace.version-details.loading") && !painted_has(h, "1 changed item"), "{:?}", painted(h));
        answer(host, "c2", Ok(before.clone()));
        answer(host, "c3", Ok(before.clone()));
        settled(h);
        assert!(painted_has(h, "0 changed items"), "{:?}", painted(h));
    });
}

/// Another session lets the comparison under way go.
#[test]
fn a_new_session_does_not_wait_for_the_comparison_left() {
    a_new_session_starts_its_own_comparison(|h, _, _| h.state_mut().session_replaced());
}

/// A finished restore puts another session in place, so it lets the comparison go as well.
#[test]
fn a_finished_restore_does_not_wait_for_the_comparison_left() {
    a_new_session_starts_its_own_comparison(|h, host, rows| {
        h.state_mut().restore_version("c1", "2026-11-06T15:05:00Z").unwrap();
        h.run_steps(3);
        answer(host, "c1", Ok(rows));
        h.run_steps(4);
        let (ended, _reading) = host.restores.lock().unwrap().remove(0);
        ended.send(Ok(())).unwrap();
    });
}

/// A save and a refused restore leave the session as it is: the comparison running goes on, no
/// second one starts, and its answer lands.
#[test]
fn a_save_or_a_refused_restore_keeps_the_comparison_running() {
    in_both(|h, host| {
        let (before, after) = before_and_after(h, host);
        opened(h, "c3");
        h.state_mut().save();
        h.run_steps(2);
        host.saves.lock().unwrap().pop().unwrap().send(Ok(true)).unwrap();
        h.run_steps(3);
        assert_eq!(asked(host), ["c2", "c3"], "a save starts no comparison");
        h.state_mut().restore_version("c1", "2026-11-06T15:05:00Z").unwrap();
        h.run_steps(3);
        answer(host, "c1", Ok(before.clone()));
        h.run_steps(4);
        let (ended, _reading) = host.restores.lock().unwrap().remove(0);
        ended.send(Err(crate::replace::JobFailure::Failed("refused".into()))).unwrap();
        h.run_steps(4);
        assert!(!h.state().is_held() && has_id(h, VIEW), "the details stay: {:?}", h.state().save_error);
        assert_eq!(asked(host), ["c2", "c3"], "and the comparison is still the first");
        answer(host, "c2", Ok(before));
        answer(host, "c3", Ok(after));
        settled(h);
        assert!(painted_has(h, "1 changed item") && asked(host).is_empty(), "{:?}", painted(h));
    });
}

const LINK: &str = "workspace.version-details.previous";
const LINK_LOADING: &str = "workspace.version-details.previous.loading";
const LINK_FAILED: &str = "workspace.version-details.previous.failed";
const LINK_RETRY: &str = "workspace.version-details.previous.retry";

/// Frames until the host has been asked for the version before and gives that read.
fn the_read(h: &mut Harness<'static, Shell>, host: &SavingHost) -> std::sync::mpsc::Sender<Result<crate::VersionsPage, String>> {
    frames_until(h, |_| !host.version_reads.lock().unwrap().is_empty());
    host.version_reads.lock().unwrap().remove(0)
}

/// A page holding only `entry`.
fn page_of(entry: crate::VersionEntry) -> Result<crate::VersionsPage, String> {
    Ok(crate::VersionsPage { versions: vec![entry], more: true })
}

/// Listed, the version before is named from the list with no read, at the design's place under
/// the last card, and opens through the comparison running; a version with no description reads
/// as its date alone, and the first version has no link.
#[test]
fn the_link_names_a_listed_version_without_a_read_and_opens_it() {
    in_both(|h, host| {
        let (before, after) = before_and_after(h, host);
        host.version_reads.lock().unwrap().clear();
        opened(h, "c3");
        h.run_steps(1);
        assert!(has_id(h, LINK) && painted_has(h, "7 Nov 09:20 · First classes"), "{:?}", painted(h));
        let dates = |h: &Harness<'static, Shell>| painted(h).iter().filter(|t| *t == "6 Nov 16:05").count();
        assert_eq!(dates(h), 1, "only c1's row while c3 is shown");
        tap(h, LINK);
        h.run_steps(1);
        assert_eq!(h.state().versions.selected.as_deref(), Some("c2"));
        assert_eq!(asked(host), ["c2", "c3"], "the comparison running goes on");
        answer(host, "c2", Ok(before.clone()));
        answer(host, "c3", Ok(after.clone()));
        next_reads(h, host);
        assert_eq!(asked(host), ["c1", "c2"], "for the version shown");
        assert_eq!(dates(h), 2, "the row and, with no description, the link");
        answer(host, "c1", Ok(before));
        answer(host, "c2", Ok(after));
        settled(h);
        // Asked only now: the version before is read once the comparison has answered, so earlier
        // this holds whether or not the list has it.
        assert!(host.version_reads.lock().unwrap().is_empty(), "nothing read for what is listed");
        let card = h.state().widgets().iter().find(|w| w.id.starts_with("workspace.version-details.change.")).unwrap().id.clone();
        assert_eq!((rect_of(h, LINK).top() - rect_of(h, &card).bottom()).round(), 43.0, "the link's line box under the last card");
        tap(h, LINK);
        h.run_steps(1);
        assert!(painted_has(h, "First version: there is nothing earlier to compare it with.") && !has_id(h, LINK) && !has_id(h, LINK_LOADING));
    });
}

/// Not listed, the version before is read after the comparison, never beside it, so the details
/// stay at the two reads a comparison takes; while only it is awaited the window wakes.
#[test]
fn a_version_before_not_listed_is_read_after_the_comparison() {
    in_both_listing(history()[..1].to_vec(), |h, host| {
        let (before, after) = before_and_after(h, host);
        host.version_reads.lock().unwrap().clear();
        opened(h, "c3");
        h.run_steps(1);
        assert_eq!(asked(host), ["c2", "c3"]);
        assert!(has_id(h, LINK_LOADING) && host.version_reads.lock().unwrap().is_empty(), "not beside the comparison");
        answer(host, "c2", Ok(before));
        answer(host, "c3", Ok(after));
        let read = the_read(h, host);
        settled(h);
        assert_eq!(*host.version_asks.lock().unwrap().last().unwrap(), Some("c3".to_string()), "the one after it in the line");
        assert_eq!(*host.version_limits.lock().unwrap().last().unwrap(), 1);
        h.run_steps(4);
        assert!(h.ctx.has_requested_repaint() && has_id(h, LINK_LOADING), "only the link is awaited, and it wakes the window");
        read.send(page_of(history().remove(1))).unwrap();
        frames_until(h, |h| has_id(h, LINK));
        assert!(painted_has(h, "7 Nov 09:20 · First classes"), "{:?}", painted(h));
        h.run_steps(4);
        assert!(!h.ctx.has_requested_repaint(), "at rest once it has landed");
    });
}

/// A failed read, an empty page and a page about another version each say so; its own Retry reads
/// only that again and leaves the changes as they were.
#[test]
fn a_failed_read_of_the_version_before_is_said_and_retried_alone() {
    in_both_listing(history()[..1].to_vec(), |h, host| {
        let rows = rows_now(h);
        for bad in [Err("Too many".to_string()), Ok(crate::VersionsPage { versions: Vec::new(), more: false }), page_of(history().remove(2))] {
            list_still(h);
            host.version_reads.lock().unwrap().clear();
            host.reads.lock().unwrap().clear();
            opened(h, "c3");
            answer(host, "c2", Ok(rows.clone()));
            answer(host, "c3", Ok(rows.clone()));
            the_read(h, host).send(bad).unwrap();
            frames_until(h, |h| has_id(h, LINK_FAILED));
            assert!(painted_has(h, "The previous version couldn't be read.") && !has_id(h, LINK), "{:?}", painted(h));
            assert!(painted_has(h, "0 changed items"), "the changes stay");
            tap(h, LINK_RETRY);
            let read = the_read(h, host);
            assert!(asked(host).is_empty(), "only the link is read again");
            read.send(page_of(history().remove(1))).unwrap();
            frames_until(h, |h| has_id(h, LINK));
        }
    });
}

/// Beside a failed comparison the changes' Retry stands for both, so there is one, and it reads
/// both again; a read that ends without an answer has failed too.
#[test]
fn the_changes_retry_covers_the_version_before() {
    in_both_listing(history()[..1].to_vec(), |h, host| {
        opened(h, "c3");
        answer(host, "c2", Err("gone".into()));
        answer(host, "c3", Err("gone".into()));
        settled(h);
        drop(the_read(h, host));
        frames_until(h, |h| has_id(h, LINK_FAILED));
        assert!(has_id(h, "workspace.version-details.retry") && !has_id(h, LINK_RETRY), "one Retry");
        tap(h, "workspace.version-details.retry");
        h.run_steps(1);
        assert_eq!(asked(host), ["c2", "c3"], "both models again");
        assert!(has_id(h, LINK_LOADING) && !has_id(h, LINK_FAILED), "and the link with them");
    });
}

/// The link's answer lands only on the two versions it is about: not on another version with the
/// same one before it, nor on the same version with another one before it.
#[test]
fn a_link_answer_lands_only_on_the_two_versions_it_is_about() {
    in_both_listing(history()[..1].to_vec(), |h, host| {
        let rows = rows_now(h);
        for (hash, previous) in [("c4", "c2"), ("c3", "c1")] {
            host.version_reads.lock().unwrap().clear();
            host.reads.lock().unwrap().clear();
            list_still(h);
            opened(h, "c3");
            answer(host, "c2", Ok(rows.clone()));
            answer(host, "c3", Ok(rows.clone()));
            let read = the_read(h, host);
            crate::version_details::open(
                h.state_mut(),
                crate::VersionEntry { hash: hash.into(), previous: Some(previous.into()), ..history().remove(0) },
            );
            h.run_steps(1);
            read.send(page_of(history().remove(1))).unwrap();
            next_reads(h, host);
            assert_eq!(asked(host), [previous, hash], "the details shown compare next");
            assert!(has_id(h, LINK_LOADING) && !has_id(h, LINK), "c3's answer stays out: {:?}", painted(h));
            answer(host, previous, Ok(rows.clone()));
            answer(host, hash, Ok(rows.clone()));
            settled(h);
        }
    });
}

/// Another session lets the link's read go, so the new session's reads do not wait for it.
#[test]
fn a_new_session_does_not_wait_for_the_link_read_left() {
    in_both_listing(history()[..1].to_vec(), |h, host| {
        let rows = rows_now(h);
        opened(h, "c3");
        answer(host, "c2", Ok(rows.clone()));
        answer(host, "c3", Ok(rows.clone()));
        let _left = the_read(h, host);
        h.state_mut().session_replaced();
        h.run_steps(2);
        versions_answer(h, host, Ok(crate::VersionsPage { versions: history()[..1].to_vec(), more: false }));
        answer_read(h, host, Ok(rows));
        list_still(h);
        host.reads.lock().unwrap().clear();
        opened(h, "c3");
        assert_eq!(asked(host), ["c2", "c3"], "its own reads, without waiting for the one left");
    });
}

const BEFORE: &str = "workspace.change-view.before";
const AFTER: &str = "workspace.change-view.after";
const ELEMENT_FAILED: &str = "workspace.change-view.failed";
const COMPARED: &str = "workspace.change-view.compared";

/// Clicks the row of the change named `name` and frames until its read has answered.
fn open_change(h: &mut Harness<'static, Shell>, name: &str) {
    let d = h.state().versions.changes.iter().find(|d| d.name.as_deref() == Some(name)).unwrap();
    let row = format!("workspace.versions.change.{}.{}", d.id, d.class.0);
    tap(h, &row);
    frames_until(h, |h| h.state().versions.element_reading.is_none() && !has_id(h, "workspace.change-view.loading"));
}

/// The sides of an element called `name` on both sides of the comparison.
fn both_sides(id: Uuid, name: &str) -> Result<Option<ElementSides>, String> {
    let card = ElementCard { class: archforge_er::SCHEMA, name: Some(name.into()) };
    Ok(Some(ElementSides { id, before: Some(card.clone()), after: Some(card), fields: Vec::new() }))
}

/// Puts a read of `id` about `key` under way, the test holding its answer.
fn reading(h: &mut Harness<'static, Shell>, id: Uuid, key: Compared) -> std::sync::mpsc::Sender<Result<Option<ElementSides>, String>> {
    let (tx, rx) = std::sync::mpsc::channel();
    h.state_mut().versions.element_reading = Some(crate::change_view::ElementReading { id, key, answer: rx });
    tx
}

/// Lands `answer` as such a read.
fn landed(h: &mut Harness<'static, Shell>, id: Uuid, key: Compared, answer: Result<Option<ElementSides>, String>) {
    reading(h, id, key).send(answer).unwrap();
    h.run_steps(1);
}

/// A schema added since the last version, opened; gives its id and what the list was compared at.
fn element_open(h: &mut Harness<'static, Shell>, host: &SavingHost) -> (Uuid, Compared) {
    before_and_after(h, host);
    open_change(h, "shop");
    (h.state().versions.changes[0].id, h.state().versions.compared.clone().unwrap())
}

/// A row opens its element in place of the details, reading nothing; the project's name closes it.
#[test]
fn a_working_change_opens_in_the_centre_without_a_read() {
    in_both(|h, host| {
        before_and_after(h, host);
        opened(h, "c3");
        host.reads.lock().unwrap().clear();
        h.state_mut().canvas.notice = Some("From earlier".into());
        open_change(h, "shop");
        assert!(!has_id(h, VIEW) && h.state().versions.selected.is_none() && asked(host).is_empty() && !painted_has(h, "From earlier"));
        assert!(
            painted_has(h, "Added") && painted_has(h, "Nothing here yet") && painted_has(h, "Compared with the version of 7 Nov 17:15"),
            "{:?}",
            painted(h)
        );
        assert_eq!(h.query_all_by_label_contains(", selected").count(), 1);
        let title = rect_of(h, "workspace.change-view.title").top();
        h.state_mut().canvas.notice = Some("Refused by a panel. ".repeat(60));
        h.run_steps(1);
        assert!(has_id(h, "workspace.canvas.notice") && rect_of(h, "workspace.change-view.title").top() > title, "said, and the title makes room");
        tap(h, "workspace.change-view.close");
        assert!(h.state().versions.element.is_none() && has_id(h, "workspace.tabs.add"));
    });
}

/// Before the first version the header says there is nothing to compare with.
#[test]
fn before_the_first_version_the_header_says_so() {
    in_both_appearances(|theme| {
        let (mut h, host) = versions_panel(theme);
        versions_answer(&mut h, &host, Ok(crate::VersionsPage { versions: Vec::new(), more: false }));
        add_schema(&mut h, "shop");
        list_still(&mut h);
        open_change(&mut h, "shop");
        assert!(painted_has(&h, "There is no version to compare with yet.") && painted_has(&h, "Added"), "{:?}", painted(&h));
    });
}

/// The line ends at the header's right margin, on the breadcrumb's line, is read out as it says,
/// and in a narrow window is cut rather than reach the breadcrumb.
#[test]
fn the_compared_line_keeps_to_the_end_of_the_header() {
    in_both(|h, host| {
        let (id, key) = element_open(h, host);
        landed(h, id, key, both_sides(id, "shop"));
        let (words, back) = (rect_of(h, COMPARED), rect_of(h, "workspace.change-view.close"));
        assert!((words.right() - rect_of(h, AFTER).right()).abs() < 1.0, "{words:?}");
        assert!((words.center().y - back.center().y).abs() < 1.0, "{words:?} {back:?}");
        assert_eq!(h.query_all_by_label("Compared with the version of 7 Nov 17:15").count(), 1);
        h.set_size(egui::vec2(600.0, 860.0));
        h.run_steps(2);
        let (cut, back) = (rect_of(h, COMPARED), rect_of(h, "workspace.change-view.close"));
        assert!(cut.width() < words.width() && cut.left() > back.right() + 37.0 + 80.0, "{cut:?} {back:?}");
    });
}

/// It ends when the list drops it, a diagram or version takes the centre or the session changes.
#[test]
fn an_opened_element_ends_with_what_it_is_about() {
    in_both(|h, host| {
        let (id, key) = element_open(h, host);
        let open = |h: &Harness<'static, Shell>| h.state().versions.element.is_some();
        tap(h, "workspace.explorers.models");
        assert!(open(h) && has_id(h, "workspace.tabs.add"), "the Models explorer has the canvas");
        tap(h, "workspace.explorers.source-control");
        crate::canvas::open(h.state_mut(), Uuid::v7(0, [0; 10]));
        assert!(!open(h), "what was opened would be hidden");
        open_change(h, "shop");
        opened(h, "c3");
        assert!(has_id(h, VIEW) && !open(h), "the details replace it");
        tap(h, "workspace.version-details.close");
        open_change(h, "shop");
        h.state_mut().session.as_mut().unwrap().execute("edit.undo", &serde_json::Value::Null).unwrap();
        list_still(h);
        assert!(!open(h), "undone, it has left the list");
        h.state_mut().session.as_mut().unwrap().execute("edit.redo", &serde_json::Value::Null).unwrap();
        list_still(h);
        open_change(h, "shop");
        let _left = reading(h, id, key);
        h.state_mut().session_replaced();
        h.run_steps(2);
        assert!(!open(h) && h.state().versions.element_reading.is_none(), "about the session left");
    });
}

/// A read lands only on what it is about; a failure for rows moved on waits, but their sides land.
#[test]
fn a_read_lands_only_on_what_it_is_about() {
    in_both(|h, host| {
        let (id, key) = element_open(h, host);
        let _waiting = reading(h, id, key.clone());
        h.run_steps(4);
        assert!(h.ctx.has_requested_repaint(), "a read on its way wakes the window");
        landed(h, Uuid::v7(1, [1; 10]), key.clone(), both_sides(id, "another element"));
        landed(h, id, (Some("elsewhere".into()), key.1), both_sides(id, "another base"));
        assert!(!painted_has(h, "another element") && !painted_has(h, "another base"), "{:?}", painted(h));
        landed(h, id, key.clone(), both_sides(id, "both"));
        assert!(painted_has(h, "Changed") && has_id(h, AFTER) && painted_has(h, "both"), "{:?}", painted(h));
        let moved = (key.0.clone(), key.1.wrapping_sub(1));
        landed(h, id, moved.clone(), Err("transient".into()));
        assert!(!has_id(h, ELEMENT_FAILED), "a failure for rows moved on is not said");
        landed(h, id, moved, both_sides(id, "edited since"));
        assert!(painted_has(h, "edited since"), "rows moved on still land: {:?}", painted(h));
        landed(h, id, key, Ok(None));
        assert!(h.state().versions.element.is_none(), "on neither side of the rows of now");
    });
}

/// A failed read says so beside Retry; a failed list closes nothing and has no Retry here.
#[test]
fn a_failure_is_said_and_never_taken_for_no_change() {
    in_both(|h, host| {
        let (id, key) = element_open(h, host);
        landed(h, id, key.clone(), Err("broken".into()));
        assert!(has_id(h, ELEMENT_FAILED) && !has_id(h, BEFORE) && !painted_has(h, "Added"), "{:?}", painted(h));
        assert!(!has_id(h, COMPARED), "nothing was compared, so no version is named");
        tap(h, "workspace.change-view.retry");
        frames_until(h, |h| painted_has(h, "Added"));
        tap(h, "workspace.change-view.close");
        let _waiting = reading(h, id, key.clone());
        let row = h.state().versions.changes[0].clone();
        crate::change_view::open(h.state_mut(), row);
        crate::versions::settle_changes(h.state_mut(), key, Err("list broken".into()));
        h.run_steps(2);
        assert!(h.state().versions.element.is_some() && has_id(h, ELEMENT_FAILED) && !has_id(h, "workspace.change-view.retry"), "{:?}", painted(h));
    });
}

/// An element with two rows is selected on both, and its pill comes from its sides, not a row.
#[test]
fn two_rows_of_one_element_open_it_once() {
    in_both(|h, host| {
        before_and_after(h, host);
        let (mut row, key) = (h.state().versions.changes[0].clone(), h.state().versions.compared.clone().unwrap());
        let mut gone = row.clone();
        (gone.class, gone.kind, gone.name, row.name) = (archforge_er::ENTITY, ChangeKind::Removed, Some("old".into()), Some("new".into()));
        let sheet = Difference { area: Area::Diagram, class: archforge_er::ATTRIBUTE, name: Some("sheet".into()), ..gone.clone() };
        crate::versions::settle_changes(h.state_mut(), key, Ok(vec![row, gone, sheet]));
        h.run_steps(2);
        open_change(h, "sheet");
        assert!(h.state().versions.element.is_none(), "a diagram's row does not open");
        open_change(h, "old");
        assert_eq!(h.query_all_by_label_contains(", selected").count(), 2, "both rows are the element");
        assert!(painted_has(h, "Added"), "the sides say Added, the row Removed: {:?}", painted(h));
    });
}

const COMPARE: &str = "workspace.version-details.compare";
const COMPARE_CLOSE: &str = "workspace.version-compare.close";
const SWAP: &str = "workspace.version-compare.swap";

/// The Compare page of the version `row`, reached from its details.
fn compared(h: &mut Harness<'static, Shell>, row: &str) {
    opened(h, row);
    tap(h, COMPARE);
    h.run_steps(1);
}

/// What the select of `side` holds, read off its own node: "Before, " is no row's name.
fn holds(h: &Harness<'static, Shell>, side: &str) -> String {
    let wanted = format!("{side}, ");
    let mut nodes = h.query_all_by_label_contains(&wanted);
    let node = nodes.next().unwrap_or_else(|| panic!("no {side} select"));
    assert!(nodes.next().is_none() && node.accesskit_node().is_disabled(), "one select, which cannot be used yet");
    assert_eq!(node.accesskit_node().role(), egui::accesskit::Role::ComboBox, "{side}");
    node.accesskit_node().label().unwrap_or_default().to_string()
}

/// Compare shows the chosen version before current work, says the direction, says the selects
/// can't be used yet and reads nothing of its own.
#[test]
fn compare_opens_on_the_chosen_version_before_current_work() {
    in_both(|h, host| {
        compared(h, "c3");
        assert_eq!(asked(host), ["c2", "c3"], "only the details' own reads");
        assert!(!has_id(h, VIEW) && has_id(h, COMPARE_CLOSE) && has_id(h, SWAP), "the page has taken the details' place");
        let before = holds(h, "Before");
        assert!(before.contains("Payment methods split out") && before.ends_with(", Not available yet"), "{before}");
        assert_eq!(holds(h, "After"), "After, Current work, Not available yet");
        assert!(h.query_by_label("Before → After").is_some(), "the direction");
        assert!(h.state().versions.selected.is_none(), "no row is marked, as in the frame");
    });
}

/// A refusal from the details ends with them, and one said while the page shows is seen over it.
#[test]
fn compare_clears_an_old_refusal_and_shows_a_new_one() {
    in_both(|h, _| {
        opened(h, "c3");
        h.state_mut().canvas.notice = Some("From the details".into());
        h.run_steps(1);
        tap(h, COMPARE);
        h.run_steps(1);
        assert_eq!(h.state().canvas.notice, None);
        h.state_mut().canvas.notice = Some("Refused by a panel".into());
        h.run_steps(1);
        assert!(painted_has(h, "Refused by a panel"), "a notice for the canvas this covers");
    });
}

/// Swap reverses the pair each time it is pressed, the direction stays, and opening the page again
/// starts from the default pair.
#[test]
fn swap_reverses_the_pair() {
    in_both(|h, host| {
        compared(h, "c3");
        let (before, after) = (holds(h, "Before"), holds(h, "After"));
        host.reads.lock().unwrap().clear();
        // With its reads gone the comparison ends on its own thread. Until a frame has seen that it
        // is still the one running, and the details opened again below would read nothing and
        // take its failure, about the same two versions, as their own.
        frames_until(h, |h| h.state().versions.details_comparing.is_none());
        tap(h, SWAP);
        h.run_steps(1);
        assert_eq!(holds(h, "Before"), "Before, Current work, Not available yet");
        assert!(holds(h, "After").contains("Payment methods split out"), "{}", holds(h, "After"));
        assert!(h.query_by_label("Before → After").is_some(), "the direction stays");
        tap(h, SWAP);
        h.run_steps(1);
        assert_eq!((holds(h, "Before"), holds(h, "After")), (before.clone(), after.clone()), "and back");
        tap(h, SWAP);
        tap(h, COMPARE_CLOSE);
        h.run_steps(1);
        compared(h, "c3");
        assert_eq!((holds(h, "Before"), holds(h, "After")), (before, after), "a new page starts over");
        assert_eq!(asked(host), ["c2", "c3"], "Swap reads nothing; only the details opened again do");
    });
}

/// The project's name closes the page; the rail gives the centre back and brings it again; a row,
/// opening a diagram, another session or another project ends it.
#[test]
fn compare_gives_way_like_the_details() {
    in_both(|h, _| {
        compared(h, "c3");
        tap(h, COMPARE_CLOSE);
        assert!(!has_id(h, SWAP) && h.state().versions.details.is_none() && has_id(h, "workspace.tabs.add"));
        compared(h, "c3");
        tap(h, "workspace.explorers.models");
        assert!(!has_id(h, SWAP) && has_id(h, "workspace.tabs.add"), "the Models explorer has the canvas");
        tap(h, "workspace.explorers.source-control");
        assert!(has_id(h, SWAP), "still open");
        opened(h, "c2");
        assert!(!has_id(h, SWAP) && has_id(h, VIEW), "a row opens its details instead");
        compared(h, "c3");
        crate::canvas::open(h.state_mut(), archforge_mof::Uuid::v7(0, [0; 10]));
        h.run_steps(1);
        assert!(!has_id(h, SWAP) && h.state().versions.details.is_none(), "what was opened would be hidden");
        compared(h, "c3");
        h.state_mut().session_replaced();
        h.run_steps(2);
        assert!(!has_id(h, SWAP) && h.state().versions.details.is_none(), "about the session left");
    });
    in_both(|h, _| {
        compared(h, "c3");
        h.state_mut().project.as_mut().unwrap().path.push("elsewhere");
        h.run_steps(2);
        assert!(!has_id(h, SWAP) && h.state().versions.details.is_none(), "about the project left");
    });
}

/// A side of `name`, a schema.
fn schema(name: &str) -> Option<ElementCard> {
    Some(ElementCard { class: archforge_er::SCHEMA, name: Some(name.into()) })
}

/// A field of the enumeration-typed Visibility with these values.
fn field(before: Option<Value>, after: Option<Value>, changed: bool) -> CardField {
    CardField { prop: archforge_uml::named_element::VISIBILITY, before, after, changed }
}

/// How many controls are named exactly `name`.
fn named(h: &Harness<'static, Shell>, name: &str) -> usize {
    h.query_all_by_label(name).count()
}

/// The palette the test is drawn in.
fn colours(h: &Harness<'static, Shell>) -> &'static crate::ux::Palette {
    crate::ux::palette(h.state().state.theme == ThemeKind::Dark)
}

/// Whether a rectangle exactly over the control `id` is filled with `fill`.
fn filled(h: &Harness<'static, Shell>, id: &str, fill: egui::Color32) -> bool {
    rect_shapes(h).iter().any(|(r, f, _)| *r == rect_of(h, id) && *f == fill)
}

/// Each type of value is worded as a person says it, a click reads nothing, and a label or
/// literal the metamodel does not give is Unknown, never a dropped row.
#[test]
fn values_are_worded_per_type_without_a_read() {
    in_both(|h, host| {
        let (id, key) = element_open(h, host);
        assert!(asked(host).is_empty(), "the open reads nothing");
        let shop = h.state().session.as_ref().unwrap().doc().by_uuid(id).unwrap();
        let cases = [
            (Value::Null, "Not set"),
            (Value::str(""), "Not set"),
            (Value::Bool(true), "Yes"),
            (Value::Bool(false), "No"),
            (Value::Int(-7), "-7"),
            (Value::Real(1.5), "1.5"),
            (Value::str("words"), "words"),
            (Value::Unlimited(None), "*"),
            (Value::Unlimited(Some(3)), "3"),
            (Value::Enum(1), "Private"),
            (Value::Enum(99), "Unknown"),
            (Value::Ref(shop), "Unknown"),
        ];
        let mut fields: Vec<CardField> = cases.iter().map(|(v, _)| field(Some(v.clone()), None, false)).collect();
        fields.push(CardField { prop: archforge_diagram::node::ELEMENT, ..field(Some(Value::str("odd")), None, false) });
        landed(h, id, key, Ok(Some(ElementSides { id, before: schema("a"), after: None, fields })));
        for (_, want) in &cases {
            let times = cases.iter().filter(|(_, w)| w == want).count();
            assert_eq!(named(h, &format!("Visibility, {want}")), times, "{want}: {:?}", painted(h));
        }
        assert_eq!(named(h, "Unknown, odd"), 1, "a label the metamodel doesn't give");
        assert!(asked(host).is_empty(), "drawing reads nothing");
    });
}

/// What differs has a sign and a tint on each card; a rename or a new kind marks the name.
#[test]
fn what_differs_is_marked_with_a_sign_and_a_tint() {
    in_both(|h, host| {
        let (id, key) = element_open(h, host);
        let p = colours(h);
        let fields =
            vec![field(Some(Value::str("old")), Some(Value::str("new")), true), field(Some(Value::str("same")), Some(Value::str("same")), false)];
        landed(h, id, key.clone(), Ok(Some(ElementSides { id, before: schema("kept"), after: schema("kept"), fields })));
        assert_eq!((named(h, "Visibility, old, Changed"), named(h, "Visibility, new, Changed"), named(h, "Visibility, same")), (1, 1, 2));
        assert_eq!((painted_times(h, "\u{2212}"), painted_times(h, "+")), (1, 1), "{:?}", painted(h));
        assert!(filled(h, &format!("{BEFORE}.field.0"), p.danger_tint) && filled(h, &format!("{AFTER}.field.0"), p.green_tint));
        assert!(!filled(h, &format!("{BEFORE}.field.1"), p.danger_tint), "a same row is plain");
        assert_eq!(named(h, "Schema, kept"), 2, "the same name is not marked");
        landed(h, id, key.clone(), Ok(Some(ElementSides { id, before: schema("was"), after: schema("is"), fields: Vec::new() })));
        assert_eq!((named(h, "Schema, was, Changed"), named(h, "Schema, is, Changed")), (1, 1), "a rename: {:?}", painted(h));
        assert_eq!((painted_times(h, "\u{2212}"), painted_times(h, "+")), (1, 1));
        assert!(filled(h, &format!("{BEFORE}.name"), p.danger_tint) && filled(h, &format!("{AFTER}.name"), p.green_tint));
        let entity = Some(ElementCard { class: archforge_er::ENTITY, name: Some("kept".into()) });
        landed(h, id, key, Ok(Some(ElementSides { id, before: schema("kept"), after: entity, fields: Vec::new() })));
        assert_eq!((named(h, "Schema, kept, Changed"), named(h, "Entity, kept, Changed")), (1, 1), "a new kind");
    });
}

/// A side the element is not on is a tinted card saying which, and each head names its side.
#[test]
fn a_side_without_the_element_is_a_tinted_card() {
    in_both(|h, host| {
        let (id, key) = element_open(h, host);
        let p = colours(h);
        assert_eq!((named(h, "Before, Nothing here yet"), named(h, "After, Current work")), (1, 1), "added");
        assert!(filled(h, BEFORE, p.field) && !has_id(h, &format!("{BEFORE}.name")));
        landed(h, id, key, Ok(Some(ElementSides { id, before: schema("gone"), after: None, fields: Vec::new() })));
        assert_eq!((named(h, "After, Removed"), named(h, "Before, Version of 7 Nov 17:15")), (1, 1), "{:?}", painted(h));
        assert!(filled(h, AFTER, p.field), "removed");
        h.state_mut().versions.page.as_mut().unwrap().versions.clear();
        h.run_steps(1);
        assert_eq!(named(h, "Before, Last version"), 1, "the list no longer holds it");
    });
}

/// The cards scroll as one, a row level with itself across them.
#[test]
fn the_cards_scroll_as_one() {
    in_both(|h, host| {
        let (id, key) = element_open(h, host);
        let fields = vec![field(Some(Value::Null), Some(Value::Null), false); 60];
        landed(h, id, key, Ok(Some(ElementSides { id, before: schema("long"), after: schema("long"), fields })));
        let (first, last) = (format!("{BEFORE}.field.0"), format!("{AFTER}.field.59"));
        assert!(has_id(h, &first) && !has_id(h, &last), "the end is out of sight");
        assert!(painted_times(h, "Not set") < 120, "rows out of sight are not drawn");
        h.event(egui::Event::PointerMoved(rect_of(h, &first).center()));
        h.event(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, -4000.0),
            phase: egui::TouchPhase::Move,
            modifiers: egui::Modifiers::NONE,
            source: egui::MouseWheelSource::Wheel,
        });
        h.run_steps(12);
        assert!(!has_id(h, &first) && has_id(h, &last), "{:?}", ids(h));
        assert_eq!(rect_of(h, &format!("{BEFORE}.field.59")).top(), rect_of(h, &last).top(), "level across the cards");
    });
}
