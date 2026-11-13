//! The question before a branch switch and the failure after it (`crate::switch_ask`).

use super::*;
use crate::SaveError;
use crate::close_ask::Ask;

const SAVE: &str = "switch-ask.save";
const SWITCH: &str = "switch-ask.switch";
const CANCEL: &str = "switch-ask.cancel";
const CLOSE: &str = "switch-ask.close";

/// Asks to switch to `elsewhere` from `main`, and draws the answer.
fn asking(h: &mut Harness<'static, Shell>, host: &SavingHost) {
    *host.branch_now.lock().unwrap() = Some("main".to_string());
    h.state_mut().ask_switch_branch("elsewhere".to_string()).unwrap();
    h.run_steps(2);
}

/// Answers the question with `answer`, then lets the switch reach the host.
fn answered(h: &mut Harness<'static, Shell>, host: &SavingHost, answer: &str) {
    asking(h, host);
    click_id(h, answer);
    h.run_steps(4);
}

/// Unsaved work asks first, saying where it stays and that undo starts fresh; Cancel does nothing.
/// Switch goes without saving, and the question's Save version and switch saves first.
#[test]
fn unsaved_work_asks_and_each_answer_does_what_it_says() {
    in_both(work_and_earlier, |mut h, host, earlier| {
        asking(&mut h, &host);
        assert!(matches!(h.state().ask, Some(Ask::Switch { .. })));
        let wording = ["Switch to elsewhere?", "main has unsaved changes. They stay on main", "won't be able to undo earlier steps"];
        assert!(wording.iter().all(|w| painted_has(&h, w)), "{:?}", painted(&h));
        click_id(&mut h, CANCEL);
        h.run_steps(4);
        assert!(h.state().ask.is_none());
        assert!(!h.state().is_held() && host.switches.lock().unwrap().is_empty() && host.saves.lock().unwrap().is_empty());

        answered(&mut h, &host, SAVE);
        assert!(h.state().is_held() && host.switches.lock().unwrap().is_empty(), "the switch waits for the save");
        assert_eq!(*host.descriptions.lock().unwrap(), [None], "an ordinary Save, its description automatic");
        host.saves.lock().unwrap().pop().unwrap().send(Ok(true)).unwrap();
        h.run_steps(6);
        let ends = host.switches.lock().unwrap().remove(0);
        assert_eq!(ends.branch, "elsewhere");
        assert!(!h.state().shown_unsaved(), "saved");
        switch_checked(&mut h, &ends, earlier);
        switch_moved(&mut h, &ends);
        the_version_is_installed(&h);
        assert!(h.state().ask.is_none() && h.state().versions.switching.is_none());

        add_schema(&mut h, "again");
        answered(&mut h, &host, SWITCH);
        assert!(h.state().is_held() && host.descriptions.lock().unwrap().len() == 1, "Switch saves nothing");
        assert_eq!(host.switches.lock().unwrap().len(), 1, "and goes at once");
    });
}

/// No unsaved work: no question, the switch starts at once.
#[test]
fn no_unsaved_work_switches_at_once() {
    in_both_appearances(|theme| {
        let (mut h, host) = saving_harness(Some(commerce()), None);
        h.state_mut().state.theme = theme;
        asking(&mut h, &host);
        assert!(h.state().ask.is_none() && h.state().is_held());
        h.run_steps(4);
        let ends = host.switches.lock().unwrap().remove(0);
        ends.check.send(Err("Gone.".to_string().into())).unwrap();
        h.run_steps(4);
        assert!(painted_has(&h, "Gone. You are still on main and your work is unchanged."), "{:?}", painted(&h));
    });
}

/// A failed save stops the switch and says whether a version was made and that nothing moved; a
/// switch failing after the save says the work is saved; one that can't write its rows is refused.
#[test]
fn a_failed_save_or_switch_says_what_became_of_the_work() {
    let no_version = SaveError::Commit { why: "disk full".into(), version: crate::VersionAfter::None };
    in_both(work_and_earlier, |mut h, host, _| {
        answered(&mut h, &host, SAVE);
        host.saves.lock().unwrap().pop().unwrap().send(Err(no_version.clone())).unwrap();
        h.run_steps(5);
        let said = ["Couldn't switch to elsewhere", "No version was made: disk full", "You are still on main and your work is unchanged."];
        assert!(said.iter().all(|w| painted_has(&h, w)) && !has_id(&h, SWITCH), "{:?}", painted(&h));
        assert!(host.switches.lock().unwrap().is_empty() && !h.state().is_held() && h.state().shown_unsaved());
        click_id(&mut h, CLOSE);
        h.run_steps(2);
        assert!(h.state().ask.is_none());

        answered(&mut h, &host, SAVE);
        host.saves.lock().unwrap().pop().unwrap().send(Ok(true)).unwrap();
        h.run_steps(6);
        let ends = host.switches.lock().unwrap().remove(0);
        ends.check.send(Err("The branch can't be opened.".to_string().into())).unwrap();
        h.run_steps(4);
        assert!(painted_has(&h, "You are still on main. Your work is saved."), "{:?}", painted(&h));
        assert!(!painted_has(&h, "unchanged") && !h.state().is_held());
        click_id(&mut h, CLOSE);

        add_schema(&mut h, "again");
        asking(&mut h, &host);
        // The host could not write the work: the switch it is asked for is refused.
        *host.write_error.lock().unwrap() = Some("No rows.".into());
        click_id(&mut h, SWITCH);
        h.run_steps(6);
        host.switches.lock().unwrap().remove(0).check.send(Err("No rows.".to_string().into())).unwrap();
        h.run_steps(4);
        let said = ["Switching is refused", "No rows. Your work could not be kept", "You are still on main."];
        assert!(said.iter().all(|w| painted_has(&h, w)), "{:?}", painted(&h));
        assert!(!h.state().is_held());
    });
}

/// A switch that fails behind another question says so, as it stood then, once that one closes and
/// its project shows again; once another session has replaced the one it ended on, it is dropped.
#[test]
fn a_failure_waits_for_the_question_up_when_it_ends() {
    in_both(work_and_earlier, |mut h, host, earlier| {
        let fail_behind_a_question = |h: &mut Harness<'static, Shell>| {
            answered(h, &host, SWITCH);
            h.state_mut().ask = Some(Ask::Unsaved { path: h.state().project.as_ref().unwrap().path.clone() });
            host.switches.lock().unwrap().remove(0).check.send(Err("Gone.".to_string().into())).unwrap();
            h.run_steps(4);
            assert!(matches!(h.state().ask, Some(Ask::Unsaved { .. })) && !h.state().is_held());
        };
        fail_behind_a_question(&mut h);
        let tab = h.state().active.unwrap();
        (h.state_mut().save_error, h.state_mut().ask) = (Some("Later.".into()), None);
        h.state_mut().stash();
        h.run_steps(2);
        assert!(h.state().ask.is_none() && h.state().versions.switching.is_some(), "kept while another project shows");
        h.state_mut().activate(tab);
        h.run_steps(3);
        assert!(painted_has(&h, "Gone. You are still on main and your work is unchanged."), "{:?}", painted(&h));
        click_id(&mut h, CLOSE);
        h.run_steps(2);
        fail_behind_a_question(&mut h);
        (h.state_mut().session, h.state_mut().ask) = (Some(h.state().build(&earlier).unwrap()), None);
        crate::switch_ask::keep(h.state_mut());
        assert!(h.state().ask.is_none() && h.state().versions.switching.is_none());
    });
}

/// Save answered to another tab's question shows that tab and closes it: a switch failure kept
/// meanwhile isn't shown over that tab's session, and is shown once its own session shows again.
#[test]
fn a_failure_outlasts_saving_and_closing_another_tab() {
    in_both(work_and_earlier, |mut h, host, _| {
        let (library, commerce) = (std::path::PathBuf::from("/tmp/Library.archforge"), commerce().path);
        h.state_mut().open_project(crate::Opened { name: "Library".into(), path: library.clone(), ..Default::default() });
        h.run_steps(2);
        add_schema(&mut h, "books");
        let ended_on = h.state().session.as_ref().unwrap().history();
        answered(&mut h, &host, SWITCH);
        h.state_mut().ask = Some(Ask::Unsaved { path: commerce.clone() });
        host.switches.lock().unwrap().remove(0).check.send(Err("Gone.".to_string().into())).unwrap();
        h.run_steps(4);
        click_id(&mut h, "close-ask.save");
        h.run_steps(3);
        assert!(h.state().project.as_ref().is_some_and(|p| p.path == commerce) && h.state().ask.is_none(), "not over Commerce");
        assert!(h.state().versions.switching.is_some(), "kept");
        host.saves.lock().unwrap().pop().unwrap().send(Ok(true)).unwrap();
        h.run_steps(3);
        host.closings.lock().unwrap().pop().unwrap().send(Ok(())).unwrap();
        h.run_steps(4);
        assert_eq!(h.state().session.as_ref().map(|s| s.history()), Some(ended_on), "Library's own session shows again");
        assert!(painted_has(&h, "Gone. You are still on main and your work is unchanged."), "{:?}", painted(&h));
    });
}

/// With a Chat turn running, its question comes first; Stop and go on leads to the question about
/// unsaved work, and the question about Chat isn't asked again.
#[test]
fn chat_is_asked_about_first_and_only_once() {
    in_both_appearances(|theme| {
        let (mut h, _rig, host, _) = chat_and_a_version(theme);
        *host.branch_now.lock().unwrap() = Some("main".to_string());
        add_schema(&mut h, "shop");
        ask_in_chat(&mut h, "A long question");
        assert_eq!(h.state_mut().ask_switch_branch("elsewhere".to_string()), Err(crate::hold::HoldRefused::ChatRunning));
        h.run();
        click_id(&mut h, "close-ask.stop-chat");
        h.run();
        assert!(matches!(h.state().ask, Some(Ask::Switch { .. })) && !h.state().is_held(), "then the question about unsaved work");
        h.run_steps(1);
        click_id(&mut h, SWITCH);
        h.run_steps(3);
        assert!(h.state().is_held(), "Chat's question isn't asked again");
    });
}

/// While the question is up, typed keys and Undo don't reach the window behind; Escape cancels.
#[test]
fn keys_stay_with_the_question_and_escape_cancels() {
    in_both_appearances(|theme| {
        let (mut h, _rig, host, _) = chat_and_a_version(theme);
        add_schema(&mut h, "shop");
        let at = rect_of(&h, "workspace.chat.composer").center();
        click(&mut h, at);
        h.event(egui::Event::Text("y".into()));
        h.run();
        let before = h.state().session.as_ref().unwrap().revision();
        asking(&mut h, &host);
        h.event(egui::Event::Text("x".into()));
        key(&mut h, egui::Key::Z, egui::Modifiers::COMMAND);
        h.run_steps(2);
        assert_eq!(h.state().chat_panel.draft, "y", "held behind the question");
        assert_eq!(h.state().session.as_ref().unwrap().revision(), before, "Undo didn't reach the model");
        key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
        h.run_steps(2);
        assert!(h.state().ask.is_none());
        asking(&mut h, &host);
        h.state_mut().session_replaced();
        h.run_steps(2);
        assert!(h.state().ask.is_none(), "about the session left");
        assert!(!h.state().is_held() && host.switches.lock().unwrap().is_empty() && host.saves.lock().unwrap().is_empty());
    });
}
