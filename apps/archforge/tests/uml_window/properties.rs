//! Properties through the window: UML's bodies and editors for a class, its members, an
//! enumeration, a data type, a package, an association's ends and templates, typed, checked and
//! written as one undo step, their refusals in place, kept in view as the body scrolls, and their
//! menus where the window has room.

use super::*;

/// A constraint's Apply refused above its buttons, then under them: each time the refusal and both
/// buttons are in view, and Apply and Cancel are pressed where they show, with no wheel.
#[test]
fn a_refused_constraint_keeps_its_buttons_in_view() {
    in_both_appearances(|theme| {
        let mut h = constraint_refused(theme);
        let braces = uml_of(h.state()).constraint.as_ref().and_then(|e| e.refused.clone()).unwrap_or_default();
        assert!(said_in_view(&h, &braces), "{theme:?}: the refusal is in view");
        let at = in_view(&h, CONSTRAINT_APPLY).expect("Apply in view after the refusal");
        assert!(in_view(&h, CONSTRAINT_CANCEL).is_some(), "{theme:?}: Cancel in view after the refusal");
        // The diagram as the element: refused under the buttons, which stay in view with it.
        let shown = h.state().canvas.active.unwrap();
        uml_of_mut(h.state_mut()).constraint.as_mut().expect("the editor").elements = vec![shown];
        click(&mut h, at);
        let edit = uml_of(h.state()).constraint.clone().expect("the editor stays open");
        let outside = archforge_ui_egui::i18n::tr("A constraint names only elements of this model.");
        assert_eq!((edit.refused.as_deref(), edit.refused_at), (Some(outside.as_str()), None), "{theme:?}");
        assert!(said_in_view(&h, &outside), "{theme:?}: the refusal under the buttons is in view");
        assert!(in_view(&h, CONSTRAINT_APPLY).is_some(), "{theme:?}: Apply in view after the second refusal");
        let at = in_view(&h, CONSTRAINT_CANCEL).expect("Cancel in view after the second refusal");
        click(&mut h, at);
        assert!(uml_of(h.state()).constraint.is_none(), "{theme:?}: Cancel closes the editor");
    });
}

/// The body scrolled away from a refused editor stays where it was put: the refusal brought the
/// editor into view once, not on every frame, and its going brings nothing into view either.
#[test]
fn a_body_scrolled_away_from_a_refusal_stays_put() {
    in_both_appearances(|theme| {
        let mut h = constraint_refused(theme);
        wheel_properties(&mut h, 240.0);
        let put = h.state().panels.scroll;
        assert!(in_view(&h, CONSTRAINT_APPLY).is_none(), "{theme:?}: Apply scrolled out of view");
        h.run_steps(3);
        assert_eq!(h.state().panels.scroll, put, "{theme:?}: nothing pulls the body back");
        // A linked note picked instead: the refusal goes, the card shrinks, and the body stays.
        let at = in_view(&h, "workspace.properties.constraint.show.note").expect("Linked note in view");
        click(&mut h, at);
        assert_eq!(uml_of(h.state()).constraint.as_ref().and_then(|e| e.refused.clone()), None, "{theme:?}: the refusal went");
        assert!(in_view(&h, CONSTRAINT_APPLY).is_none(), "{theme:?}: Apply is still out of view");
        assert_eq!(h.state().panels.scroll, put, "{theme:?}: the refusal's going moved nothing");
    });
}

/// Two editors open at once, an attribute's drawn above a constraint's: a refusal in the
/// constraint's alone brings the constraint's card into view, not the attribute's, which changed
/// nothing.
#[test]
fn a_refusal_brings_its_own_editor_into_view_when_two_are_open() {
    in_both_appearances(|theme| {
        let (mut h, order, _) = order_properties(theme);
        for i in 0..16 {
            archforge_ui_egui::testing::run(h.state_mut(), "uml.attribute.add", serde_json::json!({ "class": order, "name": format!("note{i}") }))
                .unwrap();
        }
        h.run();
        let at = reached_in_properties(&mut h, "workspace.properties.constraints.add");
        click(&mut h, at);
        let at = reached_in_properties(&mut h, "workspace.properties.attributes.add");
        click(&mut h, at);
        assert!(uml_of(h.state()).member.is_some() && uml_of(h.state()).constraint.is_some(), "{theme:?}: both editors open");
        assert!(in_view(&h, "workspace.properties.member.apply").is_some(), "{theme:?}: the attribute's editor in view");
        assert!(in_view(&h, CONSTRAINT_APPLY).is_none(), "{theme:?}: the constraint's editor under the fold");
        let why = archforge_ui_egui::i18n::tr("A constraint names only elements of this model.");
        uml_of_mut(h.state_mut()).constraint.as_mut().expect("the editor").refused = Some(why.clone());
        h.run();
        assert!(said_in_view(&h, &why), "{theme:?}: the constraint's refusal is in view");
        assert!(in_view(&h, CONSTRAINT_APPLY).is_some() && in_view(&h, CONSTRAINT_CANCEL).is_some(), "{theme:?}: its buttons too");
    });
}

/// An n-ary association's end editor moved to another end, with no refusal, is a new place: the
/// body was wheeled up from the last end's editor until the end above it sits at the fold, and that
/// end's editor, opened there, comes into view with its Apply.
#[test]
fn an_end_editor_moved_to_another_end_comes_into_view() {
    in_both_appearances(|theme| {
        let (mut h, ids) = nary_sample_in(theme, true, egui::vec2(1600.0, 1000.0));
        let a = ids["recommends"].clone();
        for _ in 0..24 {
            archforge_ui_egui::testing::run(
                h.state_mut(),
                "uml.association.add-end",
                serde_json::json!({ "association": a, "class": ids["Customer"] }),
            )
            .unwrap();
        }
        archforge_ui_egui::testing::run(h.state_mut(), "view.select", serde_json::json!({ "ids": [a] })).unwrap();
        h.run();
        let st = h.state().session.as_ref().unwrap().doc();
        let last = archforge_uml::nary::ends(st, st.by_uuid(archforge_mof::Uuid::parse(&a).unwrap()).unwrap()).len() - 1;
        let at = reached_in_properties(&mut h, &format!("workspace.properties.end.{last}.edit"));
        click(&mut h, at);
        let above = format!("workspace.properties.end.{}.edit", last - 1);
        let row = rect_of(&h, &above).center().y;
        let body = h.state().panels.body.expect("the Properties body");
        wheel_properties(&mut h, body.bottom() - 20.0 - row);
        let at = in_view(&h, &above).expect("the end above at the fold");
        let apply = format!("workspace.properties.end.{}.apply", last - 1);
        click(&mut h, at);
        assert_eq!(uml_of(h.state()).nary_end.as_ref().map(|e| e.end), Some(last - 1), "{theme:?}: the editor moved");
        assert!(in_view(&h, &apply).is_some(), "{theme:?}: its Apply in view");
    });
}

/// A member's editor moved from a new attribute to a new operation is a new place, though neither
/// has an item yet: the operation's editor, opened under a long list of operations, comes into
/// view with its Apply.
#[test]
fn a_member_editor_moved_to_another_list_comes_into_view() {
    in_both_appearances(|theme| {
        let (mut h, order, _) = order_properties(theme);
        for i in 0..16 {
            archforge_ui_egui::testing::run(h.state_mut(), "uml.operation.add", serde_json::json!({ "class": order, "name": format!("run{i}") }))
                .unwrap();
        }
        h.run();
        let at = reached_in_properties(&mut h, "workspace.properties.attributes.add");
        click(&mut h, at);
        let at = reached_in_properties(&mut h, "workspace.properties.operations.add");
        click(&mut h, at);
        let kind = uml_of(h.state()).member.as_ref().map(|e| (e.kind, e.target));
        assert_eq!(kind, Some((uml_panel::MemberKind::Operation, None)), "{theme:?}: a new operation's editor");
        assert!(in_view(&h, "workspace.properties.member.apply").is_some(), "{theme:?}: its Apply in view");
    });
}

/// An operation of 30 parameters has a card taller than the body: opened, it shows its top;
/// refused at its foot, two parameters named alike, it shows its foot with the refusal, Apply and
/// Cancel, not its top.
#[test]
fn a_refused_card_taller_than_the_body_shows_its_foot() {
    in_both_appearances(|theme| {
        let (mut h, order, _) = order_properties(theme);
        let params: Vec<_> = (0..30).map(|i| serde_json::json!({ "name": format!("p{i}"), "type": "" })).collect();
        let op = archforge_ui_egui::testing::run(
            h.state_mut(),
            "uml.operation.add",
            serde_json::json!({ "class": order, "name": "plan", "parameters": params }),
        );
        let op = op.unwrap()["id"].as_str().unwrap().to_string();
        archforge_ui_egui::testing::run(h.state_mut(), "view.select", serde_json::json!({ "ids": [order] })).unwrap();
        h.run();
        let at = reached_in_properties(&mut h, &format!("workspace.properties.operation.{op}.edit"));
        click(&mut h, at);
        let top = in_view(&h, "workspace.properties.member.name").is_some();
        assert!(top && in_view(&h, "workspace.properties.member.apply").is_none(), "{theme:?}: opened, the card shows its top");
        uml_of_mut(h.state_mut()).member.as_mut().expect("the editor").params[1].name = "p0".into();
        h.run();
        let at = reached_in_properties(&mut h, "workspace.properties.member.apply");
        click(&mut h, at);
        let why = uml_of(h.state()).member.as_ref().and_then(|e| e.refused.clone().filter(|_| e.refused_at.is_none())).expect("refused");
        assert!(said_in_view(&h, &why), "{theme:?}: the refusal is in view: {why}");
        assert!(in_view(&h, "workspace.properties.member.apply").is_some(), "{theme:?}: Apply in view");
        assert!(in_view(&h, "workspace.properties.member.cancel").is_some(), "{theme:?}: Cancel in view");
    });
}

/// A member's editor shares the card: an attribute added at the foot of a long list, refused for a
/// name another attribute has, keeps its refusal and buttons in view.
#[test]
fn a_refused_attribute_keeps_its_buttons_in_view() {
    in_both_appearances(|theme| {
        let (mut h, order, _) = order_properties(theme);
        for i in 0..16 {
            archforge_ui_egui::testing::run(h.state_mut(), "uml.attribute.add", serde_json::json!({ "class": order, "name": format!("note{i}") }))
                .unwrap();
        }
        h.run();
        let at = reached_in_properties(&mut h, "workspace.properties.attributes.add");
        click(&mut h, at);
        uml_of_mut(h.state_mut()).member.as_mut().expect("the editor").name = "note0".into();
        h.run();
        let at = in_view(&h, "workspace.properties.member.apply").expect("the editor opened in view");
        click(&mut h, at);
        let why = uml_of(h.state()).member.as_ref().and_then(|e| e.refused.clone()).expect("the name is refused");
        assert!(why.contains("note0"), "{theme:?}: {why}");
        assert!(said_in_view(&h, &why), "{theme:?}: the refusal is in view");
        assert!(in_view(&h, "workspace.properties.member.apply").is_some(), "{theme:?}: Apply in view");
        let at = in_view(&h, "workspace.properties.member.cancel").expect("Cancel in view");
        click(&mut h, at);
        assert!(uml_of(h.state()).member.is_none(), "{theme:?}: Cancel closes the editor");
    });
}

/// A type menu's choices and a property's targets are kept for one session only: another session of
/// the same model at the same revision, as a replacement or a version shown installs, builds them
/// again rather than offering what the last session held (AR-787).
#[test]
fn kept_type_and_target_choices_are_built_again_for_another_session_of_the_same_model() {
    use serde_json::json;
    let mut h = workspace_harness(ThemeKind::Light, egui::vec2(1600.0, 1000.0));
    let id = |h: &mut Harness<'static, Shell>, cmd: &str, p: serde_json::Value| {
        archforge_ui_egui::testing::run(h.state_mut(), cmd, p).unwrap()["id"].as_str().unwrap().to_string()
    };
    let sales = id(&mut h, "uml.package.add", json!({ "name": "Sales" }));
    let status = id(&mut h, "uml.enumeration.add", json!({ "package": sales, "name": "Status" }));
    id(&mut h, "uml.literal.add", json!({ "enumeration": status, "name": "open" }));
    let status = archforge_mof::Uuid::parse(&status).unwrap();
    let read = |h: &mut Harness<'static, Shell>| {
        let s = h.state_mut();
        let choices = archforge_ui_egui::testing::cx(s, |cx| uml_panel::choices_for_menu(cx, None));
        let invoice = choices.labels.iter().any(|l| l == "Invoice");
        archforge_ui_egui::testing::cx(s, |cx| uml_panel::keep_choices(cx, choices));
        let targets = archforge_ui_egui::testing::cx(s, |cx| uml_panel::take_targets(cx, "status", |_| None, |st| st.by_uuid(status)));
        let literals = targets.literals.clone().unwrap_or_default();
        archforge_ui_egui::testing::cx(s, |cx| uml_panel::keep_targets(cx, targets));
        (invoice, literals)
    };
    let first = archforge_ui_egui::testing::build_session(h.state(), &rows_now(&h)).unwrap();
    let revision = first.revision();
    archforge_ui_egui::testing::install_session(h.state_mut(), first, false);
    assert_eq!(read(&mut h), (false, vec!["open".to_string()]));
    assert_eq!(read(&mut h), (false, vec!["open".to_string()]), "kept for the same session and revision");
    // The model moves on, and another session of it is built at the revision the first was kept at.
    id(&mut h, "uml.class.add", json!({ "package": sales, "name": "Invoice" }));
    id(&mut h, "uml.literal.add", json!({ "enumeration": status.to_string(), "name": "shut" }));
    let second = archforge_ui_egui::testing::build_session(h.state(), &rows_now(&h)).unwrap();
    assert_eq!(second.revision(), revision, "the same revision, in a history of its own");
    archforge_ui_egui::testing::install_session(h.state_mut(), second, false);
    assert_eq!(read(&mut h), (true, vec!["open".to_string(), "shut".to_string()]), "built again for the new session");
}

/// A class's attributes in Properties: + opens the editor, a built-in type goes into Types in the root
/// package, and Apply adds the attribute as one undo step; a row selects it with the class's panel kept, its
/// edit refuses a multiplicity that does not read under its field, and it is made private and static, moved
/// up and removed, undo bringing it back.
#[test]
fn a_class_takes_attributes_in_properties_typed_checked_moved_and_removed_each_one_undo_step() {
    let (mut h, customer) = class_properties("Customer");
    let texts = painted(&h);
    assert!(texts.iter().any(|t| t == "Attributes · 2") && texts.iter().any(|t| t == "Operations · 1"), "{texts:?}");
    assert_eq!(member_rows(&h, customer, false), ["+ id : Integer", "- email : String"]);
    // + opens the editor with the name taking the keyboard; a built-in type makes the Types package.
    let at = rect_of(&h, "workspace.properties.attributes.add").center();
    click(&mut h, at);
    assert!(uml_of(h.state()).member.as_ref().is_some_and(|e| e.target.is_none()), "the editor is open for a new attribute");
    h.event(egui::Event::Text("rating".into()));
    h.run();
    pick_type(&mut h, "workspace.properties.member.type", "Real");
    type_into(&mut h, "workspace.properties.member.multiplicity", "0..1");
    let at = rect_of(&h, "workspace.properties.member.apply").center();
    click(&mut h, at);
    assert!(uml_of(h.state()).member.is_none(), "{:?}", uml_of(h.state()).member);
    assert_eq!(member_rows(&h, customer, false), ["+ id : Integer", "- email : String", "+ rating : Real [0..1]"]);
    let st = h.state().session.as_ref().unwrap().doc();
    assert!(
        archforge_uml::commands::all_packages(st).iter().any(|p| archforge_core::name_of(st, *p) == archforge_uml::members::TYPES),
        "Real is kept in Types"
    );
    let root = archforge_uml::commands::root_package(st).expect("the root package");
    assert!(
        archforge_uml::commands::subpackages(st, root).iter().any(|p| archforge_core::name_of(st, *p) == archforge_uml::members::TYPES),
        "Types sits in the root package"
    );
    h.run();
    assert!(painted(&h).iter().any(|t| t == "+ rating : Real [0..1]"), "the card and the row show it");
    // One undo step takes it back, redo brings it again.
    h.state_mut().session.as_mut().unwrap().undo().unwrap();
    assert_eq!(member_rows(&h, customer, false).len(), 2);
    h.state_mut().session.as_mut().unwrap().redo().unwrap();
    h.run();
    assert_eq!(member_rows(&h, customer, false).len(), 3);
    // A click on a row selects the attribute; Properties stays the class's, the row marked.
    let st = h.state().session.as_ref().unwrap().doc();
    let rating = st.uuid(archforge_uml::members::attributes(st, st.by_uuid(customer).unwrap())[2]).unwrap();
    let row = format!("workspace.properties.attribute.{rating}");
    let at = rect_of(&h, &row).left_center() + egui::vec2(20.0, 0.0);
    click(&mut h, at);
    assert_eq!(archforge_shell_api::panels::selected_id_cx(&archforge_ui_egui::testing::cx_ref(h.state())), Some(rating));
    assert!(painted(&h).iter().any(|t| t == "Attributes · 3"), "the class's panel shows for its attribute");
    // Its edit button opens the editor in its place; a multiplicity that does not read is refused.
    let at = rect_of(&h, &format!("{row}.edit")).center();
    click(&mut h, at);
    assert_eq!(uml_of(h.state()).member.as_ref().and_then(|e| e.target), Some(rating));
    assert_eq!(uml_of(h.state()).member.as_ref().unwrap().multiplicity, "0..1");
    uml_of_mut(h.state_mut()).member.as_mut().unwrap().multiplicity = "3..1".into();
    let at = rect_of(&h, "workspace.properties.member.apply").center();
    click(&mut h, at);
    let refused = uml_of(h.state()).member.as_ref().and_then(|e| e.refused.clone()).expect("refused");
    assert_eq!(refused, "The upper bound must be at least 3.", "said in the user's words");
    assert_eq!(uml_of(h.state()).member.as_ref().and_then(|e| e.refused_at), Some("multiplicity"), "under its field");
    assert_eq!(member_rows(&h, customer, false)[2], "+ rating : Real [0..1]", "nothing changed");
    // Private and static, applied.
    uml_of_mut(h.state_mut()).member.as_mut().unwrap().multiplicity = "1..*".into();
    let at = rect_of(&h, "workspace.properties.member.visibility").center();
    click(&mut h, at);
    let at = rect_of(&h, "workspace.properties.member.visibility.1").center();
    click(&mut h, at);
    let at = rect_of(&h, "workspace.properties.member.static").center();
    click(&mut h, at);
    let at = rect_of(&h, "workspace.properties.member.apply").center();
    click(&mut h, at);
    assert_eq!(member_rows(&h, customer, false)[2], "- rating : Real [1..*]");
    let st = h.state().session.as_ref().unwrap().doc();
    assert_eq!(st.get(st.by_uuid(rating).unwrap(), archforge_uml::feature::IS_STATIC).unwrap().as_bool(), Some(true));
    // Moved up past email, then removed; undo brings it back where it was.
    let at = rect_of(&h, &format!("{row}.edit")).center();
    click(&mut h, at);
    let at = rect_of(&h, "workspace.properties.member.up").center();
    click(&mut h, at);
    assert_eq!(member_rows(&h, customer, false), ["+ id : Integer", "- rating : Real [1..*]", "- email : String"]);
    let at = rect_of(&h, "workspace.properties.member.remove").center();
    click(&mut h, at);
    assert_eq!(member_rows(&h, customer, false), ["+ id : Integer", "- email : String"]);
    assert!(uml_of(h.state()).member.is_none());
    h.state_mut().session.as_mut().unwrap().undo().unwrap();
    assert_eq!(member_rows(&h, customer, false), ["+ id : Integer", "- rating : Real [1..*]", "- email : String"]);
}

/// A class turns abstract in Properties, and takes an operation with a return type and parameters typed by
/// the model's classes, made abstract; the editor grown past the body's foot is scrolled to.
#[test]
fn a_class_takes_operations_with_parameters_and_turns_abstract_in_properties() {
    let (mut h, payment) = class_properties("CardPayment");
    // The class turns abstract: its name slants on its card.
    let at = rect_of(&h, "workspace.properties.class.abstract").center();
    click(&mut h, at);
    let st = h.state().session.as_ref().unwrap().doc();
    assert_eq!(st.get(st.by_uuid(payment).unwrap(), archforge_uml::class::IS_ABSTRACT).unwrap().as_bool(), Some(true));
    // An operation with a parameter typed by a class of the model and a return type.
    let at = rect_of(&h, "workspace.properties.operations.add").center();
    click(&mut h, at);
    h.event(egui::Event::Text("refund".into()));
    h.run();
    pick_type(&mut h, "workspace.properties.member.type", "Boolean");
    let at = rect_of(&h, "workspace.properties.member.parameter.add").center();
    click(&mut h, at);
    type_into(&mut h, "workspace.properties.member.parameter.0.name", "to");
    pick_type(&mut h, "workspace.properties.member.parameter.0.type", "Customer");
    let at = rect_of(&h, "workspace.properties.member.abstract").center();
    click(&mut h, at);
    let at = rect_of(&h, "workspace.properties.member.parameter.add").center();
    click(&mut h, at);
    type_into(&mut h, "workspace.properties.member.parameter.1.name", "times");
    // The editor grew with its parameters past the body's foot: the body scrolls to the rest.
    let scroll = |h: &mut Harness<'static, Shell>| {
        let body = h.state().panels.body.unwrap();
        h.event(egui::Event::PointerMoved(body.center()));
        h.event(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, -400.0),
            modifiers: egui::Modifiers::NONE,
            phase: egui::TouchPhase::Move,
            source: egui::MouseWheelSource::Wheel,
        });
        h.run_steps(30);
    };
    scroll(&mut h);
    type_into(&mut h, "workspace.properties.member.parameter.1.multiplicity", "0..*");
    scroll(&mut h);
    let at = rect_of(&h, "workspace.properties.member.apply").center();
    click(&mut h, at);
    assert!(uml_of(h.state()).member.is_none(), "{:?}", uml_of(h.state()).member);
    assert_eq!(member_rows(&h, payment, true), ["+ refund(to : Customer, times) : Boolean"]);
    let st = h.state().session.as_ref().unwrap().doc();
    let op = archforge_uml::members::operations(st, st.by_uuid(payment).unwrap())[0];
    assert_eq!(st.get(op, archforge_uml::behavioral_feature::IS_ABSTRACT).unwrap().as_bool(), Some(true));
    assert_eq!(archforge_uml::figures::multiplicity(st, archforge_uml::members::parameters(st, op).0[1]), "*");
    // Opened again it holds what it was given; a parameter's ✕ drops it.
    let op_id = st.uuid(op).unwrap();
    let at = rect_of(&h, &format!("workspace.properties.operation.{op_id}.edit")).center();
    click(&mut h, at);
    let edit = uml_of(h.state()).member.clone().unwrap();
    assert_eq!((edit.name.as_str(), edit.params.len(), edit.is_abstract), ("refund", 2, true));
    // The built-in Boolean, by its name, which the commands resolve to the one in Types.
    assert_eq!(edit.ty, "Boolean");
    let at = rect_of(&h, "workspace.properties.member.parameter.1.remove").center();
    click(&mut h, at);
    let at = rect_of(&h, "workspace.properties.member.apply").center();
    click(&mut h, at);
    assert_eq!(member_rows(&h, payment, true), ["+ refund(to : Customer) : Boolean"]);
}

/// Properties' Package select of a classifier in a nested package shows the package's own name:
/// a path too long for the field is cut from its front (`… › Orders`), never from its end, and the
/// whole path is its accessible name and its tooltip (AR-191); in light and dark.
#[test]
fn a_package_select_cuts_a_long_path_from_its_front_so_the_package_name_shows() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, order, _) = nested_orders(theme);
        h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [order.to_string()] })).unwrap();
        h.state_mut().state.show_panel(archforge_shell_api::state::Panel::Properties);
        h.run();
        let path = {
            let st = h.state().session.as_ref().unwrap().doc();
            let root = archforge_core::name_of(st, archforge_uml::commands::root_package(st).unwrap());
            format!("{root} › Fulfilment › Orders")
        };
        let field = rect_of(&h, "workspace.properties.class.owner");
        let shown: Vec<String> = text_boxes(&h).into_iter().filter(|(_, r)| field.contains(r.center())).map(|(t, _)| t).collect();
        assert_eq!(shown, ["… › Orders"], "{theme:?}: the package's own name, its path's front folded");
        {
            use egui_kittest::kittest::Queryable;
            assert!(h.query_by_label(&format!("Owner: {path}")).is_some(), "the whole path is its accessible name");
        }
        // Hovered long enough, its tooltip says the whole path.
        h.event(egui::Event::PointerMoved(field.center()));
        for _ in 0..90 {
            h.step();
        }
        assert!(painted(&h).contains(&path), "{theme:?}: the tooltip says {path:?} in {:?}", painted(&h));
        // A package whose own name holds the separator is one part: the fold never splits it.
        let orders = {
            let st = h.state().session.as_ref().unwrap().doc();
            st.uuid(archforge_uml::commands::package_of(st, st.by_uuid(order).unwrap()).unwrap()).unwrap()
        };
        let renamed = serde_json::json!({ "package": orders.to_string(), "name": "Orders › EU" });
        h.state_mut().session.as_mut().unwrap().execute("uml.package.rename", &renamed).unwrap();
        h.event(egui::Event::PointerMoved(egui::pos2(5.0, 5.0)));
        h.run();
        let shown: Vec<String> = text_boxes(&h).into_iter().filter(|(_, r)| field.contains(r.center())).map(|(t, _)| t).collect();
        assert_eq!(shown, ["Orders › EU"], "{theme:?}: the name kept whole, never folded at its own › into `… › EU`");
    }
    // The cut keeps the last part: whole, else its front parts folded, else the last part alone,
    // which the field cuts at its end as it cuts any name. The parts are given, so a name holding
    // the separator stays one part.
    let fits = |room: usize| move |t: &str| t.chars().count() <= room;
    let cut = |parts: &[&str], room: usize| archforge_ui_egui::ux::cut_front(parts, " › ", fits(room));
    assert_eq!(cut(&["Commerce", "Orders"], 40), ("Commerce › Orders".to_string(), false));
    assert_eq!(cut(&["Commerce", "Fulfilment", "Orders"], 23), ("… › Fulfilment › Orders".to_string(), true));
    assert_eq!(cut(&["Commerce", "Fulfilment", "Orders"], 12), ("… › Orders".to_string(), true));
    assert_eq!(cut(&["Commerce", "Orders"], 4), ("Orders".to_string(), true));
    assert_eq!(cut(&["Commerce platform"], 4), ("Commerce platform".to_string(), true));
    assert_eq!(cut(&["Commerce", "Orders › EU"], 15), ("… › Orders › EU".to_string(), true), "one part, though it holds the separator");
    assert_eq!(cut(&["Commerce", "Orders › EU"], 8), ("Orders › EU".to_string(), true), "never folded inside a name");
}

/// A package's path names every ancestor up to the root, however deep (here 40 packages, past
/// any fixed cap and wider than the window): the Package field and its menu's row fold it from the
/// front, its last packages showing, and say it whole on hover. A walk that comes back on itself,
/// as a damaged file's could, ends with each element named once (AR-191).
#[test]
fn a_package_path_names_every_ancestor_and_a_cycle_ends() {
    let mut h = workspace_harness(ThemeKind::Light, egui::vec2(1600.0, 1000.0));
    let mut parent: Option<String> = None;
    for i in 1..=40 {
        let mut params = serde_json::json!({ "name": format!("Package {i}") });
        if let Some(p) = &parent {
            params["parent"] = serde_json::json!(p);
        }
        let out = h.state_mut().session.as_mut().unwrap().execute("uml.package.add", &params).unwrap();
        parent = out.value["id"].as_str().map(String::from);
    }
    let deepest_id = parent.unwrap();
    let st = h.state().session.as_ref().unwrap().doc();
    let deepest = st.by_uuid(archforge_mof::Uuid::parse(&deepest_id).unwrap()).unwrap();
    let parts = uml_panel::package_parts(st, deepest);
    let root = archforge_core::name_of(st, archforge_uml::commands::root_package(st).unwrap());
    assert_eq!(parts.len(), 41, "the root and all 40: {parts:?}");
    assert_eq!((parts[0].as_str(), parts[1].as_str(), parts[40].as_str()), (root.as_str(), "Package 1", "Package 40"));
    let whole = parts.join(" › ");
    let row_of = st_index(st, deepest);
    // A class in the deepest package: Properties' Package field and its menu's row both fold the
    // path from its front, the last packages showing, and say it whole on hover.
    let class =
        h.state_mut().session.as_mut().unwrap().execute("uml.class.add", &serde_json::json!({ "package": deepest_id, "name": "Deep" })).unwrap();
    let class = class.value["id"].as_str().unwrap().to_string();
    h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [class] })).unwrap();
    h.state_mut().state.show_panel(archforge_shell_api::state::Panel::Properties);
    h.run();
    let field = rect_of(&h, "workspace.properties.class.owner");
    let shown: Vec<String> = text_boxes(&h).into_iter().filter(|(_, r)| field.contains(r.center())).map(|(t, _)| t).collect();
    let [shown] = shown.as_slice() else { panic!("one text in the field: {shown:?}") };
    assert!(shown.ends_with("Package 40") && !shown.starts_with(root.as_str()), "the field keeps the package's own name: {shown:?}");
    click(&mut h, field.center());
    assert_eq!(h.state().panels.menu, Some(uml_state::menus::CLASS_OWNER));
    for _ in 0..=row_of {
        key(&mut h, egui::Key::ArrowDown, egui::Modifiers::NONE);
    }
    let row = rect_of(&h, &format!("workspace.properties.class.owner.{row_of}"));
    // The row's text, over what the window shows under the menu.
    let drawn: Vec<String> = text_boxes(&h).into_iter().filter(|(t, r)| row.contains(r.center()) && t.contains("Package")).map(|(t, _)| t).collect();
    let [drawn] = drawn.as_slice() else { panic!("one path in the row: {drawn:?}") };
    assert!(
        drawn.starts_with("… › ") && drawn.ends_with(" › Package 38 › Package 39 › Package 40") && !drawn.contains(root.as_str()),
        "the row's end shows: {drawn:?}"
    );
    assert!(!drawn.ends_with('…'), "never cut at its end: {drawn:?}");
    h.event(egui::Event::PointerMoved(row.center()));
    for _ in 0..90 {
        h.step();
    }
    assert!(painted(&h).contains(&whole), "the row's tooltip says the whole path");
    // A cycle, which the model refuses to make but a damaged file could hold, and a loop on itself.
    let name = |n: u32| n.to_string();
    assert_eq!(uml_panel::path_parts(0_u32, |n| Some((n + 1) % 3), name), ["2", "1", "0"]);
    assert_eq!(uml_panel::path_parts(7_u32, Some, name), ["7"]);
    assert_eq!(uml_panel::path_parts(5_u32, |n| n.checked_sub(1), name), ["0", "1", "2", "3", "4", "5"]);
}

/// No redefinition (leaf) in Properties (AR-138), in light and dark: on a classifier after No
/// subclasses (final), described by its explanation, ticked it makes the classifier leaf in one undo
/// step and its card says `{leaf}`, `{final, leaf}` beside final; in the operation editor, applied,
/// the row says `{leaf}`, one step undone and redone; in the attribute editor of a property something
/// redefines, it stays clear and a refusal names the redefining property, nothing written; and
/// Redefines lists a leaf property marked `{leaf}`, its pick refused under the field, until leaf is
/// cleared, when it is picked and applied.
#[test]
fn no_redefinition_makes_elements_leaf_and_redefining_them_is_refused() {
    use egui_kittest::kittest::{NodeT, Queryable};
    use uml_panel::{LEAF_ID, MEMBER_LEAF_ID, MemberKind};
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, _) = related_sample_in(theme, egui::vec2(1600.0, 1400.0));
        let class = |h: &Harness<'static, Shell>, name: &str| {
            let st = h.state().session.as_ref().unwrap().doc();
            st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_uml::CLASS) && archforge_core::name_of(st, *o) == name).unwrap()
        };
        let uuid = |h: &Harness<'static, Shell>, o: archforge_mof::Obj| h.state().session.as_ref().unwrap().doc().uuid(o).unwrap();
        let member = |h: &Harness<'static, Shell>, c: &str, name: &str, operation: bool| {
            let st = h.state().session.as_ref().unwrap().doc();
            let c = class(h, c);
            let all = if operation { archforge_uml::members::operations(st, c) } else { archforge_uml::members::attributes(st, c) };
            all.into_iter().find(|m| archforge_core::name_of(st, *m) == name).unwrap()
        };
        let select = |h: &mut Harness<'static, Shell>, id: archforge_mof::Uuid| {
            h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [id.to_string()] })).unwrap();
            h.state_mut().state.show_panel(archforge_shell_api::state::Panel::Properties);
            h.run();
        };
        let is_leaf =
            |h: &Harness<'static, Shell>, o: archforge_mof::Obj| archforge_uml::relations::is_leaf(h.state().session.as_ref().unwrap().doc(), o);
        let revision = |h: &Harness<'static, Shell>| h.state().session.as_ref().unwrap().revision();
        // On a classifier: its accessible name and the explanation that describes it.
        let pricing = class(&h, "PricingService");
        let pricing_id = uuid(&h, pricing);
        select(&mut h, pricing_id);
        let node = h.get_by_label("No redefinition (leaf)").accesskit_node().data().clone();
        let explained = "A subclass could not redefine PricingService where it is inherited. Shown as {leaf}.";
        assert_eq!(node.description(), Some(explained));
        assert_eq!(node.toggled(), Some(egui::accesskit::Toggled::False));
        assert!(painted(&h).iter().any(|t| t == explained));
        let at = rect_of(&h, LEAF_ID).center();
        click(&mut h, at);
        assert!(is_leaf(&h, pricing));
        assert_eq!(h.state().session.as_ref().unwrap().undo_label(), Some("Set classifier"));
        h.run();
        assert!(painted(&h).iter().any(|t| t == "{leaf}"), "the card says {{leaf}}");
        assert_eq!(h.get_by_label("No redefinition (leaf)").accesskit_node().data().toggled(), Some(egui::accesskit::Toggled::True));
        let at = rect_of(&h, uml_panel::FINAL_ID).center();
        click(&mut h, at);
        h.run();
        assert!(painted(&h).iter().any(|t| t == "{final, leaf}"), "final and leaf in one pair of braces");
        // An operation made leaf in its editor: applied, one step, its row says `{leaf}`.
        let payment = class(&h, "Payment");
        let payment_id = uuid(&h, payment);
        let authorise = member(&h, "Payment", "authorise", true);
        select(&mut h, payment_id);
        let authorise_id = uuid(&h, authorise);
        archforge_ui_egui::testing::cx(h.state_mut(), |cx| uml_panel::open(cx, payment_id, MemberKind::Operation, Some(authorise_id)));
        h.run();
        // The editor's own box, beside the classifier's: the one its explanation describes.
        let member_node = |h: &Harness<'static, Shell>| {
            h.get_all_by_label("No redefinition (leaf)")
                .map(|n| n.accesskit_node().data().clone())
                .find(|n| n.description().is_some_and(|d| d.starts_with("A subclass could not redefine this member.")))
                .unwrap()
        };
        assert_eq!(member_node(&h).description(), Some("A subclass could not redefine this member. Shown as {leaf} after it."));
        let at = rect_of(&h, MEMBER_LEAF_ID).center();
        click(&mut h, at);
        assert!(uml_of(h.state()).member.as_ref().unwrap().is_leaf);
        assert!(!is_leaf(&h, authorise), "nothing written before Apply");
        let at = rect_of(&h, "workspace.properties.member.apply").center();
        click(&mut h, at);
        assert!(uml_of(h.state()).member.is_none(), "{:?}", uml_of(h.state()).member);
        assert!(is_leaf(&h, authorise));
        assert_eq!(h.state().session.as_ref().unwrap().undo_label(), Some("Set operation"));
        h.run();
        assert!(painted(&h).iter().any(|t| t == "+ authorise() : Boolean {leaf}"), "{:?}", painted(&h));
        h.state_mut().session.as_mut().unwrap().undo().unwrap();
        assert!(!is_leaf(&h, authorise));
        h.state_mut().session.as_mut().unwrap().redo().unwrap();
        assert!(is_leaf(&h, authorise));
        // CardPayment's amount redefines Payment's: leaf on Payment's amount is refused, naming it.
        let amount = member(&h, "Payment", "amount", false);
        let amount_id = uuid(&h, amount);
        let card = class(&h, "CardPayment");
        let card_id = uuid(&h, card);
        let money = h.state().session.as_ref().unwrap().doc().get(amount, archforge_uml::typed_element::TYPE).unwrap().as_ref().unwrap();
        let money = uuid(&h, money).to_string();
        let params = serde_json::json!({ "class": card_id.to_string(), "name": "amount", "type": money, "redefines": [amount_id.to_string()] });
        let card_amount =
            h.state_mut().session.as_mut().unwrap().execute("uml.attribute.add", &params).unwrap().value["id"].as_str().unwrap().to_string();
        let card_amount = archforge_mof::Uuid::parse(&card_amount).unwrap();
        select(&mut h, payment_id);
        archforge_ui_egui::testing::cx(h.state_mut(), |cx| uml_panel::open(cx, payment_id, MemberKind::Attribute, Some(amount_id)));
        h.run();
        let before = revision(&h);
        let at = rect_of(&h, MEMBER_LEAF_ID).center();
        click(&mut h, at);
        h.run();
        let said = "CardPayment::amount redefines it. Remove that redefinition first.";
        assert!(!uml_of(h.state()).member.as_ref().unwrap().is_leaf, "the box stays clear");
        assert!(painted(&h).iter().any(|t| t == said), "{:?}", painted(&h));
        assert!(ids(&h).contains(&format!("{MEMBER_LEAF_ID}.refusal")));
        let node = member_node(&h);
        assert!(node.description().is_some_and(|d| d.ends_with(said)), "{:?}", node.description());
        assert_eq!(revision(&h), before, "nothing entered the model or its history");
        uml_of_mut(h.state_mut()).member = None;
        // With that redefinition gone, Payment's amount made leaf; CardPayment's Redefines lists it
        // marked `{leaf}`, and its pick is refused under the field.
        let clear = serde_json::json!({ "attribute": card_amount.to_string(), "redefines": [] });
        h.state_mut().session.as_mut().unwrap().execute("uml.attribute.set", &clear).unwrap();
        let leaf = serde_json::json!({ "attribute": amount_id.to_string(), "leaf": true });
        h.state_mut().session.as_mut().unwrap().execute("uml.attribute.set", &leaf).unwrap();
        select(&mut h, card_id);
        archforge_ui_egui::testing::cx(h.state_mut(), |cx| uml_panel::open(cx, card_id, MemberKind::Attribute, Some(card_amount)));
        h.run();
        let at = rect_of(&h, "workspace.properties.member.redefines").center();
        click(&mut h, at);
        h.run();
        assert!(painted(&h).iter().any(|t| t == "{leaf}"), "the leaf property is marked");
        let at = rect_of(&h, "workspace.properties.member.redefines.1").center();
        click(&mut h, at);
        h.run();
        let edit = uml_of(h.state()).member.clone().unwrap();
        assert!(edit.redefines.is_empty());
        let refusal = "amount is leaf: nothing may redefine it. Clear \"No redefinition\" on it first.";
        assert_eq!((edit.refused.as_deref(), edit.refused_at), (Some(refusal), Some("redefines")));
        assert!(painted(&h).iter().any(|t| t == refusal), "{:?}", painted(&h));
        // Leaf cleared on Payment's amount: Redefines offers it, and it is picked and applied.
        uml_of_mut(h.state_mut()).member = None;
        let clear = serde_json::json!({ "attribute": amount_id.to_string(), "leaf": false });
        h.state_mut().session.as_mut().unwrap().execute("uml.attribute.set", &clear).unwrap();
        archforge_ui_egui::testing::cx(h.state_mut(), |cx| uml_panel::open(cx, card_id, MemberKind::Attribute, Some(card_amount)));
        h.run();
        let at = rect_of(&h, "workspace.properties.member.redefines").center();
        click(&mut h, at);
        h.run();
        let at = rect_of(&h, "workspace.properties.member.redefines.1").center();
        click(&mut h, at);
        h.run();
        assert_eq!(uml_of(h.state()).member.as_ref().unwrap().redefines, [amount_id]);
        let at = rect_of(&h, "workspace.properties.member.apply").center();
        click(&mut h, at);
        let st = h.state().session.as_ref().unwrap().doc();
        let o = st.by_uuid(card_amount).unwrap();
        assert_eq!(archforge_uml::modifiers::redefines(st, o), [amount]);
    }
}

/// An enumeration in Properties: its kind line, name and package, and its literals with their `+`,
/// no attribute or operation; with none, it says so. It is offered as a type.
#[test]
fn an_enumeration_in_properties_shows_its_literals_and_is_offered_as_a_type() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, _) = related_sample_in(theme, egui::vec2(1600.0, 1000.0));
        let d = h.state().canvas.active.unwrap().to_string();
        let priority = {
            let s = h.state_mut().session.as_mut().unwrap();
            let st = s.doc();
            let commerce =
                st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_uml::PACKAGE) && archforge_core::name_of(st, *o) == "Commerce").unwrap();
            let commerce = st.uuid(commerce).unwrap().to_string();
            let out = s.execute("uml.enumeration.add", &serde_json::json!({ "package": commerce, "name": "Priority" })).unwrap();
            let id = out.value["id"].as_str().unwrap().to_string();
            s.execute("diagram.place", &serde_json::json!({ "diagram": d, "element": id, "x": 560, "y": 1000, "width": 290, "height": 136 }))
                .unwrap();
            s.execute("view.select", &serde_json::json!({ "ids": [id] })).unwrap();
            archforge_mof::Uuid::parse(&id).unwrap()
        };
        h.state_mut().state.show_panel(archforge_shell_api::state::Panel::Properties);
        h.run();
        let texts = painted(&h);
        for want in ["Enumeration · model, in every diagram", "Enumeration", "Literals · 0", "No literals yet"] {
            assert!(texts.iter().any(|t| t == want), "{want} in {texts:?}");
        }
        let found = ids(&h);
        assert!(found.iter().any(|i| i == "workspace.properties.class.owner"), "its package");
        for none in ["workspace.properties.class.abstract", "workspace.properties.attributes.add", "workspace.properties.operations.add"] {
            assert!(!found.iter().any(|i| i == none), "{none} for an enumeration");
        }
        assert!(found.iter().any(|i| i == "workspace.properties.literals.add"), "a + for literals: {found:?}");
        // With literals: each a row in their order; a click selects it and the panel stays.
        archforge_ui_egui::testing::cx(h.state_mut(), |cx| uml_ui::put_literals(cx, priority, &["Low", "Normal", "High"])).unwrap();
        h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [priority.to_string()] })).unwrap();
        h.run();
        let literals = {
            let st = h.state().session.as_ref().unwrap().doc();
            archforge_uml::commands::literals(st, st.by_uuid(priority).unwrap()).into_iter().map(|l| st.uuid(l).unwrap()).collect::<Vec<_>>()
        };
        let tops: Vec<f32> = literals.iter().map(|l| rect_of(&h, &format!("workspace.properties.literal.{l}")).top()).collect();
        assert!(tops.windows(2).all(|w| w[0] < w[1]), "in their order: {tops:?}");
        assert!(painted(&h).iter().any(|t| t == "Literals · 3"));
        let at = rect_of(&h, &format!("workspace.properties.literal.{}", literals[2])).center();
        click(&mut h, at);
        assert_eq!(h.state().session.as_ref().unwrap().view().selection.len(), 1);
        let st = h.state().session.as_ref().unwrap().doc();
        assert_eq!(st.uuid(h.state().session.as_ref().unwrap().view().selection[0]).ok(), Some(literals[2]));
        assert!(painted(&h).iter().any(|t| t == "Literals · 3"), "its enumeration's panel stays");
        // It is offered as a type: Customer's new attribute takes it.
        assert!(
            uml_panel::type_choices(h.state().session.as_ref().unwrap().doc(), None)
                .iter()
                .any(|(l, id)| l == "Priority" && *id == priority.to_string())
        );
        let customer = select_named(&mut h, archforge_uml::CLASS, "Customer");
        let at = rect_of(&h, "workspace.properties.attributes.add").center();
        click(&mut h, at);
        h.event(egui::Event::Text("priority".into()));
        h.run();
        pick_type(&mut h, "workspace.properties.member.type", "Priority");
        let at = rect_of(&h, "workspace.properties.member.apply").center();
        click(&mut h, at);
        assert!(member_rows(&h, customer, false).iter().any(|r| r == "+ priority : Priority"), "{:?}", member_rows(&h, customer, false));
    }
}

/// A data type in Properties: its kind line, name, package and description, no Abstract, and its
/// Attributes and Operations with their `+` and editors, any visibility offered. It is offered as
/// a type once, beside the built-in types and the model's own. Its Skills name every data type. In
/// light and dark.
#[test]
fn a_data_type_in_properties_takes_attributes_and_operations_and_is_offered_as_a_type_once() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, _, [money, _], _) = data_type_sample(theme);
        h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [money.to_string()] })).unwrap();
        h.state_mut().state.show_panel(archforge_shell_api::state::Panel::Properties);
        h.run();
        let texts = painted(&h);
        for want in ["Data type · model, in every diagram", "Data type", "Attributes · 0", "Operations · 0"] {
            assert!(texts.iter().any(|t| t == want), "{want} in {texts:?}");
        }
        let found = ids(&h);
        for id in [
            "workspace.properties.name",
            "workspace.properties.description",
            "workspace.properties.class.owner",
            "workspace.properties.attributes.add",
            "workspace.properties.operations.add",
        ] {
            assert!(found.iter().any(|i| i == id), "{id} in {found:?}");
        }
        for none in ["workspace.properties.class.abstract", "workspace.properties.literals.add"] {
            assert!(!found.iter().any(|i| i == none), "{none} for a data type");
        }
        // An attribute through the editor, typed by a built-in type: one step; any visibility.
        let at = rect_of(&h, "workspace.properties.attributes.add").center();
        click(&mut h, at);
        h.event(egui::Event::Text("amount".into()));
        h.run();
        pick_type(&mut h, "workspace.properties.member.type", "Real");
        let at = rect_of(&h, "workspace.properties.member.visibility").center();
        click(&mut h, at);
        let offered: Vec<String> = ids(&h).into_iter().filter(|i| i.starts_with("workspace.properties.member.visibility.")).collect();
        assert_eq!(offered.len(), 4, "every visibility, as a class's member has: {offered:?}");
        let at = rect_of(&h, "workspace.properties.member.visibility.1").center();
        click(&mut h, at);
        let at = rect_of(&h, "workspace.properties.member.apply").center();
        click(&mut h, at);
        assert!(uml_of(h.state()).member.is_none(), "{:?}", uml_of(h.state()).member);
        assert_eq!(member_rows(&h, money, false), ["- amount : Real"]);
        assert_eq!(h.state().session.as_ref().unwrap().undo_label(), Some("Add attribute"));
        // An operation returning the data type itself.
        h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [money.to_string()] })).unwrap();
        h.run();
        let at = rect_of(&h, "workspace.properties.operations.add").center();
        click(&mut h, at);
        h.event(egui::Event::Text("negate".into()));
        h.run();
        pick_type(&mut h, "workspace.properties.member.type", "Amount");
        let at = rect_of(&h, "workspace.properties.member.apply").center();
        click(&mut h, at);
        assert_eq!(member_rows(&h, money, true), ["+ negate() : Amount"]);
        let texts = painted(&h);
        for want in ["Attributes · 1", "Operations · 1"] {
            assert!(texts.iter().any(|t| t == want), "{want} in {texts:?}");
        }
        // Offered as a type once, beside the built-in types, each once too: no type is listed twice.
        let choices = uml_panel::type_choices(h.state().session.as_ref().unwrap().doc(), None);
        assert_eq!(choices.iter().filter(|(l, id)| *id == money.to_string() && l == "Amount").count(), 1, "{choices:?}");
        let mut values: Vec<&str> = choices.iter().map(|(_, v)| v.as_str()).collect();
        let all = values.len();
        values.sort_unstable();
        values.dedup();
        assert_eq!(values.len(), all, "no type is listed twice: {choices:?}");
        for built_in in archforge_uml::members::BUILT_IN {
            assert_eq!(choices.iter().filter(|(l, _)| l == built_in).count(), 1, "{built_in} once: {choices:?}");
        }
        // Customer's new attribute takes it.
        let customer = select_named(&mut h, archforge_uml::CLASS, "Customer");
        let at = rect_of(&h, "workspace.properties.attributes.add").center();
        click(&mut h, at);
        h.event(egui::Event::Text("balance".into()));
        h.run();
        pick_type(&mut h, "workspace.properties.member.type", "Amount");
        let at = rect_of(&h, "workspace.properties.member.apply").center();
        click(&mut h, at);
        assert!(member_rows(&h, customer, false).iter().any(|r| r == "+ balance : Amount"), "{:?}", member_rows(&h, customer, false));
        // A package skill for every data type is said so, and the data type carries its own.
        let commerce = named(&h, archforge_uml::PACKAGE, "Commerce");
        let skill = {
            let s = h.state_mut().session.as_mut().unwrap();
            let out =
                s.execute("skill.add", &serde_json::json!({ "subject": commerce.to_string(), "name": "Values", "applies_to": "data type" })).unwrap();
            archforge_mof::Uuid::parse(out.value["id"].as_str().unwrap()).unwrap()
        };
        let st = h.state().session.as_ref().unwrap().doc();
        assert_eq!(
            archforge_ui_egui::testing::skill_target_word(
                h.state().session.as_ref().unwrap().view(),
                archforge_ui_egui::testing::notation_uis(h.state()),
                st,
                st.by_uuid(skill).unwrap()
            ),
            "every data type"
        );
        h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [money.to_string()] })).unwrap();
        h.state_mut().state.show_panel(archforge_shell_api::state::Panel::Skills);
        h.run();
        assert!(painted(&h).iter().any(|t| t == "On this data type · 0"));
        assert!(ids(&h).iter().any(|i| i == "workspace.skills.add"), "a data type carries skills");
    }
}

/// Every type can be chosen from a type menu however many the model holds: the menu once listed
/// sixteen of the model's types and dropped the rest. Now it lays out only the rows in sight, and the
/// arrow keys reach a type far down the list, which Enter picks. In light and dark.
#[test]
fn a_type_far_down_the_type_menu_can_be_chosen_and_only_the_rows_in_sight_are_laid_out() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, _) = related_sample_in(theme, egui::vec2(1600.0, 1000.0));
        // Forty data types in Commerce, named so that the last sorts after every other type.
        {
            let s = h.state_mut().session.as_mut().unwrap();
            let st = s.doc();
            let commerce =
                st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_uml::PACKAGE) && archforge_core::name_of(st, *o) == "Commerce").unwrap();
            let commerce = st.uuid(commerce).unwrap().to_string();
            for i in 0..40 {
                s.execute("uml.data-type.add", &serde_json::json!({ "package": commerce, "name": format!("Zone{i:02}") })).unwrap();
            }
        }
        let customer = select_named(&mut h, archforge_uml::CLASS, "Customer");
        let at = rect_of(&h, "workspace.properties.attributes.add").center();
        click(&mut h, at);
        h.event(egui::Event::Text("zone".into()));
        h.run();
        let choices = uml_panel::type_choices(h.state().session.as_ref().unwrap().doc(), None);
        let wanted = choices.iter().position(|(l, _)| l == "Zone39").expect("every type is offered");
        assert!(wanted > 16 + archforge_uml::members::BUILT_IN.len(), "far past the sixteen the menu once listed: {wanted}");
        let at = rect_of(&h, "workspace.properties.member.type").center();
        click(&mut h, at);
        let laid = |h: &Harness<'static, Shell>| {
            ids(h).into_iter().filter(|i| i.strip_prefix("workspace.properties.member.type.").is_some_and(|n| n.parse::<usize>().is_ok())).count()
        };
        let shown = laid(&h);
        assert!(shown > 0 && shown < choices.len(), "only the rows in sight: {shown} of {}", choices.len());
        // The arrows walk down to it, the menu scrolling with them, and Enter picks it.
        for _ in 0..=wanted {
            key(&mut h, egui::Key::ArrowDown, egui::Modifiers::NONE);
        }
        assert!(ids(&h).iter().any(|i| *i == format!("workspace.properties.member.type.{wanted}")), "scrolled into sight");
        assert!(laid(&h) <= shown + 1, "still only the rows in sight");
        key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
        let at = rect_of(&h, "workspace.properties.member.apply").center();
        click(&mut h, at);
        assert!(member_rows(&h, customer, false).iter().any(|r| r == "+ zone : Zone39"), "{:?}", member_rows(&h, customer, false));
    }
}

/// A type menu's labels follow the interface language: switched while the model stays as it is,
/// the kept choices are built again, so None reads in the new language; with nothing changed, the
/// kept choices are reused. In light and dark.
#[test]
fn a_type_menu_s_labels_follow_the_interface_language_and_are_reused_while_nothing_changes() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, _) = related_sample_in(theme, egui::vec2(1600.0, 1000.0));
        select_named(&mut h, archforge_uml::CLASS, "Customer");
        let at = rect_of(&h, "workspace.properties.attributes.add").center();
        click(&mut h, at);
        let at = rect_of(&h, "workspace.properties.member.type").center();
        click(&mut h, at);
        let kept = |h: &Harness<'static, Shell>| uml_of(h.state()).type_choices.clone().expect("the open menu keeps its choices");
        let english = kept(&h);
        assert_eq!((english.language, english.labels.first().map(String::as_str)), ("en", Some("None")));
        assert!(painted(&h).iter().any(|t| t == "None"), "the menu shows None");
        // Nothing changed: the next frame reuses them as they are.
        h.run();
        assert_eq!(kept(&h), english, "reused while nothing changed");
        // The language switched, as Appearance switches it, the model unchanged: built again, in German.
        h.state_mut().state.language = "de".into();
        h.run();
        let german = kept(&h);
        assert_eq!((german.language, german.revision), ("de", english.revision), "the same state, another language");
        assert_eq!(german.labels.first().map(String::as_str), Some("Keine"));
        assert_eq!(german.values, english.values, "the same types");
        assert!(painted(&h).iter().any(|t| t == "Keine"), "the menu shows None in German");
        h.state_mut().state.language = "en".into();
        h.run();
        archforge_ui_egui::i18n::set_language("en");
    }
}

/// The Description field in Properties describes a UML element (AR-186): typed and committed with
/// Enter, or by leaving the field, it is kept as a comment the element owns, one undo step, and it
/// stays when the element is selected again; a package and a member take one too. In light and
/// dark.
#[test]
fn a_description_typed_in_properties_stays_on_a_uml_element_after_reselecting_it() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, _) = related_sample_in(theme, egui::vec2(1600.0, 1000.0));
        h.state_mut().state.show_panel(archforge_shell_api::state::Panel::Properties);
        let described = |h: &Harness<'static, Shell>, id: archforge_mof::Uuid| {
            let st = h.state().session.as_ref().unwrap().doc();
            archforge_uml::descriptions::description_of(st, st.by_uuid(id).unwrap())
        };
        let attribute = {
            let st = h.state().session.as_ref().unwrap().doc();
            let customer = st.by_uuid(named(&h, archforge_uml::CLASS, "Customer")).unwrap();
            st.uuid(archforge_uml::members::attributes(st, customer)[0]).unwrap()
        };
        for (i, (class, name)) in
            [(archforge_uml::CLASS, "Customer"), (archforge_uml::PACKAGE, "Commerce"), (archforge_uml::PROPERTY, "")].into_iter().enumerate()
        {
            let element = if name.is_empty() {
                h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [attribute.to_string()] })).unwrap();
                h.run();
                attribute
            } else {
                select_named(&mut h, class, name)
            };
            assert!(ids(&h).iter().any(|i| i == "workspace.properties.description"), "{:?}", ids(&h));
            let text = format!("Described {i}.");
            let at = rect_of(&h, "workspace.properties.description").center();
            click(&mut h, at);
            h.event(egui::Event::Text(text.clone()));
            h.run();
            if i == 1 {
                // Leaving the field commits it too.
                key(&mut h, egui::Key::Tab, egui::Modifiers::NONE);
            } else {
                key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
            }
            assert_eq!(h.state().panels.notice, None, "no refusal");
            assert_eq!(described(&h, element), text, "kept on {name:?}");
            assert_eq!(h.state().session.as_ref().unwrap().undo_label(), Some("Describe"));
            // Another element selected, then this one again: the field shows its description.
            select_named(&mut h, archforge_uml::CLASS, "Order");
            assert!(!painted(&h).contains(&text), "Order shows its own, none");
            if name.is_empty() {
                h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [attribute.to_string()] })).unwrap();
                h.run();
            } else {
                select_named(&mut h, class, name);
            }
            assert!(painted(&h).contains(&text), "{text} in {:?}", painted(&h));
            assert_eq!(described(&h, element), text);
        }
        // Emptied, the description goes with its comment.
        let customer = select_named(&mut h, archforge_uml::CLASS, "Customer");
        let at = rect_of(&h, "workspace.properties.description").center();
        click(&mut h, at);
        key(&mut h, egui::Key::A, egui::Modifiers::COMMAND);
        key(&mut h, egui::Key::Backspace, egui::Modifiers::NONE);
        key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
        assert_eq!(described(&h, customer), "");
        let st = h.state().session.as_ref().unwrap().doc();
        assert_eq!(archforge_uml::descriptions::description_comment(st, st.by_uuid(customer).unwrap()), None);
    }
}

/// Alt+Down opens an end's multiplicity menu only as itself: with Shift more, a Mac's Control or
/// the command key of either shape it opens nothing and the typing stays as it was. The open menu's
/// arrows and Escape act alone: with a modifier they move and close nothing.
#[test]
fn alt_down_opens_a_multiplicity_menu_only_with_alt_alone() {
    use egui::Modifiers as M;
    in_both_appearances(|theme| {
        let (mut h, _) = paid_by(theme, egui::vec2(1600.0, 1000.0));
        // With the window's own bar and its Alt letters, as on Windows and Linux (AR-538).
        h.state_mut().in_window_menu = true;
        let open = |h: &Harness<'static, Shell>| h.state().panels.menu == Some(uml_state::menus::END_MULTIPLICITY.at(1));
        let at = rect_of(&h, "workspace.properties.end.1.multiplicity").center();
        click(&mut h, at);
        for more in [M::SHIFT, M::CTRL, M::MAC_CMD | M::COMMAND, M::CTRL | M::COMMAND] {
            key(&mut h, egui::Key::ArrowDown, M::ALT | more);
            assert!(!open(&h), "{theme:?}: {more:?} more opens nothing");
            assert_eq!(typed_multiplicity(&h, 1), "0..1", "{theme:?}: {more:?} more changes no typing");
        }
        key(&mut h, egui::Key::ArrowDown, M::ALT);
        assert_eq!((open(&h), h.state().panels.highlight), (true, Some(0)), "{theme:?}: Alt+Down opens it at the current one");
        for (k, more) in [egui::Key::ArrowDown, egui::Key::Escape].into_iter().flat_map(|k| NOT_ALONE.map(|m| (k, m))) {
            key(&mut h, k, more);
            assert_eq!((open(&h), h.state().panels.highlight), (true, Some(0)), "{theme:?}: {k:?} with {more:?}");
        }
        key(&mut h, egui::Key::ArrowDown, M::NONE);
        assert_eq!(h.state().panels.highlight, Some(1), "{theme:?}: the arrow alone");
        key(&mut h, egui::Key::Escape, M::NONE);
        assert!(!open(&h), "{theme:?}: Escape alone");
    });
}

/// An association end's multiplicity menu opens below its field, or above it where the window has no room
/// below, and shows whole either way.
#[test]
fn an_end_multiplicity_menu_opens_above_its_field_when_the_window_has_no_room_below() {
    // Tall enough to show end 1's field under end 0's Qualifiers section (AR-127), its menu then
    // having no room below.
    let (mut h, _) = paid_by(ThemeKind::Light, egui::vec2(1600.0, 880.0));
    for (i, above) in [(0, false), (1, true)] {
        let at = rect_of(&h, &format!("workspace.properties.end.{i}.multiplicity.menu")).center();
        click(&mut h, at);
        let field = rect_of(&h, &format!("workspace.properties.end.{i}.multiplicity"));
        let (first, clear) = (
            rect_of(&h, &format!("workspace.properties.end.{i}.multiplicity.menu.0")),
            rect_of(&h, &format!("workspace.properties.end.{i}.multiplicity.menu.clear")),
        );
        if above {
            assert!(clear.bottom() < field.top(), "end {i}: {clear:?} over {field:?}");
        } else {
            assert!(first.top() > field.bottom(), "end {i}: {first:?} under {field:?}");
        }
        assert!(clear.bottom() <= 880.0 && first.top() >= 0.0, "end {i}: the whole menu shows");
        key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
    }
}

/// The attribute editor shows the modifiers in the design's layout: Default value, Static, Derived,
/// Read only, Ordered and Unique, Subsets and Redefines listing only the valid properties; one
/// Apply writes them all as one undo step and the card says them; Cancel and Escape leave the
/// model untouched. In light and dark.
#[test]
fn the_attribute_editor_writes_its_modifiers_in_one_apply_and_cancel_or_escape_leaves_the_model() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, made) = modifiers_sample(theme);
        let lines = made["lines"].clone();
        edit_attribute(&mut h, &lines);
        let found = ids(&h);
        for id in [
            "workspace.properties.member.default",
            "workspace.properties.member.static",
            "workspace.properties.member.derived",
            "workspace.properties.member.read-only",
            "workspace.properties.member.ordered",
            "workspace.properties.member.unique",
            "workspace.properties.member.subsets",
            "workspace.properties.member.redefines",
        ] {
            assert!(found.iter().any(|i| i == id), "{theme:?}: {id} in {found:?}");
        }
        let shown = painted(&h);
        for label in ["Default value", "Derived", "Read only", "Ordered", "Unique", "Subsets", "Redefines"] {
            assert!(shown.iter().any(|t| t == label), "{label} in {shown:?}");
        }
        // The choices are built once for this state of the project and kept while frames go by.
        let kept = uml_of(h.state()).targets.clone();
        assert_eq!(kept.len(), 1, "one list for the open editor");
        for _ in 0..3 {
            h.step();
        }
        assert_eq!(uml_of(h.state()).targets, kept, "the same list, not built again every frame");
        // Subsets lists Item's parts, which Order specializes, and nothing else.
        let at = rect_of(&h, "workspace.properties.member.subsets").center();
        click(&mut h, at);
        let offered: Vec<String> = ids(&h).into_iter().filter(|i| i.starts_with("workspace.properties.member.subsets.")).collect();
        assert_eq!(offered.len(), 3, "None, Order's end items and Item's parts: {offered:?}");
        let shown = painted(&h);
        assert!(shown.iter().any(|t| t == "items · Order") && shown.iter().any(|t| t == "parts · Item"), "{shown:?}");
        let at = rect_of(&h, "workspace.properties.member.subsets.2").center();
        click(&mut h, at);
        // A default, Derived and Ordered.
        type_into(&mut h, "workspace.properties.member.default", "none");
        for check in ["derived", "ordered"] {
            let at = rect_of(&h, &format!("workspace.properties.member.{check}")).center();
            click(&mut h, at);
        }
        let before = attribute_row(&h, &lines);
        assert_eq!(before, "+ lines : Item [*]", "nothing is written before Apply");
        let at = rect_of(&h, "workspace.properties.member.apply").center();
        click(&mut h, at);
        assert!(uml_of(h.state()).member.is_none(), "{:?}", uml_of(h.state()).member);
        assert_eq!(attribute_row(&h, &lines), "+ / lines : Item [*] = none {ordered, subsets parts}");
        assert_eq!(h.state().session.as_ref().unwrap().undo_label(), Some("Set attribute"));
        h.state_mut().session.as_mut().unwrap().undo().unwrap();
        assert_eq!(attribute_row(&h, &lines), before, "one undo step takes it all back");
        h.state_mut().session.as_mut().unwrap().redo().unwrap();
        h.run();
        assert!(painted(&h).iter().any(|t| t == "+ / lines : Item [*] = none {ordered, subsets parts}"), "the card and the row say it");
        // Cancel, then Escape: edits made and dropped, the model as it was.
        let written = attribute_row(&h, &lines);
        for way in ["cancel", "escape"] {
            edit_attribute(&mut h, &lines);
            let at = rect_of(&h, "workspace.properties.member.read-only").center();
            click(&mut h, at);
            assert!(uml_of(h.state()).member.as_ref().is_some_and(|e| e.read_only));
            if way == "cancel" {
                let at = rect_of(&h, "workspace.properties.member.cancel").center();
                click(&mut h, at);
            } else {
                key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
            }
            assert!(uml_of(h.state()).member.is_none(), "{way} closes the editor");
            assert_eq!(attribute_row(&h, &lines), written, "{way} leaves the model");
        }
    }
}

/// An enumeration-typed attribute picks its default among its literals; a default its type does
/// not read is refused under the field in the user's words, the editor kept open and nothing
/// written; a type the kept default does not fit is refused under the type, naming it; a property
/// with nothing to subset says so in a muted line. In light and dark.
#[test]
fn a_modifier_refusal_sits_under_its_field_and_an_enumeration_s_default_is_a_literal() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, made) = modifiers_sample(theme);
        let status = made["status"].clone();
        edit_attribute(&mut h, &status);
        // Status's literals, in a select.
        let at = rect_of(&h, "workspace.properties.member.default").center();
        click(&mut h, at);
        let offered: Vec<String> = ids(&h).into_iter().filter(|i| i.starts_with("workspace.properties.member.default.")).collect();
        assert_eq!(offered.len(), 3, "None, Placed and Paid: {offered:?}");
        let at = rect_of(&h, "workspace.properties.member.default.2").center();
        click(&mut h, at);
        // Nothing to subset or redefine for an attribute typed by an enumeration here.
        let shown = painted(&h);
        assert!(shown.iter().any(|t| t == "No property to subset") && shown.iter().any(|t| t == "No property to redefine"), "{shown:?}");
        let at = rect_of(&h, "workspace.properties.member.apply").center();
        click(&mut h, at);
        assert_eq!(attribute_row(&h, &status), "+ status : Status = Paid");
        // A type the default does not fit: refused under the type, naming the default.
        edit_attribute(&mut h, &status);
        pick_type(&mut h, "workspace.properties.member.type", "Integer");
        let at = rect_of(&h, "workspace.properties.member.apply").center();
        click(&mut h, at);
        let edit = uml_of(h.state()).member.clone().expect("the editor stays open");
        assert_eq!(edit.refused_at, Some("type"));
        assert_eq!(edit.refused.as_deref(), Some("The default value Paid does not fit Integer; change or clear it first."));
        assert!(painted(&h).iter().any(|t| t.contains("does not fit Integer")), "said under the field");
        assert_eq!(attribute_row(&h, &status), "+ status : Status = Paid", "nothing written");
        // A default that does not read: refused under the default.
        uml_of_mut(h.state_mut()).member.as_mut().unwrap().default = "many".into();
        h.run();
        let at = rect_of(&h, "workspace.properties.member.apply").center();
        click(&mut h, at);
        let edit = uml_of(h.state()).member.clone().expect("the editor stays open");
        assert_eq!(edit.refused_at, Some("default"));
        assert_eq!(edit.refused.as_deref(), Some("many is not a whole number, which an Integer's default is, such as 42."));
        assert_eq!(attribute_row(&h, &status), "+ status : Status = Paid", "nothing written");
        // Corrected, it is written.
        uml_of_mut(h.state_mut()).member.as_mut().unwrap().default = "7".into();
        h.run();
        let at = rect_of(&h, "workspace.properties.member.apply").center();
        click(&mut h, at);
        assert!(uml_of(h.state()).member.is_none());
        assert_eq!(attribute_row(&h, &status), "+ status : Integer = 7");
    }
}

/// An association's end shows its Modifiers summed up under a disclosure; opened, they are
/// written by their own Apply as one undo step, the line saying them beside the end; Cancel and
/// Escape bring the model's back; a refusal sits under its field in the user's words. Its role
/// written at once keeps the modifiers. In light and dark.
#[test]
fn an_association_end_s_modifiers_are_written_by_their_apply_and_its_line_says_them() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, made) = modifiers_sample(theme);
        let holds = made["holds"].clone();
        h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [holds] })).unwrap();
        h.run();
        assert!(painted(&h).iter().any(|t| t == "Modifiers"), "{:?}", painted(&h));
        let at = rect_of(&h, "workspace.properties.end.1.modifiers").center();
        click(&mut h, at);
        for id in ["default", "static", "derived", "read-only", "ordered", "unique", "subsets", "redefines", "cancel", "apply"] {
            assert!(ids(&h).iter().any(|i| *i == format!("workspace.properties.end.1.{id}")), "{theme:?}: {id}");
        }
        // Order's lines is offered to subset: the end's context is Order.
        let at = rect_of(&h, "workspace.properties.end.1.subsets").center();
        click(&mut h, at);
        assert!(painted(&h).iter().any(|t| t == "lines · Order"), "{:?}", painted(&h));
        let lines_row = ids(&h).into_iter().find(|i| i == "workspace.properties.end.1.subsets.1").expect("lines");
        let at = rect_of(&h, &lines_row).center();
        click(&mut h, at);
        type_into(&mut h, "workspace.properties.end.1.default", "none");
        for check in ["derived", "ordered"] {
            let at = rect_of(&h, &format!("workspace.properties.end.1.{check}")).center();
            click(&mut h, at);
        }
        let line = |h: &Harness<'static, Shell>| {
            let st = h.state().session.as_ref().unwrap().doc();
            let l = archforge_uml::figures::line(st, st.by_uuid(archforge_mof::Uuid::parse(&holds).unwrap()).unwrap()).unwrap();
            (l.ends[1].role.clone(), l.ends[1].text.clone())
        };
        assert_eq!(line(&h), (Some("items".into()), Some("*".into())), "nothing before Apply");
        let at = rect_of(&h, "workspace.properties.end.1.apply").center();
        click(&mut h, at);
        assert_eq!(line(&h), (Some("/items".into()), Some("* = none {ordered, subsets lines}".into())));
        assert_eq!(h.state().session.as_ref().unwrap().undo_label(), Some("Set association end"));
        assert!(uml_of(h.state()).ends.as_ref().is_some_and(|e| !e.modifiers_open[1]), "written, the Modifiers close");
        assert!(!ids(&h).iter().any(|i| i == "workspace.properties.end.1.apply"));
        h.state_mut().session.as_mut().unwrap().undo().unwrap();
        assert_eq!(line(&h), (Some("items".into()), Some("*".into())), "one step");
        h.state_mut().session.as_mut().unwrap().redo().unwrap();
        h.run();
        let shown = painted(&h);
        assert!(shown.iter().any(|t| t == "/items") && shown.iter().any(|t| t == "* = none {ordered, subsets lines}"), "the line says it: {shown:?}");
        let at = rect_of(&h, "workspace.properties.end.1.modifiers").center();
        click(&mut h, at);
        // Cancel and Escape bring the model's back.
        for way in ["cancel", "escape"] {
            let at = rect_of(&h, "workspace.properties.end.1.read-only").center();
            click(&mut h, at);
            assert!(uml_of(h.state()).ends.as_ref().is_some_and(|e| e.modifiers[1].read_only));
            if way == "cancel" {
                let at = rect_of(&h, "workspace.properties.end.1.cancel").center();
                click(&mut h, at);
            } else {
                key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
            }
            assert!(uml_of(h.state()).ends.as_ref().is_some_and(|e| !e.modifiers[1].read_only), "{way}");
            assert_eq!(line(&h).1.as_deref(), Some("* = none {ordered, subsets lines}"), "{way} leaves the model");
        }
        // A property the rules refuse, said under Redefines: Item's parts is no general's for Order's end.
        let parts = archforge_mof::Uuid::parse(&made["lines"]).unwrap();
        uml_of_mut(h.state_mut()).ends.as_mut().unwrap().modifiers[1].redefines = vec![parts];
        h.run();
        let at = rect_of(&h, "workspace.properties.end.1.apply").center();
        click(&mut h, at);
        let refused = uml_of(h.state()).ends.as_ref().and_then(|e| e.modifier_refused[1].clone());
        assert_eq!(refused, Some(("redefines", "lines is not a property this one can redefine.".to_string())));
        assert!(painted(&h).iter().any(|t| t == "lines is not a property this one can redefine."), "under its field");
        assert_eq!(line(&h).1.as_deref(), Some("* = none {ordered, subsets lines}"), "nothing written");
        assert!(uml_of(h.state()).ends.as_ref().is_some_and(|e| e.modifiers_open[1]), "refused, the Modifiers stay open");
        // Enter in the Default field applies, as Apply does, and closes the Modifiers.
        uml_of_mut(h.state_mut()).ends.as_mut().unwrap().modifiers[1].redefines.clear();
        let at = rect_of(&h, "workspace.properties.end.1.default").center();
        click(&mut h, at);
        key(&mut h, egui::Key::A, egui::Modifiers::COMMAND);
        h.event(egui::Event::Text("few".into()));
        h.run();
        key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
        assert_eq!(line(&h).1.as_deref(), Some("* = few {ordered, subsets lines}"), "Enter applied");
        assert!(uml_of(h.state()).ends.as_ref().is_some_and(|e| !e.modifiers_open[1] && e.modifier_refused[1].is_none()));
        // The role written at once keeps the modifiers.
        uml_of_mut(h.state_mut()).ends.as_mut().unwrap().modifiers[1].redefines.clear();
        let at = rect_of(&h, "workspace.properties.end.1.role").center();
        click(&mut h, at);
        key(&mut h, egui::Key::A, egui::Modifiers::COMMAND);
        h.event(egui::Event::Text("goods".into()));
        h.run();
        key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
        assert_eq!(line(&h), (Some("/goods".into()), Some("* = few {ordered, subsets lines}".into())));
    }
}

/// Escape in one end's Role gives up that role alone, and the other end's Modifiers keep their
/// change. In light and dark (AR-417).
#[test]
fn escape_in_an_end_s_role_leaves_the_other_end_s_modifiers() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let mut h = both_modifiers_open(theme, &[0]);
        let (ends, before) = (|h: &Harness<'static, Shell>| uml_of(h.state()).ends.clone().unwrap(), revision(&h));
        retype(&mut h, "workspace.properties.end.1.role", "goods");
        key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
        assert_eq!(ends(&h).role[1], "items", "{theme:?}: the role is given up");
        assert!(ends(&h).modifiers[0].read_only && ends(&h).modifiers_open[0], "{theme:?}: end 0's Modifiers keep their change");
        assert_eq!(revision(&h), before, "{theme:?}: nothing written");
    }
}

/// Escape in one end's Default field, which the toolkit lets go of as the frame begins, goes to
/// that end's Modifiers, not to the other end's changed last. In light and dark (AR-417).
#[test]
fn escape_in_an_end_s_default_goes_to_its_own_modifiers() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let mut h = both_modifiers_open(theme, &[1]);
        let changed = |h: &Harness<'static, Shell>| uml_of(h.state()).ends.as_ref().unwrap().modifiers.clone().map(|m| m.read_only);
        let at = rect_of(&h, "workspace.properties.end.0.default").center();
        click(&mut h, at);
        key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
        assert_eq!(changed(&h), [false, true], "{theme:?}: end 1's change stays");
        let at = rect_of(&h, "workspace.properties.end.1.default").center();
        click(&mut h, at);
        key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
        assert_eq!(changed(&h), [false, false], "{theme:?}: its own Default gives it up");
    }
}

/// With no field holding the keyboard, Escape goes to the Modifiers changed last (end 1); once
/// they are given up, cancelled or applied, the next goes on to end 0's. In light and dark (AR-417).
#[test]
fn escape_with_no_field_goes_on_to_the_other_end_once_the_last_is_resolved() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let pending = |h: &Harness<'static, Shell>| uml_of(h.state()).ends.clone().map(|e| [0, 1].map(|i| e.modifiers[i] != e.written_modifiers[i]));
        for way in ["escape", "cancel", "apply"] {
            let mut h = both_modifiers_open(theme, &[0, 1]);
            h.state_mut().panels.scroll = 10_000.0;
            h.run();
            match rect_of(&h, &format!("workspace.properties.end.1.{way}").replace(".escape", ".cancel")).center() {
                _ if way == "escape" => key(&mut h, egui::Key::Escape, egui::Modifiers::NONE),
                at => click(&mut h, at),
            }
            assert_eq!(pending(&h), Some([true, false]), "{theme:?} {way}: end 1 resolved, end 0 kept");
            key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
            assert_eq!(pending(&h), Some([false, false]), "{theme:?} {way}: Escape reached end 0");
        }
    }
}

/// A nested classifier's default name is free among everything its owner holds (AR-132): with an
/// attribute named like the next default, New inside picks the following free name and is made.
#[test]
fn a_new_nested_classifier_takes_a_name_free_of_its_owners_members() {
    let (mut h, _) = related_sample_in(ThemeKind::Light, egui::vec2(1600.0, 1000.0));
    let order = select_named(&mut h, archforge_uml::CLASS, "Order");
    h.state_mut()
        .session
        .as_mut()
        .unwrap()
        .execute("uml.attribute.add", &serde_json::json!({ "class": order.to_string(), "name": "enumeration 1" }))
        .unwrap();
    let made = archforge_ui_egui::testing::cx(h.state_mut(), |cx| uml_nesting::add_nested(cx, order, uml_classifier::Classifier::Enumeration))
        .expect("made");
    let st = h.state().session.as_ref().unwrap().doc();
    assert_eq!(archforge_core::name_of(st, st.by_uuid(made).unwrap()), "Enumeration 2", "the next name the owner leaves free");
    assert_eq!(h.state().explorer.rename.as_ref().map(|r| r.text.as_str()), Some("Enumeration 2"));
}

/// A name another parameter has, whatever the case, and a default that does not fit are refused in
/// place, nothing written; removing a parameter a binding uses is refused under the rows naming the
/// binding (C5), and Show the binding selects the binding itself (AR-131). In light and dark.
#[test]
fn template_parameter_refusals_are_said_in_place_and_nothing_is_written() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, made) = templates_sample(theme);
        let repo = made["Repository"].clone();
        h.state_mut()
            .session
            .as_mut()
            .unwrap()
            .execute("uml.template-parameter.add", &serde_json::json!({ "classifier": repo, "name": "T", "constraint": made["Entity"] }))
            .unwrap();
        h.run();
        let before = (signature_of(&h, &repo), h.state().session.as_ref().unwrap().undo_label().map(str::to_string));
        let at = rect_of(&h, "workspace.properties.class.templateParameters.add").center();
        click(&mut h, at);
        h.event(egui::Event::Text("t".into()));
        h.run();
        let at = rect_of(&h, "workspace.properties.class.templateParameter.1.apply").center();
        click(&mut h, at);
        let edit = uml_of(h.state()).template.clone().expect("the editor stays open");
        assert_eq!(edit.refused_at, Some("name"));
        let said = "Repository already has a template parameter named T. Pick another name.";
        assert!(painted(&h).iter().any(|t| t == said), "{theme:?}: said in place, {:?}", painted(&h));
        assert_eq!((signature_of(&h, &repo), h.state().session.as_ref().unwrap().undo_label().map(str::to_string)), before, "nothing written");
        uml_of_mut(h.state_mut()).template = None;
        h.run();
        // T's default offers only what fits Entity: Order, Entity; Repository is left out.
        let at = rect_of(&h, "workspace.properties.class.templateParameter.0.edit").center();
        click(&mut h, at);
        let at = rect_of(&h, "workspace.properties.class.templateParameter.0.default").center();
        click(&mut h, at);
        let shown = painted(&h);
        assert!(
            shown.iter().any(|t| t == "Order") && !ids(&h).iter().any(|i| i == "workspace.properties.class.templateParameter.0.default.3"),
            "{shown:?}"
        );
        key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
        // Typed in by hand, a default that does not fit is refused under Default.
        if let Some(e) = uml_of_mut(h.state_mut()).template.as_mut() {
            e.default = repo.clone();
        }
        h.run();
        let at = rect_of(&h, "workspace.properties.class.templateParameter.0.apply").center();
        click(&mut h, at);
        let edit = uml_of(h.state()).template.clone().expect("the editor stays open");
        assert_eq!(edit.refused_at, Some("default"), "{:?}", edit.refused);
        assert_eq!(signature_of(&h, &repo), ["T : Class > Entity"], "nothing written");
        uml_of_mut(h.state_mut()).template = None;
        h.run();
        // In the sample model OrderRepository binds Repository: Key's removal is refused, naming
        // the binding (C5).
        let mut h = workspace_harness(theme, egui::vec2(1600.0, 1500.0));
        archforge_ui_egui::testing::cx(h.state_mut(), uml_ui::open_sample).unwrap();
        h.run();
        let (sample_id, _) = archforge_ui_egui::testing::cx(h.state_mut(), |cx| uml_ui::put_repository(cx, true)).unwrap_or_else(|e| panic!("{e}"));
        h.state_mut().state.show_panel(archforge_shell_api::state::Panel::Properties);
        h.run();
        let rows_before = signature_of(&h, &sample_id.to_string());
        let at = rect_of(&h, "workspace.properties.class.templateParameter.1.edit").center();
        click(&mut h, at);
        let at = rect_of(&h, "workspace.properties.class.templateParameter.1.remove").center();
        click(&mut h, at);
        h.run();
        assert_eq!(signature_of(&h, &sample_id.to_string()), rows_before, "nothing removed");
        let said = "OrderRepository binds Key -> Integer. Change or remove that binding first, then remove Key.";
        assert!(painted(&h).iter().any(|t| t == said), "{theme:?}: {:?}", painted(&h));
        assert!(ids(&h).iter().any(|i| i == "workspace.properties.class.templateParameter.refusal"));
        let at = rect_of(&h, "workspace.properties.class.templateParameter.showBinding").center();
        click(&mut h, at);
        let s = h.state().session.as_ref().unwrap();
        let chosen = s.view().selection.first().copied().unwrap();
        assert_eq!(s.doc().class_of(chosen).unwrap(), archforge_uml::TEMPLATE_BINDING, "the binding itself is selected");
        assert_eq!(
            archforge_uml::templates::bound_element(s.doc(), chosen).map(|b| archforge_core::name_of(s.doc(), b)).as_deref(),
            Some("OrderRepository")
        );
        h.run();
        assert!(ids(&h).iter().any(|i| i == "workspace.properties.templateBinding.substitution.0.argument"), "its Properties show");
    }
}

/// A binding's substitutions are edited in its Properties (C2), each pick one undo step: Key given
/// String, then back to its default; another template opens the draft over the binding until
/// Apply; a draft is dropped when the selection moves on. Models writes binds Repository after the
/// bound class. In light and dark.
#[test]
fn a_template_binding_s_substitutions_are_edited_in_properties() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, made, _) = binding_sample(theme);
        let s = h.state_mut().session.as_mut().unwrap();
        let b = s
            .execute(
                "uml.template-binding.add",
                &serde_json::json!({ "bound": made["OrderRepository"], "template": made["Repository"], "arguments": { "T": made["Order"] } }),
            )
            .unwrap()
            .value["id"]
            .as_str()
            .unwrap()
            .to_string();
        h.run();
        assert!(painted(&h).iter().any(|t| t == "Template binding"), "{theme:?}: its heading");
        let label = |h: &Harness<'static, Shell>| {
            let st = h.state().session.as_ref().unwrap().doc();
            archforge_uml::bindings::label(st, st.by_uuid(archforge_mof::Uuid::parse(&b).unwrap()).unwrap())
        };
        // Key's menu: Default (Integer), then the built-in types and data types; String is one.
        let at = rect_of(&h, "workspace.properties.templateBinding.substitution.1.argument").center();
        click(&mut h, at);
        assert!(painted(&h).iter().any(|t| t == "Default (Integer)"), "{:?}", painted(&h));
        let entries: Vec<String> =
            ids(&h).into_iter().filter(|i| i.starts_with("workspace.properties.templateBinding.substitution.1.argument.")).collect();
        assert!(!entries.is_empty());
        key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
        // Default (Integer) first, then the built-in types in their order: Integer, String, ….
        let string_at = (archforge_uml::members::BUILT_IN.iter().position(|b| *b == "String").unwrap() + 1).to_string();
        pick_entry(&mut h, "workspace.properties.templateBinding.substitution.1.argument", &string_at);
        assert_eq!(label(&h), "«bind» <T -> Order, Key -> String>");
        assert_eq!(h.state().session.as_ref().unwrap().undo_label(), Some("Set template binding"));
        pick_entry(&mut h, "workspace.properties.templateBinding.substitution.1.argument", "0");
        assert_eq!(label(&h), "«bind» <T -> Order, Key -> Integer>", "back to the default");
        h.state_mut().session.as_mut().unwrap().undo().unwrap();
        assert_eq!(label(&h), "«bind» <T -> Order, Key -> String>", "undone");
        h.state_mut().session.as_mut().unwrap().redo().unwrap();
        // Models names the template after the bound class.
        let all: Vec<archforge_mof::Uuid> = {
            let st = h.state().session.as_ref().unwrap().doc();
            st.alive().filter_map(|o| st.uuid(o).ok()).collect()
        };
        h.state_mut().explorer.expanded.extend(all);
        h.run();
        assert!(painted(&h).iter().any(|t| t == "binds Repository"), "{:?}", painted(&h));
        // Another template opens the draft over the binding; selecting something else drops it.
        let s = h.state_mut().session.as_mut().unwrap();
        let pair = s
            .execute("uml.class.add", &serde_json::json!({ "package": archforge_uml::commands::package_of(s.doc(), s.doc().by_uuid(archforge_mof::Uuid::parse(&made["Repository"]).unwrap()).unwrap()).and_then(|p| s.doc().uuid(p).ok()).unwrap().to_string(), "name": "Pair" }))
            .unwrap()
            .value["id"]
            .as_str()
            .unwrap()
            .to_string();
        s.execute("uml.template-parameter.add", &serde_json::json!({ "classifier": pair, "name": "A" })).unwrap();
        s.execute("view.select", &serde_json::json!({ "ids": [b] })).unwrap();
        h.run();
        let before = label(&h);
        // Pair is the first template by name, Repository the second.
        pick_entry(&mut h, "workspace.properties.templateBinding.template", "0");
        assert!(uml_of(h.state()).binding.as_ref().is_some_and(|d| d.binding.is_some()), "the draft over the binding");
        assert!(ids(&h).iter().any(|i| i == "workspace.properties.templateBinding.apply"));
        assert_eq!(label(&h), before, "nothing written yet");
        h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [made["Order"]] })).unwrap();
        h.run();
        assert!(uml_of(h.state()).binding.is_none(), "{theme:?}: the draft goes with the selection");
        assert_eq!(label(&h), before);
    }
}
