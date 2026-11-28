//! Chat over a UML model: a question with its chips sent and answered, the changes a turn made to
//! the model counted, undone and redone as its own, and requests in a conversation's name refused
//! once its program ends.

use super::*;

/// A question is kept before the agent gets it, with its chips as a context line; Enter sends, Shift+Enter starts a line.
#[test]
fn a_question_is_kept_before_it_is_sent_with_its_chips_as_a_context_line_and_streams_back() {
    use archforge_ui_egui::chat::ChatEvent;
    let (mut h, rig) = chat_harness(ThemeKind::Light, true);
    // A removed chip stays removed for the conversation and is not sent.
    let at = rect_of(&h, "workspace.chat.chip.project.remove").center();
    click(&mut h, at);
    assert!(!ids(&h).iter().any(|i| i == "workspace.chat.chip.project.remove"));
    ask_in_chat(&mut h, "Which classes have no description yet?");
    assert_eq!(*rig.store.log.lock().unwrap(), ["create", "chips", "ask"], "made, its removed chip kept, and asked before the agent gets it");
    assert_eq!(h.state().chat_panel.draft, "", "the composer empties once the question is kept");
    let started = rig.agents.started.lock().unwrap().clone();
    assert_eq!(started.len(), 1);
    let start = &started[0];
    assert_eq!(
        (start.provider, start.model.as_str(), start.origin.as_str(), start.allow_changes, start.resume),
        (archforge_ui_egui::ai_agents::Provider::Claude, "sonnet", "Chat 1", false, false)
    );
    assert_eq!(chat_calls(&rig).1, 1, "sent once");
    let chat = kept_chat(&h, &rig);
    assert_eq!((chat.row.origin.as_str(), chat.row.title.as_str()), ("Chat 1", "Which classes have no description yet?"));
    // The chips go as a line after the question, by name and id; the removed one does not.
    let order = archforge_ui_egui::chat_fixture::element(h.state(), archforge_uml::CLASS, "Order").unwrap();
    let line = archforge_ui_egui::chat_panel::context_line(
        &archforge_ui_egui::chat_panel::chips(h.state())
            .into_iter()
            .filter(|c| c.chip != archforge_ui_egui::chat_panel::Chip::Project)
            .flat_map(|c| c.anchors)
            .collect::<Vec<_>>(),
    );
    assert!(line.contains("selected \"Order\"") && line.contains(&order.to_string()) && line.contains("open diagram \"Commerce\""), "{line}");
    assert!(!line.contains("project"), "{line}");
    // The reply streams, then completes; Started names the model on the badge.
    chat_event(&mut h, &rig, ChatEvent::Started { model: archforge_ui_egui::chat_fixture::NAMED.into(), access: true });
    assert!(painted_has(&h, "Claude · Sonnet 4.5"));
    chat_event(&mut h, &rig, ChatEvent::Text { delta: "Five of the eight ".into() });
    assert!(painted_has(&h, "Five of the eight ▍") || painted_has(&h, "Five of the eight"), "{:?}", painted(&h));
    assert!(ids(&h).iter().any(|i| i == "workspace.chat.stop") && !ids(&h).iter().any(|i| i == "workspace.chat.send"));
    assert!(painted_has(&h, "Claude is replying… you can type the next message"));
    chat_event(&mut h, &rig, ChatEvent::Message { text: "Five of the **eight** classes.".into() });
    chat_event(&mut h, &rig, ChatEvent::TurnEnded(archforge_ui_egui::chat::TurnEnd::Completed));
    assert!(painted_has(&h, "Five of the eight classes."), "rendered as Markdown: {:?}", painted(&h));
    let chat = kept_chat(&h, &rig);
    assert_eq!(chat.turns[0].outcome, archforge_ui_egui::chat_store::Outcome::Completed);
    assert!(
        matches!(&chat.messages[1].body, archforge_ui_egui::chat_store::StoredBody::Reply { text, complete: true, .. } if text == "Five of the **eight** classes.")
    );
    // Enter sends, Shift+Enter makes a new line.
    let at = rect_of(&h, "workspace.chat.composer").center();
    click(&mut h, at);
    h.event(egui::Event::Text("Line one".into()));
    key(&mut h, egui::Key::Enter, egui::Modifiers::SHIFT);
    h.event(egui::Event::Text("line two".into()));
    h.run();
    assert_eq!(h.state().chat_panel.draft, "Line one\nline two");
    assert_eq!(chat_calls(&rig).1, 1, "Shift+Enter sent nothing");
}

/// While the program of a conversation is still ending after Allow changes went off, a question
/// asked meanwhile waits for its end; requests in that conversation's name are refused until then,
/// so the ending program cannot act for the new, read-only turn (AR-228). In both appearances.
#[test]
fn requests_in_the_name_of_a_conversation_whose_program_ends_are_refused() {
    use archforge_ui_egui::chat::ChatEvent;
    use serde_json::json;
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, rig) = chat_harness(theme, true);
        let at = rect_of(&h, "workspace.chat.allow-changes").center();
        click(&mut h, at);
        ask_in_chat(&mut h, "Describe Customer.");
        chat_event(&mut h, &rig, ChatEvent::TurnEnded(archforge_ui_egui::chat::TurnEnd::Completed));
        let origin = h.state().chat_panel.chat.as_ref().and_then(|c| c.origin.clone()).unwrap();
        // Allow changes off: its program is asked to end, and is slow to.
        rig.agents.hold_ends.store(true, std::sync::atomic::Ordering::Relaxed);
        let at = rect_of(&h, "workspace.chat.allow-changes").center();
        click(&mut h, at);
        assert_eq!(h.state().chat_panel.chat.as_ref().map(|c| c.session), Some(archforge_ui_egui::chat_panel::Session::Ending), "{theme:?}");
        ask_in_chat(&mut h, "And Product?");
        let chat = h.state().chat_panel.chat.as_ref().unwrap();
        assert!(chat.queued.is_some(), "{theme:?}: the new question waits for the end");
        assert_eq!(
            chat.turns.last().map(|t| t.state.clone()),
            Some(archforge_ui_egui::chat_panel::TurnState::Running),
            "{theme:?}: as a running turn"
        );
        let change = json!({ "command": "uml.class.set", "params": {} });
        // Its token is known by the key it was granted for (AR-230).
        let key = archforge_ui_egui::chat_fixture::key(h.state());
        let refused = archforge_ui_egui::testing::chat_gate(h.state(), Some(&origin), Some(&key), "engine.execute", &change);
        assert!(refused.as_ref().unwrap_err().contains("is being ended"), "{theme:?}: refused while its program ends: {refused:?}");
        // Known by its key, not its label: under another label it is refused the same way.
        let other = format!("{origin}!");
        let refused = archforge_ui_egui::testing::chat_gate(h.state(), Some(&other), Some(&key), "engine.execute", &change);
        assert!(refused.as_ref().unwrap_err().contains("is being ended"), "{theme:?}: refused by its key under another label: {refused:?}");
    }
}

/// Undo this turn's changes takes back exactly the turn's changes, keeps the person's, and says when it cannot.
#[test]
fn undo_this_turns_changes_takes_back_exactly_the_turns_changes_and_keeps_the_persons() {
    use archforge_ui_egui::chat::ChatEvent;
    use archforge_ui_egui::chat_fixture as fx;
    let describe = |h: &mut Harness<'static, Shell>, name: &str, text: &str| {
        let id = fx::element(h.state(), archforge_uml::CLASS, name).unwrap();
        let answer = fx::call(
            h.state_mut(),
            "engine.execute",
            serde_json::json!({ "command": "element.describe", "params": { "element": id.to_string(), "text": text } }),
        );
        h.run();
        answer
    };
    let description = |h: &Harness<'static, Shell>, name: &str| {
        let s = h.state();
        let st = s.session.as_ref().unwrap().doc();
        let id = fx::element(s, archforge_uml::CLASS, name).unwrap();
        archforge_ui_egui::testing::description_now(s, st.by_uuid(id).unwrap())
    };
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, rig) = chat_harness(theme, true);
        // The person's own edit first.
        let order = fx::element(h.state(), archforge_uml::CLASS, "Order").unwrap();
        h.state_mut()
            .session
            .as_mut()
            .unwrap()
            .execute("element.describe", &serde_json::json!({ "element": order.to_string(), "text": "Mine" }))
            .unwrap();
        // Read-only, a change is refused and its row says so.
        ask_in_chat(&mut h, "Describe Customer.");
        assert!(describe(&mut h, "Customer", "A buyer").is_err());
        assert!(painted_has(&h, "Changed Customer") && painted_has(&h, "refused"), "{theme:?}: {:?}", painted(&h));
        chat_event(&mut h, &rig, ChatEvent::TurnEnded(archforge_ui_egui::chat::TurnEnd::Completed));
        // Changes allowed: two changes, one step of the undo history.
        let at = rect_of(&h, "workspace.chat.allow-changes").center();
        click(&mut h, at);
        assert!(h.state().chat_panel.chat.as_ref().unwrap().allow_changes);
        ask_in_chat(&mut h, "Describe Customer and Product.");
        describe(&mut h, "Customer", "A buyer").unwrap();
        describe(&mut h, "Product", "A thing sold").unwrap();
        chat_event(&mut h, &rig, ChatEvent::Message { text: "Done.".into() });
        chat_event(&mut h, &rig, ChatEvent::TurnEnded(archforge_ui_egui::chat::TurnEnd::Completed));
        assert!(
            painted_has(&h, "2 changes to the model") && painted_has(&h, "Made in this turn, as one step of the project's undo history."),
            "{theme:?}"
        );
        assert_eq!(h.state().session.as_ref().unwrap().undo_label(), Some("Chat 1: Turn 2"));
        let kept = kept_chat(&h, &rig);
        let change = kept.turns[1].change.clone().unwrap();
        assert_eq!((change.changes, change.state), (2, archforge_ui_egui::chat_store::ChangeState::Applied));
        let at = rect_of(&h, "workspace.chat.turn.2.undo").center();
        click(&mut h, at);
        assert_eq!(
            (description(&h, "Customer"), description(&h, "Product")),
            (String::new(), String::new()),
            "{theme:?}: the turn's changes are gone"
        );
        assert_eq!(description(&h, "Order"), "Mine", "{theme:?}: the person's own edit stays");
        assert!(painted_has(&h, "2 changes undone"), "{theme:?}");
        assert_eq!(kept_chat(&h, &rig).turns[1].change.as_ref().map(|c| c.state), Some(archforge_ui_egui::chat_store::ChangeState::Undone));
        // Redo brings them back, and the record follows.
        h.state_mut().session.as_mut().unwrap().redo().unwrap();
        h.run();
        assert_eq!(description(&h, "Customer"), "A buyer");
        assert_eq!(kept_chat(&h, &rig).turns[1].change.as_ref().map(|c| c.state), Some(archforge_ui_egui::chat_store::ChangeState::Applied));
        // Another change since: the turn's changes can no longer be undone on their own.
        h.state_mut()
            .session
            .as_mut()
            .unwrap()
            .execute("element.describe", &serde_json::json!({ "element": order.to_string(), "text": "Mine again" }))
            .unwrap();
        h.run();
        assert!(!ids(&h).iter().any(|i| i == "workspace.chat.turn.2.undo"), "{theme:?}");
        assert!(painted_has(&h, "The model changed after this turn, so its changes can no longer be undone on their own."), "{theme:?}");
    }
}

/// A failed turn sent again from its card goes with its chips' context line again; a question back
/// in the composer is the one sent and shows once; a lost connection in a turn that changed the
/// model leaves its question where it is, and the composer asks anew.
#[test]
fn send_again_keeps_the_context_and_the_composer_never_sticks() {
    use archforge_ui_egui::chat::ChatEvent;
    use archforge_ui_egui::chat::FailureKind;
    use archforge_ui_egui::chat_fixture as fx;
    let (mut h, rig) = chat_harness(ThemeKind::Light, true);
    ask_in_chat(&mut h, "First");
    chat_event(&mut h, &rig, failed_turn(FailureKind::Other));
    let at = rect_of(&h, "workspace.chat.turn.1.retry").center();
    click(&mut h, at);
    let sent = sent_texts(&rig);
    assert_eq!(sent.len(), 2);
    assert!(sent.iter().all(|t| t.starts_with("First") && t.contains("[Context the person chose:")), "{sent:?}");
    chat_event(&mut h, &rig, ChatEvent::TurnEnded(archforge_ui_egui::chat::TurnEnd::Completed));
    // Back in the composer: the card's Send again sends what the composer holds, once.
    ask_in_chat(&mut h, "Second");
    chat_event(&mut h, &rig, failed_turn(FailureKind::Network));
    assert_eq!(h.state().chat_panel.draft, "Second");
    let at = rect_of(&h, "workspace.chat.turn.2.retry").center();
    click(&mut h, at);
    assert_eq!(h.state().chat_panel.draft, "", "the composer emptied");
    assert_eq!(painted_times(&h, "Second"), 1, "shown once");
    chat_event(&mut h, &rig, ChatEvent::TurnEnded(archforge_ui_egui::chat::TurnEnd::Completed));
    assert_eq!(kept_chat(&h, &rig).turns.len(), 2);
    // A turn that changed the model and then lost its connection: its question stays in its bubble
    // and the composer asks a new question.
    let at = rect_of(&h, "workspace.chat.allow-changes").center();
    click(&mut h, at);
    ask_in_chat(&mut h, "Third");
    let customer = fx::element(h.state(), archforge_uml::CLASS, "Customer").unwrap();
    fx::call(
        h.state_mut(),
        "engine.execute",
        serde_json::json!({ "command": "element.describe", "params": { "element": customer.to_string(), "text": "A buyer" } }),
    )
    .unwrap();
    chat_event(&mut h, &rig, failed_turn(FailureKind::Network));
    assert_eq!(h.state().chat_panel.draft, "", "not back in the composer");
    assert_eq!(painted_times(&h, "Third"), 1);
    ask_in_chat(&mut h, "Fourth");
    assert_eq!(kept_chat(&h, &rig).turns.len(), 4, "a new turn");
}

/// A request on behalf of a conversation acts only while that conversation's turn runs in the
/// project shown: never for one let go of, nor for a conversation parked in another project, even
/// under the same label, as each is known by its key (AR-230); a key the window does not know never
/// stands in for a label it knows, a conversation's request without a label is still checked, and
/// the person's own token naming a label keeps the label rules. It never undoes the person's own
/// step. In both appearances.
#[test]
fn a_conversations_origin_never_acts_for_another_and_never_undoes_the_persons_step() {
    use archforge_ui_egui::chat_fixture as fx;
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, rig) = chat_harness(theme, true);
        let at = rect_of(&h, "workspace.chat.allow-changes").center();
        click(&mut h, at);
        ask_in_chat(&mut h, "Describe Order");
        let order = fx::element(h.state(), archforge_uml::CLASS, "Order").unwrap();
        // The person's own edit on top: the conversation may not undo it.
        h.state_mut()
            .session
            .as_mut()
            .unwrap()
            .execute("element.describe", &serde_json::json!({ "element": order.to_string(), "text": "Mine" }))
            .unwrap();
        let undo = fx::call(h.state_mut(), "engine.undo", serde_json::json!({}));
        assert!(undo.unwrap_err().contains("may undo only its own changes"));
        let undo = fx::call(h.state_mut(), "engine.execute", serde_json::json!({ "command": "edit.undo", "params": {} }));
        assert!(undo.unwrap_err().contains("may undo only its own changes"), "the edit.undo command alike");
        // Known by its key, not by the label a request names: refused under another label alike.
        let key = fx::key(h.state());
        let other = format!("{}!", h.state().chat_panel.chat.as_ref().and_then(|c| c.origin.clone()).unwrap());
        let undo = archforge_ui_egui::testing::chat_gate(h.state(), Some(&other), Some(&key), "engine.undo", &serde_json::json!({}));
        assert!(undo.unwrap_err().contains("may undo only its own changes"), "{theme:?}: under another label alike");
        assert_eq!(archforge_ui_egui::testing::description_now(h.state(), h.state().session.as_ref().unwrap().doc().by_uuid(order).unwrap()), "Mine");
        // Its own change it may undo.
        fx::call(
            h.state_mut(),
            "engine.execute",
            serde_json::json!({ "command": "element.describe", "params": { "element": order.to_string(), "text": "Its" } }),
        )
        .unwrap();
        // Which steps are its own is decided by its key, whatever label the request names.
        let undo = archforge_ui_egui::testing::chat_gate(h.state(), Some(&other), Some(&key), "engine.undo", &serde_json::json!({}));
        assert!(undo.is_ok(), "{theme:?}: its own step, under another label: {undo:?}");
        assert!(fx::call(h.state_mut(), "engine.undo", serde_json::json!({})).is_ok());
        // Another project open with its own Chat 1 running, while this one's is still running parked:
        // each is known by its key (AR-230), so the shown one acts for itself and the parked one,
        // under the same label, is refused there.
        let origin = h.state().chat_panel.chat.as_ref().unwrap().origin.clone();
        let parked_key = fx::key(h.state());
        let _ = &rig;
        h.state_mut().open_project(archforge_ui_egui::Opened { name: "Second".into(), path: "/tmp/Second.archforge".into(), ..Default::default() });
        h.run();
        archforge_ui_egui::snapshot::cx(h.state_mut(), uml_ui::open_sample).unwrap();
        h.state_mut().state.show_panel(archforge_shell_api::state::Panel::Agent);
        h.run();
        ask_in_chat(&mut h, "Here");
        // Labels are numbered per project: the fake keeps one list for both, so the label is set as
        // the other project would have given it.
        h.state_mut().chat_panel.chat.as_mut().unwrap().origin = origin.clone();
        assert!(fx::call(h.state_mut(), "document.inspect", serde_json::json!({})).is_ok(), "the shown one, by its own key");
        let inspect = serde_json::json!({});
        let gate = |h: &Harness<'static, Shell>, origin: Option<&str>, key: Option<&str>| {
            archforge_ui_egui::testing::chat_gate(h.state(), origin, key, "document.inspect", &inspect)
        };
        let parked = gate(&h, origin.as_deref(), Some(&parked_key));
        assert!(parked.unwrap_err().contains("has no turn running"), "the parked one, under the same label, is refused here");
        // The person's own token naming that label keeps the label rules: refused while the parked
        // one runs under it.
        assert!(gate(&h, origin.as_deref(), None).unwrap_err().contains("has no turn running"), "the label, unbound");
        // A key the window does not know may not act under a label it knows.
        let stranger = "00000000-0000-4000-8000-00000000beef";
        assert!(gate(&h, origin.as_deref(), Some(stranger)).unwrap_err().contains("is not one Archforge knows"), "an unknown key");
        // A conversation's request without a label is still checked by its key.
        let shown_key = fx::key(h.state());
        chat_event(&mut h, &rig, archforge_ui_egui::chat::ChatEvent::TurnEnded(archforge_ui_egui::chat::TurnEnd::Completed));
        let no_label = gate(&h, None, Some(&shown_key)).unwrap_err();
        assert!(
            no_label.contains("has no turn running") && no_label.contains(origin.as_deref().unwrap()),
            "no label, no turn running, named by its own: {no_label}"
        );
        // A conversation let go of: its label is refused until its program has ended.
        let (mut h, rig) = chat_harness(theme, true);
        ask_in_chat(&mut h, "One");
        chat_event(&mut h, &rig, archforge_ui_egui::chat::ChatEvent::TurnEnded(archforge_ui_egui::chat::TurnEnd::Completed));
        let key = fx::key(h.state());
        let origin = h.state().chat_panel.chat.as_ref().unwrap().origin.clone();
        let at = rect_of(&h, "workspace.chat.new").center();
        click(&mut h, at);
        // The fake's program ends at once; one still ending is held as a slow program's would be.
        archforge_ui_egui::testing::letting_go(h.state_mut()).push((key.clone(), origin.clone()));
        let inspect = serde_json::json!({});
        let ends = archforge_ui_egui::testing::chat_gate(h.state(), origin.as_deref(), Some(&key), "document.inspect", &inspect);
        assert!(ends.as_ref().unwrap_err().contains("is being ended"), "refused while it ends, for that reason: {ends:?}");
        // Known by its key: under another label it is refused the same way.
        let other = format!("{}!", origin.as_deref().unwrap_or_default());
        let ends = archforge_ui_egui::testing::chat_gate(h.state(), Some(&other), Some(&key), "document.inspect", &inspect);
        assert!(ends.as_ref().unwrap_err().contains("is being ended"), "{theme:?}: refused by its key under another label: {ends:?}");
        h.run();
        assert!(archforge_ui_egui::testing::letting_go(h.state_mut()).is_empty(), "its end taken, it is forgotten");
        assert!(
            archforge_ui_egui::testing::chat_gate(h.state(), origin.as_deref(), Some(&key), "document.inspect", &inspect).is_ok(),
            "known no more once ended"
        );
    }
}

/// A turn's changes are counted from its steps still applied: a change the person undid between
/// two of the turn's changes is neither counted nor listed, and the person's own change or
/// another chat's between is not the turn's.
#[test]
fn a_turns_changes_leave_out_what_was_undone_between_and_the_persons_own_change() {
    use archforge_ui_egui::chat::ChatEvent;
    use archforge_ui_egui::chat_fixture as fx;
    use archforge_ui_egui::chat_store::ChangeKind;
    let describe = |h: &mut Harness<'static, Shell>, name: &str, text: &str| {
        let id = fx::element(h.state(), archforge_uml::CLASS, name).unwrap();
        fx::call(
            h.state_mut(),
            "engine.execute",
            serde_json::json!({ "command": "element.describe", "params": { "element": id.to_string(), "text": text } }),
        )
        .unwrap();
        h.run();
        id
    };
    let (mut h, rig) = chat_harness(ThemeKind::Light, true);
    let at = rect_of(&h, "workspace.chat.allow-changes").center();
    click(&mut h, at);
    // An undo between: the undone change is not the turn's.
    ask_in_chat(&mut h, "Describe two classes");
    describe(&mut h, "Customer", "A buyer");
    h.state_mut().session.as_mut().unwrap().undo().unwrap();
    let product = describe(&mut h, "Product", "A thing sold");
    chat_event(&mut h, &rig, ChatEvent::TurnEnded(archforge_ui_egui::chat::TurnEnd::Completed));
    let kept = kept_chat(&h, &rig);
    let (_, changes) = kept.ends.last().unwrap();
    let changes = changes.clone().unwrap();
    assert_eq!(changes.changes, 1, "the undone change is not counted");
    assert_eq!(changes.changed.iter().map(|e| (e.element, e.kind)).collect::<Vec<_>>(), [(product, ChangeKind::Changed)]);
    assert!(painted_has(&h, "1 change to the model"), "{:?}", painted(&h));
    assert!(ids(&h).iter().any(|i| i == "workspace.chat.turn.1.undo"), "one step: it can be undone");
    // The person's own change between two of the turn's: listed is the turn's, not the person's.
    ask_in_chat(&mut h, "Describe two more");
    let catalog = describe(&mut h, "Catalog", "Products together");
    let order = fx::element(h.state(), archforge_uml::CLASS, "Order").unwrap();
    h.state_mut()
        .session
        .as_mut()
        .unwrap()
        .execute("element.describe", &serde_json::json!({ "element": order.to_string(), "text": "Mine" }))
        .unwrap();
    let payment = describe(&mut h, "Payment", "Money paid");
    chat_event(&mut h, &rig, ChatEvent::TurnEnded(archforge_ui_egui::chat::TurnEnd::Completed));
    let kept = kept_chat(&h, &rig);
    let changes = kept.ends.last().unwrap().1.clone().unwrap();
    let mut listed: Vec<_> = changes.changed.iter().map(|e| e.element).collect();
    listed.sort();
    let mut expected = vec![catalog, payment];
    expected.sort();
    assert_eq!((changes.changes, listed), (2, expected));
    // Another chat's change between two of the turn's: listed in full are the turn's own, each as
    // changed, and the card counts the turn's two changes, with no undo across the other's step.
    ask_in_chat(&mut h, "Describe two others");
    let card = describe(&mut h, "CardPayment", "Paid by card");
    let customer = fx::element(h.state(), archforge_uml::CLASS, "Customer").unwrap();
    h.state_mut()
        .session
        .as_mut()
        .unwrap()
        .execute_from("element.describe", &serde_json::json!({ "element": customer.to_string(), "text": "Theirs" }), Some("Chat 99"))
        .unwrap();
    let line = describe(&mut h, "OrderLine", "One product ordered");
    chat_event(&mut h, &rig, ChatEvent::TurnEnded(archforge_ui_egui::chat::TurnEnd::Completed));
    let changes = kept_chat(&h, &rig).ends.last().unwrap().1.clone().unwrap();
    let mut listed: Vec<_> = changes.changed.iter().map(|e| (e.element, e.kind, e.metaclass.clone())).collect();
    listed.sort_by_key(|(id, _, _)| *id);
    let mut expected = vec![(card, ChangeKind::Changed, "uml_class".to_string()), (line, ChangeKind::Changed, "uml_class".to_string())];
    expected.sort_by_key(|(id, _, _)| *id);
    assert_eq!((changes.changes, changes.elements, listed), (2, 2, expected), "nothing of Chat 99's change to Customer");
    assert!(painted_has(&h, "2 changes to the model"), "{:?}", painted(&h));
    assert!(!ids(&h).iter().any(|i| i == "workspace.chat.turn.3.undo"), "two steps apart: no undo of the turn as one");
}

/// A conversation's turn undoes and redoes only its own steps: an earlier turn's step on top, or
/// to redo, is refused through both undo entry points, and the turn's own step is undone and
/// redone.
#[test]
fn a_turn_undoes_and_redoes_only_its_own_steps() {
    use archforge_ui_egui::chat::ChatEvent;
    use archforge_ui_egui::chat_fixture as fx;
    let describe = |h: &mut Harness<'static, Shell>, name: &str, text: &str| {
        let id = fx::element(h.state(), archforge_uml::CLASS, name).unwrap();
        fx::call(
            h.state_mut(),
            "engine.execute",
            serde_json::json!({ "command": "element.describe", "params": { "element": id.to_string(), "text": text } }),
        )
        .unwrap();
        h.run();
        id
    };
    let text = |h: &Harness<'static, Shell>, id| {
        archforge_ui_egui::testing::description_now(h.state(), h.state().session.as_ref().unwrap().doc().by_uuid(id).unwrap())
    };
    let (mut h, rig) = chat_harness(ThemeKind::Light, true);
    let at = rect_of(&h, "workspace.chat.allow-changes").center();
    click(&mut h, at);
    ask_in_chat(&mut h, "Describe Customer");
    let customer = describe(&mut h, "Customer", "A buyer");
    chat_event(&mut h, &rig, ChatEvent::TurnEnded(archforge_ui_egui::chat::TurnEnd::Completed));
    // The next turn, before it changes anything: the earlier turn's step is on top, and refused.
    ask_in_chat(&mut h, "Describe Product");
    let undo = fx::call(h.state_mut(), "engine.undo", serde_json::json!({}));
    assert!(undo.unwrap_err().contains("may undo only its own changes in this turn"));
    let undo = fx::call(h.state_mut(), "engine.execute", serde_json::json!({ "command": "edit.undo", "params": {} }));
    assert!(undo.unwrap_err().contains("may undo only its own changes in this turn"));
    assert_eq!(text(&h, customer), "A buyer", "the earlier turn's change stays");
    // Its own step it undoes and redoes; once undone, the earlier turn's step under it is refused.
    let product = describe(&mut h, "Product", "A thing sold");
    assert!(fx::call(h.state_mut(), "engine.undo", serde_json::json!({})).is_ok());
    assert_eq!((text(&h, product), text(&h, customer)), (String::new(), "A buyer".to_string()));
    assert!(fx::call(h.state_mut(), "engine.undo", serde_json::json!({})).is_err());
    assert!(fx::call(h.state_mut(), "engine.execute", serde_json::json!({ "command": "edit.redo", "params": {} })).is_ok());
    assert_eq!(text(&h, product), "A thing sold");
    chat_event(&mut h, &rig, ChatEvent::TurnEnded(archforge_ui_egui::chat::TurnEnd::Completed));
    // The person takes the second turn back: the third turn may not redo it, through either entry.
    h.state_mut().session.as_mut().unwrap().undo().unwrap();
    h.run();
    ask_in_chat(&mut h, "Bring it back");
    let redo = fx::call(h.state_mut(), "engine.redo", serde_json::json!({}));
    assert!(redo.unwrap_err().contains("may redo only its own changes in this turn"));
    let redo = fx::call(h.state_mut(), "engine.execute", serde_json::json!({ "command": "edit.redo", "params": {} }));
    assert!(redo.is_err());
    assert_eq!(text(&h, product), "", "the earlier turn's step stays undone");
}

/// The changes card says what truly happened: a turn the person changed the model during is
/// refused as changed during it, not after; taken back step by step it was not one step, while a
/// turn of one step undone from its card was.
#[test]
fn the_changes_card_says_during_and_one_step_only_when_true() {
    use archforge_ui_egui::chat::ChatEvent;
    use archforge_ui_egui::chat_fixture as fx;
    let describe = |h: &mut Harness<'static, Shell>, name: &str, text: &str| {
        let id = fx::element(h.state(), archforge_uml::CLASS, name).unwrap();
        fx::call(
            h.state_mut(),
            "engine.execute",
            serde_json::json!({ "command": "element.describe", "params": { "element": id.to_string(), "text": text } }),
        )
        .unwrap();
        h.run();
    };
    let (mut h, rig) = chat_harness(ThemeKind::Light, true);
    let at = rect_of(&h, "workspace.chat.allow-changes").center();
    click(&mut h, at);
    // The person's change during the turn: two steps, refused as changed during the turn.
    ask_in_chat(&mut h, "Describe two classes");
    describe(&mut h, "Customer", "A buyer");
    let order = fx::element(h.state(), archforge_uml::CLASS, "Order").unwrap();
    h.state_mut()
        .session
        .as_mut()
        .unwrap()
        .execute("element.describe", &serde_json::json!({ "element": order.to_string(), "text": "Mine" }))
        .unwrap();
    describe(&mut h, "Product", "A thing sold");
    chat_event(&mut h, &rig, ChatEvent::TurnEnded(archforge_ui_egui::chat::TurnEnd::Completed));
    assert!(!ids(&h).iter().any(|i| i == "workspace.chat.turn.1.undo"));
    assert!(painted_has(&h, "The model changed during this turn, so its changes can no longer be undone on their own."), "{:?}", painted(&h));
    assert!(!painted_has(&h, "The model changed after this turn, so its changes can no longer be undone on their own."));
    // Taken back step by step, the person's change between too: not one step.
    for _ in 0..3 {
        h.state_mut().session.as_mut().unwrap().undo().unwrap();
        h.run();
    }
    assert!(painted_has(&h, "2 changes undone"), "{:?}", painted(&h));
    assert!(painted_has(&h, "Taken back in the project's undo history."), "{:?}", painted(&h));
    assert!(!painted_has(&h, "Taken back in one step of the project's undo history."));
    // Brought back step by step: applied again, and still refused as changed during the turn.
    for _ in 0..3 {
        h.state_mut().session.as_mut().unwrap().redo().unwrap();
        h.run();
    }
    assert!(painted_has(&h, "The model changed during this turn, so its changes can no longer be undone on their own."), "{:?}", painted(&h));
    // A turn of one step, undone from its card: one step.
    ask_in_chat(&mut h, "Describe one more");
    describe(&mut h, "Catalog", "Products together");
    chat_event(&mut h, &rig, ChatEvent::TurnEnded(archforge_ui_egui::chat::TurnEnd::Completed));
    let at = rect_of(&h, "workspace.chat.turn.2.undo").center();
    click(&mut h, at);
    assert!(painted_has(&h, "Taken back in one step of the project's undo history."), "{:?}", painted(&h));
}

/// A turn's change that cleared the whole history, merged into the turn's step, is still the
/// turn's: counted with the change before it, with nothing left to undo.
#[test]
fn a_turns_change_that_clears_the_history_still_counts() {
    use archforge_ui_egui::chat::ChatEvent;
    use archforge_ui_egui::chat_fixture as fx;
    let describe = |h: &mut Harness<'static, Shell>, name: &str, text: &str| {
        let id = fx::element(h.state(), archforge_uml::CLASS, name).unwrap();
        fx::call(
            h.state_mut(),
            "engine.execute",
            serde_json::json!({ "command": "element.describe", "params": { "element": id.to_string(), "text": text } }),
        )
        .unwrap();
        h.run();
    };
    let (mut h, rig) = chat_harness(ThemeKind::Light, true);
    // The project's model again, under a history capped small enough for one long description to
    // clear it.
    let (tables, junctions) = archforge_notations::NOTATIONS.storage();
    let old = h.state_mut().session.take().unwrap();
    let rows = archforge_mof::rows::snapshot(old.doc(), &tables, &junctions).unwrap();
    // Ids at a later time than the sample's, so none is given twice.
    let mut n = 0u64;
    let mint = move || {
        n += 1;
        let mut r = [0u8; 10];
        r[2..].copy_from_slice(&n.to_be_bytes());
        archforge_mof::Uuid::v7(1_761_670_900_000, r)
    };
    let st = archforge_notations::NOTATIONS.store(Box::new(mint)).unwrap();
    let st = archforge_mof::rows::restore_into_empty(st, &rows, &tables, &junctions).unwrap();
    let options = archforge_engine::Options { max_weight: 4_096, ..archforge_engine::Options::default() };
    h.state_mut().session = Some(archforge_engine::Session::with_view(
        archforge_notations::NOTATIONS.registry().unwrap(),
        st,
        old.counter(),
        options,
        archforge_notations::NOTATIONS.view(),
    ));
    h.run();
    let at = rect_of(&h, "workspace.chat.allow-changes").center();
    click(&mut h, at);
    ask_in_chat(&mut h, "Describe two classes");
    describe(&mut h, "Customer", "A buyer");
    assert!(h.state().session.as_ref().unwrap().can_undo(), "the first change fits the cap");
    describe(&mut h, "Product", &"x".repeat(archforge_uml::descriptions::MAX_DESCRIPTION));
    assert!(!h.state().session.as_ref().unwrap().can_undo(), "the second cleared the history");
    chat_event(&mut h, &rig, ChatEvent::TurnEnded(archforge_ui_egui::chat::TurnEnd::Completed));
    let changes = kept_chat(&h, &rig).ends.last().unwrap().1.clone().unwrap();
    assert_eq!(changes.changes, 2, "both of the turn's changes");
    assert!(painted_has(&h, "2 changes to the model"), "{:?}", painted(&h));
    assert!(!ids(&h).iter().any(|i| i == "workspace.chat.turn.1.undo"), "nothing left to undo");
    assert!(
        painted_has(&h, "The undo history no longer holds this turn's changes, so they can no longer be undone here."),
        "the model did not change after the turn: its history went: {:?}",
        painted(&h)
    );
}

/// The app's notations count each of their kinds in its own words, every relationship of
/// either as a line, a primitive type as a data type, and leave out the parts of a diagram and
/// a description's comment; the shell counts diagrams, skills and the rest as elements.
#[test]
fn the_app_notations_count_their_kinds_in_their_words() {
    let uis = SCREENS.notation_uis();
    archforge_ui_egui::i18n::set_language("en");
    let read = |metaclasses: &[&str]| ToolFacts {
        tool: "engine.execute".into(),
        command: Some("model.find".into()),
        detail: None,
        status: ToolStatus::Done,
        changed_model: false,
        element_count: None,
        anchors: metaclasses.iter().zip(1u8..).map(|(m, n)| at(n, AnchorRole::Read, m, "")).collect(),
    };
    for (table, two) in [
        ("uml_class", "2 classes"),
        ("uml_interface", "2 interfaces"),
        ("uml_enumeration", "2 enumerations"),
        ("uml_data_type", "2 data types"),
        ("uml_primitive_type", "2 data types"),
        ("uml_package", "2 packages"),
        ("uml_property", "2 attributes"),
        ("uml_operation", "2 operations"),
        ("uml_enumeration_literal", "2 literals"),
        ("er_entity", "2 tables"),
        ("er_attribute", "2 columns"),
        ("er_schema", "2 schemas"),
        ("diagram_diagram", "2 diagrams"),
        ("core_skill", "2 skills"),
        ("uml_constraint", "2 elements"),
    ] {
        assert_eq!(tool_words(uis, &read(&[table, table])).0, format!("Read {two}"), "{table}");
    }
    let lines = [
        "uml_association",
        "uml_association_class",
        "uml_generalization",
        "uml_dependency",
        "uml_realization",
        "uml_abstraction",
        "uml_usage",
        "uml_interface_realization",
        "er_relationship",
        "er_generalization",
    ];
    assert_eq!(tool_words(uis, &read(&lines)).0, "Read 10 lines", "every relationship of either notation");
    assert_eq!(tool_words(uis, &read(&["uml_data_type", "uml_primitive_type", "er_entity"])).0, "Read 2 data types, 1 table");
    let parts = ["diagram_node", "diagram_edge", "core_comment", "uml_comment", "er_entity"];
    assert_eq!(tool_words(uis, &read(&parts)).0, "Read 1 table", "a diagram's parts and a description's comment are not counted");
    // Elements of tables no one counts are told apart by their table.
    assert_eq!(tool_words(uis, &read(&["core_folder", "core_folder", "core_model"])).0, "Read 2 elements, 1 element");
}
