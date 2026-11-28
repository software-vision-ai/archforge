//! Profiles and stereotypes through the window: profiles made in Models, edited in Properties and
//! applied to packages; stereotypes applied, their values, their picker, and what deleting a
//! stereotyped element asks.

use super::*;

/// A stereotype made in a profile through the narrow context, as New ▸ Stereotype makes it, asks
/// Models, in this order, to open the rows above it, to open its name for typing and to show
/// itself, and Models does all three once the call returns (AR-134, F2).
#[test]
fn a_new_stereotype_is_named_in_models_once_the_call_returns() {
    use archforge_shell_api::cx::Request;
    use archforge_shell_api::state::Explorer;
    let (mut h, _) = related_sample_in(ThemeKind::Light, egui::vec2(1600.0, 1000.0));
    let (_, persistence, _) = archforge_ui_egui::testing::cx(h.state_mut(), |cx| uml_profiles::put_profiles(cx, false)).unwrap();
    let persistence = archforge_mof::Uuid::parse(&persistence).unwrap();
    let shell = h.state_mut();
    let profile = shell.session.as_ref().unwrap().doc().by_uuid(persistence).unwrap();
    shell.explorer.expanded.clear();
    (shell.explorer.filter, shell.explorer.rename, shell.explorer.focused) = ("zzz".into(), None, false);
    (shell.state.explorer, shell.state.explorer_collapsed) = (Explorer::Diagrams, true);
    let (made, asked) = archforge_ui_egui::testing::cx(shell, |cx| {
        let made = uml_profiles::add_inside(cx, "stereotype", profile).expect("offered").expect("made");
        (made, cx.requests.clone())
    });
    let [Request::ExpandRows(rows), Request::NameInModels(named, name), Request::ShowExplorer(Explorer::Models)] = &asked[..] else {
        panic!("the rows, the name, then Models: {asked:?}")
    };
    assert!(rows.contains(&persistence) && *named == made && name == "Stereotype 1", "{asked:?}");
    assert!(rows.iter().all(|row| shell.explorer.expanded.contains(row)), "the rows above it open");
    assert_eq!(shell.explorer.rename, Some(archforge_ui_egui::testing::fresh_rename(made, "Stereotype 1".into())), "its name open for typing");
    assert!(shell.explorer.filter.is_empty() && shell.explorer.focused, "listed, with the keyboard");
    assert_eq!((shell.state.explorer, shell.state.explorer_collapsed), (Explorer::Models, false), "Models shows, unfolded");
}

/// Profiles and stereotypes (AR-134, design page 08, F1, F2, F5 and F6), in light and dark: a
/// right click on a package's row in Models opens New ▸, which offers Package and Profile before
/// the classifiers; a profile is made in one undo step and named in place, `profile` after its
/// row; New ▸ Stereotype on the profile makes one; its Properties' Extends `+` lists the
/// metaclasses Archforge draws and those it does not, disabled with why, and a pick extends it in
/// one step, `extends Class` after its row; a property is added through the member editor, typed
/// by a built-in type; the package's Applied profiles `+` applies the profile in one step,
/// `applies …` after the package's row. With the profile in use, Unapply is refused under the
/// rows, the uses counted, nothing written, and Show the uses selects them.
#[test]
fn profiles_are_made_in_models_edited_in_properties_and_applied_to_packages() {
    use egui_kittest::kittest::Queryable;
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, _) = related_sample_in(theme, egui::vec2(1600.0, 1100.0));
        let commerce = select_named(&mut h, archforge_uml::PACKAGE, "Commerce");
        // F2: New ▸ on the package.
        let row = rect_of(&h, &format!("workspace.explorer.row.{commerce}"));
        right_click(&mut h, row.center());
        let at = rect_of(&h, "workspace.explorer.contextMenu.newInside").center();
        click(&mut h, at);
        assert!(painted(&h).iter().any(|t| t == "NEW IN COMMERCE"), "{theme:?}: {:?}", painted(&h));
        for kind in ["package", "profile", "class", "interface", "enumeration"] {
            assert!(ids(&h).iter().any(|i| *i == format!("workspace.explorer.contextMenu.newInside.{kind}")), "{kind}");
        }
        let at = rect_of(&h, "workspace.explorer.contextMenu.newInside.profile").center();
        click(&mut h, at);
        assert_eq!(h.state().session.as_ref().unwrap().undo_label(), Some("Add profile"));
        let rename = h.state().explorer.rename.clone().expect("named in place");
        assert_eq!(rename.text, "Profile 1");
        let profile = rename.target;
        h.state_mut().explorer.rename = None;
        h.run();
        assert!(painted(&h).iter().any(|t| t == "profile"), "{theme:?}: `profile` after its row");
        // New ▸ Stereotype on the profile.
        let row = rect_of(&h, &format!("workspace.explorer.row.{profile}"));
        right_click(&mut h, row.center());
        let at = rect_of(&h, "workspace.explorer.contextMenu.newInside").center();
        click(&mut h, at);
        let at = rect_of(&h, "workspace.explorer.contextMenu.newInside.stereotype").center();
        click(&mut h, at);
        assert_eq!(h.state().session.as_ref().unwrap().undo_label(), Some("Add stereotype"));
        let stereotype = h.state().explorer.rename.clone().expect("named in place").target;
        h.state_mut().explorer.rename = None;
        h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [stereotype.to_string()] })).unwrap();
        h.state_mut().state.show_panel(archforge_shell_api::state::Panel::Properties);
        h.run();
        // F1: Extends · 0, its + listing what may be extended and what may not.
        assert!(painted(&h).iter().any(|t| t == "Extends · 0"), "{theme:?}: {:?}", painted(&h));
        assert!(painted(&h).iter().any(|t| t.starts_with("Stereotype · in profile Profile 1")));
        assert!(h.query_by_label("Profile: Profile 1").is_some(), "the Profile select's accessible name");
        let at = rect_of(&h, uml_profiles::EXTENDS_ADD_ID).center();
        click(&mut h, at);
        assert_eq!(h.state().panels.menu, Some(uml_state::menus::STEREOTYPE_EXTENDS));
        assert!(painted(&h).iter().any(|t| t == "not drawn by Archforge yet"), "the rest disabled with why");
        let before = h.state().session.as_ref().unwrap().revision();
        let refused = format!("{}.{}", uml_profiles::EXTENDS_ADD_ID, archforge_uml::profiles::METACLASSES.len());
        let at = rect_of(&h, &refused).center();
        click(&mut h, at);
        assert_eq!(h.state().session.as_ref().unwrap().revision(), before, "a metaclass not drawn picks nothing");
        let at = rect_of(&h, &format!("{}.0", uml_profiles::EXTENDS_ADD_ID)).center();
        click(&mut h, at);
        assert_eq!(h.state().session.as_ref().unwrap().undo_label(), Some("Change stereotype"));
        h.run();
        assert!(painted(&h).iter().any(|t| t == "Extends · 1"));
        assert!(painted(&h).iter().any(|t| t == "extends Class"), "{theme:?}: after its row in Models");
        // A property through the member editor.
        assert!(painted(&h).iter().any(|t| t == "Properties · 0"));
        let at = rect_of(&h, "workspace.properties.attributes.add").center();
        click(&mut h, at);
        h.event(egui::Event::Text("table".into()));
        h.run();
        pick_type(&mut h, "workspace.properties.member.type", "String");
        let at = rect_of(&h, "workspace.properties.member.apply").center();
        click(&mut h, at);
        {
            let st = h.state().session.as_ref().unwrap().doc();
            let props = archforge_uml::profiles::properties(st, st.by_uuid(stereotype).unwrap());
            assert_eq!(props.iter().map(|a| archforge_uml::figures::attribute_text(st, *a)).collect::<Vec<_>>(), ["+ table : String"]);
        }
        // F5: Commerce applies it.
        select_named(&mut h, archforge_uml::PACKAGE, "Commerce");
        h.run();
        assert!(painted(&h).iter().any(|t| t == "Applied profiles · 0"), "{theme:?}: {:?}", painted(&h));
        let at = rect_of(&h, uml_profiles::APPLIED_ADD_ID).center();
        click(&mut h, at);
        let at = rect_of(&h, &format!("{}.0", uml_profiles::APPLIED_ADD_ID)).center();
        click(&mut h, at);
        assert_eq!(h.state().session.as_ref().unwrap().undo_label(), Some("Apply profile"));
        h.run();
        assert!(painted(&h).iter().any(|t| t == "Applied profiles · 1"));
        assert!(painted(&h).iter().any(|t| t == "applies Profile 1"), "after the package's row");

        // F6: in use, Unapply is refused with the uses counted, and they can be selected.
        let (mut h, _) = related_sample_in(theme, egui::vec2(1600.0, 1100.0));
        let (commerce, persistence, _) = archforge_ui_egui::testing::cx(h.state_mut(), |cx| uml_profiles::put_profiles(cx, true)).unwrap();
        h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [commerce] })).unwrap();
        h.state_mut().state.show_panel(archforge_shell_api::state::Panel::Properties);
        h.run();
        let at = rect_of(&h, &format!("{}.{persistence}.edit", uml_profiles::APPLIED_ROW_ID)).center();
        click(&mut h, at);
        let before = (h.state().session.as_ref().unwrap().revision(), h.state().session.as_ref().unwrap().undo_label().map(str::to_string));
        let at = rect_of(&h, &format!("{}.unapply", uml_profiles::APPLIED_EDIT_ID)).center();
        click(&mut h, at);
        let after = (h.state().session.as_ref().unwrap().revision(), h.state().session.as_ref().unwrap().undo_label().map(str::to_string));
        assert_eq!(after, before, "{theme:?}: nothing written");
        let why = "Persistence is in use in Commerce: «Entity» on Order and Customer, «Id» on 2 elements, «Reference» on places. Remove those first.";
        assert!(painted(&h).iter().any(|t| t == why), "{theme:?}: {:?}", painted(&h));
        assert!(painted(&h).iter().any(|t| t == "Show the 5 uses"));
        let at = rect_of(&h, &format!("{}.show", uml_profiles::USES_ID)).center();
        click(&mut h, at);
        assert_eq!(h.state().session.as_ref().unwrap().view().selection.len(), 5, "the uses selected");
    }
}

/// Show stereotype values refused says why under Properties in plain words, as a sentence, never as
/// the engine words it: a model that can't be changed at the moment. Nothing changes. In light and
/// dark (AR-431).
#[test]
fn show_stereotype_values_refused_says_why_in_plain_words() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, _) = related_sample_in(theme, egui::vec2(1600.0, 1100.0));
        archforge_ui_egui::testing::cx(h.state_mut(), |cx| uml_profiles::put_profiles(cx, false)).unwrap();
        select_named(&mut h, archforge_uml::CLASS, "Order");
        h.state_mut().state.show_panel(archforge_shell_api::state::Panel::Properties);
        h.run();
        let revision = h.state().session.as_ref().unwrap().revision();
        h.state_mut().session.as_mut().unwrap().set_read_only(true).unwrap();
        let at = rect_of(&h, uml_stereotypes::SHOW_ID).center();
        click(&mut h, at);
        h.state_mut().session.as_mut().unwrap().set_read_only(false).unwrap();
        let notice = h.state().panels.notice.clone().unwrap_or_else(|| panic!("{theme:?}: refused"));
        assert_eq!(notice, archforge_ui_egui::i18n::tr("The model cannot be changed at the moment; nothing was changed."), "{theme:?}");
        assert_eq!(h.state().session.as_ref().unwrap().revision(), revision, "{theme:?}: nothing changed");
    }
}

/// Deleting an element its stereotypes are applied to asks first, from Models too, and the dialog
/// names the applications that go with it and with what it takes along (owner decision for
/// AR-135, the design silent: as the nested classifiers are named); the confirmation removes them in
/// one step. A member selected on its class's panel has the section for itself.
#[test]
fn deleting_a_stereotyped_element_names_its_applications() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, _) = related_sample_in(theme, egui::vec2(1600.0, 1100.0));
        archforge_ui_egui::testing::cx(h.state_mut(), |cx| uml_profiles::put_profiles(cx, true)).unwrap();
        // A member selected: the section is its own.
        let id_attr = {
            let st = h.state().session.as_ref().unwrap().doc();
            let o = st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_uml::CLASS) && archforge_core::name_of(st, *o) == "Order").unwrap();
            st.uuid(archforge_uml::members::attributes(st, o).into_iter().find(|a| archforge_core::name_of(st, *a) == "id").unwrap()).unwrap()
        };
        h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [id_attr.to_string()] })).unwrap();
        h.state_mut().state.show_panel(archforge_shell_api::state::Panel::Properties);
        h.run();
        assert!(painted_has(&h, "Stereotypes of id · 1"), "{theme:?}: {:?}", painted(&h));
        assert!(painted_has(&h, "«Id» + id : Integer"), "the member's row says its stereotype");
        // Order goes with «Entity» and its id's «Id».
        select_named(&mut h, archforge_uml::CLASS, "Order");
        archforge_ui_egui::explorer::remove_selection(h.state_mut());
        h.run();
        let asked = h.state().explorer.delete.clone().expect("a dialog");
        let said = "3 stereotype applications go with it: «Entity» on Order, «Id» on id, «Reference» on places.";
        assert_eq!(asked.consequences, ["4 relationships go with it.", said]);
        assert!(painted_has(&h, said), "{theme:?}: {:?}", painted(&h));
        let at = rect_of(&h, "workspace.explorer.delete.confirm").center();
        click(&mut h, at);
        let st = h.state().session.as_ref().unwrap().doc();
        let left: Vec<String> = st
            .alive()
            .filter(|o| archforge_uml::applications::is_application(st, *o))
            .filter_map(|i| archforge_uml::profiles::element_of(st, i).map(|e| archforge_core::name_of(st, e)))
            .collect();
        assert_eq!(left, ["Customer", "id"], "{theme:?}: Customer's stay");
    }
}

/// Removing a member from its editor asks first when stereotype applications would go with it, the
/// dialog naming them as every other delete of a carrier does, and writes nothing until confirmed;
/// a member without one goes at once, as before (AR-135).
#[test]
fn a_members_remove_asks_before_its_stereotype_applications_go() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, _) = related_sample_in(theme, egui::vec2(1600.0, 1100.0));
        archforge_ui_egui::testing::cx(h.state_mut(), |cx| uml_profiles::put_profiles(cx, true)).unwrap();
        let order = select_named(&mut h, archforge_uml::CLASS, "Order");
        h.state_mut().state.show_panel(archforge_shell_api::state::Panel::Properties);
        let member = |h: &Harness<'static, Shell>, n: &str| {
            let st = h.state().session.as_ref().unwrap().doc();
            let o = st.by_uuid(order).unwrap();
            archforge_uml::members::attributes(st, o).into_iter().find(|a| archforge_core::name_of(st, *a) == n).and_then(|a| st.uuid(a).ok())
        };
        let id_attr = member(&h, "id").unwrap();
        archforge_ui_egui::testing::cx(h.state_mut(), |cx| uml_panel::open(cx, order, uml_panel::MemberKind::Attribute, Some(id_attr)));
        h.run();
        let before = h.state().session.as_ref().unwrap().revision();
        let at = rect_of(&h, "workspace.properties.member.remove").center();
        click(&mut h, at);
        assert_eq!(h.state().session.as_ref().unwrap().revision(), before, "{theme:?}: nothing written before the answer");
        let asked = h.state().explorer.delete.clone().expect("a dialog");
        assert_eq!(asked.consequences, ["1 stereotype application goes with it: «Id» on id."]);
        assert!(painted_has(&h, "1 stereotype application goes with it: «Id» on id."), "{theme:?}: {:?}", painted(&h));
        let at = rect_of(&h, "workspace.explorer.delete.confirm").center();
        click(&mut h, at);
        assert_eq!(member(&h, "id"), None, "{theme:?}: removed once confirmed");
        // One without an application goes at once.
        let placed = member(&h, "placedAt").unwrap();
        archforge_ui_egui::testing::cx(h.state_mut(), |cx| uml_panel::open(cx, order, uml_panel::MemberKind::Attribute, Some(placed)));
        h.run();
        let at = rect_of(&h, "workspace.properties.member.remove").center();
        click(&mut h, at);
        assert!(h.state().explorer.delete.is_none());
        assert_eq!(member(&h, "placedAt"), None);
    }
}

/// A nested package carrying a stereotype is not removed unasked from Models: the dialog names the
/// application and says its contents move up, and once confirmed the toast says where they went,
/// as a removal without the dialog does (AR-135). With the Stereotypes picker open, Backspace edits
/// its filter and never deletes the selected card.
#[test]
fn a_stereotyped_nested_package_asks_and_the_picker_keeps_backspace() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, _) = related_sample_in(theme, egui::vec2(1600.0, 1100.0));
        let (commerce, persistence, _) = archforge_ui_egui::testing::cx(h.state_mut(), |cx| uml_profiles::put_profiles(cx, false)).unwrap();
        {
            let s = h.state_mut().session.as_mut().unwrap();
            let module =
                s.execute("uml.stereotype.add", &serde_json::json!({ "profile": persistence, "name": "Module", "extends": ["Package"] })).unwrap();
            let billing = s.execute("uml.package.add", &serde_json::json!({ "name": "Billing", "parent": commerce })).unwrap();
            let billing = billing.value["id"].as_str().unwrap().to_string();
            s.execute("uml.class.add", &serde_json::json!({ "package": billing, "name": "Invoice" })).unwrap();
            s.execute("uml.stereotype.apply", &serde_json::json!({ "element": billing, "stereotype": module.value["id"] })).unwrap();
        }
        select_named(&mut h, archforge_uml::PACKAGE, "Billing");
        let before = h.state().session.as_ref().unwrap().revision();
        archforge_ui_egui::explorer::remove_selection(h.state_mut());
        h.run();
        assert_eq!(h.state().session.as_ref().unwrap().revision(), before, "{theme:?}: nothing removed unasked");
        let asked = h.state().explorer.delete.clone().expect("a dialog");
        assert_eq!(asked.consequences, ["1 stereotype application goes with it: «Module» on Billing."]);
        assert!(painted_has(&h, "What it holds moves to Commerce."), "{theme:?}: {:?}", painted(&h));
        let at = rect_of(&h, "workspace.explorer.delete.confirm").center();
        click(&mut h, at);
        let toast = h.state().canvas.toast.as_ref().map(|t| t.text.clone());
        assert_eq!(toast.as_deref(), Some("Billing removed; what it held moved to Commerce"));
        let st = h.state().session.as_ref().unwrap().doc();
        let invoice =
            st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_uml::CLASS) && archforge_core::name_of(st, *o) == "Invoice").unwrap();
        assert_eq!(archforge_uml::commands::package_of(st, invoice).map(|p| archforge_core::name_of(st, p)).as_deref(), Some("Commerce"));
        // Backspace with the picker open edits its filter, the selected card stays.
        let order = select_named(&mut h, archforge_uml::CLASS, "Order");
        h.state_mut().state.show_panel(archforge_shell_api::state::Panel::Properties);
        h.state_mut().canvas.focused = true;
        h.run();
        let shown = |h: &Harness<'static, Shell>| {
            let st = h.state().session.as_ref().unwrap().doc();
            archforge_diagram::views_of(st, st.by_uuid(order).unwrap()).2.len()
        };
        let cards = shown(&h);
        let at = rect_of(&h, uml_stereotypes::ADD_ID).center();
        click(&mut h, at);
        h.state_mut().canvas.focused = true;
        h.event(egui::Event::Text("en".into()));
        h.run();
        key(&mut h, egui::Key::Backspace, egui::Modifiers::NONE);
        assert_eq!(uml_of(h.state()).stereotypes.filter, "e", "{theme:?}: Backspace edits the filter");
        assert_eq!(shown(&h), cards, "{theme:?}: the card stays on the diagram");
        assert_eq!(h.state().panels.menu, Some(uml_state::menus::STEREOTYPE_PICK));
    }
}

/// With the Stereotypes picker open while Models has the keys, Backspace and the arrows stay the
/// picker's: the selected class is neither removed nor left for another row (AR-135, AR-787).
#[test]
fn the_picker_keeps_its_keys_from_the_models_tree() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, _) = related_sample_in(theme, egui::vec2(1600.0, 1100.0));
        archforge_ui_egui::testing::cx(h.state_mut(), |cx| uml_profiles::put_profiles(cx, false)).unwrap();
        let order = select_named(&mut h, archforge_uml::CLASS, "Order");
        h.state_mut().state.show_panel(archforge_shell_api::state::Panel::Properties);
        h.run();
        let at = rect_of(&h, uml_stereotypes::ADD_ID).center();
        click(&mut h, at);
        assert_eq!(h.state().panels.menu, Some(uml_state::menus::STEREOTYPE_PICK), "{theme:?}: the picker is open");
        (h.state_mut().explorer.focused, h.state_mut().canvas.focused) = (true, false);
        h.event(egui::Event::Text("en".into()));
        h.run();
        let before = h.state().session.as_ref().unwrap().revision();
        for k in [egui::Key::Backspace, egui::Key::ArrowDown] {
            key(&mut h, k, egui::Modifiers::NONE);
            assert!(h.state().explorer.delete.is_none(), "{theme:?}: {k:?} asks Models to remove nothing");
            assert_eq!(h.state().session.as_ref().unwrap().revision(), before, "{theme:?}: {k:?} removes nothing");
            assert_eq!(
                archforge_shell_api::panels::selected_id_cx(&archforge_ui_egui::testing::cx_ref(h.state())),
                Some(order),
                "{theme:?}: {k:?} leaves the class selected"
            );
        }
        assert_eq!(uml_of(h.state()).stereotypes.filter, "e", "{theme:?}: Backspace edited the picker's filter");
    }
}

/// The stereotype picker's filter ignores capitals and accents (AR-747), in light and dark:
/// *STRASSE* finds *Straße* and *prufung* finds *Prüfung*, each leaving the other out.
#[test]
fn the_stereotype_picker_filter_ignores_capitals_and_accents() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, _) = related_sample_in(theme, egui::vec2(1600.0, 1100.0));
        let (_, persistence, _) = archforge_ui_egui::testing::cx(h.state_mut(), |cx| uml_profiles::put_profiles(cx, false)).unwrap();
        let row = |h: &mut Harness<'static, Shell>, name: &str| {
            let s = h.state_mut().session.as_mut().unwrap();
            let made = s.execute("uml.stereotype.add", &serde_json::json!({ "profile": persistence, "name": name, "extends": ["Class"] })).unwrap();
            format!("{}.{}", uml_stereotypes::ADD_ID, made.value["id"].as_str().unwrap())
        };
        let (street, check) = (row(&mut h, "Straße"), row(&mut h, "Prüfung"));
        select_named(&mut h, archforge_uml::CLASS, "Order");
        h.state_mut().state.show_panel(archforge_shell_api::state::Panel::Properties);
        h.run();
        let at = rect_of(&h, uml_stereotypes::ADD_ID).center();
        click(&mut h, at);
        for (typed, found, left) in [("STRASSE", &street, &check), ("prufung", &check, &street)] {
            uml_of_mut(h.state_mut()).stereotypes.filter.clear();
            h.event(egui::Event::Text(typed.into()));
            h.run();
            let shown = ids(&h);
            assert!(shown.contains(found) && !shown.contains(left), "{theme:?}: {typed} finds its stereotype alone: {shown:?}");
        }
    }
}
