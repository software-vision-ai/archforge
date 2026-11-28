//! UML's refusals said in the chosen language through the whole window, in words that take rows
//! of the shell's catalogue too: the engine's typed refusals of UML's commands and rules, what a
//! move or a removal would leave behind, and the applications the delete dialog names.

use super::*;

/// A template parameter rule broken is said in the chosen language in the template editor's words,
/// never in the engine's English: a kind not supported, a parameter too many, a constraint and a
/// default of another kind, a parameter a binding gives no argument, a kind a binding's argument
/// would not fit, and a parameter a binding substitutes or a template's last. In light and dark
/// (AR-442).
#[test]
fn a_template_rule_broken_is_said_in_the_chosen_language() {
    use archforge_ui_egui::i18n::{fmt, tr};
    use serde_json::json;
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let mut h = workspace_harness(theme, egui::vec2(1600.0, 1120.0));
        h.state_mut().state.language = "de".into();
        h.run();
        let mut id =
            |cmd: &str, p: serde_json::Value| archforge_ui_egui::testing::run(h.state_mut(), cmd, p).unwrap()["id"].as_str().unwrap().to_string();
        let sales = id("uml.package.add", json!({ "name": "Sales" }));
        let [repository, order, entity, big] =
            ["Repository", "Order", "Entity", "Big"].map(|n| id("uml.class.add", json!({ "package": sales, "name": n })));
        let status = id("uml.enumeration.add", json!({ "package": sales, "name": "Status" }));
        let [money, price] = ["Money", "Price"].map(|n| id("uml.data-type.add", json!({ "package": sales, "name": n })));
        let t = id("uml.template-parameter.add", json!({ "classifier": repository, "name": "T" }));
        id("uml.template-binding.add", json!({ "bound": order, "template": repository, "arguments": { "T": entity } }));
        let m = id("uml.template-parameter.add", json!({ "classifier": money, "name": "M", "kind": "data-type", "default": status }));
        id("uml.template-binding.add", json!({ "bound": price, "template": money }));
        for i in 0..archforge_uml::templates::MAX_PARAMETERS {
            id("uml.template-parameter.add", json!({ "classifier": big, "name": format!("P{i}") }));
        }
        let on = |mut p: serde_json::Value| {
            p["classifier"] = json!(repository);
            p
        };
        let (add, remove) = ("uml.template-parameter.add", "element.remove");
        let not_yet = fmt(&tr("{kind} template parameters are not supported yet."), &[("kind", &tr("Value"))]);
        let most = fmt(&tr("A template has at most {max} parameters."), &[("max", "32")]);
        let must = fmt(&tr("Arguments for {name} must be classes."), &[("name", "C")]);
        let misfit = &tr("{default} does not fit {name}: pick a default its arguments could be.");
        let gives = &tr("{bound} binds {template} and gives {name} no argument, so {name} needs a default.");
        let unfit = &tr("{bound} binds {name} -> {actual}, which would no longer fit. Change or remove that binding first.");
        let in_use = &tr("{bound} binds {name} -> {actual}. Change or remove that binding first, then remove {name}.");
        let last = &tr("{bound} binds {template}, and {name} is its last parameter. Remove that binding first, then remove {name}.");
        let order_t = [("bound", "Order"), ("name", "T"), ("actual", "Entity")];
        for (cmd, params, said) in [
            (add, on(json!({ "name": "V", "kind": "value" })), not_yet),
            (add, json!({ "classifier": big, "name": "Q" }), most),
            (add, on(json!({ "name": "C", "constraint": money })), must),
            (add, on(json!({ "name": "D", "default": status })), fmt(misfit, &[("default", "Status"), ("name", "D")])),
            (add, on(json!({ "name": "U" })), fmt(gives, &[("bound", "Order"), ("template", "Repository"), ("name", "U")])),
            ("uml.template-parameter.set", json!({ "parameter": t, "kind": "interface" }), fmt(unfit, &order_t)),
            (remove, json!({ "ids": [t] }), fmt(in_use, &order_t)),
            (remove, json!({ "ids": [m] }), fmt(last, &[("bound", "Price"), ("template", "Money"), ("name", "M")])),
        ] {
            let refused = archforge_ui_egui::testing::run(h.state_mut(), cmd, params).err().unwrap_or_default();
            assert_eq!(refused, said, "{theme:?}: in German");
        }
        assert_ne!(tr("A template has at most {max} parameters."), "A template has at most {max} parameters.", "{theme:?}: the language is German");
    }
    archforge_ui_egui::i18n::set_language("en");
}

/// A profile unapplied, a stereotype removed, moved or given other metaclasses, and a package that
/// applies or holds a profile removed, while elements use the stereotypes, are said in the chosen
/// language with the uses named or counted, never in the engine's English. In light and dark
/// (AR-437).
#[test]
fn a_use_that_refuses_is_said_in_the_chosen_language() {
    use archforge_engine::{InUse, Reason, Used};
    use archforge_ui_egui::i18n::{fmt, tr, trn};
    use serde_json::json;
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let mut h = workspace_harness(theme, egui::vec2(1600.0, 1120.0));
        h.state_mut().state.language = "de".into();
        h.run();
        let id = |h: &mut egui_kittest::Harness<'_, archforge_ui_egui::Shell>, cmd: &str, p: serde_json::Value| {
            archforge_ui_egui::testing::run(h.state_mut(), cmd, p).unwrap()["id"].as_str().unwrap().to_string()
        };
        let refused = |h: &mut egui_kittest::Harness<'_, archforge_ui_egui::Shell>, cmd: &str, p: serde_json::Value| {
            archforge_ui_egui::testing::run(h.state_mut(), cmd, p).err().unwrap_or_default()
        };
        let commerce = id(&mut h, "uml.package.add", json!({ "name": "Commerce" }));
        let [order, customer, invoice] =
            ["Order", "Customer", "Invoice"].map(|n| id(&mut h, "uml.class.add", json!({ "package": commerce, "name": n })));
        let [persistence, elsewhere] = ["Persistence", "Elsewhere"].map(|n| id(&mut h, "uml.profile.add", json!({ "name": n })));
        let entity = id(&mut h, "uml.stereotype.add", json!({ "profile": persistence, "name": "Entity", "extends": ["Class"] }));
        id(&mut h, "uml.profile.apply", json!({ "package": commerce, "profile": persistence }));
        for class in [&order, &customer] {
            id(&mut h, "uml.stereotype.apply", json!({ "element": class, "stereotype": entity }));
        }
        let on = |elements: &str| fmt(&tr("«{stereotype}» on {elements}"), &[("stereotype", "Entity"), ("elements", elements)]);
        let two = on(&fmt(&tr("{first} and {second}"), &[("first", "Order"), ("second", "Customer")]));
        let in_use = tr("{name} is in use: {uses}. Remove those first.");
        assert_ne!(in_use, "{name} is in use: {uses}. Remove those first.", "{theme:?}: the language is German");
        let named = fmt(&in_use, &[("name", "Entity"), ("uses", &two)]);
        let unapply = json!({ "package": commerce, "profile": persistence });
        let extension = tr("«{stereotype}» on {metaclass} is in use: {uses}. Remove those first.");
        let on_class = fmt(&extension, &[("stereotype", "Entity"), ("metaclass", "Class"), ("uses", &two)]);
        let in_package = |uses: &str| {
            fmt(
                &tr("{profile} is in use in {package}: {uses}. Remove those first."),
                &[("profile", "Persistence"), ("package", "Commerce"), ("uses", uses)],
            )
        };
        for (cmd, p, said) in [
            ("uml.profile.unapply", unapply.clone(), in_package(&two)),
            ("element.remove", json!({ "ids": [entity] }), named.clone()),
            ("uml.stereotype.set", json!({ "stereotype": entity, "profile": elsewhere }), named.clone()),
            ("uml.stereotype.set", json!({ "stereotype": entity, "extends": ["Interface"] }), on_class.clone()),
            (
                "element.remove",
                json!({ "ids": [commerce] }),
                fmt(&tr("{name} applies the profiles its elements use: {uses}. Remove those first."), &[("name", "Commerce"), ("uses", &two)]),
            ),
        ] {
            assert_eq!(refused(&mut h, cmd, p), said, "{theme:?}: in German");
        }
        // What Models says before it asks, and what Properties says under Extends, are those words.
        let st = h.state().session.as_ref().unwrap().doc();
        let before_asking = uml_profiles::removal_refusal(
            archforge_ui_egui::testing::notation_uis(h.state()),
            st,
            st.by_uuid(archforge_mof::Uuid::parse(&entity).unwrap()).unwrap(),
        );
        assert_eq!(before_asking, Some(named.clone()), "{theme:?}");
        archforge_ui_egui::testing::run(h.state_mut(), "view.select", json!({ "ids": [entity] })).unwrap();
        h.state_mut().state.show_panel(archforge_shell_api::state::Panel::Properties);
        h.run();
        for control in [format!("{}.Class.edit", uml_profiles::EXTENDS_ROW_ID), format!("{}.remove", uml_profiles::EXTENSION_ID)] {
            let at = rect_of(&h, &control).center();
            click(&mut h, at);
        }
        assert_eq!(uml_of(h.state()).profile_refused.as_ref().map(|r| r.why.clone()), Some(on_class.clone()), "{theme:?}");
        // A third element, and the uses are counted.
        id(&mut h, "uml.stereotype.apply", json!({ "element": invoice, "stereotype": entity }));
        assert_eq!(refused(&mut h, "uml.profile.unapply", unapply), in_package(&on(&trn("{n} element", "{n} elements", 3))), "{theme:?}");
        // A package that would take a used profile along has no parent, which no command makes.
        let held = Used::HeldProfile { package: "Tools".into(), profile: "Audit".into() };
        let said = archforge_shell_api::panels::said(
            SCREENS.notation_uis(),
            &Reason::InUse(Box::new(InUse { what: held, uses: vec![("Entity".into(), vec!["Order".into()], 1)] })),
        );
        let holds = tr("{name} holds the profile {profile}, which is in use: {uses}. Remove those first.");
        assert_eq!(said, Some(fmt(&holds, &[("name", "Tools"), ("profile", "Audit"), ("uses", &on("Order"))])), "{theme:?}");
        // UML's own: a metaclass, named, and the library that keeps it.
        let (class, library) = {
            let st = h.state().session.as_ref().unwrap().doc();
            let id = |o: Option<archforge_mof::Obj>| st.uuid(o.unwrap()).unwrap().to_string();
            (id(archforge_uml::profiles::metaclass(st, "Class")), id(archforge_uml::profiles::library(st)))
        };
        let a_metaclass = fmt(&tr("{name} is a UML metaclass, kept in the UML library. It never changes."), &[("name", "Class")]);
        assert_eq!(refused(&mut h, "uml.class.rename", json!({ "class": class, "name": "Klass" })), a_metaclass, "{theme:?}");
        let the_library = tr("The UML library holds the metaclasses that stereotypes extend. It never changes.");
        assert!(a_metaclass.starts_with("Class ist") && the_library.starts_with("Die UML-Bibliothek"), "{theme:?}: {a_metaclass} {the_library}");
        assert_eq!(refused(&mut h, "element.remove", json!({ "ids": [library] })), the_library, "{theme:?}");
    }
    archforge_ui_egui::i18n::set_language("en");
}

/// An element in use that has no name is said by its kind, with its article, in the user's
/// language, never as an empty name (AR-807): an unnamed generalization beside a named
/// association, an unnamed association and an association end with no role, in English and in
/// German; a kind the catalogues don't name is an element.
#[test]
fn an_unnamed_element_in_use_is_said_by_its_kind() {
    use archforge_engine::{InUse, Named, Reason, Used};
    // The app's shell registers its catalogues and its notations', whose rows this says.
    let _shell = archforge_ui_egui::Shell::new(archforge_ui_egui::Services::default(), &archforge_notations::NOTATIONS, &SCREENS);
    let unnamed = |kind| Named::Unnamed { kind };
    let said = |names: Vec<Named>| {
        let n = names.len();
        let held = Used::HeldProfile { package: "Tools".into(), profile: "Audit".into() };
        archforge_shell_api::panels::said(
            SCREENS.notation_uis(),
            &Reason::InUse(Box::new(InUse { what: held, uses: vec![("Tracked".into(), names, n)] })),
        )
    };
    let cases = [
        (vec!["bills".into(), unnamed("generalization")], "bills and a generalization", "bills und eine Generalisierung"),
        (vec![unnamed("association")], "an association", "eine Assoziation"),
        (vec![unnamed("association end")], "an association end", "ein Assoziationsende"),
        (vec![unnamed("untyped kind")], "an element", "ein Element"),
    ];
    for (names, english, german) in cases {
        archforge_ui_egui::i18n::set_language("en");
        let en = format!("Tools holds the profile Audit, which is in use: «Tracked» on {english}. Remove those first.");
        assert_eq!(said(names.clone()), Some(en));
        archforge_ui_egui::i18n::set_language("de");
        let de = format!("Tools enthält das Profil Audit, das verwendet wird: «Tracked» auf {german}. Diese zuerst entfernen.");
        assert_eq!(said(names), Some(de));
    }
    archforge_ui_egui::i18n::set_language("en");
}

/// A literal a value names is refused naming the element its stereotype is applied to by its
/// name or, having none, by its kind, and an application on no element as on none, to be
/// changed in the file, in English and in French, and in German, with the preposition the
/// kind words take; never by an id or an empty name (AR-808).
#[test]
fn a_literal_valued_names_its_element_by_its_kind_or_none() {
    use archforge_engine::{AppliedTo, LiteralValued, Named, Reason};
    // The app's shell registers its catalogues and its notations', whose rows this says.
    let _shell = archforge_ui_egui::Shell::new(archforge_ui_egui::Services::default(), &archforge_notations::NOTATIONS, &SCREENS);
    let said = |element| {
        let valued = LiteralValued { literal: "lazy".into(), property: "fetch".into(), stereotype: "Entity".into(), element };
        archforge_shell_api::panels::said(SCREENS.notation_uis(), &Reason::LiteralValued(Box::new(valued)))
    };
    let on = |element: &str, french: &str| {
        (
            format!("The literal lazy is the value of fetch in «Entity» on {element}. Set another value there first."),
            format!("Le littéral lazy est la valeur de fetch dans «Entity» sur {french}. Donnez-y d’abord une autre valeur."),
        )
    };
    let nowhere = (
        "The literal lazy is the value of fetch in «Entity» on no element. Change that value in the file first.".to_string(),
        "Le littéral lazy est la valeur de fetch dans «Entity», qui n’est appliqué à aucun élément. Modifiez d’abord cette valeur dans le fichier."
            .to_string(),
    );
    let cases = [
        (AppliedTo::from("Order"), on("Order", "Order")),
        (AppliedTo::Element(Named::Unnamed { kind: "association" }), on("an association", "une association")),
        (AppliedTo::Element(Named::Unnamed { kind: "untyped kind" }), on("an element", "un élément")),
        (AppliedTo::Nothing, nowhere),
    ];
    for (element, (english, french)) in cases {
        archforge_ui_egui::i18n::set_language("en");
        assert_eq!(said(element.clone()), Some(english));
        archforge_ui_egui::i18n::set_language("fr");
        assert_eq!(said(element), Some(french));
    }
    archforge_ui_egui::i18n::set_language("de");
    let german =
        |element: &str| format!("Das Literal lazy ist der Wert von fetch in «Entity» auf {element}. Setzen Sie dort zuerst einen anderen Wert.");
    assert_eq!(said(AppliedTo::from("Order")), Some(german("Order")));
    assert_eq!(said(AppliedTo::Element(Named::Unnamed { kind: "association" })), Some(german("eine Assoziation")));
    let nowhere =
        "Das Literal lazy ist der Wert von fetch in «Entity», das auf kein Element angewendet ist. Ändern Sie diesen Wert zuerst in der Datei.";
    assert_eq!(said(AppliedTo::Nothing), Some(nowhere.to_string()));
    archforge_ui_egui::i18n::set_language("en");
}

/// A constraint's and a description's own rules, refused by the engine, are said in the chosen
/// language: an expression that states no rule, a context that owns none, an element no constraint
/// names or one named twice, too many elements, an element that takes no description and a
/// description too long; and braces with nothing to sit by are said under the editor's form
/// buttons, any other refusal of Apply under it, with nothing written, and Cancel closes the
/// editor. In a window of the design's height, where the German panel's Constraints section is cut
/// by the fold, so the body is scrolled to the section and the editor before they are pressed
/// (AR-467); a refusal brings the editor's buttons back into view (AR-469). In light and dark
/// (AR-447).
#[test]
fn a_constraint_rule_refused_is_said_in_the_chosen_language() {
    use archforge_engine::Reason;
    use archforge_ui_egui::i18n::{fmt, tr};
    use serde_json::json;
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, nodes) = related_sample_in(theme, egui::vec2(1600.0, 1000.0));
        h.state_mut().state.language = "de".into();
        h.run();
        let refused = |h: &mut Harness<'static, Shell>, cmd: &str, p: serde_json::Value| {
            archforge_ui_egui::testing::run(h.state_mut(), cmd, p).err().unwrap_or_default()
        };
        let [order, payment] = ["Order", "Payment"].map(|n| named(&h, archforge_uml::CLASS, n).to_string());
        let (diagram, node) = (h.state().canvas.active.unwrap().to_string(), nodes["Order"].clone());
        let made = |constrained: serde_json::Value| json!({ "context": order, "expression": "x", "constrained": constrained });
        let none = tr("Type an expression. A constraint without one states no rule.");
        assert_eq!(
            none, "Geben Sie einen Ausdruck ein. Eine Einschränkung ohne Ausdruck legt keine Regel fest.",
            "{theme:?}: the language is German"
        );
        assert_eq!(refused(&mut h, "uml.constraint.add", json!({ "context": order, "expression": " " })), none, "{theme:?}");
        let owns = tr("Only a classifier of the model's own or a package owns constraints.");
        assert_eq!(refused(&mut h, "uml.constraint.add", json!({ "context": node, "expression": "x" })), owns, "{theme:?}");
        let outside = tr("A constraint names only elements of this model.");
        assert_eq!(refused(&mut h, "uml.constraint.add", made(json!([diagram]))), outside, "{theme:?}");
        let twice = tr("An element is named twice. Name each element once.");
        assert_eq!(refused(&mut h, "uml.constraint.add", made(json!([order, payment, payment]))), twice, "{theme:?}");
        let most = fmt(&tr("A constraint names at most {max} elements."), &[("max", "256")]);
        assert_eq!(most, "Eine Einschränkung nennt höchstens 256 Elemente.", "{theme:?}");
        assert_eq!(refused(&mut h, "uml.constraint.add", made(json!(vec![&order; 257]))), most, "{theme:?}");
        // A description: on a constraint, which takes none, and one too long for a class.
        let rule =
            archforge_ui_egui::testing::run(h.state_mut(), "uml.constraint.add", json!({ "context": payment, "expression": "x" })).unwrap()["id"]
                .clone();
        let takes_none = tr("This element takes no description.");
        assert_eq!(refused(&mut h, "element.describe", json!({ "element": rule, "text": "About it" })), takes_none, "{theme:?}");
        let long = fmt(&tr("It can't be longer than {max} characters."), &[("max", "4096")]);
        assert_eq!(refused(&mut h, "element.describe", json!({ "element": order, "text": "x".repeat(4097) })), long, "{theme:?}");
        // An element with 64 comments, which only a file holds, is said from its reason.
        let comments = tr("This element owns {max} comments or more, so its description can't be found. Remove some in the file first.");
        assert_eq!(
            archforge_shell_api::panels::said(SCREENS.notation_uis(), &Reason::TooManyComments { max: 64 }),
            Some(fmt(&comments, &[("max", "64")])),
            "{theme:?}"
        );
        // Braces picked in the editor for a class the diagram no longer shows: said under the form.
        archforge_ui_egui::testing::run(h.state_mut(), "diagram.unplace", json!({ "nodes": [node] })).unwrap();
        archforge_ui_egui::testing::run(h.state_mut(), "view.select", json!({ "ids": [order] })).unwrap();
        h.state_mut().state.show_panel(archforge_shell_api::state::Panel::Properties);
        h.run();
        let at = reached_in_properties(&mut h, "workspace.properties.constraints.add");
        click(&mut h, at);
        let edit = uml_of_mut(h.state_mut()).constraint.as_mut().expect("the editor");
        (edit.expression, edit.form, edit.form_chosen) = ("total > 0".into(), archforge_uml::constraints::Form::Braces, true);
        h.run();
        let revision = h.state().session.as_ref().unwrap().revision();
        let at = reached_in_properties(&mut h, "workspace.properties.constraint.apply");
        click(&mut h, at);
        let braces = tr("Braces sit by an element they constrain, and this diagram shows none of them. Show it as a linked note.");
        let edit = uml_of(h.state()).constraint.clone().expect("the editor stays open");
        assert_eq!((edit.refused.as_deref(), edit.refused_at), (Some(braces.as_str()), Some("show")), "{theme:?}");
        assert!(braces.starts_with("Geschweifte Klammern") && painted(&h).contains(&braces), "{theme:?}: said in German in the editor");
        assert_eq!(h.state().session.as_ref().unwrap().revision(), revision, "{theme:?}: nothing written");
        // Any other refusal of Apply is said under it, as a sentence: here the diagram as the element.
        // The refusal brought the editor's foot into view (AR-469): Apply is pressed where it shows.
        let shown = h.state().canvas.active.unwrap();
        uml_of_mut(h.state_mut()).constraint.as_mut().expect("the editor").elements = vec![shown];
        h.run();
        let at = rect_of(&h, "workspace.properties.constraint.apply").center();
        click(&mut h, at);
        let edit = uml_of(h.state()).constraint.clone().expect("the editor stays open");
        assert_eq!((edit.refused.as_deref(), edit.refused_at), (Some(outside.as_str()), None), "{theme:?}");
        assert_eq!(h.state().session.as_ref().unwrap().revision(), revision, "{theme:?}: nothing written");
        // A control above what shows is not listed either.
        let above = "workspace.properties.attributes.add";
        assert!(!ids(&h).iter().any(|i| i == above), "{theme:?}: Attributes is scrolled out above the editor's foot");
        // Cancel, under the refusal and in view with it, closes the editor and writes nothing.
        let at = rect_of(&h, "workspace.properties.constraint.cancel").center();
        click(&mut h, at);
        assert!(uml_of(h.state()).constraint.is_none(), "{theme:?}: Cancel closes the editor");
        assert_eq!(h.state().session.as_ref().unwrap().revision(), revision, "{theme:?}: nothing written");
        // The control above is found by wheeling up.
        let at = reached_in_properties(&mut h, above);
        assert!(h.state().panels.body.is_some_and(|b| b.contains(at)) && rect_of(&h, above).center() == at, "{theme:?}: reached by wheeling up");
    }
    archforge_ui_egui::i18n::set_language("en");
}

/// The constraint commands' parameters, refused by the engine, are said in the chosen language: an
/// id missing, as the id of what the parameter takes, or of no element, an element listed as no id,
/// an element that is no constraint or no class diagram, a place past the last, one coordinate
/// alone or one too far; so are a coordinate that is no number and a constraint among no element's
/// constraints. In a window of the design's height, in light and dark (AR-476).
#[test]
fn a_constraint_parameter_refused_is_said_in_the_chosen_language() {
    use archforge_engine::Reason;
    use archforge_ui_egui::i18n::{fmt, tr};
    use serde_json::json;
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, _) = related_sample_in(theme, egui::vec2(1600.0, 1000.0));
        h.state_mut().state.language = "de".into();
        h.run();
        let refused = |h: &mut Harness<'static, Shell>, cmd: &str, p: serde_json::Value| {
            archforge_ui_egui::testing::run(h.state_mut(), cmd, p).err().unwrap_or_default()
        };
        let order = named(&h, archforge_uml::CLASS, "Order").to_string();
        let diagram = h.state().canvas.active.unwrap().to_string();
        let c = archforge_ui_egui::testing::run(h.state_mut(), "uml.constraint.add", json!({ "context": order, "expression": "x" })).unwrap()["id"]
            .clone();
        let no_constraint = tr("No constraint is given. Give its id.");
        assert_eq!(no_constraint, "Es ist keine Einschränkung angegeben. Geben Sie ihre ID an.", "{theme:?}: the language is German");
        assert_eq!(refused(&mut h, "uml.constraint.set", json!({ "constraint": "", "name": "n" })), no_constraint, "{theme:?}");
        let no_element = tr("No element is given. Give its id.");
        assert_eq!(refused(&mut h, "uml.constraint.add", json!({ "context": "", "expression": "x" })), no_element, "{theme:?}");
        let no_diagram = refused(&mut h, "uml.constraint.show", json!({ "diagram": "", "constraint": c }));
        assert_eq!(no_diagram, tr("No class diagram is given. Give its id."), "{theme:?}");
        let nope = fmt(&tr("There is no element with the id {id}."), &[("id", "nope")]);
        assert_eq!(refused(&mut h, "uml.constraint.set", json!({ "constraint": c, "constrained": ["nope"] })), nope, "{theme:?}");
        let listed = refused(&mut h, "uml.constraint.set", json!({ "constraint": c, "constrained": [7] }));
        assert_eq!(listed, tr("Give the elements as a list of ids."), "{theme:?}");
        let not_one = fmt(&tr("{name} is not a constraint of this model."), &[("name", "Order")]);
        assert_eq!(refused(&mut h, "uml.constraint.set", json!({ "constraint": order, "name": "n" })), not_one, "{theme:?}");
        let not_diagram = fmt(&tr("{name} is not a class diagram."), &[("name", "Order")]);
        assert_eq!(refused(&mut h, "uml.constraint.show", json!({ "diagram": order, "constraint": c })), not_diagram, "{theme:?}");
        // A place past the last, one coordinate alone and one too far.
        let place = tr("There is no place {at} among the constraints. Give a place from 0 to {last}, or -1 for the last.");
        assert_eq!(
            refused(&mut h, "uml.constraint.move", json!({ "constraint": c, "at": 3 })),
            fmt(&place, &[("at", "3"), ("last", "0")]),
            "{theme:?}"
        );
        let alone = refused(&mut h, "uml.constraint.show", json!({ "diagram": diagram, "constraint": c, "x": 10 }));
        assert_eq!(alone, tr("Give both x and y, or neither to place it where it reads."), "{theme:?}");
        let far = refused(&mut h, "uml.constraint.show", json!({ "diagram": diagram, "constraint": c, "x": 0, "y": 2e6 }));
        assert_eq!(far, fmt(&tr("The {axis} is farther than {max} from the origin."), &[("axis", "y"), ("max", "1000000")]), "{theme:?}");
        // No number and a constraint outside any element's constraints, which only a file or a caller gives.
        let nan = archforge_shell_api::panels::said(SCREENS.notation_uis(), &Reason::CoordinateNotANumber { axis: "x" });
        assert_eq!(nan, Some(fmt(&tr("The {axis} is not a number."), &[("axis", "x")])), "{theme:?}");
        let unlisted = archforge_shell_api::panels::said(SCREENS.notation_uis(), &Reason::NoContext);
        assert_eq!(unlisted, Some(tr("This constraint is among no element's constraints.")), "{theme:?}");
    }
    archforge_ui_egui::i18n::set_language("en");
}

/// The lists of properties a property subsets and redefines, refused by the engine, are said in
/// the chosen language: too many for each list, an object with another key, an item that is no id,
/// no such element and a property named twice, with nothing written; so is leaf refused for a
/// built-in type and for a property something redefines. In light and dark (AR-466). An empty id
/// is said as no id given, never as no element with the id (AR-480).
#[test]
fn a_target_list_or_leaf_refused_is_said_in_the_chosen_language() {
    use archforge_ui_egui::i18n::{fmt, tr};
    use serde_json::json;
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let mut h = workspace_harness(theme, egui::vec2(1600.0, 1120.0));
        h.state_mut().state.language = "de".into();
        h.run();
        let id = |h: &mut egui_kittest::Harness<'_, archforge_ui_egui::Shell>, cmd: &str, p: serde_json::Value| {
            archforge_ui_egui::testing::run(h.state_mut(), cmd, p).unwrap()["id"].as_str().unwrap().to_string()
        };
        let package = id(&mut h, "uml.package.add", json!({ "name": "Shop" }));
        let [item, order] = ["Item", "Order"].map(|n| id(&mut h, "uml.class.add", json!({ "package": package, "name": n })));
        let all = id(&mut h, "uml.attribute.add", json!({ "class": item, "name": "all", "type": item, "multiplicity": "*" }));
        let lines = id(&mut h, "uml.attribute.add", json!({ "class": order, "name": "lines", "type": item, "multiplicity": "*" }));
        let revision = h.state().session.as_ref().unwrap().revision();
        let mut refused = |key: &str, list: serde_json::Value| {
            archforge_ui_egui::testing::run(h.state_mut(), "uml.attribute.set", json!({ "attribute": lines, key: list })).err().unwrap_or_default()
        };
        let most = |said: String| fmt(&said, &[("max", "32")]);
        let subsets_most = most(tr("A property subsets at most {max} properties."));
        assert_eq!(subsets_most, "Eine Eigenschaft ist Teilmenge von höchstens 32 Eigenschaften.", "{theme:?}: the language is German");
        assert_eq!(refused("subsets", json!(vec![&all; 33])), subsets_most, "{theme:?}");
        assert_eq!(refused("redefines", json!(vec![&all; 33])), most(tr("A property redefines at most {max} properties.")), "{theme:?}");
        let key = fmt(&tr("{key} is not a key here. An item holds an id and a name."), &[("key", "kind")]);
        assert_eq!(refused("subsets", json!([{ "id": all, "kind": "x" }])), key, "{theme:?}");
        assert_eq!(refused("redefines", json!([7])), tr("Give the properties as a list of ids."), "{theme:?}");
        let none = fmt(&tr("There is no element with the id {id}."), &[("id", "nope")]);
        assert_eq!(refused("subsets", json!(["nope"])), none, "{theme:?}");
        let empty = tr("An item in the list has no id. Give each property's id.");
        assert_ne!(empty, fmt(&tr("There is no element with the id {id}."), &[("id", "")]), "{theme:?}");
        assert_eq!(refused("subsets", json!([""])), empty, "{theme:?}");
        assert_eq!(refused("redefines", json!([{ "id": "" }])), empty, "{theme:?}");
        assert_eq!(refused("redefines", json!([all, all])), tr("An element is named twice. Name each element once."), "{theme:?}");
        assert_eq!(h.state().session.as_ref().unwrap().revision(), revision, "{theme:?}: nothing written");
        // Made leaf: refused for a built-in type, and for a property something redefines.
        id(&mut h, "uml.attribute.add", json!({ "class": order, "name": "count", "type": "Integer" }));
        id(&mut h, "uml.generalization.add", json!({ "specific": order, "general": item }));
        archforge_ui_egui::testing::run(h.state_mut(), "uml.attribute.set", json!({ "attribute": lines, "redefines": [all] })).unwrap();
        let integer = named(&h, archforge_uml::PRIMITIVE_TYPE, "Integer").to_string();
        let leaf = |h: &mut egui_kittest::Harness<'_, archforge_ui_egui::Shell>, cmd: &str, p: serde_json::Value| {
            archforge_ui_egui::testing::run(h.state_mut(), cmd, p).err().unwrap_or_default()
        };
        let built_in = fmt(&tr("{name} is a built-in type, which is never made leaf."), &[("name", "Integer")]);
        assert_eq!(leaf(&mut h, "uml.classifier.set", json!({ "classifier": integer, "leaf": true })), built_in, "{theme:?}");
        let redefined = uml_panel::leaf_refusal(&["Order::lines".to_string()]);
        assert!(redefined.starts_with("Order::lines redefiniert"), "{theme:?}: {redefined}");
        assert_eq!(leaf(&mut h, "uml.attribute.set", json!({ "attribute": all, "leaf": true })), redefined, "{theme:?}");
    }
    archforge_ui_egui::i18n::set_language("en");
}

/// A property's default and the properties it subsets and redefines, refused by the engine, are
/// said in the chosen language: a default each type's default does not read, one too long, one a
/// new type does not fit, typed anew or left out, and a target for the rule it breaks, with nothing
/// written. In light and dark (AR-475).
#[test]
fn a_default_or_target_refused_is_said_in_the_chosen_language() {
    use archforge_engine::{DefaultKind, Reason};
    use archforge_ui_egui::i18n::{fmt, tr};
    use serde_json::json;
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let mut h = workspace_harness(theme, egui::vec2(1600.0, 1120.0));
        h.state_mut().state.language = "de".into();
        h.run();
        let id = |h: &mut egui_kittest::Harness<'_, archforge_ui_egui::Shell>, cmd: &str, p: serde_json::Value| {
            archforge_ui_egui::testing::run(h.state_mut(), cmd, p).unwrap()["id"].as_str().unwrap().to_string()
        };
        let package = id(&mut h, "uml.package.add", json!({ "name": "Shop" }));
        let [item, order] = ["Item", "Order"].map(|n| id(&mut h, "uml.class.add", json!({ "package": package, "name": n })));
        let status = id(&mut h, "uml.enumeration.add", json!({ "package": package, "name": "Status" }));
        id(&mut h, "uml.generalization.add", json!({ "specific": order, "general": item }));
        let attribute = |h: &mut egui_kittest::Harness<'_, archforge_ui_egui::Shell>, class: &str, name: &str, ty: &str, more: serde_json::Value| {
            let mut p = json!({ "class": class, "name": name, "type": ty, "multiplicity": "*" });
            p.as_object_mut().unwrap().extend(more.as_object().cloned().unwrap_or_default());
            id(h, "uml.attribute.add", p)
        };
        let label = attribute(&mut h, &item, "label", "String", json!({}));
        let kept = attribute(&mut h, &item, "kept", &item, json!({ "leaf": true }));
        let one = attribute(&mut h, &item, "one", &item, json!({ "multiplicity": "1" }));
        let all = attribute(&mut h, &order, "all", &item, json!({}));
        let size = attribute(&mut h, &order, "size", "Integer", json!({ "multiplicity": "1", "default": "3" }));
        let revision = h.state().session.as_ref().unwrap().revision();
        let mut refused = |cmd: &str, p: serde_json::Value| archforge_ui_egui::testing::run(h.state_mut(), cmd, p).err().unwrap_or_default();
        let unread = fmt(&tr("{value} is not a whole number, which an Integer's default is, such as 42."), &[("value", "1.5")]);
        assert_eq!(unread, "1.5 ist keine ganze Zahl, wie sie der Standardwert eines Integer ist, etwa 42.", "{theme:?}: the language is German");
        let add = |ty: &str, default: &str| json!({ "class": order, "name": "x", "type": ty, "default": default });
        assert_eq!(refused("uml.attribute.add", add("Integer", "1.5")), unread, "{theme:?}");
        let value = |said: String, v: &str| fmt(&said, &[("value", v)]);
        for (ty, v, said) in [
            ("Real", "inf", tr("{value} is not a number, which a Real's default is, such as 2.5.")),
            ("Boolean", "yes", tr("{value} is neither true nor false, which a Boolean's default is.")),
            ("UnlimitedNatural", "-1", tr("{value} is neither a whole number of 0 or more nor *, which an UnlimitedNatural's default is.")),
        ] {
            assert_eq!(refused("uml.attribute.add", add(ty, v)), value(said, v), "{theme:?}: {ty}");
        }
        let literal = fmt(&tr("{value} is not a literal of {enumeration}."), &[("value", "Shipped"), ("enumeration", "Status")]);
        assert_eq!(refused("uml.attribute.add", add(&status, "Shipped")), literal, "{theme:?}");
        let most = fmt(&tr("A default value is at most {n} characters."), &[("n", "1000")]);
        assert_eq!(refused("uml.attribute.add", add("String", &"x".repeat(1001))), most, "{theme:?}");
        let unfit = |ty: &str| fmt(&tr("The default value {value} does not fit {type}; change or clear it first."), &[("value", "3"), ("type", ty)]);
        assert_eq!(refused("uml.attribute.set", json!({ "attribute": size, "type": "Boolean", "default": "3" })), unfit("Boolean"), "{theme:?}");
        assert_eq!(refused("uml.attribute.set", json!({ "attribute": size, "type": "Boolean" })), unfit("Boolean"), "{theme:?}");
        // A target, for the rule it breaks, in the list it is in.
        let target = |said: String, name: &str| fmt(&said, &[("name", name)]);
        for (key, t, name, said) in [
            ("subsets", &order, "Order", tr("{name} is not a property this one can subset.")),
            ("redefines", &all, "all", tr("{name} is not a property this one can redefine.")),
            ("subsets", &label, "label", tr("{name} has a type this property's type does not conform to.")),
            ("redefines", &one, "one", tr("This property's multiplicity does not lie within {name}'s.")),
            ("redefines", &kept, "kept", tr("{name} is leaf: nothing may redefine it. Clear \"No redefinition\" on it first.")),
        ] {
            assert_eq!(refused("uml.attribute.set", json!({ "attribute": all, key: [t] })), target(said, name), "{theme:?}: {key} {name}");
        }
        assert_eq!(h.state().session.as_ref().unwrap().revision(), revision, "{theme:?}: nothing written");
        // A default no other type reads, and one that does not fit having no type.
        let text = Reason::DefaultUnread { value: "x".into(), kind: DefaultKind::Text };
        assert_eq!(
            archforge_shell_api::panels::said(SCREENS.notation_uis(), &text),
            Some(value(tr("{value} cannot be read as a default."), "x")),
            "{theme:?}"
        );
        let untyped = Reason::DefaultUnfit { value: "3".into(), ty: None };
        assert_eq!(archforge_shell_api::panels::said(SCREENS.notation_uis(), &untyped), Some(unfit(&tr("None"))), "{theme:?}");
    }
    archforge_ui_egui::i18n::set_language("en");
}

/// A move that would take used stereotypes out of the packages that apply their profiles is said in
/// the chosen language, the uses named: a class and a package dropped on another package in Models,
/// and a class given another owner. So are a list of metaclasses that holds no names and a
/// stereotype in no profile. Nothing moves. In light and dark (AR-449).
#[test]
fn a_move_that_strands_stereotypes_is_said_in_the_chosen_language() {
    use archforge_engine::Reason;
    use archforge_ui_egui::i18n::{fmt, tr};
    use serde_json::json;
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let mut h = workspace_harness(theme, egui::vec2(1600.0, 1120.0));
        h.state_mut().state.language = "de".into();
        h.run();
        let id = |h: &mut egui_kittest::Harness<'_, archforge_ui_egui::Shell>, cmd: &str, p: serde_json::Value| {
            archforge_ui_egui::testing::run(h.state_mut(), cmd, p).unwrap()["id"].as_str().unwrap().to_string()
        };
        let [commerce, other] = ["Commerce", "Other"].map(|n| id(&mut h, "uml.package.add", json!({ "name": n })));
        let billing = id(&mut h, "uml.package.add", json!({ "name": "Billing", "parent": commerce }));
        let order = id(&mut h, "uml.class.add", json!({ "package": commerce, "name": "Order" }));
        let invoice = id(&mut h, "uml.class.add", json!({ "package": billing, "name": "Invoice" }));
        let holder = id(&mut h, "uml.class.add", json!({ "package": other, "name": "Holder" }));
        let persistence = id(&mut h, "uml.profile.add", json!({ "name": "Persistence" }));
        let entity = id(&mut h, "uml.stereotype.add", json!({ "profile": persistence, "name": "Entity", "extends": ["Class"] }));
        id(&mut h, "uml.profile.apply", json!({ "package": commerce, "profile": persistence }));
        for class in [&order, &invoice] {
            id(&mut h, "uml.stereotype.apply", json!({ "element": class, "stereotype": entity }));
        }
        let away = |moved: &str, on: &str| {
            let uses = fmt(&tr("«{stereotype}» on {elements}"), &[("stereotype", "Entity"), ("elements", on)]);
            let said =
                tr("Moving {name} there takes stereotypes out of the packages that apply their profiles: {uses}. Apply the profile there first.");
            fmt(&said, &[("name", moved), ("uses", &uses)])
        };
        let german = "Order dorthin zu verschieben nimmt Stereotypen aus den Paketen, die ihre Profile anwenden: «Entity» auf Order. \
                      Das Profil zuerst dort anwenden.";
        assert_eq!(away("Order", "Order"), german, "{theme:?}: the language is German");
        let home = |h: &egui_kittest::Harness<'_, archforge_ui_egui::Shell>, element: &str| {
            let st = h.state().session.as_ref().unwrap().doc();
            let package = archforge_uml::commands::package_of(st, st.by_uuid(archforge_mof::Uuid::parse(element).unwrap()).unwrap());
            package.map(|p| archforge_core::name_of(st, p))
        };
        // Dropped on Other in Models: the class, then the package that holds a used one.
        let root = {
            let st = h.state().session.as_ref().unwrap().doc();
            st.uuid(archforge_uml::commands::root_package(st).unwrap()).unwrap()
        };
        h.state_mut().state.show_explorer(archforge_shell_api::state::Explorer::Models);
        h.state_mut().explorer.expanded.extend([root, archforge_mof::Uuid::parse(&commerce).unwrap()]);
        h.run();
        for (dragged, said) in [(&order, away("Order", "Order")), (&billing, away("Billing", "Invoice"))] {
            h.state_mut().explorer.notice = None;
            let [from, to] = [dragged, &other].map(|id| rect_of(&h, &format!("workspace.explorer.row.{id}")).center());
            drag(&mut h, from, to);
            assert_eq!(h.state().explorer.notice.as_deref(), Some(said.as_str()), "{theme:?}");
            assert_eq!(home(&h, dragged).as_deref(), Some("Commerce"), "{theme:?}: it stays where it was");
        }
        // Given another owner, as the Owner menu does.
        let owner = json!({ "classifier": order, "owner": holder });
        assert_eq!(archforge_ui_egui::testing::run(h.state_mut(), "uml.classifier.set-owner", owner).err(), Some(german.to_string()), "{theme:?}");
        // A list of metaclasses that holds a number, and a stereotype in no profile.
        let extends = json!({ "stereotype": entity, "extends": [7] });
        let names = archforge_ui_egui::testing::run(h.state_mut(), "uml.stereotype.set", extends).err();
        assert_eq!(names.as_deref(), Some("Die Metaklassen als Liste von Namen angeben."), "{theme:?}");
        assert_eq!(names, archforge_shell_api::panels::said(SCREENS.notation_uis(), &Reason::NamesExpected { kind: "metaclass" }), "{theme:?}");
        assert_eq!(
            archforge_shell_api::panels::said(SCREENS.notation_uis(), &Reason::InNoProfile).as_deref(),
            Some("Dieser Stereotyp liegt in keinem Profil."),
            "{theme:?}"
        );
    }
    archforge_ui_egui::i18n::set_language("en");
}

/// The engine's typed refusals are said in the chosen language, never in the engine's English: a
/// name another schema has, a package moved into one it holds, a class depending on itself, a
/// model that can't be changed at the moment, and a package named Types moved in Properties beside
/// the package Types (AR-563). In light and dark (AR-429).
#[test]
fn the_engines_refusals_are_said_in_the_chosen_language() {
    let renamed = |h: &mut Harness<'static, Shell>, name: &str| {
        archforge_ui_egui::explorer::add(h.state_mut(), "schema");
        h.run();
        key(h, egui::Key::Backspace, egui::Modifiers::NONE);
        h.event(egui::Event::Text(name.into()));
        h.run();
        key(h, egui::Key::Enter, egui::Modifiers::NONE);
        h.state().explorer.rename.as_ref().and_then(|r| r.refused.clone())
    };
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let mut h = workspace_harness(theme, egui::vec2(1600.0, 1120.0));
        h.state_mut().state.language = "de".into();
        h.run();
        // A name another schema has, whatever the case: the kind and the other's name in German.
        assert_eq!(renamed(&mut h, "Shop"), None, "{theme:?}: the first takes the name");
        let refused = renamed(&mut h, "shop").unwrap_or_default();
        let schema = archforge_ui_egui::i18n::tr_ctx("refusal", "a schema");
        assert_ne!(schema, "a schema", "{theme:?}: the catalogue has the kind in German");
        assert!(refused.to_lowercase().contains(&schema.to_lowercase()) && refused.contains("Shop"), "{theme:?}: {refused}");
        assert!(!refused.contains("named"), "{theme:?}: not the engine's English: {refused}");
        // The model can't be changed at the moment.
        archforge_ui_egui::explorer::add(h.state_mut(), "schema");
        h.run();
        h.state_mut().session.as_mut().unwrap().set_read_only(true).unwrap();
        h.event(egui::Event::Text("Lager".into()));
        h.run();
        key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
        h.state_mut().session.as_mut().unwrap().set_read_only(false).unwrap();
        let said = archforge_ui_egui::i18n::tr("The model cannot be changed at the moment; nothing was changed.");
        assert_ne!(said, "The model cannot be changed at the moment; nothing was changed.", "{theme:?}: the catalogue has it in German");
        assert_eq!(h.state().explorer.rename.as_ref().and_then(|r| r.refused.clone()), Some(said), "{theme:?}");
        key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
        // A package dropped onto a package it holds, in Models.
        let id = |v: serde_json::Value| archforge_mof::Uuid::parse(v["id"].as_str().unwrap()).unwrap();
        let (root, sales, lines, order) = {
            let s = h.state_mut().session.as_mut().unwrap();
            let sales = id(s.execute("uml.package.add", &serde_json::json!({ "name": "Sales" })).unwrap().value);
            let lines = id(s.execute("uml.package.add", &serde_json::json!({ "name": "Lines", "parent": sales.to_string() })).unwrap().value);
            let order = id(s.execute("uml.class.add", &serde_json::json!({ "package": sales.to_string(), "name": "Order" })).unwrap().value);
            let st = s.doc();
            (st.uuid(archforge_uml::commands::root_package(st).unwrap()).unwrap(), sales, lines, order)
        };
        h.state_mut().state.show_explorer(archforge_shell_api::state::Explorer::Models);
        h.state_mut().explorer.expanded.extend([root, sales]);
        h.run();
        let (from, target) =
            (rect_of(&h, &format!("workspace.explorer.row.{sales}")).center(), rect_of(&h, &format!("workspace.explorer.row.{lines}")).center());
        drag(&mut h, from, target);
        let said = archforge_ui_egui::i18n::tr("A package cannot go into itself or a package it holds.");
        assert_ne!(said, "A package cannot go into itself or a package it holds.", "{theme:?}: the catalogue has it in German");
        assert_eq!(h.state().explorer.notice.as_deref(), Some(said.as_str()), "{theme:?}");
        // A class that would depend on itself: the reason a canvas or a panel shows.
        let refused = archforge_ui_egui::testing::run(
            h.state_mut(),
            "uml.dependency.add",
            serde_json::json!({ "client": order.to_string(), "supplier": order.to_string() }),
        );
        let refused = refused.err().unwrap_or_default();
        let class = archforge_ui_egui::i18n::tr_ctx("refusal", "a class");
        assert_ne!(class, "a class", "{theme:?}: the catalogue has the kind in German");
        assert!(refused.to_lowercase().starts_with(&class.to_lowercase()), "{theme:?}: {refused}");
        assert!(refused.starts_with(char::is_uppercase) && !refused.contains("depend"), "{theme:?}: a sentence, not the engine's English: {refused}");
        // A package named Types moved in Properties to the root, where the package Types is (AR-563).
        let nested = {
            let s = h.state_mut().session.as_mut().unwrap();
            let count = serde_json::json!({ "class": order.to_string(), "name": "count", "type": "Integer" });
            s.execute("uml.attribute.add", &count).unwrap();
            id(s.execute("uml.package.add", &serde_json::json!({ "name": "Types", "parent": sales.to_string() })).unwrap().value)
        };
        h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [nested.to_string()] })).unwrap();
        h.state_mut().state.show_panel(archforge_shell_api::state::Panel::Properties);
        h.run();
        let (to_root, label) = {
            let s = h.state().session.as_ref().unwrap();
            let (st, package) = (s.doc(), s.doc().by_uuid(nested).unwrap());
            let choices = archforge_uml::commands::all_packages(st).into_iter().filter(|o| !archforge_uml::commands::is_within(st, *o, package));
            (choices.collect::<Vec<_>>().iter().position(|o| *o == st.by_uuid(root).unwrap()).unwrap(), s.undo_label().map(str::to_string))
        };
        let at = rect_of(&h, "workspace.properties.package.parent").center();
        click(&mut h, at);
        let at = rect_of(&h, &format!("workspace.properties.package.parent.{to_root}")).center();
        click(&mut h, at);
        let said = "Es gibt bereits ein Paket Types, das die eingebauten Typen enthält; ein zweites kann nicht daneben stehen.";
        assert_eq!(h.state().panels.notice.as_deref(), Some(said), "{theme:?}");
        assert!(ids(&h).iter().any(|i| i == "workspace.properties.notice"), "{theme:?}: Properties says it");
        let s = h.state().session.as_ref().unwrap();
        let (owner, sales) = (archforge_uml::commands::package_of(s.doc(), s.doc().by_uuid(nested).unwrap()), s.doc().by_uuid(sales));
        assert_eq!((owner, s.undo_label().map(str::to_string)), (sales, label), "{theme:?}: not moved, nothing to undo");
    }
    archforge_ui_egui::i18n::set_language("en");
}

/// The delete dialog of an element its stereotypes are applied to names the applications that go
/// with it in the chosen language, in UML's own words, after the relationships that go. In light and
/// dark.
#[test]
fn the_delete_dialog_names_the_applications_in_the_chosen_language() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, _) = related_sample_in(theme, egui::vec2(1600.0, 1100.0));
        archforge_ui_egui::testing::cx(h.state_mut(), |cx| uml_profiles::put_profiles(cx, true)).unwrap();
        h.state_mut().state.language = "de".into();
        h.run();
        select_named(&mut h, archforge_uml::CLASS, "Order");
        archforge_ui_egui::explorer::remove_selection(h.state_mut());
        h.run();
        let asked = h.state().explorer.delete.clone().unwrap_or_else(|| panic!("{theme:?}: the dialog asks"));
        let said = "3 Stereotyp-Anwendungen gehen mit: «Entity» auf Order, «Id» auf id, «Reference» auf places.";
        assert_eq!(asked.consequences, ["4 Beziehungen gehen mit.", said], "{theme:?}");
        for line in asked.consequences.iter().map(String::as_str).chain(["Was es enthält, geht mit."]) {
            assert!(painted_has(&h, line), "{theme:?}: {line} in {:?}", painted(&h));
        }
        h.state_mut().explorer.delete = None;
        h.run();
    }
    archforge_ui_egui::i18n::set_language("en");
}
