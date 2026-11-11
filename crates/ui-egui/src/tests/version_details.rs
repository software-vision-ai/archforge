use std::sync::Arc;

use archforge_mof::rows::Rows;
use archforge_mof::{Uuid, Value};
use egui_kittest::kittest::{NodeT, Queryable};

use super::*;
use crate::version_details::compare::results::{Held, Key};
use crate::versions::Compared;
use archforge_notations::differences::{
    Area, CardField, CardList, CardLists, ChangeKind, Difference, ElementCard, ElementSides, ListKind, ListRow, Named,
};

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
fn answer(host: &SavingHost, commit: &str, rows: crate::VersionRead) {
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
        answer(host, "c2", Err("Too many".to_string().into()));
        answer(host, "c3", Err("Too many".to_string().into()));
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
        answer(host, "c2", Err("gone".to_string().into()));
        answer(host, "c3", Err("gone".to_string().into()));
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
const SUMMARY: &str = "workspace.change-view.summary";
const NOT_SHOWN: &str = "Relationships aren't shown here yet.";

/// Clicks the row of the change named `name` and frames until its read has answered.
fn open_change(h: &mut Harness<'static, Shell>, name: &str) {
    let d = h.state().versions.changes.iter().find(|d| d.name.as_deref() == Some(name)).unwrap();
    let row = format!("workspace.versions.change.{}.{}", d.id, d.class.0);
    tap(h, &row);
    frames_until(h, |h| h.state().versions.element_reading.is_none() && !has_id(h, "workspace.change-view.loading"));
}

/// The sides of `id` with these cards and `fields`, and no lists.
fn sided(id: Uuid, before: Option<ElementCard>, after: Option<ElementCard>, fields: Vec<CardField>) -> ElementSides {
    ElementSides { id, before, after, fields, lists: Default::default() }
}

/// The sides of an element called `name` on both sides of the comparison.
fn both_sides(id: Uuid, name: &str) -> Result<Option<ElementSides>, String> {
    let card = ElementCard { class: archforge_er::SCHEMA, name: Some(name.into()) };
    Ok(Some(sided(id, Some(card.clone()), Some(card), Vec::new())))
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
    assert!(nodes.next().is_none() && !node.accesskit_node().is_disabled(), "one select, which can be used");
    assert_eq!(node.accesskit_node().role(), egui::accesskit::Role::ComboBox, "{side}");
    node.accesskit_node().label().unwrap_or_default().to_string()
}

/// Compare shows the chosen version before current work, says the direction and reads nothing of
/// its own.
#[test]
fn compare_opens_on_the_chosen_version_before_current_work() {
    in_both(|h, host| {
        compared(h, "c3");
        assert_eq!(asked(host), ["c2", "c3"], "only the details' own reads");
        assert!(!has_id(h, VIEW) && has_id(h, COMPARE_CLOSE) && has_id(h, SWAP), "the page has taken the details' place");
        let before = holds(h, "Before");
        assert!(before.contains("Payment methods split out"), "{before}");
        assert_eq!(holds(h, "After"), "After, Current work");
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
        assert_eq!(holds(h, "Before"), "Before, Current work");
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
        assert_eq!(asked(host), ["c3"], "Swap reads nothing: this is the page's one read, which the details opened again wait for");
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

/// What the Compare page has found: each difference's name and kind, or why it could not tell.
fn found(h: &Harness<'static, Shell>) -> Option<Result<Vec<(String, ChangeKind)>, String>> {
    let found = &h.state().versions.compare_results.found.as_ref()?.1;
    Some(found.as_ref().map(|items| items.iter().map(|d| (d.name.clone().unwrap_or_default(), d.kind)).collect()).map_err(Clone::clone))
}

/// Adds a schema, opens Compare on `c3` and waits for the page's read. Gives the rows without the schema.
fn page_reading(h: &mut Harness<'static, Shell>, host: &SavingHost) -> Arc<Rows> {
    let (before, _) = before_and_after(h, host);
    compared(h, "c3");
    assert_eq!(asked(host), ["c2", "c3"], "the details first: the host limits the reads under way");
    answer(host, "c2", Ok(before.clone()));
    answer(host, "c3", Ok(before.clone()));
    frames_until(h, |_| asked(host) == ["c3"]);
    before
}

/// Frames until the page holds `n` differences.
fn found_many(h: &mut Harness<'static, Shell>, n: usize) {
    frames_until(h, |h| matches!(found(h), Some(Ok(items)) if items.len() == n));
}

/// Before is read once and current work is a snapshot, so the session stays. The window wakes for
/// the answer and rests after it; Swap reverses what is found, and an edit is compared, without a read.
#[test]
fn a_version_is_compared_with_current_work_and_read_once() {
    in_both(|h, host| {
        let generation = h.state().session_generation();
        let before = page_reading(h, host);
        h.run_steps(4);
        assert!(h.ctx.has_requested_repaint(), "waiting on another thread");
        answer(host, "c3", Ok(before));
        found_many(h, 1);
        assert_eq!(found(h), Some(Ok(vec![("shop".to_string(), ChangeKind::Added)])));
        h.run_steps(4);
        assert!(!h.ctx.has_requested_repaint(), "at rest once it has landed");
        tap(h, SWAP);
        frames_until(h, |h| found(h) == Some(Ok(vec![("shop".to_string(), ChangeKind::Removed)])));
        add_schema(h, "more");
        found_many(h, 2);
        assert!(asked(host).is_empty() && h.state().session_generation() == generation, "{:?}", asked(host));
    });
}

/// Lands `answer` for `sides` at `revision`, the page kept from starting anything of its own.
fn landing(h: &mut Harness<'static, Shell>, sides: (&str, bool), revision: u64, answer: Result<Arc<Vec<Difference>>, String>) {
    let (waiting, read) = std::sync::mpsc::channel();
    let (tx, comparison) = std::sync::mpsc::channel();
    tx.send(answer).unwrap();
    let key = Key { sides: (sides.0.to_string(), sides.1), revision };
    let r = &mut h.state_mut().versions.compare_results;
    (r.held, r.found, r.reading, r.comparing) = (None, None, Some(("c3".to_string(), read)), Some((key, comparison)));
    h.run_steps(1);
    drop(waiting);
}

/// An answer for rows that moved on still lands, or edits in a stream would hide every one; its
/// failure does not, and neither does an answer for the other direction or another version. While
/// one runs no other starts, and the window wakes for it.
#[test]
fn a_comparison_runs_alone_and_lands_only_on_its_sides() {
    in_both(|h, host| {
        let before = page_reading(h, host);
        let now = h.state().session.as_ref().unwrap().revision();
        let none = || Ok(Arc::new(Vec::new()));
        landing(h, ("c3", false), now - 1, none());
        assert_eq!(found(h), Some(Ok(Vec::new())), "moved on");
        landing(h, ("c3", false), now - 1, Err("Old".into()));
        assert_eq!(found(h), None, "a failure says nothing of the newest rows");
        landing(h, ("c3", false), now, Err("Now".into()));
        assert_eq!(found(h), Some(Err("Now".to_string())));
        landing(h, ("c3", true), now, none());
        assert_eq!(found(h), None, "the other direction");
        landing(h, ("c2", false), now, none());
        assert_eq!(found(h), None, "another version");
        let (_waiting, pending) = std::sync::mpsc::channel();
        let key = Key { sides: ("c3".to_string(), false), revision: now - 1 };
        let r = &mut h.state_mut().versions.compare_results;
        (r.held, r.reading, r.comparing) = (Some(Held { version: "c3".into(), rows: before, keep: true }), None, Some((key.clone(), pending)));
        h.run_steps(4);
        assert!(h.ctx.has_requested_repaint() && h.state().versions.compare_results.comparing.as_ref().is_some_and(|c| c.0 == key));
    });
}

/// A failed read is a failure, not no differences; an edit does not read again but the other
/// direction does, and a read that stops without an answer is a failure too.
#[test]
fn a_failed_read_waits_and_a_stopped_one_is_a_failure() {
    in_both(|h, host| {
        let _ = page_reading(h, host);
        answer(host, "c3", Err("Too many".to_string().into()));
        frames_until(h, |h| found(h).is_some());
        assert_eq!(found(h), Some(Err("Too many".to_string())));
        add_schema(h, "more");
        h.run_steps(3);
        assert!(asked(host).is_empty(), "{:?}", asked(host));
        tap(h, SWAP);
        frames_until(h, |_| asked(host) == ["c3"]);
        h.state_mut().versions.compare_results.found = None;
        host.reads.lock().unwrap().clear();
        frames_until(h, |h| matches!(found(h), Some(Err(_))));
    });
}

/// A late answer about another version is dropped, failure or rows, and rows held are of one
/// version: choosing another reads it. Against nothing, the model's root is still no change.
#[test]
fn another_version_is_read_and_a_late_answer_dropped() {
    in_both(|h, host| {
        let before = page_reading(h, host);
        tap(h, SELECT_BEFORE);
        tap(h, &row_id(2));
        answer(host, "c3", Err("Late".to_string().into()));
        frames_until(h, |_| asked(host) == ["c2"]);
        assert_eq!(found(h), None);
        answer(host, "c2", Ok(before.clone()));
        found_many(h, 1);
        tap(h, SELECT_BEFORE);
        tap(h, &row_id(3));
        frames_until(h, |_| asked(host) == ["c1"]);
        let nothing = Rows { tables: vec![Vec::new(); before.tables.len()], junctions: vec![Vec::new(); before.junctions.len()] };
        answer(host, "c1", Ok(Arc::new(nothing)));
        let of_c1 = |h: &Harness<'static, Shell>| h.state().versions.compare_results.found.clone().filter(|(key, _)| key.sides.0 == "c1");
        frames_until(h, |h| of_c1(h).is_some());
        let items = of_c1(h).unwrap().1.unwrap();
        assert!(!items.is_empty() && items.iter().all(|d| d.class != archforge_core::MODEL), "the model's root is no change of its own");
    });
}

/// Rows too large to hold go once compared, so the next edit reads them again. Held rows retry a
/// failed comparison on the next edit. A page gone holds nothing, a session left no read either.
#[test]
fn what_is_held_goes_when_too_large_or_left() {
    in_both(|h, host| {
        let before = page_reading(h, host);
        answer(host, "c3", Ok(before));
        found_many(h, 1);
        h.state_mut().versions.compare_results.found.as_mut().unwrap().1 = Err("Failed".into());
        h.state_mut().versions.compare_results.held.as_mut().unwrap().keep = false;
        add_schema(h, "more");
        found_many(h, 2);
        assert!(h.state().versions.compare_results.held.is_none() && asked(host).is_empty());
        add_schema(h, "third");
        frames_until(h, |_| asked(host) == ["c3"]);
        answer(host, "c3", Ok(rows_now(h)));
        frames_until(h, |h| h.state().versions.compare_results.held.is_some());
        tap(h, COMPARE_CLOSE);
        h.run_steps(1);
        assert!(h.state().versions.compare_results.held.is_none() && found(h).is_none());
    });
    in_both(|h, host| {
        let _ = page_reading(h, host);
        h.state_mut().session_replaced();
        h.run_steps(2);
        assert!(h.state().versions.compare_results.reading.is_none(), "about the session left");
    });
}

/// The page and the details take turns at reading, as the host limits the reads under way.
#[test]
fn the_page_and_the_details_take_turns_at_reading() {
    in_both(|h, host| {
        let before = page_reading(h, host);
        opened(h, "c2");
        h.run_steps(2);
        assert_eq!(asked(host), ["c3"], "still only the page's");
        answer(host, "c3", Ok(before));
        frames_until(h, |_| asked(host) == ["c1", "c2"]);
    });
    in_both_listing(vec![history().remove(0)], |h, host| {
        let rows = rows_now(h);
        compared(h, "c3");
        answer(host, "c2", Ok(rows.clone()));
        answer(host, "c3", Ok(rows));
        frames_until(h, |h| h.state().versions.details_reading.is_some());
        h.run_steps(2);
        assert!(asked(host).is_empty(), "behind the link's read: {:?}", asked(host));
    });
}

/// Nothing is compared during an edit.
#[test]
fn an_edit_under_way_holds_the_comparison() {
    in_both(|h, host| {
        let before = page_reading(h, host);
        h.state_mut().session.as_mut().unwrap().begin_interaction("er.schema.add").unwrap();
        answer(host, "c3", Ok(before));
        frames_until(h, |h| h.state().versions.compare_results.held.is_some());
        h.run_steps(2);
        assert!(found(h).is_none() && h.state().versions.compare_results.comparing.is_none());
        h.state_mut().session.as_mut().unwrap().cancel().unwrap();
        found_many(h, 1);
    });
}

/// While a version shows, current work is the session set aside, not the one in view, and its
/// revision is what a failure is told by.
#[test]
fn current_work_is_the_session_set_aside_while_a_version_shows() {
    in_both(|h, host| {
        let (before, _) = before_and_after(h, host);
        opened(h, "c2");
        tap(h, VIEW);
        h.run_steps(3);
        while asked(host).iter().any(|c| c == "c2") {
            answer(host, "c2", Ok(before.clone()));
        }
        frames_until(h, |h| h.state().viewing() == Some("c2"));
        host.reads.lock().unwrap().clear();
        crate::version_details::open(h.state_mut(), history().remove(2));
        h.run_steps(2);
        tap(h, COMPARE);
        frames_until(h, |_| asked(host) == ["c1"]);
        answer(host, "c1", Ok(before));
        found_many(h, 1);
        let now = h.state().current_work().unwrap().revision();
        landing(h, ("c1", false), now, Err("Now".into()));
        assert_eq!(found(h), Some(Err("Now".to_string())));
    });
}

/// A page waiting behind a comparison the details have left still wakes the window.
#[test]
fn a_page_waiting_its_turn_wakes_the_window() {
    in_both(|h, host| {
        opened(h, "c3");
        compared(h, "c1");
        h.run_steps(4);
        assert!(h.ctx.has_requested_repaint() && asked(host) == ["c2", "c3"], "{:?}", asked(host));
    });
}

const COMPARE_LOADING: &str = "workspace.version-compare.loading";

/// The page says how many differences there are and by area, and lists them in groups as cards;
/// bars stand in until the first answer, and the band stays over all of it.
#[test]
fn the_differences_are_counted_and_grouped() {
    in_both(|h, host| {
        let before = page_reading(h, host);
        h.run_steps(1);
        assert!(has_id(h, COMPARE_LOADING) && !painted(h).iter().any(|t| t.ends_with("differences")), "{:?}", painted(h));
        let the_panels = painted_times(h, "MODEL ELEMENTS");
        answer(host, "c3", Ok(before));
        found_many(h, 1);
        h.run_steps(1);
        assert!(painted_has(h, "1 difference") && painted_has(h, "1 model element") && painted_times(h, "MODEL ELEMENTS") == the_panels + 1);
        assert!(!painted_has(h, "DIAGRAMS") && !has_id(h, COMPARE_LOADING) && has_id(h, SWAP), "{:?}", painted(h));
        assert_eq!(h.query_all_by_label_contains(", Added").count(), 2, "the list's row and the page's card");
    });
}

/// A list of other sides starts at its top.
#[test]
fn a_new_pair_lists_from_the_top() {
    in_both(|h, host| {
        let before = page_reading(h, host);
        (0..14).for_each(|i| add_schema(h, &format!("s{i}")));
        answer(host, "c3", Ok(before));
        found_many(h, 15);
        h.state_mut().versions.compare_results.scroll.offset = 40.0;
        h.run_steps(1);
        assert_eq!(h.state().versions.compare_results.scroll.offset, 40.0, "a list longer than the page");
        tap(h, SWAP);
        frames_until(h, |h| matches!(found(h), Some(Ok(items)) if items[0].1 == ChangeKind::Removed));
        assert_eq!(h.state().versions.compare_results.scroll.offset, 0.0);
    });
}

/// A failure is said with Retry, and no count; Retry reads again.
#[test]
fn a_failed_comparison_on_the_page_is_said_and_retried() {
    in_both(|h, host| {
        let _ = page_reading(h, host);
        answer(host, "c3", Err("Too many".to_string().into()));
        frames_until(h, |h| has_id(h, "workspace.version-compare.failed"));
        assert!(!painted(h).iter().any(|t| t.contains("difference")), "{:?}", painted(h));
        tap(h, "workspace.version-compare.retry");
        frames_until(h, |_| asked(host) == ["c3"]);
        assert!(has_id(h, COMPARE_LOADING) && !has_id(h, "workspace.version-compare.failed"));
    });
}

/// Nothing to list is said in words, and two saved versions are not compared yet.
#[test]
fn an_empty_result_and_two_versions_are_said() {
    in_both(|h, host| {
        let _ = page_reading(h, host);
        answer(host, "c3", Ok(rows_now(h)));
        frames_until(h, |h| has_id(h, "workspace.version-compare.said"));
        assert!(painted_has(h, "This version is the same as your current work.") && !painted_has(h, "0 differences"), "{:?}", painted(h));
        tap(h, SELECT_AFTER);
        tap(h, &row_id(2));
        h.run_steps(1);
        assert!(painted_has(h, "Not available yet") && asked(host).is_empty(), "{:?}", painted(h));
        tap(h, SELECT_AFTER);
        tap(h, &row_id(0));
        tap(h, SELECT_BEFORE);
        tap(h, &row_id(0));
        h.run_steps(1);
        assert!(painted_has(h, "Both sides are your current work."), "{:?}", painted(h));
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
        landed(h, id, key, Ok(Some(sided(id, schema("a"), None, fields))));
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
        landed(h, id, key.clone(), Ok(Some(sided(id, schema("kept"), schema("kept"), fields))));
        assert_eq!((named(h, "Visibility, old, Changed"), named(h, "Visibility, new, Changed"), named(h, "Visibility, same")), (1, 1, 2));
        assert_eq!((painted_times(h, "\u{2212}"), painted_times(h, "+")), (1, 1), "{:?}", painted(h));
        assert!(filled(h, &format!("{BEFORE}.field.0"), p.danger_tint) && filled(h, &format!("{AFTER}.field.0"), p.green_tint));
        assert!(!filled(h, &format!("{BEFORE}.field.1"), p.danger_tint), "a same row is plain");
        assert_eq!(named(h, "Schema, kept"), 2, "the same name is not marked");
        landed(h, id, key.clone(), Ok(Some(sided(id, schema("was"), schema("is"), Vec::new()))));
        assert_eq!((named(h, "Schema, was, Changed"), named(h, "Schema, is, Changed")), (1, 1), "a rename: {:?}", painted(h));
        assert_eq!((painted_times(h, "\u{2212}"), painted_times(h, "+")), (1, 1));
        assert!(filled(h, &format!("{BEFORE}.name"), p.danger_tint) && filled(h, &format!("{AFTER}.name"), p.green_tint));
        let entity = Some(ElementCard { class: archforge_er::ENTITY, name: Some("kept".into()) });
        landed(h, id, key, Ok(Some(sided(id, schema("kept"), entity, Vec::new()))));
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
        landed(h, id, key, Ok(Some(sided(id, schema("gone"), None, Vec::new()))));
        assert_eq!((named(h, "After, Removed"), named(h, "Before, Version of 7 Nov 17:15")), (1, 1), "{:?}", painted(h));
        assert!(filled(h, AFTER, p.field), "removed");
        h.state_mut().versions.page.as_mut().unwrap().versions.clear();
        h.run_steps(1);
        assert_eq!(named(h, "Before, Last version"), 1, "the list no longer holds it");
    });
}

/// The summary says exactly this: painted once each and read whole.
fn says(h: &Harness<'static, Shell>, head: &str, body: &str) {
    assert_eq!((painted_times(h, head), painted_times(h, body)), (1, 1), "{:?}", painted(h));
    assert_eq!(named(h, &format!("{head}, {body}")), 1, "read whole");
}

/// An element on one side says so, with nothing to count.
#[test]
fn the_summary_says_added_or_removed() {
    in_both(|h, host| {
        let (id, key) = element_open(h, host);
        says(h, "This element was added", &format!("It is only on the After side, so there is nothing to compare. {NOT_SHOWN}"));
        landed(h, id, key, Ok(Some(sided(id, schema("gone"), None, Vec::new()))));
        says(h, "This element was removed", &format!("It is only on the Before side, so there is nothing to compare. {NOT_SHOWN}"));
    });
}

/// Each difference is counted and said in a whole sentence, the name and kind first, a same value not at all;
/// members are said not to be shown, so the count isn't read as all there is.
#[test]
fn the_summary_counts_and_says_each_difference() {
    in_both(|h, host| {
        let (id, key) = element_open(h, host);
        let visibility = field(Some(Value::Null), Some(Value::Enum(1)), true);
        let same = field(Some(Value::str("same")), Some(Value::str("same")), false);
        let fields = vec![visibility.clone(), same];
        landed(h, id, key.clone(), Ok(Some(sided(id, schema("kept"), schema("kept"), fields))));
        says(h, "1 value changed", &format!("Visibility changed from Not set to Private. {NOT_SHOWN}"));
        let entity = Some(ElementCard { class: archforge_er::ENTITY, name: None });
        landed(h, id, key, Ok(Some(sided(id, schema("was"), entity, vec![visibility]))));
        let said = "The name changed from was to Unnamed. The kind changed from Schema to Entity. Visibility changed from Not set to Private.";
        says(h, "3 values changed", &format!("{said} {NOT_SHOWN}"));
    });
}

/// Past three differences the rest are counted, in the plural the number takes; three are all written out.
#[test]
fn the_summary_counts_what_it_does_not_write_out() {
    in_both(|h, host| {
        let (id, key) = element_open(h, host);
        let line = "Visibility changed from a to b.";
        let three = format!("{line} {line} {line}");
        for (n, more) in [(3, String::new()), (4, " 1 more value changed.".to_owned()), (5, " 2 more values changed.".to_owned())] {
            let fields = vec![field(Some(Value::str("a")), Some(Value::str("b")), true); n];
            landed(h, id, key.clone(), Ok(Some(sided(id, schema("k"), schema("k"), fields))));
            says(h, &format!("{n} values changed"), &format!("{three}{more} {NOT_SHOWN}"));
        }
    });
}

/// A property only one side's class has is said to be there alone, not to have been empty.
#[test]
fn the_summary_says_a_property_is_on_one_side_alone() {
    in_both(|h, host| {
        let (id, key) = element_open(h, host);
        let fields = vec![field(Some(Value::str("a")), None, true), field(None, Some(Value::str("b")), true)];
        landed(h, id, key, Ok(Some(sided(id, schema("k"), schema("k"), fields))));
        let said = "Visibility is only on the Before side. Visibility is only on the After side.";
        says(h, "2 values changed", &format!("{said} {NOT_SHOWN}"));
    });
}

/// Unmarked cards on a listed change say only members or ownership changed; a failed list can't vouch
/// for a change, and neither reads as nothing having changed.
#[test]
fn unmarked_cards_never_read_as_nothing_changed() {
    in_both(|h, host| {
        let (id, key) = element_open(h, host);
        let fields = vec![field(Some(Value::str("same")), Some(Value::str("same")), false)];
        landed(h, id, key, Ok(Some(sided(id, schema("k"), schema("k"), fields))));
        says(h, "Only what these cards don't show changed", &format!("The cards show the same on both sides. {NOT_SHOWN}"));
        h.state_mut().versions.compare_failed = Some("the list failed".into());
        h.run_steps(1);
        says(h, "The cards show no difference", NOT_SHOWN);
    });
}

/// A long value is cut in the sentence and whole on its card.
#[test]
fn the_summary_cuts_a_long_value() {
    in_both(|h, host| {
        let (id, key) = element_open(h, host);
        let long = "x".repeat(60);
        let fields = vec![field(Some(Value::str(&long)), Some(Value::str("b")), true)];
        landed(h, id, key, Ok(Some(sided(id, schema("k"), schema("k"), fields))));
        says(h, "1 value changed", &format!("Visibility changed from {}… to b. {NOT_SHOWN}", "x".repeat(40)));
        assert_eq!(named(h, &format!("Visibility, {long}, Changed")), 1, "the card holds it whole");
    });
}

/// The summary sits as the frames place it: across both cards, above their eyebrows, on a surface.
#[test]
fn the_summary_sits_above_the_cards() {
    in_both(|h, host| {
        let (id, key) = element_open(h, host);
        landed(h, id, key, Ok(Some(sided(id, schema("k"), schema("k"), Vec::new()))));
        let (area, before, after) = (rect_of(h, SUMMARY), rect_of(h, BEFORE), rect_of(h, AFTER));
        assert_eq!((area.left(), area.right()), (before.left(), after.right()), "as wide as the two cards");
        assert_eq!(area.height(), 86.0, "one row of sentences, as the frame");
        assert_eq!(before.top() - area.bottom(), 18.0 + 24.0, "the room and the eyebrow");
        assert!(filled(h, SUMMARY, colours(h).surface));
    });
}

/// The sides of `id`, called `name` on both sides, holding `lists` and no values.
fn listed(id: Uuid, name: &str, lists: CardLists) -> Result<Option<ElementSides>, String> {
    Ok(Some(ElementSides { lists: Arc::new(lists), ..sided(id, schema(name), schema(name), Vec::new()) }))
}

/// A list of `kind` holding rows that say each `(words, change)`.
fn list(kind: ListKind, rows: Vec<(Named, Option<ChangeKind>)>) -> CardList {
    let row = |(name, change)| ListRow { id: Uuid([7; 16]), class: archforge_er::ATTRIBUTE, name, change };
    CardList { kind, rows: rows.into_iter().map(row).collect() }
}

/// Lists are headed and grouped; what came, went or differs carries a sign and a tint and says so.
#[test]
fn lists_are_headed_and_their_changes_marked() {
    in_both(|h, host| {
        let (id, key) = element_open(h, host);
        let p = colours(h);
        let before = vec![
            list(ListKind::Attributes, vec![(Named::Said("id".into()), None), (Named::Said("status : String".into()), Some(ChangeKind::Changed))]),
            list(ListKind::Operations, vec![(Named::Said("old()".into()), Some(ChangeKind::Removed))]),
        ];
        let after = vec![
            list(ListKind::Attributes, vec![(Named::Said("id".into()), None), (Named::Said("status : Kind".into()), Some(ChangeKind::Changed))]),
            list(ListKind::Operations, vec![(Named::Said("new()".into()), Some(ChangeKind::Added))]),
        ];
        let lists = CardLists { before, after, ..CardLists::default() };
        landed(h, id, key, listed(id, "kept", lists));
        assert_eq!(named(h, "Attributes, id"), 2);
        assert_eq!(named(h, "Attributes, status : String, Changed"), 1, "{:?}", painted(h));
        assert_eq!(named(h, "Attributes, status : Kind, Changed"), 1);
        assert_eq!(named(h, "Operations, old(), Removed"), 1);
        assert_eq!(named(h, "Operations, new(), Added"), 1);
        assert!(!painted_has(h, "The owner changed."), "a member is not the owner");
        assert_eq!((painted_times(h, "ATTRIBUTES"), painted_times(h, "OPERATIONS")), (2, 2));
        assert_eq!((painted_times(h, "\u{2212}"), painted_times(h, "+")), (2, 2), "text marks as well as colour: {:?}", painted(h));
        assert!(filled(h, &format!("{BEFORE}.list.0.1"), p.danger_tint) && filled(h, &format!("{AFTER}.list.0.1"), p.green_tint));
        assert!(!filled(h, &format!("{BEFORE}.list.0.0"), p.danger_tint), "a same row is plain");
        let order = |h: &Harness<'static, Shell>, list: usize, row: usize| rect_of(h, &format!("{AFTER}.list.{list}.{row}")).top();
        assert!(order(h, 0, 1) < order(h, 1, 0), "grouped in order");
        assert!(rect_of(h, AFTER).bottom() >= rect_of(h, &format!("{AFTER}.list.1.0")).bottom(), "the card holds its rows");
    });
}

/// What isn't named says so in words: an owner that can't be read, no owner, and a nameless row by its kind.
#[test]
fn a_missing_name_is_said_in_words() {
    in_both(|h, host| {
        let (id, key) = element_open(h, host);
        let owner = |name| vec![list(ListKind::Ownership, vec![(name, None)])];
        let lists = CardLists { before: owner(Named::Unavailable), after: owner(Named::Nothing), ..CardLists::default() };
        landed(h, id, key.clone(), listed(id, "a", lists));
        assert_eq!(named(h, "Ownership, Not available on this side"), 1, "{:?}", painted(h));
        assert_eq!(named(h, "Ownership, None"), 1);
        let lists = CardLists { before: owner(Named::Unnamed), after: owner(Named::Said(String::new())), ..CardLists::default() };
        landed(h, id, key, listed(id, "a", lists));
        assert_eq!(named(h, "Ownership, Attribute"), 2, "a nameless row goes by its kind");
    });
}

/// A list far longer than the view draws only what shows, and scrolling reaches its last row.
#[test]
fn a_long_list_draws_only_rows_in_view() {
    in_both(|h, host| {
        let (id, key) = element_open(h, host);
        let rows = |tail: &str| vec![list(ListKind::Members, (0..20_000).map(|i| (Named::Said(format!("{tail}{i}")), None)).collect())];
        let lists = CardLists { before: rows("a"), after: rows("b"), ..CardLists::default() };
        landed(h, id, key, listed(id, "long", lists));
        let (first, last) = (format!("{BEFORE}.list.0.0"), format!("{BEFORE}.list.0.19999"));
        assert!(has_id(h, &first) && !has_id(h, &last));
        let drawn = |h: &Harness<'static, Shell>| painted(h).iter().filter(|t| t.starts_with('a') && t[1..].parse::<u32>().is_ok()).count();
        assert!((1..=100).contains(&drawn(h)), "rows out of sight are not drawn: {}", drawn(h));
        h.event(egui::Event::PointerMoved(rect_of(h, &first).center()));
        h.event(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, -600_000.0),
            phase: egui::TouchPhase::Move,
            modifiers: egui::Modifiers::NONE,
            source: egui::MouseWheelSource::Wheel,
        });
        h.run_steps(40);
        assert!(!has_id(h, &first) && has_id(h, &last), "the end is reached: {:?}", ids(h));
        assert!(drawn(h) <= 100, "still only what shows");
    });
}

/// Members that differ are counted beside the values, the same count however many rows hold it;
/// a changed owner is said in words, past the sentences a summary cuts, and is no value in the count.
#[test]
fn the_summary_counts_members_and_says_the_owner() {
    in_both(|h, host| {
        let (id, key) = element_open(h, host);
        let some = |n| CardLists { member_changes: n, ..CardLists::default() };
        landed(h, id, key.clone(), listed(id, "a", some(2)));
        says(h, "2 members changed", NOT_SHOWN);
        let fields = vec![field(Some(Value::Null), Some(Value::Enum(1)), true)];
        landed(h, id, key.clone(), Ok(Some(ElementSides { lists: Arc::new(some(1)), ..sided(id, schema("a"), schema("a"), fields) })));
        says(h, "1 value changed \u{b7} 1 member changed", &format!("Visibility changed from Not set to Private. {NOT_SHOWN}"));
        let moved = || vec![list(ListKind::Ownership, vec![(Named::Said("p".into()), Some(ChangeKind::Changed))])];
        let owned = |n| CardLists { before: moved(), after: moved(), member_changes: n };
        landed(h, id, key.clone(), listed(id, "a", owned(0)));
        says(h, "The owner changed", &format!("The owner changed. {NOT_SHOWN}"));
        let changed = field(Some(Value::Null), Some(Value::Enum(1)), true);
        let with_owner = |fields: Vec<CardField>| Ok(Some(ElementSides { lists: Arc::new(owned(0)), ..sided(id, schema("a"), schema("a"), fields) }));
        landed(h, id, key.clone(), with_owner(vec![changed.clone()]));
        says(h, "1 value changed", &format!("Visibility changed from Not set to Private. The owner changed. {NOT_SHOWN}"));
        landed(h, id, key, with_owner(vec![changed; 4]));
        let one = "Visibility changed from Not set to Private.";
        says(h, "4 values changed", &format!("{one} {one} {one} 1 more value changed. The owner changed. {NOT_SHOWN}"));
    });
}

/// The cards scroll as one, a row level with itself across them.
#[test]
fn the_cards_scroll_as_one() {
    in_both(|h, host| {
        let (id, key) = element_open(h, host);
        let fields = vec![field(Some(Value::Null), Some(Value::Null), false); 60];
        landed(h, id, key, Ok(Some(sided(id, schema("long"), schema("long"), fields))));
        let (first, last) = (format!("{BEFORE}.field.0"), format!("{AFTER}.field.59"));
        assert!(has_id(h, &first) && !has_id(h, &last), "the end is out of sight");
        assert!(has_id(h, SUMMARY), "at the top the summary shows");
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
        assert!(!has_id(h, SUMMARY), "the summary scrolls away with the cards");
        assert!(rect_of(h, BEFORE).bottom() <= h.ctx.content_rect().bottom(), "the scroll reaches the end of the cards");
    });
}

const SELECT_BEFORE: &str = "workspace.version-compare.before";
const SELECT_AFTER: &str = "workspace.version-compare.after";

/// The id of row `n` of a select's list, 0 being current work.
fn row_id(n: usize) -> String {
    format!("workspace.version-compare.list.row.{n}")
}

/// The row the keys are on, none with the list closed.
fn at(h: &Harness<'static, Shell>) -> Option<usize> {
    crate::version_details::compare::highlight(h.state())
}

/// A select lists This branch, current work and each version, marks what it holds and reads
/// nothing; a pick sets only its side and closes it, and a press elsewhere closes it too.
#[test]
fn a_select_lists_the_versions_and_a_pick_sets_its_side() {
    in_both(|h, host| {
        compared(h, "c3");
        let reads = (asked(host), host.version_asks.lock().unwrap().len());
        tap(h, SELECT_AFTER);
        assert!(painted_has(h, "THIS BRANCH") && has_id(h, &row_id(3)) && !has_id(h, &row_id(4)), "{:?}", ids(h));
        let marked: Vec<_> =
            h.query_all_by_role(egui::accesskit::Role::ListBoxOption).filter(|n| n.accesskit_node().is_selected() == Some(true)).collect();
        assert!(marked.len() == 1 && marked[0].accesskit_node().label().unwrap_or_default().starts_with("Current work"), "the one held");
        tap(h, &row_id(2));
        assert!(at(h).is_none() && holds(h, "After").contains("First classes") && holds(h, "Before").contains("Payment"), "its side");
        tap(h, SELECT_BEFORE);
        assert_eq!(at(h), Some(1), "it opens on the row it holds");
        tap(h, &row_id(0));
        assert_eq!(holds(h, "Before"), "Before, Current work");
        assert!(!has_id(h, VIEW) && !has_id(h, "workspace.version-details.failed"), "nothing compared yet");
        assert_eq!((asked(host), host.version_asks.lock().unwrap().len()), reads, "the list reads nothing");
        tap(h, SELECT_AFTER);
        tap(h, SELECT_AFTER);
        assert_eq!(at(h), None, "its own select closes it");
        tap(h, SELECT_AFTER);
        tap(h, SWAP);
        assert_eq!(at(h), None, "a press elsewhere closes it");
        tap(h, SELECT_AFTER);
        h.state_mut().state.explorer = crate::state::Explorer::Models;
        h.run_steps(2);
        h.state_mut().state.explorer = crate::state::Explorer::SourceControl;
        h.run_steps(2);
        assert_eq!(at(h), None, "it goes with the page, or its keys would stay taken");
    });
}

/// The arrows stay in the list, Enter and Space pick or open, Esc closes, the keyboard stays on the
/// select until a closed list lets Tab move on, and an open list keeps keys from the window behind.
#[test]
fn the_keyboard_moves_picks_closes_and_keeps_the_keys() {
    use egui::Key::{ArrowDown, ArrowUp, Enter, Escape, F10, Space, Tab};
    let press = |h: &mut Harness<'static, Shell>, k| key(h, k, egui::Modifiers::NONE);
    let shortcut = |h: &mut Harness<'static, Shell>| key(h, egui::Key::H, egui::Modifiers::COMMAND.plus(egui::Modifiers::SHIFT));
    in_both(|h, _| {
        h.state_mut().in_window_menu = true;
        compared(h, "c3");
        tap(h, SELECT_BEFORE);
        for (k, row) in [(ArrowDown, 2), (ArrowDown, 3), (ArrowDown, 3), (ArrowUp, 2)] {
            press(h, k);
            assert!(at(h) == Some(row) && focused_on(h, SELECT_BEFORE), "{k:?}");
        }
        shortcut(h);
        press(h, F10);
        assert!(at(h) == Some(2) && !has_id(h, "workspace.header.version.filter") && h.state().menu_bar.open.is_none(), "held behind the list");
        press(h, Enter);
        assert!(at(h).is_none() && holds(h, "Before").contains("First classes"), "{}", holds(h, "Before"));
        press(h, Enter);
        press(h, Escape);
        assert!(at(h).is_none() && holds(h, "Before").contains("First classes") && focused_on(h, SELECT_BEFORE), "Esc changes nothing");
        press(h, Space);
        assert_eq!(at(h), Some(2), "Space opens it too");
        press(h, ArrowUp);
        press(h, Space);
        assert!(at(h).is_none() && holds(h, "Before").contains("Payment"), "Space picks: {}", holds(h, "Before"));
        press(h, Tab);
        assert!(!focused_on(h, SELECT_BEFORE), "closed, Tab moves on again");
        press(h, F10);
        assert!(h.state().menu_bar.open.is_some(), "and the bar's key is free again");
        press(h, Escape);
        tap(h, SELECT_BEFORE);
        // Not by a press, which would close the list anyway: the native menu's Close has none.
        h.state_mut().close_project();
        frames_until(h, |h| h.state().project.is_none());
        press(h, F10);
        assert!(at(h).is_none() && h.state().menu_bar.open.is_some(), "Home gets its keys back");
    });
}

/// A long list scrolls with the keys, reads the next page only when its end shows, says when that
/// failed with Retry, adds what it read, and stops on an empty page whatever that says.
#[test]
fn the_list_reads_on_at_its_end_and_says_when_that_failed() {
    let press = |h: &mut Harness<'static, Shell>| key(h, egui::Key::ArrowDown, egui::Modifiers::NONE);
    let row = |i: usize| version_row(&format!("v{i}"), Some("A version"), (None, false), &format!("2026-11-{:02}T10:00:00Z", 20 - i));
    let send = |h: &mut Harness<'static, Shell>, host: &SavingHost, answer| {
        host.version_reads.lock().unwrap().pop().expect("a read is on its way").send(answer).unwrap();
        h.run_steps(2);
    };
    in_both_listing((0..15).map(row).collect(), |h, host| {
        h.state_mut().versions.page.as_mut().unwrap().more = true;
        h.run_steps(1);
        compared(h, "v3");
        let asks = || host.version_asks.lock().unwrap().iter().filter(|a| a.is_some()).count();
        tap(h, SELECT_AFTER);
        (0..11).for_each(|_| press(h));
        assert!(at(h) == Some(11) && has_id(h, &row_id(11)) && asks() == 0, "the row followed the keys; nothing read before the end shows");
        (0..4).for_each(|_| press(h));
        assert_eq!((at(h), asks()), (Some(15), 1), "the end shows: one read");
        send(h, host, Err("disk gone".into()));
        assert!(h.query_by_label("The next versions couldn't be read.").is_some() && asks() == 1, "no read after a failure until Retry");
        tap(h, "workspace.version-compare.list.retry");
        assert!(asks() == 2 && at(h).is_some(), "Retry asks again and keeps the list");
        send(h, host, Ok(crate::VersionsPage { versions: vec![row(16)], more: true }));
        press(h);
        assert!(at(h) == Some(16) && asks() == 3, "the page read is a row, and its end shows");
        send(h, host, Ok(crate::VersionsPage { versions: Vec::new(), more: true }));
        assert!(asks() == 3 && !h.state().versions.page.as_ref().unwrap().more, "an empty page ends it");
    });
}

const RESTORE: &str = "workspace.version-details.restore";
const ON_BANNER: &str = "workspace.banner.restore";
const GO: &str = "restore-ask.restore";
const CLOSE: &str = "restore-ask.close";

/// Asks to restore the version `row` from its details.
fn asking(h: &mut Harness<'static, Shell>, row: &str) {
    opened(h, row);
    tap(h, RESTORE);
    h.run_steps(2);
}

/// Answers the question open with Restore or Retry; the read answered, its job waits for the test.
fn restoring(h: &mut Harness<'static, Shell>, host: &SavingHost, commit: &str) -> RestoreEnds {
    tap(h, GO);
    h.run_steps(3);
    answer_all(h, host, commit);
    h.run_steps(4);
    host.restores.lock().unwrap().remove(0)
}

/// Answers every read of `commit`, a comparison's among them.
fn answer_all(h: &Harness<'static, Shell>, host: &SavingHost, commit: &str) {
    while asked(host).iter().any(|c| c == commit) {
        answer(host, commit, Ok(rows_now(h)));
    }
}

/// Shows the version `row` from its details.
fn viewing(h: &mut Harness<'static, Shell>, host: &SavingHost, row: &str) {
    opened(h, row);
    tap(h, VIEW);
    h.run_steps(3);
    answer_all(h, host, row);
    h.run_steps(4);
    assert_eq!(h.state().viewing(), Some(row));
}

/// The details ask before anything starts and say what the restore makes, unsaved work included;
/// Cancel starts nothing, and the job is told the date the question quotes.
#[test]
fn the_details_ask_first_and_say_what_a_restore_makes() {
    in_both(|h, host| {
        asking(h, "c2");
        assert!(painted_has(h, "The version of 7 Nov 09:20 · First classes") && painted_has(h, "Undo starts fresh"), "{:?}", painted(h));
        assert!(painted_has(h, "\"Restored the version of 7 Nov 09:20\"") && !painted_has(h, "Before restoring"), "nothing unsaved");
        tap(h, CLOSE);
        assert!(h.state().ask.is_none() && host.restores.lock().unwrap().is_empty() && !h.state().is_held());
        add_schema(h, "shop");
        list_still(h);
        asking(h, "c3");
        assert!(painted_has(h, "\"Before restoring\""), "the last version is on offer with unsaved work: {:?}", painted(h));
        let _job = restoring(h, host, "c3");
        assert!(h.state().is_held() && h.state().ask.is_none());
        assert_eq!(*host.restore_dates.lock().unwrap(), ["7 Nov 17:15"]);
    });
}

/// The banner asks too, naming the version in view, and restoring ends the viewing first.
#[test]
fn the_banner_asks_and_restoring_ends_the_viewing() {
    in_both(|h, host| {
        viewing(h, host, "c2");
        tap(h, ON_BANNER);
        h.run_steps(2);
        assert!(painted_has(h, "The version of 7 Nov 09:20"), "{:?}", painted(h));
        let _job = restoring(h, host, "c2");
        assert!(h.state().viewing().is_none() && h.state().is_held());
    });
}

/// The last version with nothing unsaved is current work, and a version that can't be read can't
/// be restored: neither is offered, in the details or the banner.
#[test]
fn restore_is_not_offered_for_current_work_or_an_unreadable_version() {
    in_both(|h, host| {
        opened(h, "c3");
        assert!(has_id(h, VIEW) && !has_id(h, RESTORE), "the last version");
        h.state_mut().versions.unavailable.insert("c2".into(), Some("gone".into()));
        opened(h, "c2");
        assert!(has_id(h, VIEW) && !has_id(h, RESTORE), "unreadable");
        h.state_mut().versions.unavailable.clear();
        opened(h, "c2");
        assert!(has_id(h, RESTORE), "offered");
        viewing(h, host, "c3");
        versions_answer(h, host, Ok(crate::VersionsPage { versions: history(), more: false }));
        h.run_steps(2);
        assert!(!has_id(h, ON_BANNER), "the last version in view");
    });
}

/// A failure brings the question back saying what happened, with Retry unless trying again can't
/// help; one before anything is written says that nothing changed.
#[test]
fn a_failure_comes_back_and_says_what_happened() {
    use crate::replace::JobFailure::{Failed, WorkSaved};
    in_both(|h, host| {
        asking(h, "c2");
        let (ended, _reading) = restoring(h, host, "c2");
        ended.send(Err(WorkSaved("Your unsaved work was kept as a version.".into()))).unwrap();
        h.run_steps(4);
        assert!(
            painted_has(h, "The restore did not finish")
                && painted_has(h, "Your unsaved work was kept")
                && painted_has(h, "Retry")
                && !h.state().is_held()
        );
        let (ended, _reading) = restoring(h, host, "c2");
        ended.send(Err(Failed("This version is the same as your current work.".into()))).unwrap();
        h.run_steps(4);
        assert!(painted_has(h, "This version is the same") && !has_id(h, GO), "nothing to try again");
        tap(h, CLOSE);
        assert!(h.state().ask.is_none() && h.state().versions.details.is_some());
        tap(h, RESTORE);
        h.run_steps(2);
        tap(h, GO);
        h.run_steps(3);
        answer(host, "c2", Err("gone".to_string().into()));
        h.run_steps(4);
        assert!(painted_has(h, "gone Nothing was changed.") && has_id(h, GO), "{:?}", painted(h));
    });
}

/// With a Chat turn running, Restore asks about it next and Keep working ends it; typing goes
/// nowhere behind the question, and Escape cancels it.
#[test]
fn chat_still_asks_before_a_restore_and_keys_stay_with_the_question() {
    use crate::close_ask::Ask;
    in_both_appearances(|theme| {
        let (mut h, _rig, _host, _) = chat_and_a_version(theme);
        h.state_mut().state.explorer = crate::state::Explorer::SourceControl;
        crate::version_details::open(h.state_mut(), history().remove(1));
        ask_in_chat(&mut h, "A long question");
        let at = rect_of(&h, "workspace.chat.composer").center();
        click(&mut h, at);
        h.event(egui::Event::Text("y".into()));
        h.run();
        crate::restore_ask::ask(h.state_mut(), ("c2".into(), "d".into(), "d".into()), None);
        h.run_steps(2);
        h.event(egui::Event::Text("x".into()));
        h.run();
        assert_eq!(h.state().chat_panel.draft, "y", "held behind the question");
        key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
        assert!(h.state().ask.is_none());
        for answer in ["close-ask.keep", "close-ask.stop-chat"] {
            tap(&mut h, RESTORE);
            h.run_steps(2);
            tap(&mut h, GO);
            assert!(matches!(h.state().ask, Some(Ask::StopChat { .. })) && !h.state().is_held());
            tap(&mut h, answer);
            h.run_steps(1);
            assert_eq!(h.state().is_held(), answer == "close-ask.stop-chat", "{answer}");
            assert_eq!(h.state().versions.restoring.is_some(), h.state().is_held());
        }
        crate::restore_ask::ask(h.state_mut(), ("c2".into(), "d".into(), "d".into()), None);
        h.state_mut().session_replaced();
        h.run_steps(2);
        assert!(h.state().ask.is_none(), "about the session left");
    });
}
