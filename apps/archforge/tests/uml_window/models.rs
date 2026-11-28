//! Models with UML's elements: packages and classifiers added, named, moved, nested and deleted
//! there, the delete dialog it asks with, its refusals and notices read out; and a project from
//! before the root package opened with its packages in one.

use super::*;

/// An association class drawn without a name has a nameless row in Models; deleted from there, the
/// dialog names it by its ends as its line runs, `Order → Entity`, as a generalization set's
/// members read (`CardPayment → Payment`) (AR-216). In light and dark.
#[test]
fn deleting_a_nameless_association_class_from_models_names_it_by_its_ends() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, made) = templates_sample(theme);
        let s = h.state_mut().session.as_mut().unwrap();
        let mut run = |cmd: &str, p: serde_json::Value| s.execute(cmd, &p).unwrap().value["id"].as_str().unwrap_or_default().to_string();
        let ac = run("uml.association-class.add", serde_json::json!({ "from": made["Order"], "to": made["Entity"] }));
        for (element, x) in [(&made["Order"], 200), (&made["Entity"], 700)] {
            run("diagram.place", serde_json::json!({ "diagram": made["diagram"], "element": element, "x": x, "y": 500 }));
        }
        run("diagram.connect-all", serde_json::json!({ "diagram": made["diagram"] }));
        run("view.select", serde_json::json!({ "ids": [] }));
        let all: Vec<archforge_mof::Uuid> = {
            let st = h.state().session.as_ref().unwrap().doc();
            st.alive().filter_map(|o| st.uuid(o).ok()).collect()
        };
        h.state_mut().explorer.expanded.extend(all);
        h.run();
        let rows = archforge_ui_egui::testing::models_rows(
            archforge_ui_egui::testing::notation_uis(h.state()),
            h.state().session.as_ref().unwrap().doc(),
            &h.state().explorer,
        );
        let row = rows.iter().find(|r| r.id.to_string() == ac).expect("its row in Models");
        assert_eq!(row.label, "", "{theme:?}: a nameless row");
        let at = rect_of(&h, &format!("workspace.explorer.row.{}", row.id)).center();
        click(&mut h, at);
        h.state_mut().explorer.focused = true;
        key(&mut h, egui::Key::Delete, egui::Modifiers::NONE);
        h.run();
        let asked = h.state().explorer.delete.clone().expect("the dialog asks: a diagram shows it");
        assert_eq!(asked.name, "Order → Entity", "{theme:?}");
        assert_eq!(delete_title(&h), "Delete Order → Entity from the model?", "{theme:?}");
        let at = rect_of(&h, "workspace.explorer.delete.cancel").center();
        click(&mut h, at);
        assert!(h.state().explorer.delete.is_none());
    }
}

/// Delete refused in Models is read out as a status under the tree, and goes with the next
/// Delete; a name refused in place is read out under its field until Escape closes it. In light
/// and dark (AR-425).
#[test]
fn the_explorer_s_refusals_are_read_out_as_a_status() {
    use egui_kittest::kittest::Queryable;
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, _) = related_sample_in(theme, egui::vec2(1600.0, 1000.0));
        select_named(&mut h, archforge_uml::PACKAGE, "Types");
        archforge_ui_egui::explorer::remove_selection(h.state_mut());
        h.run();
        let said = "Types keeps the built-in types, so it stays while it holds them";
        read_out(&h, said, "workspace.explorer.notice");
        select_named(&mut h, archforge_uml::CLASS, "Customer");
        archforge_ui_egui::explorer::remove_selection(h.state_mut());
        h.run();
        assert_eq!(h.state().explorer.notice, None, "{theme:?}: gone with the next Delete");
        assert!(h.query_by_label(said).is_none(), "{theme:?}: and no longer read out");
        h.state_mut().explorer.delete = None;
        h.state_mut().explorer.focused = true;
        key(&mut h, egui::Key::F2, egui::Modifiers::NONE);
        h.state_mut().explorer.rename.as_mut().expect("F2 opens the name").text = String::new();
        key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
        let refused = h.state().explorer.rename.as_ref().and_then(|r| r.refused.clone()).expect("an empty name is refused");
        assert_eq!(refused, archforge_ui_egui::i18n::tr("A name can't be empty."), "{theme:?}: in Properties' words (AR-427)");
        read_out(&h, &refused, "workspace.explorer.rename.notice");
        // The refused field kept the keyboard, so Escape reaches it with no click (AR-426).
        key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
        assert!(h.state().explorer.rename.is_none(), "{theme:?}: Escape closes the name");
        assert!(h.query_by_label(&refused).is_none(), "{theme:?}: and its refusal is no longer read out");
    }
}

/// A nested package whose class diagram would move up to a parent with no diagram yet stays when
/// Delete removes it from Models: the refusal is read out in the chosen language under the tree,
/// and the model and its history are as they were. In light and dark (AR-673).
#[test]
fn a_package_whose_diagrams_cannot_move_up_stays_and_models_says_why() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, _) = related_sample_in(theme, egui::vec2(1600.0, 1000.0));
        h.state_mut().state.language = "de".into();
        h.run();
        let s = h.state_mut().session.as_mut().unwrap();
        let mut id = |cmd: &str, p: serde_json::Value| s.execute(cmd, &p).unwrap().value["id"].as_str().unwrap().to_string();
        let vertrieb = id("uml.package.add", serde_json::json!({ "name": "Vertrieb" }));
        let archiv = id("uml.package.add", serde_json::json!({ "name": "Archiv", "parent": vertrieb }));
        id("diagram.add", serde_json::json!({ "kind": archforge_uml::STRUCTURE_DIAGRAM, "element": archiv, "name": "Archiv" }));
        select_named(&mut h, archforge_uml::PACKAGE, "Archiv");
        let before = h.state().session.as_ref().unwrap().token();
        archforge_ui_egui::explorer::remove_selection(h.state_mut());
        h.run();
        // The wording is the engine refusal test's; here, that Models shows it, in German.
        let said = h.state().explorer.notice.clone().unwrap_or_default();
        assert!(said.starts_with("Vertrieb hat noch kein Diagramm"), "{theme:?}: {said}");
        read_out(&h, &said, "workspace.explorer.notice");
        assert_eq!(h.state().session.as_ref().unwrap().token(), before, "{theme:?}: nothing changed");
        assert!(h.state().canvas.toast.is_none(), "{theme:?}: no Undo offered");
    }
    archforge_ui_egui::i18n::set_language("en");
}

/// The Diagrams panel's notice is read out as a status under its tree until a diagram is opened
/// from it. In light and dark (AR-425).
#[test]
fn the_diagrams_panel_s_notice_is_read_out_as_a_status() {
    use egui_kittest::kittest::Queryable;
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, _) = related_sample_in(theme, egui::vec2(1600.0, 1000.0));
        h.state_mut().state.show_explorer(archforge_shell_api::state::Explorer::Diagrams);
        // Nothing in the panel refuses yet, so the notice is set as a refusal would set it.
        let said = "The diagram could not be opened";
        h.state_mut().diagrams.notice = Some(said.into());
        h.run();
        read_out(&h, said, "workspace.diagrams.notice");
        let row = ids(&h).into_iter().find(|i| i.starts_with("workspace.diagrams.row.")).expect("a diagram is listed");
        let at = rect_of(&h, &row).center();
        click(&mut h, at);
        assert_eq!(h.state().diagrams.notice, None, "{theme:?}: gone once a diagram is opened");
        assert!(h.query_by_label(said).is_none(), "{theme:?}: and no longer read out");
    }
}

/// A package added through UML's offer to Models with nothing selected goes into the root package, each under
/// a name the root has free: Package 1, then Package 2.
#[test]
fn a_package_added_with_nothing_selected_takes_a_name_free_in_the_root_package() {
    use archforge_shell_api::notation_ui::NotationUi;
    let mut h = workspace_harness(ThemeKind::Light, egui::vec2(1600.0, 1000.0));
    let names = |h: &Harness<'static, Shell>| {
        let st = h.state().session.as_ref().unwrap().doc();
        let root = archforge_uml::commands::root_package(st).expect("the root package");
        archforge_uml::commands::subpackages(st, root).into_iter().map(|p| archforge_core::name_of(st, p)).collect::<Vec<_>>()
    };
    for _ in 0..2 {
        let added = archforge_ui_egui::snapshot::cx(h.state_mut(), |cx| uml_ui::UmlUi.add(cx, "package", None)).expect("a package offer");
        assert!(added.as_ref().err().is_none(), "{:?}", added.err());
        h.run();
    }
    assert_eq!(names(&h), ["Package 1", "Package 2"], "the second package is named apart from the first, both in the root");
}

/// The Models delete dialog for a nested package that hands up what it holds (AR-789): a stereotype
/// applied to it makes Models ask, and once confirmed the toast says where what it held moved, then
/// what the removal does to the constraint that named it beside Payment, which stays. In light and
/// dark.
#[test]
fn a_nested_package_deleted_through_the_dialog_says_where_its_contents_went_and_then_the_rule() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, _) = related_sample_in(theme, egui::vec2(1600.0, 1000.0));
        let payment = named(&h, archforge_uml::CLASS, "Payment").to_string();
        let (pkg, outer) = {
            let st = h.state().session.as_ref().unwrap().doc();
            let p = archforge_uml::commands::package_of(st, st.by_uuid(archforge_mof::Uuid::parse(&payment).unwrap()).unwrap()).unwrap();
            (st.uuid(p).unwrap().to_string(), archforge_core::name_of(st, p))
        };
        let s = h.state_mut().session.as_mut().unwrap();
        let id = |v: serde_json::Value| v["id"].as_str().unwrap().to_string();
        let profile = id(s.execute("uml.profile.add", &serde_json::json!({ "name": "Layers" })).unwrap().value);
        let module = serde_json::json!({ "profile": profile, "name": "Module", "extends": ["Package"] });
        let module = id(s.execute("uml.stereotype.add", &module).unwrap().value);
        s.execute("uml.profile.apply", &serde_json::json!({ "package": pkg, "profile": profile })).unwrap();
        let inner = id(s.execute("uml.package.add", &serde_json::json!({ "parent": pkg, "name": "Inner" })).unwrap().value);
        s.execute("uml.class.add", &serde_json::json!({ "package": inner, "name": "Held" })).unwrap();
        s.execute("uml.stereotype.apply", &serde_json::json!({ "element": inner, "stereotype": module })).unwrap();
        let rule = serde_json::json!({ "context": payment, "name": "kept", "expression": "x", "constrained": [inner, payment] });
        s.execute("uml.constraint.add", &rule).unwrap();
        s.execute("view.select", &serde_json::json!({ "ids": [inner] })).unwrap();
        h.run();
        archforge_ui_egui::explorer::remove_selection(h.state_mut());
        h.run();
        let asked = h.state().explorer.delete.clone().unwrap_or_else(|| panic!("{theme:?}: its stereotype application makes Models ask"));
        assert_eq!(asked.hands_to, Some((outer.clone(), true)), "{theme:?}: what it holds moves to its parent");
        let at = rect_of(&h, "workspace.explorer.delete.confirm").center();
        click(&mut h, at);
        let toast = h.state().canvas.toast.as_ref().map(|t| t.text.clone());
        let want = format!("Inner removed; what it held moved to {outer} · kept now constrains Payment only");
        assert_eq!(toast, Some(want), "{theme:?}");
    }
}

/// The delete dialog of a nested package counts what goes with the package alone, as
/// `element.remove` takes it (AR-843): Billing hands Invoice up with its two associations, which
/// stay, so ⇧⌫ on its card asks with no relationship line; with a dependency on Billing from
/// Payment it says that one goes, and once confirmed the dependency has gone and the associations
/// stay. Removed from Models with a stereotype applied to it, it counts the dependency alone too.
/// In light and dark.
#[test]
fn a_nested_package_s_delete_dialog_counts_only_what_goes_with_the_package() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, _) = related_sample_in(theme, egui::vec2(1600.0, 1000.0));
        let payment = named(&h, archforge_uml::CLASS, "Payment").to_string();
        let (pkg, d) = {
            let st = h.state().session.as_ref().unwrap().doc();
            let p = archforge_uml::commands::package_of(st, st.by_uuid(archforge_mof::Uuid::parse(&payment).unwrap()).unwrap()).unwrap();
            (st.uuid(p).unwrap().to_string(), h.state().canvas.active.unwrap().to_string())
        };
        let s = h.state_mut().session.as_mut().unwrap();
        let mut run = |cmd: &str, p: serde_json::Value| s.execute(cmd, &p).unwrap().value["id"].as_str().unwrap_or_default().to_string();
        let billing = run("uml.package.add", serde_json::json!({ "parent": pkg, "name": "Billing" }));
        let invoice = run("uml.class.add", serde_json::json!({ "package": billing, "name": "Invoice" }));
        let audit = run("uml.class.add", serde_json::json!({ "package": pkg, "name": "Audit" }));
        let associations = [&payment, &audit].map(|to| run("uml.association.add", serde_json::json!({ "from": invoice, "to": to })));
        let node = run("diagram.place", serde_json::json!({ "diagram": d, "element": billing, "x": 900, "y": 640 }));
        let ask = |h: &mut Harness<'static, Shell>| {
            archforge_ui_egui::snapshot::run(h.state_mut(), "view.select", serde_json::json!({ "ids": [node] })).unwrap();
            h.state_mut().canvas.focused = true;
            h.run();
            key(h, egui::Key::Delete, egui::Modifiers::SHIFT);
            h.run();
            h.state().explorer.delete.clone().unwrap_or_else(|| panic!("{theme:?}: the dialog asks"))
        };
        let said = |h: &Harness<'static, Shell>| painted(h).into_iter().filter(|t| t.contains("relationship")).collect::<Vec<_>>();
        assert_eq!(ask(&mut h).consequences, Vec::<String>::new(), "{theme:?}: Invoice's associations stay, handed up with it");
        assert_eq!(said(&h), Vec::<String>::new(), "{theme:?}: no relationship line");
        h.state_mut().explorer.delete = None;
        let s = h.state_mut().session.as_mut().unwrap();
        let p = serde_json::json!({ "client": payment, "supplier": billing });
        let dependency = s.execute("uml.dependency.add", &p).unwrap().value["id"].as_str().unwrap().to_string();
        assert_eq!(ask(&mut h).consequences, ["1 relationship goes with it."], "{theme:?}: the dependency on Billing goes with it");
        assert!(said(&h).iter().any(|t| t.contains("1 relationship goes with it.")), "{theme:?}: {:?}", said(&h));
        let at = rect_of(&h, "workspace.explorer.delete.confirm").center();
        click(&mut h, at);
        let alive = |h: &Harness<'static, Shell>, id: &str| {
            let st = h.state().session.as_ref().unwrap().doc();
            st.by_uuid(archforge_mof::Uuid::parse(id).unwrap()).is_some_and(|o| st.is_alive(o))
        };
        assert!(!alive(&h, &billing) && !alive(&h, &dependency), "{theme:?}: Billing went with its dependency");
        assert!(associations.iter().all(|a| alive(&h, a)), "{theme:?}: the associations stayed, as the dialog said");
        h.state_mut().session.as_mut().unwrap().undo().unwrap();
        h.run();
        // Removed from Models, which asks as a stereotype is applied to it.
        let s = h.state_mut().session.as_mut().unwrap();
        let mut run = |cmd: &str, p: serde_json::Value| s.execute(cmd, &p).unwrap().value["id"].as_str().unwrap_or_default().to_string();
        let profile = run("uml.profile.add", serde_json::json!({ "name": "Layers" }));
        let module = run("uml.stereotype.add", serde_json::json!({ "profile": profile, "name": "Module", "extends": ["Package"] }));
        run("uml.profile.apply", serde_json::json!({ "package": pkg, "profile": profile }));
        run("uml.stereotype.apply", serde_json::json!({ "element": billing, "stereotype": module }));
        run("view.select", serde_json::json!({ "ids": [billing] }));
        h.run();
        archforge_ui_egui::explorer::remove_selection(h.state_mut());
        h.run();
        let asked = h.state().explorer.delete.clone().unwrap_or_else(|| panic!("{theme:?}: its stereotype application makes Models ask"));
        assert_eq!(
            asked.consequences,
            ["1 relationship goes with it.", "1 stereotype application goes with it: «Module» on Billing."],
            "{theme:?}: the dependency alone"
        );
        assert!(said(&h).iter().any(|t| t.contains("1 relationship goes with it.")), "{theme:?}: {:?}", said(&h));
    }
}

/// A nested package removed from Models goes at once, what it holds moved up, only while nothing
/// goes with it: with a dependency on it from Payment, Models asks first and says the dependency
/// goes, as Delete from the model does, and nothing is removed unasked; an empty nested package
/// beside it goes without a dialog, the toast naming it. In light and dark.
#[test]
fn a_nested_package_with_a_dependency_on_it_asks_from_models() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, _) = related_sample_in(theme, egui::vec2(1600.0, 1000.0));
        let payment = named(&h, archforge_uml::CLASS, "Payment").to_string();
        let (pkg, outer) = {
            let st = h.state().session.as_ref().unwrap().doc();
            let p = archforge_uml::commands::package_of(st, st.by_uuid(archforge_mof::Uuid::parse(&payment).unwrap()).unwrap()).unwrap();
            (st.uuid(p).unwrap().to_string(), archforge_core::name_of(st, p))
        };
        let s = h.state_mut().session.as_mut().unwrap();
        let mut run = |cmd: &str, p: serde_json::Value| s.execute(cmd, &p).unwrap().value["id"].as_str().unwrap_or_default().to_string();
        let billing = run("uml.package.add", serde_json::json!({ "parent": pkg, "name": "Billing" }));
        run("uml.class.add", serde_json::json!({ "package": billing, "name": "Invoice" }));
        let calm = run("uml.package.add", serde_json::json!({ "parent": pkg, "name": "Calm" }));
        run("uml.dependency.add", serde_json::json!({ "client": payment, "supplier": billing }));
        let remove = |h: &mut Harness<'static, Shell>, id: &str| {
            h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [id] })).unwrap();
            h.run();
            archforge_ui_egui::explorer::remove_selection(h.state_mut());
            h.run();
        };
        let alive = |h: &Harness<'static, Shell>, id: &str| {
            let st = h.state().session.as_ref().unwrap().doc();
            st.by_uuid(archforge_mof::Uuid::parse(id).unwrap()).is_some_and(|o| st.is_alive(o))
        };
        remove(&mut h, &billing);
        let asked = h.state().explorer.delete.clone().unwrap_or_else(|| panic!("{theme:?}: the dependency on Billing makes Models ask"));
        assert_eq!(asked.consequences, ["1 relationship goes with it."], "{theme:?}");
        assert!(alive(&h, &billing), "{theme:?}: nothing removed unasked");
        assert!(painted_has(&h, "1 relationship goes with it."), "{theme:?}: {:?}", painted(&h));
        assert!(painted_has(&h, &format!("What it holds moves to {outer}.")), "{theme:?}: {:?}", painted(&h));
        let at = rect_of(&h, "workspace.explorer.delete.cancel").center();
        click(&mut h, at);
        assert!(h.state().explorer.delete.is_none() && alive(&h, &billing), "{theme:?}: cancelled, Billing stays");
        remove(&mut h, &calm);
        assert!(h.state().explorer.delete.is_none(), "{theme:?}: nothing goes with Calm, so nothing is asked");
        assert!(!alive(&h, &calm), "{theme:?}: Calm went at once");
        let toast = h.state().canvas.toast.as_ref().map(|t| t.text.clone());
        assert_eq!(toast.as_deref(), Some("Calm removed"), "{theme:?}");
    }
}

/// A second click and F2 on a class's, an interface's and a package's row in Models each open its
/// name for typing: Models opens a rename wherever the notation renames the row.
#[test]
fn a_second_click_and_f2_open_the_name_of_a_class_an_interface_and_a_package_in_models() {
    let (mut h, _) = related_sample();
    let commerce = named(&h, archforge_uml::PACKAGE, "Commerce");
    let payable = {
        let s = h.state_mut().session.as_mut().unwrap();
        let out = s.execute("uml.interface.add", &serde_json::json!({ "package": commerce.to_string(), "name": "Payable" })).unwrap();
        archforge_mof::Uuid::parse(out.value["id"].as_str().unwrap()).unwrap()
    };
    let order = named(&h, archforge_uml::CLASS, "Order");
    for (id, name) in [(order, "Order"), (payable, "Payable"), (commerce, "Commerce")] {
        // Selected, a click on its row opens its name.
        h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [id.to_string()] })).unwrap();
        h.state_mut().explorer.focused = true;
        h.run();
        let row = rect_of(&h, &format!("workspace.explorer.row.{id}")).center();
        click(&mut h, row);
        let rename = h.state().explorer.rename.clone().unwrap_or_else(|| panic!("a second click opens {name}'s name"));
        assert_eq!((rename.target, rename.text.as_str()), (id, name));
        key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
        assert!(h.state().explorer.rename.is_none(), "Escape closes it");
        // F2 opens it too.
        h.state_mut().explorer.focused = true;
        key(&mut h, egui::Key::F2, egui::Modifiers::NONE);
        let rename = h.state().explorer.rename.clone().unwrap_or_else(|| panic!("F2 opens {name}'s name"));
        assert_eq!((rename.target, rename.text.as_str()), (id, name));
        key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
    }
}

/// The package Types is not removed from Models while it keeps the built-in types, which Models
/// does not show: Delete says why under the tree, in the user's words, and nothing changes. Its
/// rows agree with what it is said to hold: emptied of what the user put there, it holds nothing
/// to show. In light and dark.
#[test]
fn models_says_why_the_package_types_stays_and_counts_only_what_it_shows() {
    use archforge_shell_api::notation_ui::NotationUi;
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, _) = related_sample_in(theme, egui::vec2(1600.0, 1000.0));
        let types = select_named(&mut h, archforge_uml::PACKAGE, "Types");
        let revision = h.state().session.as_ref().unwrap().revision();
        archforge_ui_egui::explorer::remove_selection(h.state_mut());
        h.run();
        let said = "Types keeps the built-in types, so it stays while it holds them";
        assert_eq!(h.state().explorer.notice.as_deref(), Some(said));
        assert!(painted(&h).iter().any(|t| t == said), "said under the tree");
        assert!(h.state().explorer.delete.is_none(), "no dialog is asked");
        assert_eq!(h.state().session.as_ref().unwrap().revision(), revision, "nothing changed");
        // It holds Date and Money to show; without them, nothing, though the built-in types stay.
        let st = h.state().session.as_ref().unwrap().doc();
        assert!(uml_ui::UmlUi.has_children(st, st.by_uuid(types).unwrap()));
        let own: Vec<String> = ["Date", "Money"].iter().map(|n| named(&h, archforge_uml::PRIMITIVE_TYPE, n).to_string()).collect();
        h.state_mut().session.as_mut().unwrap().execute("element.remove", &serde_json::json!({ "ids": own })).unwrap();
        h.run();
        let st = h.state().session.as_ref().unwrap().doc();
        let t = st.by_uuid(types).unwrap();
        assert!(!uml_ui::UmlUi.has_children(st, t), "the built-in types are not shown, so not counted");
        assert!(!archforge_uml::commands::primitive_types(st, t).is_empty(), "though they are there");
    }
}

/// With only an association class's line selected, its card on no diagram, Shift+Delete still asks
/// to delete it from the model, and the dialog's confirm deletes it with its line. In light and
/// dark.
#[test]
fn shift_delete_on_an_association_class_line_without_its_card_asks_and_deletes_it() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let mut r = reviews(theme);
        let (ac, _, card) = r.review();
        let d = r.diagram.clone();
        archforge_ui_egui::snapshot::run(r.h.state_mut(), "diagram.unplace", serde_json::json!({ "nodes": [card] })).unwrap();
        archforge_ui_egui::snapshot::run(r.h.state_mut(), "diagram.connect-all", serde_json::json!({ "diagram": d })).unwrap();
        r.h.run();
        let before = r.shown();
        let at = r.line_middle() + egui::vec2(-60.0, 0.0);
        click(&mut r.h, at);
        assert_eq!(r.selection(), [ac.as_str()]);
        key(&mut r.h, egui::Key::Delete, egui::Modifiers::SHIFT);
        let asked = r.h.state().explorer.delete.clone().expect("the dialog asks");
        assert_eq!((asked.ids, asked.name.as_str(), asked.node), (vec![archforge_mof::Uuid::parse(&ac).unwrap()], "Review", None));
        assert_eq!(r.association_classes(), ["Review"], "nothing goes before the confirm");
        let at = rect_of(&r.h, "workspace.explorer.delete.confirm").center();
        click(&mut r.h, at);
        assert!(r.association_classes().is_empty(), "deleted from the model");
        assert_eq!(r.shown(), (before.0, before.1 - 1), "with its line");
    }
}

/// An association class shown only by its line, with no card, on two diagrams: deleted from
/// Models, the dialog says it disappears from both, each once (AR-187). In light and dark.
#[test]
fn deleting_an_association_class_shown_by_its_line_alone_on_two_diagrams_from_models_says_two() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let mut r = reviews(theme);
        let (ac, _, card) = r.review();
        let (package, customer, product) = (r.package.clone(), r.ids["Customer"].clone(), r.ids["Product"].clone());
        let s = r.h.state_mut().session.as_mut().unwrap();
        let mut run = |cmd: &str, p: serde_json::Value| s.execute(cmd, &p).unwrap().value["id"].as_str().unwrap_or_default().to_string();
        // Its card off the first diagram, which takes its line too; the line drawn again alone.
        run("diagram.unplace", serde_json::json!({ "nodes": [card] }));
        run("diagram.connect-all", serde_json::json!({ "diagram": r.diagram }));
        let d2 = run("diagram.add", serde_json::json!({ "kind": archforge_uml::STRUCTURE_DIAGRAM, "element": package, "name": "Reviews 2" }));
        for (class, x) in [(&customer, 80), (&product, 760)] {
            run("diagram.place", serde_json::json!({ "diagram": d2, "element": class, "x": x, "y": 520, "width": 240, "height": 136 }));
        }
        run("diagram.connect-all", serde_json::json!({ "diagram": d2 }));
        r.h.run();
        {
            let st = r.h.state().session.as_ref().unwrap().doc();
            let (_, edges, nodes) = archforge_diagram::views_of(st, st.by_uuid(archforge_mof::Uuid::parse(&ac).unwrap()).unwrap());
            assert_eq!((edges.len(), nodes.len()), (2, 0), "a line on each diagram and no card");
        }
        select_named(&mut r.h, archforge_uml::ASSOCIATION_CLASS, "Review");
        archforge_ui_egui::explorer::remove_selection(r.h.state_mut());
        assert_eq!(asked_diagrams(&mut r.h), (2, vec!["It disappears from 2 diagrams.".to_string()]));
        assert_eq!(r.association_classes(), ["Review"], "cancelled, the model keeps it");
    }
}

/// The delete dialog of a class names every relationship that goes with it, as `element.remove`
/// takes them: Product's association class Review, and the dependency on Review, which goes with
/// Review. In light and dark.
#[test]
fn the_delete_dialog_counts_what_goes_with_an_association_class_too() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let mut r = reviews(theme);
        let (ac, _, _) = r.review();
        let store = r.ids["Store"].clone();
        archforge_ui_egui::snapshot::run(r.h.state_mut(), "uml.dependency.add", serde_json::json!({ "client": store, "supplier": ac })).unwrap();
        r.h.run();
        let at = r.at("Product");
        click(&mut r.h, at);
        key(&mut r.h, egui::Key::Delete, egui::Modifiers::SHIFT);
        let asked = r.h.state().explorer.delete.clone().expect("the dialog asks");
        assert_eq!(asked.consequences, ["2 relationships go with it."], "Review and the dependency on it");
        let at = rect_of(&r.h, "workspace.explorer.delete.confirm").center();
        click(&mut r.h, at);
        let st = r.h.state().session.as_ref().unwrap().doc();
        let gone = st.alive().filter(|o| st.class_of(*o).is_ok_and(archforge_uml::is_relationship)).count();
        assert_eq!(gone, 0, "both went, as the dialog said");
    }
}

/// An association class dragged from Models onto a diagram that shows both its classes places its
/// card with its line, as one step; dragged again it is said to be there already. In light and dark.
#[test]
fn an_association_class_dragged_from_models_places_its_card_with_its_line() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let mut r = reviews(theme);
        let (ac, _, card) = r.review();
        // Off this diagram, then back from Models.
        archforge_ui_egui::snapshot::run(r.h.state_mut(), "diagram.unplace", serde_json::json!({ "nodes": [card] })).unwrap();
        r.h.state_mut().state.show_explorer(archforge_shell_api::state::Explorer::Models);
        for id in [&r.package, &ac] {
            r.h.state_mut().explorer.expanded.insert(archforge_mof::Uuid::parse(id).unwrap());
        }
        if let Some(root) =
            r.h.state().session.as_ref().and_then(|s| archforge_uml::commands::root_package(s.doc()).and_then(|o| s.doc().uuid(o).ok()))
        {
            r.h.state_mut().explorer.expanded.insert(root);
        }
        r.h.run();
        let before = r.shown();
        let row = rect_of(&r.h, &format!("workspace.explorer.row.{ac}")).center();
        let to = egui::pos2(r.line_middle().x, r.at("Customer").y - 260.0);
        drag(&mut r.h, row, to);
        assert_eq!(r.shown(), (before.0 + 1, before.1 + 1), "its card and its line; notice {:?}", r.h.state().canvas.notice);
        assert_eq!(r.h.state().session.as_ref().unwrap().undo_label(), Some("Place association class"));
        assert_eq!(r.selection(), [ac.as_str()]);
        r.h.state_mut().session.as_mut().unwrap().undo().unwrap();
        r.h.run();
        assert_eq!(r.shown(), before, "one step");
        r.h.state_mut().session.as_mut().unwrap().redo().unwrap();
        r.h.run();
        let row = rect_of(&r.h, &format!("workspace.explorer.row.{ac}")).center();
        drag(&mut r.h, row, to);
        assert_eq!(r.h.state().canvas.notice.as_deref(), Some("Already on this diagram"));
        assert_eq!(r.shown(), (before.0 + 1, before.1 + 1));
    }
}

/// A project saved before the root package, with its packages at the top level, opens with them in a root
/// package named after the project, in their order, as one step undo takes back.
#[test]
fn a_project_from_before_the_root_package_opens_with_its_packages_in_the_root_as_one_undoable_step() {
    // A model as older projects saved it: two packages at the top level, no root package.
    // Each session its own ids, so none is given twice.
    let ids = |at: u64| -> Box<dyn FnMut() -> archforge_mof::Uuid + Send> {
        let mut n: u64 = 0;
        Box::new(move || {
            n += 1;
            let mut r = [0u8; 10];
            r[2..].copy_from_slice(&n.to_be_bytes());
            archforge_mof::Uuid::v7(at, r)
        })
    };
    let (tables, junctions) = archforge_notations::NOTATIONS.storage();
    let fresh = archforge_notations::NOTATIONS.new_project_session("Commerce platform", ids(1_761_670_800_000)).unwrap();
    let rows = archforge_mof::rows::snapshot(fresh.doc(), &tables, &junctions).unwrap();
    let older = archforge_notations::NOTATIONS
        .session_from_rows_repaired(&rows, ids(1_761_670_810_000), |st| {
            let model = st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_core::MODEL)).unwrap();
            for name in ["Commerce", "Billing"] {
                let p = st.create(archforge_uml::PACKAGE).unwrap();
                st.set(p, archforge_uml::named_element::NAME, archforge_mof::Value::str(name)).unwrap();
                st.push(model, archforge_core::model::MEMBER, archforge_mof::Value::Ref(p)).unwrap();
            }
        })
        .unwrap();
    let rows = archforge_mof::rows::snapshot(older.doc(), &tables, &junctions).unwrap();
    assert_eq!(archforge_uml::commands::packages(older.doc()).len(), 2, "the older shape");
    // Opened: the two go into the root, named after the project.
    let mut h = workspace_harness(ThemeKind::Light, egui::vec2(1600.0, 1000.0));
    h.state_mut().open_project(archforge_ui_egui::Opened {
        name: "Commerce platform".into(),
        path: "/Users/robin/Documents/Archforge/Older.archforge".into(),
        model: Some(std::sync::Arc::new(rows)),
        ..Default::default()
    });
    h.run();
    let st = h.state().session.as_ref().unwrap().doc();
    let root = archforge_uml::commands::root_package(st).expect("the root is made on opening");
    assert_eq!(archforge_core::name_of(st, root), "Commerce platform");
    assert_eq!(archforge_uml::commands::packages(st), vec![root]);
    let held: Vec<String> = archforge_uml::commands::subpackages(st, root).into_iter().map(|p| archforge_core::name_of(st, p)).collect();
    assert_eq!(held, ["Commerce", "Billing"], "nothing lost, in their order");
    // One step: undo puts the older shape back.
    h.state_mut().session.as_mut().unwrap().undo().unwrap();
    let st = h.state().session.as_ref().unwrap().doc();
    assert!(archforge_uml::commands::root_package(st).is_none());
    assert_eq!(archforge_uml::commands::packages(st).len(), 2);
}

/// Nested classifiers (AR-132, design page 08, D1 to D4), in light and dark: a right click on
/// Order's row in Models opens its menu, New inside Order ▸ offers the kinds and Package disabled,
/// a pick makes one nested classifier in one undo step, named in place, `nested` after its row;
/// ⇧F10 opens the menu from the keys and Escape closes it; an enumeration's New inside is disabled
/// with the reason. Its Properties name its owner and the Owner menu lists the owners, the current
/// one ticked, itself and a class with a member of its name disabled with the reason; a pick moves
/// it in one step, kept by id. On a diagram without Order its card reads `Order::Enumeration 1`.
/// Deleting Order asks first, naming the nested classifier, and takes it along in one step.
#[test]
fn nested_classifiers_are_made_from_models_owned_in_properties_and_named_on_the_canvas() {
    use egui_kittest::kittest::Queryable;
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, _) = related_sample_in(theme, egui::vec2(1600.0, 1000.0));
        let order = select_named(&mut h, archforge_uml::CLASS, "Order");
        // D1: the menu on Order's row.
        let row = rect_of(&h, &format!("workspace.explorer.row.{order}"));
        right_click(&mut h, row.center());
        assert!(h.state().explorer.context.is_some(), "{theme:?}: the menu opened");
        for id in [
            "workspace.explorer.contextMenu.newInside",
            "workspace.explorer.contextMenu.rename",
            "workspace.explorer.contextMenu.show",
            "workspace.explorer.contextMenu.delete",
        ] {
            assert!(ids(&h).iter().any(|i| i == id), "{theme:?}: {id}");
        }
        assert!(painted(&h).iter().any(|t| t == "New inside Order"));
        let at = rect_of(&h, "workspace.explorer.contextMenu.newInside").center();
        click(&mut h, at);
        assert!(painted(&h).iter().any(|t| t == "NEW INSIDE ORDER"), "{:?}", painted(&h));
        for kind in ["class", "interface", "enumeration", "data-type", "package"] {
            assert!(ids(&h).iter().any(|i| *i == format!("workspace.explorer.contextMenu.newInside.{kind}")), "{kind}");
        }
        assert!(painted(&h).iter().any(|t| t == "not inside a class"), "Package disabled with its reason");
        let at = rect_of(&h, "workspace.explorer.contextMenu.newInside.enumeration").center();
        click(&mut h, at);
        assert_eq!(h.state().session.as_ref().unwrap().undo_label(), Some("Add nested classifier"));
        let made = {
            let st = h.state().session.as_ref().unwrap().doc();
            let o = st.by_uuid(order).unwrap();
            let nested = archforge_uml::nesting::nested(st, o);
            assert_eq!(nested.len(), 1, "{theme:?}: one nested classifier");
            assert_eq!(st.class_of(nested[0]).ok(), Some(archforge_uml::ENUMERATION));
            st.uuid(nested[0]).unwrap()
        };
        let rename = h.state().explorer.rename.clone().expect("named in place");
        assert_eq!((rename.target, rename.text.as_str()), (made, "Enumeration 1"));
        h.state_mut().explorer.rename = None;
        h.run();
        assert!(painted(&h).iter().any(|t| t == "nested"), "{theme:?}: `nested` after its row");
        // Order's Properties list it under Nested classifiers, whose + offers the same kinds.
        select_named(&mut h, archforge_uml::CLASS, "Order");
        h.state_mut().state.show_panel(archforge_shell_api::state::Panel::Properties);
        h.run();
        assert!(painted(&h).iter().any(|t| t == "Nested classifiers · 1"), "{theme:?}: {:?}", painted(&h));
        assert!(ids(&h).contains(&format!("{}.{made}", uml_nesting::NESTED_ROW_ID)));
        let at = rect_of(&h, uml_nesting::NESTED_ADD_ID).center();
        click(&mut h, at);
        let at = rect_of(&h, &format!("{}.1", uml_nesting::NESTED_ADD_ID)).center();
        click(&mut h, at);
        assert_eq!(h.state().session.as_ref().unwrap().undo_label(), Some("Add nested classifier"));
        assert_eq!(h.state().explorer.rename.as_ref().map(|r| r.text.as_str()), Some("Interface 1"), "named in place in Models");
        h.state_mut().explorer.rename = None;
        h.state_mut().session.as_mut().unwrap().undo().unwrap();
        select_named(&mut h, archforge_uml::CLASS, "Order");
        // ⇧F10 opens the menu on the selected row; Escape closes it.
        select_named(&mut h, archforge_uml::CLASS, "Order");
        h.state_mut().explorer.focused = true;
        key(&mut h, egui::Key::F10, egui::Modifiers::SHIFT);
        assert!(h.state().explorer.context.is_some(), "the keys open the menu");
        key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
        assert!(h.state().explorer.context.is_none(), "Escape closes it");
        // An enumeration owns none: New inside is listed, disabled, the reason its description.
        h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [made.to_string()] })).unwrap();
        h.run();
        let row = rect_of(&h, &format!("workspace.explorer.row.{made}"));
        right_click(&mut h, row.center());
        assert!(painted(&h).iter().any(|t| t == "New inside Enumeration 1"));
        let before = (h.state().session.as_ref().unwrap().revision(), h.state().session.as_ref().unwrap().doc().alive().count());
        let at = rect_of(&h, "workspace.explorer.contextMenu.newInside").center();
        click(&mut h, at);
        // Disabled, it makes nothing and opens no name.
        let st = h.state().session.as_ref().unwrap().doc();
        assert_eq!((h.state().session.as_ref().unwrap().revision(), st.alive().count()), before, "{theme:?}: the model unchanged");
        assert!(archforge_uml::nesting::nested(st, st.by_uuid(made).unwrap()).is_empty(), "nothing nested in the enumeration");
        assert!(h.state().explorer.rename.is_none(), "no name opened for typing");
        h.state_mut().explorer.context = None;
        // D2: its Properties name its owner.
        h.state_mut().state.show_panel(archforge_shell_api::state::Panel::Properties);
        h.run();
        assert!(painted(&h).iter().any(|t| t.starts_with("Enumeration · nested in Order")), "{:?}", painted(&h));
        assert!(h.query_by_label("Owner: Order").is_some(), "the Owner select's accessible name");
        // D3: the Owner menu, itself and a clash disabled with the reason; a pick moves it.
        let customer = select_named(&mut h, archforge_uml::CLASS, "Customer");
        h.state_mut()
            .session
            .as_mut()
            .unwrap()
            .execute("uml.nested.add", &serde_json::json!({ "owner": customer.to_string(), "kind": "class", "name": "Enumeration 1" }))
            .unwrap();
        h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [made.to_string()] })).unwrap();
        h.run();
        let field = rect_of(&h, uml_nesting::OWNER_ID);
        click(&mut h, field.center());
        assert_eq!(h.state().panels.menu, Some(uml_state::menus::CLASS_OWNER));
        assert!(painted(&h).iter().any(|t| t == "OWNER"));
        assert!(painted(&h).iter().any(|t| t == "itself"), "{theme:?}: {:?}", painted(&h));
        assert!(painted(&h).iter().any(|t| t == "class Enumeration 1 already holds that name"), "Customer owns one of its name");
        let rows = {
            let st = h.state().session.as_ref().unwrap().doc();
            uml_nesting::owner_rows(st, st.by_uuid(made).unwrap())
        };
        assert!(rows.iter().any(|r| r.current && r.parts == ["Order"]), "Order ticked");
        let to = rows.iter().position(|r| r.parts == ["Product"] && r.why.is_none()).expect("Product may own it");
        let at = rect_of(&h, &format!("{}.{to}", uml_nesting::OWNER_ID)).center();
        click(&mut h, at);
        assert_eq!(h.state().session.as_ref().unwrap().undo_label(), Some("Change owner"));
        let owner_name = |h: &Harness<'static, Shell>| {
            let st = h.state().session.as_ref().unwrap().doc();
            archforge_uml::nesting::owner_of(st, st.by_uuid(made).unwrap()).map(|o| archforge_core::name_of(st, o))
        };
        assert_eq!(owner_name(&h).as_deref(), Some("Product"), "the same id, under Product");
        h.state_mut().session.as_mut().unwrap().undo().unwrap();
        assert_eq!(owner_name(&h).as_deref(), Some("Order"));
        // D4: placed on a diagram without Order, its card names its owner.
        let root = h.state_mut().session.as_mut().unwrap().execute("uml.package.add", &serde_json::json!({ "name": "Elsewhere" })).unwrap();
        let root = root.value["id"].as_str().unwrap().to_string();
        let other = h
            .state_mut()
            .session
            .as_mut()
            .unwrap()
            .execute("diagram.add", &serde_json::json!({ "kind": archforge_uml::STRUCTURE_DIAGRAM, "name": "Alone", "element": root }))
            .unwrap();
        let other = archforge_mof::Uuid::parse(other.value["id"].as_str().unwrap()).unwrap();
        archforge_ui_egui::canvas::open(h.state_mut(), other);
        h.run();
        // Dropped there from Models, as a row dragged onto the canvas is.
        let d = h.state().session.as_ref().unwrap().doc().by_uuid(other).unwrap();
        archforge_ui_egui::snapshot::cx(h.state_mut(), |cx| {
            archforge_shell_api::kinds::DiagramKind::drop(&uml_ui::UmlKind, cx, d, made, (200.0, 200.0))
        });
        assert_eq!(h.state().session.as_ref().unwrap().undo_label(), Some("Place enumeration"), "notice {:?}", h.state().canvas.notice);
        h.run();
        assert!(painted(&h).iter().any(|t| t == "Order::Enumeration 1"), "{theme:?}: the qualified name, {:?}", painted(&h));
        // Deleting Order asks first and names it; the confirmation takes both in one step.
        select_named(&mut h, archforge_uml::CLASS, "Order");
        archforge_ui_egui::explorer::remove_selection(h.state_mut());
        h.run();
        let asked = h.state().explorer.delete.clone().expect("a dialog");
        assert_eq!(asked.consequences, ["4 relationships go with it.", "1 nested classifier goes with it: Order::Enumeration 1."]);
        assert!(painted(&h).iter().any(|t| t == "1 nested classifier goes with it: Order::Enumeration 1."), "{:?}", painted(&h));
        let at = rect_of(&h, "workspace.explorer.delete.confirm").center();
        click(&mut h, at);
        let gone = |h: &Harness<'static, Shell>| {
            let st = h.state().session.as_ref().unwrap().doc();
            st.by_uuid(made).is_none_or(|o| !st.is_alive(o))
        };
        assert!(gone(&h), "the nested classifier went with Order");
        h.state_mut().session.as_mut().unwrap().undo().unwrap();
        assert!(!gone(&h), "one step brings both back");
    }
}

/// A nested classifier dropped in Models (AR-132): onto its owner's row, or onto another classifier
/// of its own package, it stays where it is; onto its package's row it moves out into the package;
/// onto a classifier of another package it moves into that package.
#[test]
fn a_nested_classifier_dropped_within_its_package_stays_and_onto_a_package_row_moves() {
    use archforge_shell_api::notation_ui::NotationUi;
    let (mut h, _) = related_sample_in(ThemeKind::Light, egui::vec2(1600.0, 1000.0));
    let (priority, order) = archforge_ui_egui::snapshot::cx(h.state_mut(), uml_ui::put_nested_priority).unwrap();
    let other = h.state_mut().session.as_mut().unwrap().execute("uml.package.add", &serde_json::json!({ "name": "Billing" })).unwrap();
    let other = other.value["id"].as_str().unwrap().to_string();
    let invoice =
        h.state_mut().session.as_mut().unwrap().execute("uml.class.add", &serde_json::json!({ "package": other, "name": "Invoice" })).unwrap();
    let invoice = invoice.value["id"].as_str().unwrap().to_string();
    let st = h.state().session.as_ref().unwrap().doc();
    let obj = |id: &str| st.by_uuid(archforge_mof::Uuid::parse(id).unwrap()).unwrap();
    let (p, o) = (obj(&priority), obj(&order));
    let customer = st.alive().find(|c| st.class_of(*c).ok() == Some(archforge_uml::CLASS) && archforge_core::name_of(st, *c) == "Customer").unwrap();
    let ui = uml_ui::UmlUi;
    assert_eq!(ui.drop_on(st, p, o), None, "onto its owner's row it stays");
    assert_eq!(ui.drop_on(st, p, customer), None, "onto a classifier of its own package it stays");
    let home = archforge_uml::nesting::home_package(st, p).unwrap();
    let (cmd, params) = ui.drop_on(st, p, home).expect("onto its package's row it moves out");
    assert_eq!((cmd, params["package"].as_str()), ("uml.enumeration.move", Some(st.uuid(home).unwrap().to_string().as_str())));
    let (cmd, params) = ui.drop_on(st, p, obj(&invoice)).expect("onto a classifier of another package it moves there");
    assert_eq!((cmd, params["package"].as_str()), ("uml.enumeration.move", Some(other.as_str())));
    assert_eq!(ui.drop_on(st, customer, o), None, "a package member onto another of its package stays, as before");
}

/// A nested classifier made through the narrow context asks Models, in this order, to open the
/// rows above it, to open its name for typing and to show itself, and Models does all three once
/// the call returns (AR-132, D1).
#[test]
fn a_new_nested_classifier_is_named_in_models_once_the_call_returns() {
    use archforge_shell_api::cx::Request;
    use archforge_shell_api::state::Explorer;
    let (mut h, _) = related_sample_in(ThemeKind::Light, egui::vec2(1600.0, 1000.0));
    let order = select_named(&mut h, archforge_uml::CLASS, "Order");
    let shell = h.state_mut();
    shell.explorer.expanded.clear();
    (shell.explorer.filter, shell.explorer.rename, shell.explorer.focused) = ("zzz".into(), None, false);
    (shell.state.explorer, shell.state.explorer_collapsed) = (Explorer::Diagrams, true);
    let (made, asked) = archforge_ui_egui::snapshot::cx(shell, |cx| {
        let made = uml_nesting::add_nested(cx, order, uml_classifier::Classifier::Enumeration).expect("made");
        (made, cx.requests.clone())
    });
    let [Request::ExpandRows(rows), Request::NameInModels(named, name), Request::ShowExplorer(Explorer::Models)] = &asked[..] else {
        panic!("the rows, the name, then Models: {asked:?}")
    };
    assert!(rows.contains(&order) && *named == made && name == "Enumeration 1", "{asked:?}");
    assert!(rows.iter().all(|row| shell.explorer.expanded.contains(row)), "the rows above it open");
    assert_eq!(shell.explorer.rename, Some(archforge_ui_egui::testing::fresh_rename(made, "Enumeration 1".into())), "its name open for typing");
    assert!(shell.explorer.filter.is_empty() && shell.explorer.focused, "listed, with the keyboard");
    assert_eq!((shell.state.explorer, shell.state.explorer_collapsed), (Explorer::Models, false), "Models shows, unfolded");
}
