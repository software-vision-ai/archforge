//! The rail's panels and About over a UML model: each rail icon opens its own panel, the Skills
//! panel offers and adds skills where the selection carries them, and About opens its pages and
//! leaves what is behind it as it was.

use super::*;

/// Each rail icon opens its own panel, in light and dark, and each draws a shape no other of them uses.
#[test]
fn each_rail_icon_opens_its_own_panel_in_both_appearances() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, _) = class_properties("Order");
        h.state_mut().state.theme = theme;
        h.run();
        for (id, panel) in [
            ("agent", archforge_shell_api::state::Panel::Agent),
            ("appearance", archforge_shell_api::state::Panel::Appearance),
            ("skills", archforge_shell_api::state::Panel::Skills),
            ("properties", archforge_shell_api::state::Panel::Properties),
        ] {
            let at = rect_of(&h, &format!("workspace.panels.{id}")).center();
            click(&mut h, at);
            assert_eq!((h.state().state.panel, h.state().state.panel_collapsed), (panel, false), "{theme:?}: {id}");
        }
    }
    // Four shapes of their own, none the drawing another control uses.
    let rail = [
        &archforge_ui_egui::ux::icons::CHAT_BUBBLE,
        &archforge_ui_egui::ux::icons::PALETTE,
        &archforge_ui_egui::ux::icons::PROPERTIES,
        &archforge_ui_egui::ux::icons::SKILLS,
    ];
    for (i, a) in rail.iter().enumerate() {
        for b in rail.iter().skip(i + 1) {
            assert!(!std::ptr::eq(*a, *b));
        }
    }
}

/// The Skills panel offers `+` only where the model lets the selection carry a skill: a literal and
/// an association say in a muted line that they carry none, and a class keeps its `+`; the panel
/// and `skill.add` ask the same rule. In light and dark.
#[test]
fn the_skills_panel_offers_add_skill_only_where_the_selection_carries_skills() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, _) = related_sample_in(theme, egui::vec2(1600.0, 1000.0));
        let priority = {
            let s = h.state_mut().session.as_mut().unwrap();
            let st = s.doc();
            let commerce =
                st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_uml::PACKAGE) && archforge_core::name_of(st, *o) == "Commerce").unwrap();
            let commerce = st.uuid(commerce).unwrap().to_string();
            let out = s.execute("uml.enumeration.add", &serde_json::json!({ "package": commerce, "name": "Priority" })).unwrap();
            archforge_mof::Uuid::parse(out.value["id"].as_str().unwrap()).unwrap()
        };
        archforge_ui_egui::snapshot::cx(h.state_mut(), |cx| uml_ui::put_literals(cx, priority, &["Low"])).unwrap();
        h.state_mut().state.show_panel(archforge_shell_api::state::Panel::Skills);
        h.run();
        let low = {
            let st = h.state().session.as_ref().unwrap().doc();
            st.uuid(archforge_uml::commands::literals(st, st.by_uuid(priority).unwrap())[0]).unwrap()
        };
        let paid_by = {
            let st = h.state().session.as_ref().unwrap().doc();
            let a = st
                .alive()
                .find(|o| st.class_of(*o).ok() == Some(archforge_uml::ASSOCIATION) && archforge_core::name_of(st, *o) == "paid by")
                .unwrap();
            st.uuid(a).unwrap()
        };
        let customer = named(&h, archforge_uml::CLASS, "Customer");
        for (selected, title, line) in [
            (low, "On this literal · 0", Some("Literals carry no skills")),
            (paid_by, "On this association · 0", Some("Relationships carry no skills")),
            (customer, "On this class · 0", None),
        ] {
            h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [selected.to_string()] })).unwrap();
            h.run();
            let texts = painted(&h);
            assert!(texts.iter().any(|t| t == title), "{title} in {texts:?}");
            let offered = ids(&h).iter().any(|i| i == "workspace.skills.add");
            assert_eq!(offered, line.is_none(), "{title}: + only where a skill can sit");
            for said in ["Literals carry no skills", "Relationships carry no skills"] {
                assert_eq!(texts.iter().any(|t| t == said), line == Some(said), "{title}: {said}");
            }
            // The panel and the engine ask the same rule.
            let s = h.state().session.as_ref().unwrap();
            let o = s.doc().by_uuid(selected).unwrap();
            assert_eq!(archforge_core::skills::subject_problem(s.view(), s.doc(), o).is_none(), offered);
            let s = h.state_mut().session.as_mut().unwrap();
            let added = s.execute("skill.add", &serde_json::json!({ "subject": selected.to_string(), "name": "Probe" }));
            assert_eq!(added.is_ok(), offered, "{title}: skill.add agrees with the panel");
            if added.is_ok() {
                s.undo().unwrap();
            }
        }
    }
}

/// A class takes a skill from the Skills panel's +, which opens in its tab and shows in Models under the
/// class's row, above its members; an attribute shows the class's skill as inherited and takes one of its
/// own.
#[test]
fn a_class_and_an_attribute_take_a_skill_from_the_skills_panel_and_the_tree_lists_the_class_skill() {
    let (mut h, customer) = class_properties("Customer");
    h.state_mut().state.show_panel(archforge_shell_api::state::Panel::Skills);
    h.run();
    let at = rect_of(&h, "workspace.skills.add").center();
    click(&mut h, at);
    h.run();
    let on = |h: &Harness<'static, Shell>, subject: archforge_mof::Uuid| {
        let st = h.state().session.as_ref().unwrap().doc();
        let o = st.by_uuid(subject).unwrap();
        archforge_core::skills::skills(st).into_iter().filter(|s| archforge_core::skills::subject_of(st, *s) == Some(o)).collect::<Vec<_>>()
    };
    let skills = on(&h, customer);
    assert_eq!(skills.len(), 1, "the class carries the new skill");
    let skill = h.state().session.as_ref().unwrap().doc().uuid(skills[0]).unwrap();
    assert_eq!(h.state().canvas.active, Some(skill), "it opens in its tab");
    // The class's row in the Models tree opens onto its skill, above its members.
    let package = {
        let st = h.state().session.as_ref().unwrap().doc();
        st.uuid(st.container(st.by_uuid(customer).unwrap()).unwrap().unwrap().0).unwrap()
    };
    h.state_mut().explorer.expanded.extend([package, customer]);
    h.run();
    let found = ids(&h);
    let row = |id: archforge_mof::Uuid| found.iter().position(|i| *i == format!("workspace.explorer.row.{id}"));
    assert!(row(skill).is_some_and(|s| row(customer).is_some_and(|c| s > c)), "{found:?}");
    // An attribute takes one too, and the class's skill shows on it as inherited.
    let attribute = {
        let st = h.state().session.as_ref().unwrap().doc();
        st.uuid(archforge_uml::members::attributes(st, st.by_uuid(customer).unwrap())[0]).unwrap()
    };
    h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [attribute.to_string()] })).unwrap();
    h.state_mut().state.show_panel(archforge_shell_api::state::Panel::Skills);
    h.run();
    assert!(ids(&h).contains(&format!("workspace.skills.row.{skill}")), "inherited from the class");
    let at = rect_of(&h, "workspace.skills.add").center();
    click(&mut h, at);
    h.run();
    assert_eq!(on(&h, attribute).len(), 1, "the attribute carries its own");
}

/// About's support destinations, once set up, each say they open in the browser and open their page by a
/// click or by Tab and Enter; the workspace's keys behind the dialog change nothing, and Done leaves the
/// project open.
#[test]
fn a_set_up_destination_opens_its_page_by_click_or_keyboard_and_the_project_stays_as_it_was() {
    let (mut h, _order) = class_properties("Order");
    h.state_mut().services.support = archforge_ui_egui::support::Destinations {
        github_sponsors: Some("https://github.com/sponsors/DejanWho".into()),
        ko_fi: Some("https://ko-fi.com/archforge".into()),
    };
    let revision = h.state().session.as_ref().unwrap().revision();
    h.state_mut().open_about();
    h.run();
    assert_eq!(painted(&h).iter().filter(|t| *t == "Opens in your browser").count(), 2, "{:?}", painted(&h));
    // A click opens the provider's page in the browser.
    let row = rect_of(&h, "about.support.github-sponsors").center();
    h.event(egui::Event::PointerMoved(row));
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton { pos: row, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE });
    }
    h.step();
    assert_eq!(opened_links(&h), ["https://github.com/sponsors/DejanWho"]);
    h.run();
    // From the keyboard: Tab to a destination and Enter.
    h.state_mut().canvas.focused = true;
    for _ in 0..8 {
        if h.ctx.memory(|m| m.has_focus(archforge_ui_egui::widgets::egui_id("about.support.ko-fi"))) {
            break;
        }
        key(&mut h, egui::Key::Tab, egui::Modifiers::NONE);
    }
    assert!(h.ctx.memory(|m| m.has_focus(archforge_ui_egui::widgets::egui_id("about.support.ko-fi"))), "Tab reaches Ko-fi");
    h.event(egui::Event::Key { key: egui::Key::Enter, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE });
    h.step();
    assert_eq!(opened_links(&h), ["https://ko-fi.com/archforge"]);
    h.event(egui::Event::Key { key: egui::Key::Enter, physical_key: None, pressed: false, repeat: false, modifiers: egui::Modifiers::NONE });
    h.run();
    // Behind it the workspace's keys rest, and the project is as it was.
    key(&mut h, egui::Key::Delete, egui::Modifiers::NONE);
    key(&mut h, egui::Key::Z, egui::Modifiers::COMMAND);
    assert_eq!(h.state().session.as_ref().unwrap().revision(), revision, "nothing behind the dialog changed");
    let at = rect_of(&h, "about.done").center();
    click(&mut h, at);
    assert!(h.state().about.is_none());
    assert!(painted(&h).iter().any(|t| t == "Order"), "the project is still open");
}

/// Escape alone closes About, Escape with another modifier closes nothing, and an attribute draft open in
/// Properties behind it stays.
#[test]
fn escape_closes_about_and_leaves_a_draft_behind_it_as_it_was() {
    let (mut h, _customer) = class_properties("Customer");
    let at = rect_of(&h, "workspace.properties.attributes.add").center();
    click(&mut h, at);
    assert!(uml_of(h.state()).member.is_some(), "an attribute draft is open");
    h.state_mut().open_about();
    h.run();
    for more in NOT_ALONE {
        key(&mut h, egui::Key::Escape, more);
        assert!(h.state().about.is_some() && uml_of(h.state()).member.is_some(), "Escape with {more:?} closes nothing");
    }
    key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
    assert!(h.state().about.is_none(), "Escape closes About");
    assert!(uml_of(h.state()).member.is_some(), "and the draft behind it stays");
}

/// About closes the bar's search, and the arrows and Enter pressed while it is open stay the dialog's: a
/// search open behind it neither moves nor runs, and the panel and the selection stay.
#[test]
fn about_closes_the_bars_search_and_its_keys_never_reach_the_search_behind_it() {
    let (mut h, _order) = class_properties("Order");
    archforge_ui_egui::snapshot::open_home_search(h.state_mut(), "order");
    h.run();
    assert!(h.state().home_search.open);
    h.state_mut().open_about();
    h.run();
    assert!(!h.state().home_search.open, "About closes the search");
    // Were the search open behind it, Enter and the arrows stay the dialog's.
    h.state_mut().home_search.open = true;
    h.state_mut().home_search.query = "order".into();
    let shown = h.state().state.panel;
    let selection = h.state().session.as_ref().unwrap().view().selection.clone();
    for k in [egui::Key::ArrowDown, egui::Key::Enter] {
        h.event(egui::Event::Key { key: k, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE });
        h.step();
        h.event(egui::Event::Key { key: k, physical_key: None, pressed: false, repeat: false, modifiers: egui::Modifiers::NONE });
        h.run();
    }
    assert!(h.state().about.is_some(), "About is still open");
    assert_eq!(h.state().home_search.highlight, 0, "the search's highlight did not move");
    assert!(h.state().home_search.open, "nothing in the search ran");
    assert_eq!(h.state().state.panel, shown);
    assert_eq!(h.state().session.as_ref().unwrap().view().selection, selection, "the selection is as it was");
}
