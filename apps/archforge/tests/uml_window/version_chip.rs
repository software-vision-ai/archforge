//! Leaving current work for a version over UML's model: a notice sits below the path bar, and what
//! Properties holds (an association's ends, a refused role or description) stops the leaving, or goes
//! as Cancel when the selection moves on, before a replacement.

use super::*;

/// Runs `test` on an open project, once in each appearance.
fn in_both(test: impl Fn(Harness<'static, Shell>, std::sync::Arc<SavingHost>)) {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, host) = saving_harness(Some(commerce()), None);
        h.state_mut().state.theme = theme;
        host.version_reads.lock().unwrap().clear();
        test(h, host);
    }
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

const ID: archforge_mof::Uuid = archforge_mof::Uuid([7; 16]);

/// An association's ends stop leaving only while holding something not yet written: the panel
/// fills them in for any association, and an open fold holds nothing.
#[test]
fn an_end_typed_on_the_selected_association_stops_leaving() {
    use archforge_ui_egui::hold::HoldRefused::Editing;
    use uml_relate::{EndModifiers, EndsEdit};
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
            modifiers_last: None,
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
        uml_of_mut(s).ends = Some(untouched());
        assert!(archforge_ui_egui::testing::clear_to_leave(s, None).is_ok(), "the panel filled them in and nothing was typed");
        uml_of_mut(s).ends.as_mut().unwrap().modifiers_open = [true, true];
        assert!(archforge_ui_egui::testing::clear_to_leave(s, None).is_ok(), "an open fold holds nothing");
        for (what, change) in typed {
            uml_of_mut(s).ends = Some(untouched());
            change(uml_of_mut(s).ends.as_mut().unwrap());
            assert_eq!(archforge_ui_egui::testing::clear_to_leave(s, None).err(), Some(Editing), "{what}");
        }
    });
}

/// A click off an association ends its draft: what reads is written though its field never saw the
/// click, the rest goes as Cancel, and a replacement without the association has nothing to lose.
#[test]
fn a_moved_selection_ends_an_associations_draft_before_a_replacement() {
    in_both(|mut h, _| {
        let earlier = rows_now(&h);
        archforge_ui_egui::testing::cx(h.state_mut(), uml_ui::open_sample).unwrap();
        h.state_mut().state.show_panel(archforge_shell_api::state::Panel::Properties);
        h.run();
        let away = rect_of(&h, "workspace.canvas").left_top() + egui::vec2(30.0, 30.0);
        for (field, typed) in [("role", "payer"), ("multiplicity", "2..5")] {
            select_named(&mut h, archforge_uml::ASSOCIATION, "paid by");
            let before = revision(&h);
            retype(&mut h, &format!("workspace.properties.end.1.{field}"), typed);
            assert_eq!(revision(&h), before, "{field}: still selected, it is only typed");
            uml_of_mut(h.state_mut()).ends.as_mut().unwrap().modifiers[1].derived = true;
            click(&mut h, away);
            assert!(uml_of(h.state()).ends.is_none(), "{field}");
        }
        let paid = select_named(&mut h, archforge_uml::ASSOCIATION, "paid by");
        let st = h.state().session.as_ref().unwrap().doc();
        let end = archforge_uml::figures::member_ends(st, st.by_uuid(paid).unwrap()).unwrap()[1];
        let model = (archforge_core::name_of(st, end), archforge_uml::figures::multiplicity(st, end), archforge_uml::modifiers::is_derived(st, end));
        assert_eq!(model, ("payer".to_string(), "2..5".to_string(), false), "what read is written, Modifiers not Applied are not");
        let before = revision(&h);
        let ends = uml_of_mut(h.state_mut()).ends.as_mut().unwrap();
        (ends.multiplicity[0], ends.refused[0]) = ("3".into(), Some("refused".into()));
        ends.multiplicity[1] = "5..2".into();
        assert_eq!(
            archforge_ui_egui::testing::clear_to_leave(h.state_mut(), None).err(),
            Some(archforge_ui_egui::hold::HoldRefused::Editing),
            "selected, it holds work"
        );
        click(&mut h, away);
        assert!(uml_of(h.state()).ends.is_none() && revision(&h) == before, "a refused or unreadable value is dropped, not written");
        let read = start_replacing(&mut h, None);
        read.send(Ok(earlier)).unwrap();
        h.steps(4);
        assert!(!h.state().is_held() && h.state().session.as_ref().unwrap().doc().by_uuid(paid).is_none(), "replaced without it");
        assert!(uml_of(h.state()).ends.is_none() && h.state().panels.notice.is_none());
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
        archforge_ui_egui::testing::cx(h.state_mut(), uml_ui::open_sample).unwrap();
        h.state_mut().state.show_panel(archforge_shell_api::state::Panel::Properties);
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
        assert!(uml_of(h.state()).ends.is_none() && role(&h, paid, 1) == "payment", "never written, though nothing refuses it now");
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

/// A Chat turn running on a project a version can be read from.
fn turn_running(theme: ThemeKind) -> (Harness<'static, Shell>, archforge_ui_egui::chat_fixture::Rig, std::sync::Arc<SavingHost>) {
    let (mut h, rig, host, _) = chat_and_a_version(theme);
    ask_in_chat(&mut h, "A long question");
    (h, rig, host)
}

/// That the turn is left running, not stopped, with no question up.
#[track_caller]
fn turn_left_alone(h: &Harness<'static, Shell>, rig: &archforge_ui_egui::chat_fixture::Rig, what: &str) {
    assert_eq!(
        (h.state().ask.clone(), chat_calls(rig).2, chat_turns(h).last().unwrap().state.clone()),
        (None, 0, archforge_ui_egui::chat_panel::TurnState::Running),
        "{what}"
    );
}

/// A text already refused stops the leaving anyway, so Chat isn't asked about.
#[test]
fn a_refused_description_stops_leaving_before_the_question() {
    in_both_appearances(|theme| {
        let (mut h, rig, _host) = turn_running(theme);
        let target = archforge_ui_egui::chat_fixture::element(h.state(), archforge_uml::CLASS, "Customer").unwrap();
        h.state_mut().panels.description = Some(archforge_shell_api::panels::DescriptionDraft { target, text: "a\u{7}b".into(), refused: true });
        assert_eq!(h.state_mut().view_version("c1"), Err(archforge_ui_egui::hold::HoldRefused::Description));
        turn_left_alone(&h, &rig, "no question");
    });
}
