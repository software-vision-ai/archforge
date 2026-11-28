//! The window tests that ask both notations: the shell asking ER and UML the same thing, each
//! answering its own way (their kinds' presses, keys, overlays, removals and badges, their words
//! and refusals), and the shell's own behaviour shown on both.

use super::*;

/// A draft left on the element selected before and refused when the selection moves on names
/// that element at the panel's foot, so it is not read as about the one shown, until the selection
/// moves again; a description's draft stays on its element. An association's ends too, which have
/// no name. In light and dark (AR-417).
#[test]
fn a_draft_refused_as_the_selection_moves_on_names_its_element() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, _host, class, table) = described_project(theme);
        let select = |h: &mut Harness<'static, Shell>, id: String| {
            h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [id] })).unwrap();
            h.run();
        };
        select(&mut h, class.to_string());
        h.state_mut().panels.description =
            Some(archforge_shell_api::panels::DescriptionDraft { target: class, text: "a\u{7}b".into(), refused: false });
        select(&mut h, table.to_string());
        let notice = h.state().panels.notice.clone().unwrap_or_default();
        assert!(notice.starts_with("Not kept on Order: ") && notice.contains("control"), "{theme:?}: {notice}");
        assert!(painted(&h).contains(&notice), "{theme:?}: at the panel's foot");
        assert!(h.state().panels.description.as_ref().is_some_and(|d| d.target == class && d.refused), "{theme:?}: the draft stays");
        select(&mut h, class.to_string());
        assert_eq!(h.state().panels.notice, None, "{theme:?}: gone with the next selection");
        let (mut h, made) = modifiers_sample(theme);
        select(&mut h, made["holds"].clone());
        uml_of_mut(h.state_mut()).ends.as_mut().unwrap().role[1] = "a\u{7}b".into();
        select(&mut h, made["Order"].clone());
        let notice = h.state().panels.notice.clone().unwrap_or_default();
        assert!(notice.starts_with("Not kept on the element selected before: "), "{theme:?}: {notice}");
    }
}

/// The diagram commands' refusals are said in the chosen language: placing with no diagram, an
/// element a class diagram does not show, one outside it and one shown already, a text set on a box
/// that shows an element or on an element that is no box, and a note's link routed; so are the
/// other ids and kinds, places, sizes, nesting, bends and a notation's elements it does not remove.
/// In a window of the design's height, in light and dark (AR-439).
#[test]
fn a_diagram_refusal_is_said_in_the_chosen_language() {
    use archforge_engine::Reason;
    use archforge_ui_egui::i18n::{fmt, tr};
    use serde_json::json;
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, nodes) = related_sample_in(theme, egui::vec2(1600.0, 1000.0));
        h.state_mut().state.language = "de".into();
        h.run();
        let run = |h: &mut Harness<'static, Shell>, cmd: &str, p: serde_json::Value| archforge_ui_egui::snapshot::run(h.state_mut(), cmd, p);
        let refused = |h: &mut Harness<'static, Shell>, cmd: &str, p: serde_json::Value| run(h, cmd, p).err().unwrap_or_default();
        let id = |h: &mut Harness<'static, Shell>, cmd: &str, p: serde_json::Value| run(h, cmd, p).unwrap()["id"].as_str().unwrap().to_string();
        let order = named(&h, archforge_uml::CLASS, "Order").to_string();
        let d = h.state().canvas.active.unwrap().to_string();
        let place = |element: &str| json!({ "diagram": d, "element": element, "x": 0, "y": 0 });
        let no_diagram = refused(&mut h, "diagram.place", json!({ "diagram": "", "element": order }));
        assert_eq!(no_diagram, "Es ist kein Diagramm angegeben. Geben Sie seine ID an.", "{theme:?}: the language is German");
        let status = id(&mut h, "uml.attribute.add", json!({ "class": order, "name": "status" }));
        let unshown = fmt(&tr("A diagram of this kind does not show {name}."), &[("name", "status")]);
        assert_eq!(refused(&mut h, "diagram.place", place(&status)), unshown, "{theme:?}");
        let profile = id(&mut h, "uml.profile.add", json!({ "name": "Persistence" }));
        let entity = id(&mut h, "uml.stereotype.add", json!({ "profile": profile, "name": "Entity", "extends": ["Class"] }));
        let outside = fmt(&tr("{name} is outside what this diagram shows."), &[("name", "Entity")]);
        assert_eq!(refused(&mut h, "diagram.place", place(&entity)), outside, "{theme:?}");
        let shown = fmt(&tr("This diagram shows {name} already."), &[("name", "Order")]);
        assert_eq!(refused(&mut h, "diagram.place", place(&order)), shown, "{theme:?}");
        let own = tr("This box shows an element and draws its text. Only a note takes a text of its own.");
        assert_eq!(refused(&mut h, "diagram.set-text", json!({ "node": nodes["Order"], "text": "x" })), own, "{theme:?}");
        let not_box = fmt(&tr("{name} is not a box on a diagram."), &[("name", "Order")]);
        assert_eq!(refused(&mut h, "diagram.set-text", json!({ "node": order, "text": "x" })), not_box, "{theme:?}");
        let note = id(&mut h, "diagram.note", json!({ "diagram": d, "text": "Hinweis", "x": 0, "y": 0 }));
        let link = id(&mut h, "diagram.link-note", json!({ "diagram": d, "note": note, "target": nodes["Order"] }));
        let straight = tr("A note's link is a straight line and does not bend.");
        assert_eq!(refused(&mut h, "diagram.route", json!({ "edge": link, "points": [] })), straight, "{theme:?}");
        // The rest, which a file or a caller gives, said from the reason alone.
        let said = |r: Reason| archforge_shell_api::panels::said(SCREENS.notation_uis(), &r).unwrap_or_default();
        let size = [("min", "20"), ("max", "10000")];
        for (reason, words) in [
            (Reason::IdEmpty { kind: "node" }, tr("No box is given. Give its id.")),
            (Reason::IdEmpty { kind: "edge" }, tr("No line is given. Give its id.")),
            (Reason::IdEmpty { kind: "label" }, tr("No label is given. Give its id.")),
            (Reason::NotA { kind: "diagram", name: "N".into() }, fmt(&tr("{name} is not a diagram."), &[("name", "N")])),
            (Reason::NotA { kind: "edge", name: "N".into() }, fmt(&tr("{name} is not a line on a diagram."), &[("name", "N")])),
            (Reason::NotA { kind: "label", name: "N".into() }, fmt(&tr("{name} is not a label on a line."), &[("name", "N")])),
            (Reason::CoordinateNotANumber { axis: "x" }, fmt(&tr("The {axis} is not a number."), &[("axis", "x")])),
            (
                Reason::CoordinateTooFar { axis: "y" },
                fmt(&tr("The {axis} is farther than {max} from the origin."), &[("axis", "y"), ("max", "1000000")]),
            ),
            (Reason::SizeOutOfRange { side: "width" }, fmt(&tr("The width is not between {min} and {max}."), &size)),
            (Reason::SizeOutOfRange { side: "height" }, fmt(&tr("The height is not between {min} and {max}."), &size)),
            (Reason::OtherDiagram, tr("That box is on another diagram.")),
            (Reason::TooMany { kind: "nesting level", max: 16 }, fmt(&tr("Boxes nest at most {max} deep."), &[("max", "16")])),
            (Reason::TooMany { kind: "waypoint", max: 256 }, fmt(&tr("A line bends at most {max} times."), &[("max", "256")])),
            (Reason::NotRemovable { notation: "ER" }, fmt(&tr("{notation} elements cannot be removed yet."), &[("notation", "ER")])),
        ] {
            assert_eq!(said(reason), words, "{theme:?}");
        }
    }
    archforge_ui_egui::i18n::set_language("en");
}

/// A list of elements selected or removed, or an element described, that a shared command refuses
/// is said in the chosen language: no list of ids, an empty id, an empty list to remove, an element
/// named twice, an id of no element, and a diagram, which belongs to no notation, removed or
/// described; a notation's elements it does not describe said from the reason alone. In light and
/// dark (AR-517).
#[test]
fn a_shared_commands_element_refused_is_said_in_the_chosen_language() {
    use serde_json::json;
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, _) = related_sample_in(theme, egui::vec2(1600.0, 1000.0));
        h.state_mut().state.language = "de".into();
        h.run();
        let order = named(&h, archforge_uml::CLASS, "Order").to_string();
        let d = h.state().canvas.active.unwrap();
        let st = h.state().session.as_ref().unwrap().doc();
        let not_of = format!("{} ist kein Element einer Notation dieses Projekts.", archforge_core::name_of(st, st.by_uuid(d).unwrap()));
        let d = d.to_string();
        let mut refused = |cmd: &str, p: serde_json::Value| archforge_ui_egui::snapshot::run(h.state_mut(), cmd, p).err().unwrap_or_default();
        assert_eq!(refused("view.select", json!({ "ids": [7] })), "Geben Sie die Elemente als Liste von IDs an.", "{theme:?}");
        assert_eq!(refused("view.select", json!({ "ids": [""] })), "Es ist kein Element angegeben. Geben Sie seine ID an.", "{theme:?}");
        assert_eq!(refused("element.remove", json!({ "ids": [] })), "Geben Sie mindestens ein Element an.", "{theme:?}");
        let twice = "Ein Element ist zweimal genannt. Jedes Element nur einmal nennen.";
        assert_eq!(refused("element.remove", json!({ "ids": [&order, &order] })), twice, "{theme:?}");
        assert_eq!(refused("element.describe", json!({ "element": "nope" })), "Es gibt kein Element mit der ID nope.", "{theme:?}");
        assert_eq!(refused("element.remove", json!({ "ids": [&d] })), not_of, "{theme:?}");
        assert_eq!(refused("element.describe", json!({ "element": &d })), not_of, "{theme:?}");
        let undescribed = archforge_shell_api::panels::said(SCREENS.notation_uis(), &archforge_engine::Reason::NotDescribable { notation: "ER" });
        assert_eq!(undescribed.as_deref(), Some("ER-Elemente lassen sich noch nicht beschreiben."), "{theme:?}");
    }
    archforge_ui_egui::i18n::set_language("en");
}

/// The engine's own refusals are said in the chosen language: a command not available now by its
/// reason, a reason the catalogues don't hold in the engine's words, a gesture under way, undo
/// inside another change and a session that could not be put back, each in its sentence, and an
/// unknown command, a parameter refused and an internal error as one sentence that repeats no
/// engine text. In light and dark (AR-440). A change made for another revision of the model, or for
/// none, is said in its sentence too (AR-574), as is each way a package's default diagram binding
/// can be unsound, with the choice that mends it (AR-576), and a diagram a package's binding keeps,
/// its own default or another element's diagram, with the package named (AR-578), as are a nested
/// package's diagrams that cannot move up to a parent with no diagram yet or no valid default
/// (AR-673). Each package move
/// or rename the root package or the package Types refuses is said in its own sentence (AR-563), as
/// is a default diagram chosen for no package, for something that is none or among another
/// element's diagrams (AR-580), and a default's replacement that is the diagram removed, is no
/// other diagram of the same kind of its package, or is named for a diagram that is no default,
/// each in a sentence of its own that speaks of replacing, not of choosing a default (AR-640), and
/// one given both as another diagram and as a new one, or a new one's name that is no text (AR-641).
/// A relationship chosen to reuse that does not join its two elements, or no longer does, is said
/// with the choice to make again when others remain, and as such when none does (AR-699).
#[test]
fn an_engine_refusal_is_said_in_the_chosen_language() {
    use archforge_engine::EngineError;
    use serde_json::json;
    let internal = "Dies war wegen eines internen Fehlers nicht möglich. Es wurde nichts geändert.";
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, nodes) = related_sample_in(theme, egui::vec2(1600.0, 1000.0));
        h.state_mut().state.language = "de".into();
        h.run();
        let refused = |h: &mut Harness<'static, Shell>, cmd: &str, p: serde_json::Value| {
            archforge_ui_egui::snapshot::run(h.state_mut(), cmd, p).err().unwrap_or_default()
        };
        let fill = json!({ "ids": [&nodes["Order"]], "fill": "#3366cc" });
        archforge_ui_egui::snapshot::run(h.state_mut(), "diagram.style", fill.clone()).unwrap();
        assert_eq!(refused(&mut h, "edit.redo", json!({})), "Es gibt nichts zu wiederholen.", "{theme:?}");
        let unheld = EngineError::Disabled { id: "edit.redo".into(), reason: "a reason no catalogue holds" };
        assert_eq!(archforge_shell_api::panels::refusal(SCREENS.notation_uis(), &unheld), "A reason no catalogue holds.", "{theme:?}");
        let session = h.state_mut().session.as_mut().unwrap();
        let in_batch = session.batch("Undo inside", |b| b.execute("edit.undo", &json!({})).map(|_| ())).unwrap_err();
        assert!(matches!(in_batch, EngineError::NotInBatch { .. }), "{theme:?}: {in_batch:?}");
        let said = "Rückgängig und Wiederholen können nicht Teil einer anderen Änderung sein.";
        assert_eq!(archforge_shell_api::panels::refusal(SCREENS.notation_uis(), &in_batch), said, "{theme:?}");
        session.begin_interaction("er.schema.add").unwrap();
        let running = "Eine andere Änderung läuft noch. Versuchen Sie es erneut, sobald sie beendet ist.";
        assert_eq!(refused(&mut h, "diagram.style", fill.clone()), running, "{theme:?}");
        h.state_mut().session.as_mut().unwrap().cancel().unwrap();
        let poisoned = EngineError::Poisoned { message: "the rollback failed".into() };
        let reopen = "Das Modell konnte nach einem Fehler nicht wiederhergestellt werden. Öffnen Sie das Projekt erneut, um fortzufahren.";
        assert_eq!(archforge_shell_api::panels::refusal(SCREENS.notation_uis(), &poisoned), reopen, "{theme:?}");
        assert_eq!(refused(&mut h, "no.such.command", json!({})), internal, "{theme:?}");
        assert_eq!(refused(&mut h, "diagram.style", json!({ "ids": [&nodes["Order"]], "shade": 1 })), internal, "{theme:?}");
        let stopped = EngineError::Internal { id: "diagram.style".into(), message: "a handler panicked".into() };
        assert_eq!(archforge_shell_api::panels::refusal(SCREENS.notation_uis(), &stopped), internal, "{theme:?}");
        let at = archforge_engine::Revision { history: 2, revision: 5 };
        let stale = |expected| {
            let reason = archforge_engine::Reason::StaleRevision { expected, current: at };
            let error = archforge_engine::CommandError::about("expected_revision", "stale").because(reason);
            archforge_shell_api::panels::refusal(SCREENS.notation_uis(), &EngineError::Failed { id: "uml.package.default.set".into(), error })
        };
        let changed = "Das Modell hat sich geändert, seit es gelesen wurde, oder ein anderes Projekt wurde gelesen. \
                       Lesen Sie es erneut und versuchen Sie es noch einmal. Es wurde nichts geändert.";
        assert_eq!(stale(Some(archforge_engine::Revision { revision: 4, ..at })), changed, "{theme:?}");
        let none = "Die Änderung gibt nicht an, für welchen Stand des Modells sie gedacht ist. \
                    Lesen Sie das Modell und versuchen Sie es noch einmal. Es wurde nichts geändert.";
        assert_eq!(stale(None), none, "{theme:?}");
        use archforge_engine::BindingFault::{DeadTarget, Duplicate, Missing, WrongKind, WrongOwner};
        let unsound = [Missing, Duplicate, DeadTarget, WrongKind, WrongOwner].map(|fault| {
            let error = archforge_engine::CommandError::about("package", "unsound").because(archforge_engine::Reason::InvalidBinding(fault));
            archforge_shell_api::panels::refusal(SCREENS.notation_uis(), &EngineError::Failed { id: "uml.package.default.get".into(), error })
        });
        let missing = "Das Paket hat Klassendiagramme, aber keines ist sein Standarddiagramm. Wählen Sie eines davon als Standarddiagramm.";
        assert_eq!(unsound[0], missing, "{theme:?}");
        let mut distinct = unsound.to_vec();
        distinct.sort();
        distinct.dedup();
        assert_eq!(distinct.len(), 5, "{theme:?}: each fault its own sentence {unsound:?}");
        assert!(unsound.iter().all(|s| s.contains("Standarddiagramm")), "{theme:?}: {unsound:?}");
        let kept = |fault| {
            let reason = archforge_engine::Reason::DefaultDiagramOf { package: "Vertrieb".into(), fault };
            let error = archforge_engine::CommandError::about("diagram", "kept").because(reason);
            archforge_shell_api::panels::refusal(SCREENS.notation_uis(), &EngineError::Failed { id: "diagram.remove".into(), error })
        };
        let own = "Dieses Diagramm ist das Standarddiagramm von Vertrieb. \
                   Wählen Sie zuerst ein anderes Klassendiagramm als Standarddiagramm von Vertrieb.";
        assert_eq!(kept(None), own, "{theme:?}");
        let foreign = "Vertrieb hat das Diagramm eines anderen Elements als Standarddiagramm, daher bleibt dieses Diagramm. \
                       Wählen Sie zuerst eines seiner eigenen Klassendiagramme.";
        assert_eq!(kept(Some(WrongOwner)), foreign, "{theme:?}");
        // A nested package's diagrams that cannot move up: a parent with no diagram yet, or with no valid default (AR-673).
        let up = |fault| {
            let error = archforge_engine::CommandError::about("ids", "up")
                .because(archforge_engine::Reason::ParentDefaultUnsound { parent: "Vertrieb".into(), fault });
            archforge_shell_api::panels::refusal(SCREENS.notation_uis(), &EngineError::Failed { id: "element.remove".into(), error })
        };
        let none_yet = "Vertrieb hat noch kein Diagramm, das die Diagramme dieses Pakets aufnehmen kann. Erstellen Sie zuerst eines dafür. Es wurde nichts geändert.";
        let invalid = "Vertrieb hat kein gültiges Standarddiagramm, das die Diagramme dieses Pakets aufnehmen kann. Wählen Sie zuerst sein Standarddiagramm. Es wurde nichts geändert.";
        assert_eq!([up(None), up(Some(Missing)), up(Some(WrongOwner))], [none_yet, invalid, invalid], "{theme:?}");
        // A dependency moved onto the client and supplier another has (AR-697).
        let exists = "Diese Verbindung besteht bereits: die Abhängigkeit zahlt. Wählen Sie die bestehende Beziehung aus.";
        let repeated = archforge_engine::Reason::ConnectionExists { dependency: "zahlt".into() };
        assert_eq!(archforge_shell_api::panels::said(SCREENS.notation_uis(), &repeated), Some(exists.to_string()), "{theme:?}");
        // A relationship chosen to reuse that does not join the two (any more): none left, or some to choose among (AR-699).
        let stale = "Die gewählte Beziehung verbindet diese beiden Elemente nicht oder nicht mehr. \
                     Wählen Sie erneut unter denen, die sie verbinden. Es wurde nichts geändert.";
        let none_left = "Die gewählte Beziehung verbindet diese beiden Elemente nicht oder nicht mehr, und keine andere tut es jetzt. \
                         Es wurde nichts geändert.";
        let chosen = [Box::default(), vec!["d-2".into()].into_boxed_slice()]
            .map(|candidates| archforge_shell_api::panels::said(SCREENS.notation_uis(), &archforge_engine::Reason::StaleChoice { candidates }));
        assert_eq!(chosen, [Some(none_left.to_string()), Some(stale.to_string())], "{theme:?}: none left, or some to choose among");
        // A default diagram chosen for no package, for something that is none, or among another's diagrams (AR-580).
        use archforge_engine::Reason::{IdEmpty, NotA, NotOwnDiagram};
        let other = NotOwnDiagram { diagram: "Rechnungen".into(), package: "Vertrieb".into() };
        let chosen = [IdEmpty { kind: "package" }, NotA { kind: "package", name: "Vertrieb".into() }, other]
            .map(|r| archforge_shell_api::panels::said(SCREENS.notation_uis(), &r));
        let theirs =
            "Rechnungen ist keines der Klassendiagramme von Vertrieb. Wählen Sie eines seiner eigenen Klassendiagramme als Standarddiagramm.";
        let chosen_said = ["Es ist kein Paket angegeben. Geben Sie seine ID an.", "Vertrieb ist kein Paket.", theirs].map(|s| Some(s.to_string()));
        assert_eq!(chosen, chosen_said, "{theme:?}");
        // A replacement that is the diagram removed, or one named for a diagram that is no default (AR-640).
        use archforge_engine::Reason::{NotReplacement, ReplacementIsRemoved, ReplacementNotNeeded};
        let foreign = NotReplacement { diagram: "Rechnungen".into(), package: "Vertrieb".into() };
        let replaced = [ReplacementIsRemoved, ReplacementNotNeeded { diagram: "Rechnungen".into() }, foreign]
            .map(|r| archforge_shell_api::panels::said(SCREENS.notation_uis(), &r));
        let itself = "Ein Diagramm kann sich beim Entfernen nicht selbst ersetzen. Wählen Sie ein anderes Diagramm als Ersatz.";
        let not_needed = "Rechnungen ist kein Standarddiagramm und wird daher ohne Ersatz entfernt.";
        let not_own = "Rechnungen ist kein anderes Diagramm derselben Art von Vertrieb und kann das zu entfernende Diagramm daher nicht ersetzen.";
        assert_eq!(replaced, [itself, not_needed, not_own].map(|s| Some(s.to_string())), "{theme:?}");
        let both = "Wählen Sie als Ersatz entweder ein vorhandenes Diagramm oder ein neues, nicht beides.";
        assert_eq!(
            archforge_shell_api::panels::said(SCREENS.notation_uis(), &archforge_engine::Reason::OneReplacement),
            Some(both.to_string()),
            "{theme:?}"
        );
        assert_eq!(
            archforge_shell_api::panels::said(SCREENS.notation_uis(), &archforge_engine::Reason::NameNotText),
            Some("Ein Name muss Text sein.".to_string()),
            "{theme:?}"
        );
        // A package move or rename the root package or the package Types refuses, each its own sentence.
        use archforge_engine::Reason::{HoldsBuiltInName, RootPackageStays, TypesPackageExists, TypesPackageKept};
        let held = HoldsBuiltInName { kind: "class", name: "String".into() };
        let package = [RootPackageStays, TypesPackageKept, TypesPackageExists, held]
            .map(|r| archforge_shell_api::panels::said(SCREENS.notation_uis(), &r).unwrap_or_default());
        let mut distinct = package.to_vec();
        distinct.sort();
        distinct.dedup();
        assert_eq!(distinct.len(), 4, "{theme:?}: each its own sentence {package:?}");
        assert!(package.iter().all(|s| s.contains("Paket")), "{theme:?}: in German {package:?}");
        assert!(package[3].contains("eine Klasse den Namen String"), "{theme:?}: {}", package[3]);
    }
    archforge_ui_egui::i18n::set_language("en");
}

/// A name another element has, whatever the case, is said in the chosen language with that
/// element's kind and name, never in the engine's English: an attribute of a class, a column of a
/// table, and a skill on one subject, renamed and added. In light and dark (AR-435).
#[test]
fn a_name_another_member_column_or_skill_has_is_said_in_the_chosen_language() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let mut h = workspace_harness(theme, egui::vec2(1600.0, 1120.0));
        h.state_mut().state.language = "de".into();
        h.run();
        let mut id =
            |cmd: &str, p: serde_json::Value| archforge_ui_egui::snapshot::run(h.state_mut(), cmd, p).unwrap()["id"].as_str().unwrap().to_string();
        let sales = id("uml.package.add", serde_json::json!({ "name": "Sales" }));
        let order = id("uml.class.add", serde_json::json!({ "package": sales, "name": "Order" }));
        id("uml.attribute.add", serde_json::json!({ "class": order, "name": "status" }));
        let schema = id("er.schema.add", serde_json::json!({ "name": "Shop" }));
        let table = id("er.entity.add", serde_json::json!({ "schema": schema, "name": "Kunde" }));
        id("er.attribute.add", serde_json::json!({ "entity": table, "name": "id", "type": "BIGINT" }));
        id("skill.add", serde_json::json!({ "subject": sales, "name": "Naming" }));
        let style = id("skill.add", serde_json::json!({ "subject": sales, "name": "Style" }));
        for (cmd, params, kind, name) in [
            ("uml.attribute.add", serde_json::json!({ "class": order, "name": "STATUS" }), "an attribute", "status"),
            ("er.attribute.add", serde_json::json!({ "entity": table, "name": "ID", "type": "TEXT" }), "a column", "id"),
            ("skill.rename", serde_json::json!({ "skill": style, "name": "naming" }), "a skill", "Naming"),
            ("skill.add", serde_json::json!({ "subject": sales, "name": "NAMING" }), "a skill", "Naming"),
        ] {
            let what = archforge_ui_egui::i18n::tr_ctx("refusal", kind);
            assert_ne!(what, kind, "{theme:?}: the catalogue has the kind in German");
            let said = archforge_ui_egui::i18n::fmt(
                &archforge_ui_egui::i18n::tr("There is {what} named {name} already."),
                &[("what", &what), ("name", name)],
            );
            let refused = archforge_ui_egui::snapshot::run(h.state_mut(), cmd, params).err().unwrap_or_default();
            assert_eq!(refused.to_lowercase(), said.to_lowercase(), "{theme:?} {cmd}: the kind and the other's name in German");
            assert!(refused.starts_with(char::is_uppercase) && refused.contains(name), "{theme:?} {cmd}: a sentence naming {name}: {refused}");
        }
    }
    archforge_ui_egui::i18n::set_language("en");
}

/// A refusal names each kind of element in its catalogue row, with its article, as before the
/// notations said their own (AR-785): the shell's for the kinds the core, the diagrams or both
/// notations name, and one notation's for each of the rest; a kind no row names is none.
#[test]
fn each_kind_a_refusal_names_is_said_by_the_shell_or_one_notation() {
    // The app's shell registers its notations' catalogues, whose rows this says.
    let _shell = archforge_ui_egui::Shell::new(archforge_ui_egui::Services::default(), &SCREENS);
    use archforge_ui_egui::i18n::{set_language, tr_ctx};
    let uis = SCREENS.notation_uis();
    let own = [("element", "an element"), ("constraint", "a constraint"), ("skill", "a skill")];
    let er = [
        ("schema", "a schema"),
        ("table", "a table"),
        ("column", "a column"),
        ("relationship", "a relationship"),
        ("domain", "a domain"),
        ("enum type", "an enum type"),
        ("sequence", "a sequence"),
        ("view", "a view"),
        ("primary key", "a primary key"),
        ("unique constraint", "a unique constraint"),
        ("foreign key", "a foreign key"),
        ("check", "a check"),
        ("index", "an index"),
        ("subtype", "a subtype"),
        ("key", "a key"),
        ("key column", "a key column"),
    ];
    let uml = [
        ("class", "a class"),
        ("association class", "an association class"),
        ("interface", "an interface"),
        ("enumeration", "an enumeration"),
        ("data type", "a data type"),
        ("primitive type", "a primitive type"),
        ("package", "a package"),
        ("profile", "a profile"),
        ("stereotype", "a stereotype"),
        ("attribute", "an attribute"),
        ("operation", "an operation"),
        ("literal", "a literal"),
        ("qualifier", "a qualifier"),
        ("template parameter", "a template parameter"),
        ("generalization set", "a generalization set"),
        ("association end", "an association end"),
    ];
    set_language("de");
    assert_ne!(tr_ctx("refusal", "a table"), "a table", "the rows checked are really translations");
    for (owner, kinds) in [(None, &own[..]), (Some(archforge_er::INDEX), &er[..]), (Some(archforge_uml::INDEX), &uml[..])] {
        for (kind, row) in kinds {
            assert_eq!(archforge_shell_api::panels::a_kind(SCREENS.notation_uis(), kind), Some(tr_ctx("refusal", row)), "{kind}");
            let by: Vec<u16> = uis.iter().filter(|ui| ui.a_kind(kind).is_some()).map(|ui| ui.metamodel()).collect();
            assert_eq!(by, owner.into_iter().collect::<Vec<_>>(), "{kind} is said by the notations of the metamodels {by:?}");
        }
    }
    for kind in ["waypoint", "nesting level", "Table", ""] {
        assert_eq!(archforge_shell_api::panels::a_kind(SCREENS.notation_uis(), kind), None, "{kind:?}");
    }
    set_language("en");
}

/// What the shell asks a notation's state: whether something is being edited or drawn, what holds
/// the canvas's keys, what a project set aside ends, and what moving the selection closes, a
/// binding's menu in the shell's one menu slot among it; an editor opened this frame stays (AR-787).
#[test]
fn a_notation_s_state_answers_what_is_edited_held_set_aside_and_closed() {
    use archforge_shell_api::notation_ui::NotationStates;
    let uis = SCREENS.notation_uis();
    let id = archforge_mof::Uuid([7; 16]);
    let mut states = NotationStates::new(uis);
    uml_state::uml_mut(&mut states).unwrap().line_menu = Some(egui::pos2(1.0, 1.0));
    assert!(states.holds_keys() && !states.editing(), "the generalization lines' menu holds the keys and edits nothing");
    let uml = uml_state::uml_mut(&mut states).unwrap();
    (uml.line_menu, uml.nary_menu) = (None, Some((egui::pos2(1.0, 1.0), id)));
    assert!(states.holds_keys(), "an n-ary association's menu holds the keys");
    uml_state::uml_mut(&mut states).unwrap().nary = Some(uml_nary::Nary::AddEnd { diagram: id, association: id });
    assert!(states.editing(), "an n-ary association under way is being drawn");
    states.set_aside();
    let uml = uml_state::uml(&states).unwrap();
    assert!(uml.nary.is_none() && uml.nary_menu.is_none(), "a drawing and its menu end with the project set aside");
    assert!(!states.editing() && !states.holds_keys());

    let key = |opened| er_keys::KeyEdit {
        target: None,
        entity: id,
        primary: false,
        name: String::new(),
        columns: Vec::new(),
        nulls_distinct: true,
        refused: None,
        opened,
    };
    er_state::er_mut(&mut states).unwrap().role = Some(er_relation::RoleEdit { target: id, end: 0, text: "buyer".into() });
    assert!(states.editing(), "a relationship's role is being typed");
    er_state::er_mut(&mut states).unwrap().role = None;

    er_state::er_mut(&mut states).unwrap().key = Some(key(true));
    let uml = uml_state::uml_mut(&mut states).unwrap();
    uml.nary_remove = Some(uml_nary::RemoveAsk { association: id, end: 0, opened: true });
    uml.final_refused = Some(id);
    uml.stereotypes.filter = "Ent".into();
    uml.template_in_use = Some(uml_templates::InUse { parameter: id, why: "bound".into(), binding: None });
    assert!(states.editing(), "an editor card and a question are open");
    let mut menu = Some(uml_state::menus::BINDING_TEMPLATE);
    states.close_editors(&mut menu);
    assert_eq!(menu, None, "a binding's menu closes with the editors");
    assert_eq!(er_state::er(&states).unwrap().key, Some(key(true)), "an editor opened this frame stays");
    let uml = uml_state::uml(&states).unwrap();
    assert!(uml.nary_remove.is_none() && uml.final_refused.is_none() && uml.template_in_use.is_none(), "the question and the refusals close");
    assert_eq!(uml.stereotypes, uml_stereotypes::Stereotypes::default(), "the Stereotypes picker's filter goes with them");
    er_state::er_mut(&mut states).unwrap().key = Some(key(false));
    let mut menu = Some(er_state::menus::KEY_COLUMN);
    states.close_editors(&mut menu);
    assert_eq!(menu, Some(er_state::menus::KEY_COLUMN), "another menu is not the editors' to close");
    assert!(er_state::er(&states).unwrap().key.is_none(), "an editor shown already closes");
    assert!(!states.editing());
}

/// Only a notation's own menu that takes typing keeps the keys from the canvas and the Models tree:
/// UML's Stereotypes picker does; another of its menus, or a menu of another owner under the
/// picker's key, does not (AR-135, AR-787).
#[test]
fn only_a_notation_s_typed_menu_takes_the_keys() {
    use archforge_shell_api::notation_ui::NotationStates;
    let uis = SCREENS.notation_uis();
    use archforge_shell_api::notation_ui::MenuId;
    let states = NotationStates::new(uis);
    let pick = uml_state::menus::STEREOTYPE_PICK;
    assert!(states.menu_takes_keys(Some(pick)), "the Stereotypes picker takes the keys");
    assert!(!states.menu_takes_keys(None), "no menu open takes nothing");
    assert!(!states.menu_takes_keys(Some(uml_state::menus::STEREOTYPE_PROFILE)), "a select of UML's leaves the keys");
    assert!(!states.menu_takes_keys(Some(MenuId::of(archforge_er::INDEX, pick.key))), "ER's menu under the picker's key is not the picker");
    assert!(!states.menu_takes_keys(Some(MenuId::shell(pick.key))), "nor is the shell's");
    assert!(!states.menu_takes_keys(Some(pick.at(1))), "the picker is shown once");

    let mut binding = Some(uml_state::menus::BINDING_ARGUMENT.at(2));
    let mut states = NotationStates::new(uis);
    states.close_editors(&mut binding);
    assert_eq!(binding, None, "a binding's argument menu closes with the editors, whichever parameter it is on");
    let theirs = MenuId::of(archforge_er::INDEX, uml_state::menus::BINDING_TEMPLATE.key);
    let mut other = Some(theirs);
    states.close_editors(&mut other);
    assert_eq!(other, Some(theirs), "UML closes only its own binding menus, not another owner's of the same key");
}

/// A name of nothing but spaces is refused before it is sent, except for a diagram and for what a
/// notation's rename command may leave unnamed: a relationship's, in ER and in UML (AR-786).
#[test]
fn an_empty_name_is_refused_unless_the_rename_may_leave_it_unnamed() {
    archforge_ui_egui::i18n::set_language("en");
    let mut shell = Shell::new(Services::default(), &SCREENS);
    for cmd in ["diagram.rename", "er.relationship.rename", "uml.relationship.rename"] {
        assert!(
            !archforge_ui_egui::snapshot::cx(&mut shell, |cx| archforge_shell_api::panels::refuse_empty_name_cx(cx, cmd, "  ")),
            "{cmd} may leave its element unnamed"
        );
        assert_eq!(shell.panels.notice, None, "{cmd}");
    }
    for cmd in ["er.schema.rename", "uml.class.rename"] {
        assert!(
            archforge_ui_egui::snapshot::cx(&mut shell, |cx| archforge_shell_api::panels::refuse_empty_name_cx(cx, cmd, "  ")),
            "{cmd} needs a name"
        );
        assert_eq!(shell.panels.notice.take().as_deref(), Some("A name can't be empty."), "{cmd}");
    }
}

/// A row dropped in Models runs as its notation runs it (AR-788): UML moves a package dropped into
/// another as Properties' Parent package does, with a toast offering Undo; ER's notation, which
/// keeps the default, runs the same command plainly, with no toast. In light and dark.
#[test]
fn a_drop_in_models_runs_as_its_notation_runs_it() {
    use archforge_shell_api::notation_ui::NotationUi;
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let mut h = workspace_harness(theme, egui::vec2(1600.0, 1000.0));
        let s = h.state_mut().session.as_mut().unwrap();
        let mut add =
            |name: &str| s.execute("uml.package.add", &serde_json::json!({ "name": name })).unwrap().value["id"].as_str().unwrap().to_string();
        let (sales, orders, billing) = (add("Sales"), add("Orders"), add("Billing"));
        let parent = |shell: &Shell, package: &str| {
            let st = shell.session.as_ref().unwrap().doc();
            let p = archforge_uml::commands::package_of(st, st.by_uuid(archforge_mof::Uuid::parse(package).unwrap()).unwrap()).unwrap();
            archforge_core::name_of(st, p)
        };
        let shell = h.state_mut();
        let params = |package: &str| serde_json::json!({ "package": package, "to": sales });
        archforge_ui_egui::snapshot::cx(shell, |cx| er_ui::ErUi.run_drop(cx, "uml.package.move", params(&orders))).unwrap();
        assert_eq!((parent(shell, &orders).as_str(), shell.canvas.toast.is_none()), ("Sales", true), "{theme:?}: run plainly");
        archforge_ui_egui::snapshot::cx(shell, |cx| uml_ui::UmlUi.run_drop(cx, "uml.package.move", params(&billing))).unwrap();
        assert_eq!(parent(shell, &billing), "Sales", "{theme:?}");
        assert_eq!(shell.canvas.toast.as_ref().map(|t| t.text.as_str()), Some("Billing moved into Sales"), "{theme:?}: as Properties moves it");
        assert_eq!(shell.session.as_ref().unwrap().undo_label(), Some("Move package"), "{theme:?}");
        let refused = archforge_ui_egui::snapshot::cx(shell, |cx| {
            uml_ui::UmlUi.run_drop(cx, "uml.package.move", serde_json::json!({ "package": sales, "to": billing }))
        });
        assert!(refused.is_err(), "{theme:?}: into a package it holds, refused in words: {refused:?}");
    }
}

/// Every notation answers which page Properties shows for its elements: ER a column's table and
/// any other element its own, UML each element its own.
#[test]
fn each_notation_answers_the_page_of_its_elements() {
    let mut n: u128 = 0;
    let ids = Box::new(move || {
        n += 1;
        archforge_mof::Uuid(n.to_le_bytes())
    });
    let mut s = archforge_notations::new_project_session("P", ids).unwrap();
    let id = |v: serde_json::Value| v["id"].as_str().unwrap().to_string();
    let schema = id(s.execute("er.schema.add", &serde_json::json!({ "name": "Shop" })).unwrap().value);
    let table = id(s.execute("er.entity.add", &serde_json::json!({ "schema": schema, "name": "Customer" })).unwrap().value);
    let column = id(s.execute("er.attribute.add", &serde_json::json!({ "entity": table, "name": "id", "type": "BIGINT" })).unwrap().value);
    let root = id(s.execute("uml.model.root", &serde_json::json!({})).unwrap().value);
    let store = s.doc();
    let obj = |id: &str| store.by_uuid(archforge_mof::Uuid::parse(id).unwrap()).unwrap();
    let er = archforge_shell_api::notation_ui::of(SCREENS.notation_uis(), store, obj(&table)).unwrap();
    assert_eq!(er.page(store, obj(&column)), Some(obj(&table)), "a column's page is its table's");
    assert_eq!(er.page(store, obj(&table)), Some(obj(&table)));
    assert_eq!(er.page(store, obj(&schema)), Some(obj(&schema)));
    let uml = archforge_shell_api::notation_ui::of(SCREENS.notation_uis(), store, obj(&root)).unwrap();
    assert_eq!(uml.page(store, obj(&root)), Some(obj(&root)));
}

/// The app's screens parts, ER's before UML's, give a shell the lists the shell knew before it
/// was given parts: ER's notation and kind, then UML's, each the part's own.
#[test]
fn the_apps_screens_parts_give_a_shell_er_then_uml() {
    let shell = Shell::new(Services::default(), &SCREENS);
    let metamodels = |uis: &[&dyn archforge_shell_api::notation_ui::NotationUi]| uis.iter().map(|ui| ui.metamodel()).collect::<Vec<_>>();
    assert_eq!(
        metamodels(archforge_ui_egui::testing::notation_uis(&shell)),
        metamodels(&[archforge_er_ui::ER.ui, archforge_uml_ui::UML.ui]),
        "ER's notation, then UML's"
    );
    assert_eq!(metamodels(archforge_ui_egui::testing::notation_uis(&shell)), [archforge_er::INDEX, archforge_uml::INDEX]);
    let kinds = |kinds: &[&dyn archforge_shell_api::kinds::DiagramKind]| kinds.iter().map(|k| k.kind()).collect::<Vec<_>>();
    assert_eq!(
        kinds(archforge_ui_egui::testing::kinds(&shell)),
        kinds(&[archforge_er_ui::ER.kinds, archforge_uml_ui::UML.kinds].concat()),
        "ER's kind, then UML's"
    );
    assert_eq!(kinds(archforge_ui_egui::testing::kinds(&shell)), [archforge_er::DIAGRAM_KIND, archforge_uml::STRUCTURE_DIAGRAM]);
}

#[test]
fn an_er_project_makes_a_class_diagram_beside_its_er_diagrams_and_an_agent_makes_one_too() {
    let (mut h, _customer, er_diagram) = diagram_harness();
    let at = rect_of(&h, "workspace.tabs.add").center();
    click(&mut h, at);
    assert_eq!(h.state().diagrams.new.as_ref().unwrap().kind, archforge_er::DIAGRAM_KIND, "an ER project starts at ER");
    let at = rect_of(&h, "workspace.new-diagram.type.uml.structure").center();
    click(&mut h, at);
    let at = rect_of(&h, "workspace.new-diagram.continue").center();
    click(&mut h, at);
    h.state_mut().diagrams.new.as_mut().unwrap().name = "Domain".into();
    let at = rect_of(&h, "workspace.new-diagram.create").center();
    click(&mut h, at);
    assert_eq!(class_diagram_shown(&h).0, archforge_uml::STRUCTURE_DIAGRAM);
    assert_eq!(h.state().canvas.tabs.len(), 2, "beside the ER diagram's tab");
    assert!(h.state().canvas.tabs.contains(&archforge_mof::Uuid::parse(&er_diagram).unwrap()));
    // An agent, through the control channel's commands, as its MCP tools call them.
    let mut call = |command: &str, params: serde_json::Value| {
        let out = archforge_ui_egui::control::answer(h.state_mut(), "engine.execute", &serde_json::json!({ "command": command, "params": params }))
            .unwrap_or_else(|e| panic!("{command}: {e}"));
        out["result"]["id"].as_str().unwrap_or_default().to_string()
    };
    let pkg = call("uml.package.add", serde_json::json!({ "name": "Billing" }));
    let d = call("diagram.add", serde_json::json!({ "kind": archforge_uml::STRUCTURE_DIAGRAM, "element": pkg, "name": "Billing" }));
    let invoice = call("uml.class.add", serde_json::json!({ "package": pkg, "name": "Invoice" }));
    call("diagram.place", serde_json::json!({ "diagram": d, "element": invoice, "x": 100, "y": 100 }));
    let st = h.state().session.as_ref().unwrap().doc();
    let d = st.by_uuid(archforge_mof::Uuid::parse(&d).unwrap()).expect("the agent's class diagram");
    assert_eq!(st.list(d, archforge_diagram::diagram::NODE).unwrap().len(), 1);
}

/// A project opened again resumes its tabs, the diagram it showed and its view.
#[test]
fn a_project_opened_again_resumes_its_tabs_the_diagram_it_showed_and_its_view() {
    let mut h = workspace_harness(ThemeKind::Light, egui::vec2(1600.0, 1000.0));
    archforge_ui_egui::snapshot::cx(h.state_mut(), uml_ui::open_sample).unwrap();
    h.run();
    let class_diagram = h.state().canvas.active.expect("the sample's class diagram");
    // An ER diagram beside it, then the class diagram shown again, panned and zoomed.
    let er = {
        let s = h.state_mut().session.as_mut().unwrap();
        let schema = s.execute("er.schema.add", &serde_json::json!({ "name": "shop" })).unwrap().value["id"].as_str().unwrap().to_string();
        let d =
            s.execute("diagram.add", &serde_json::json!({ "kind": archforge_er::DIAGRAM_KIND, "element": schema, "name": "Shop" })).unwrap().value;
        archforge_mof::Uuid::parse(d["id"].as_str().unwrap()).unwrap()
    };
    archforge_ui_egui::canvas::open(h.state_mut(), er);
    h.run();
    archforge_ui_egui::canvas::open(h.state_mut(), class_diagram);
    h.run();
    h.state_mut().canvas.zoom = 1.5;
    h.state_mut().canvas.pan = egui::vec2(-120.0, 40.0);
    h.run();
    let path = h.state().project.as_ref().unwrap().path.clone();
    let kept = h.state().state.view_of(&path).cloned().expect("the view is kept with the recent entry");
    assert_eq!(kept.tabs, [class_diagram.to_string(), er.to_string()]);
    assert_eq!((kept.active.as_deref(), kept.zoom, kept.pan), (Some(class_diagram.to_string().as_str()), 1.5, [-120.0, 40.0]));
    assert_eq!(kept.label, "Commerce", "the shown diagram's name, for Home");
    // Home's recent row says it.
    h.state_mut().go_home();
    h.run();
    assert!(painted(&h).iter().any(|t| t.ends_with("Last view: Commerce")), "{:?}", painted(&h));
    let at = rect_of(&h, "workspace.header.project.0").center();
    click(&mut h, egui::pos2(at.x - 20.0, at.y));
    h.run();
    // Saved, closed and opened again in a new window with the same preferences: the same tabs,
    // the same diagram shown, panned and zoomed as it was, and nothing changed or undoable.
    let (tables, junctions) = archforge_notations::storage();
    let rows = std::sync::Arc::new(archforge_mof::rows::snapshot(h.state().session.as_ref().unwrap().doc(), &tables, &junctions).unwrap());
    let reopen = |state: archforge_ui_egui::state::UiState| {
        let mut shell = Shell::new(Services { ids: Some(test_ids()), ..Services::default() }, &SCREENS);
        shell.state = state;
        shell.open_project(archforge_ui_egui::Opened {
            name: "Commerce platform".into(),
            path: path.clone(),
            model: Some(rows.clone()),
            unsaved: false,
            notice: None,
        });
        harness_over(shell)
    };
    let back = reopen(h.state().state.clone());
    let canvas = &back.state().canvas;
    assert_eq!((canvas.tabs.clone(), canvas.active), (vec![class_diagram, er], Some(class_diagram)));
    assert_eq!((canvas.zoom, canvas.pan), (1.5, egui::vec2(-120.0, 40.0)));
    let session = back.state().session.as_ref().unwrap();
    assert!(!session.can_undo() && !session.is_dirty(), "resuming the view changes nothing");
    // A tab whose diagram is gone is left out; a view that does not read opens on no tab.
    let mut state = h.state().state.clone();
    let gone = archforge_mof::Uuid::v7(1_761_670_900_000, [5; 10]).to_string();
    state.keep_view(
        &path,
        archforge_ui_egui::state::SavedView {
            tabs: vec![gone.clone(), er.to_string()],
            active: Some(gone),
            pan: [0.0, 0.0],
            zoom: 1.0,
            label: String::new(),
        },
    );
    let back = reopen(state.clone());
    assert_eq!((back.state().canvas.tabs.clone(), back.state().canvas.active), (vec![er], Some(er)));
    state.keep_view(
        &path,
        archforge_ui_egui::state::SavedView { tabs: vec!["not an id".into()], active: None, pan: [f32::NAN, 0.0], zoom: 99.0, label: String::new() },
    );
    state.sanitize();
    assert!(state.view_of(&path).is_none());
    assert!(reopen(state).state().canvas.tabs.is_empty());
}

/// A class diagram's menu closes on an ER tab, so the arrows nudge there.
#[test]
fn a_menu_left_open_on_a_class_diagram_closes_on_an_er_tab_and_the_arrows_nudge_there() {
    let (mut h, customer, er_diagram) = diagram_harness();
    let uml = {
        let s = h.state_mut().session.as_mut().unwrap();
        let id = |v: serde_json::Value| v["id"].as_str().unwrap().to_string();
        let pkg = id(s.execute("uml.package.add", &serde_json::json!({ "name": "Billing" })).unwrap().value);
        let d = id(s
            .execute("diagram.add", &serde_json::json!({ "kind": archforge_uml::STRUCTURE_DIAGRAM, "element": pkg, "name": "Billing" }))
            .unwrap()
            .value);
        let invoice = id(s.execute("uml.class.add", &serde_json::json!({ "package": pkg, "name": "Invoice" })).unwrap().value);
        s.execute("diagram.place", &serde_json::json!({ "diagram": d, "element": invoice, "x": 100, "y": 100 })).unwrap();
        archforge_mof::Uuid::parse(&d).unwrap()
    };
    archforge_ui_egui::canvas::open(h.state_mut(), uml);
    h.run();
    let node_at = |h: &Harness<'static, Shell>| {
        let id = node_ids(h).into_iter().next().expect("a node");
        (id.clone(), rect_of(h, &id))
    };
    // With the Relationship menu open, the arrows still nudge the selected card, as before.
    let (invoice, was) = node_at(&h);
    click(&mut h, was.center());
    let at = rect_of(&h, "workspace.tools.relationship-kinds").center();
    click(&mut h, at);
    assert_eq!(h.state().canvas.popover, Some("relationship-kinds"));
    key(&mut h, egui::Key::ArrowRight, egui::Modifiers::NONE);
    assert!(rect_of(&h, &invoice).left() > was.left(), "the card moved");
    assert_eq!(h.state().canvas.popover, Some("relationship-kinds"));
    // The ER tab: the class diagram's menu closes, though ER's tool has the same key.
    archforge_ui_egui::canvas::open(h.state_mut(), archforge_mof::Uuid::parse(&er_diagram).unwrap());
    h.run();
    assert_eq!((h.state().canvas.popover, h.state().canvas.menu_focus), (None, None));
    let node = node_ids(&h)
        .into_iter()
        .find(|n| {
            let st = h.state().session.as_ref().unwrap().doc();
            let o = st.by_uuid(archforge_mof::Uuid::parse(n.trim_start_matches("workspace.canvas.node.")).unwrap()).unwrap();
            st.get(o, archforge_diagram::node::ELEMENT).unwrap().as_ref().and_then(|e| st.uuid(e).ok()).map(|u| u.to_string())
                == Some(customer.clone())
        })
        .expect("Customer's node");
    let was = rect_of(&h, &node);
    click(&mut h, was.center());
    key(&mut h, egui::Key::ArrowRight, egui::Modifiers::NONE);
    assert!(rect_of(&h, &node).left() > was.left(), "the arrows nudge on the ER canvas");
}

/// What each notation says of a removal of its own (AR-789), worked out before it runs: UML the
/// generalization sets a line alone made up and what it does to the constraints, and its toast
/// for the line naming those sets; ER nothing beyond `{what} removed`. In light and dark.
#[test]
fn each_notation_says_what_a_removal_of_its_own_does() {
    use archforge_shell_api::notation_ui::NotationUi;
    use archforge_shell_api::notation_ui::Removal;
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (h, gens, _) = sample_with_a_set_and_a_rule(theme);
        let st = h.state().session.as_ref().unwrap().doc();
        let o = |id: &str| st.by_uuid(archforge_mof::Uuid::parse(id).unwrap()).unwrap();
        let said = uml_ui::UmlUi.before_removal(st, &[o(&gens[1])]);
        let want = Removal { went: vec!["Kinds".into()], after: Some("settled now constrains Payment only".into()) };
        assert_eq!(said, want, "{theme:?}");
        let text = uml_ui::UmlUi.removed_text("Generalization", &said);
        assert_eq!(text, "Generalization removed · Kinds had no other member and went too", "{theme:?}");
        // CardPayment's generalization is in no set and no rule names it.
        let plain = uml_ui::UmlUi.before_removal(st, &[o(&gens[0])]);
        assert_eq!(plain, Removal::default(), "{theme:?}");
        assert_eq!(uml_ui::UmlUi.removed_text("Generalization", &plain), "Generalization removed", "{theme:?}");
        // ER says nothing before, and its toast names no set even when given one.
        assert_eq!(er_ui::ErUi.before_removal(st, &[]), Removal::default(), "{theme:?}");
        assert_eq!(er_ui::ErUi.removed_text("Relationship", &want), "Relationship removed", "{theme:?}");
    }
}

/// An enumeration that holds the most literals it may takes no more: Properties' editor says so in
/// the user's words and writes nothing; the cap is ER's enum types' too. In light and dark.
#[test]
fn an_enumeration_at_its_literal_cap_says_so_and_takes_no_more() {
    assert_eq!(archforge_uml::members::MAX_LITERALS, archforge_er::rules::MAX_LITERALS, "one rule across the notations");
    let names: Vec<String> = (0..archforge_uml::members::MAX_LITERALS).map(|i| format!("C{i}")).collect();
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, _, status, _) = literal_sample(theme, &names);
        h.state_mut().state.show_panel(archforge_shell_api::state::Panel::Properties);
        h.run();
        let revision = h.state().session.as_ref().unwrap().revision();
        let at = rect_of(&h, "workspace.properties.literals.add").center();
        click(&mut h, at);
        h.event(egui::Event::Text("More".into()));
        h.run();
        key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
        let refused = uml_of(h.state()).member.as_ref().and_then(|e| e.refused.clone()).expect("refused");
        assert_eq!(refused, "An enumeration holds at most 256 literals.");
        assert_eq!((literal_names(&h, status).len(), h.state().session.as_ref().unwrap().revision()), (256, revision), "nothing written");
    }
}

#[test]
fn the_removal_toast_keeps_clear_of_the_minimap_at_every_width_and_its_undo_still_undoes() {
    let (mut h, _customer, _diagram) = diagram_harness();
    let schema = {
        let st = h.state().session.as_ref().unwrap().doc();
        st.uuid(st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_er::SCHEMA)).unwrap()).unwrap()
    };
    let tables = |h: &Harness<'static, Shell>| {
        let st = h.state().session.as_ref().unwrap().doc();
        st.alive().filter(|o| st.class_of(*o).ok() == Some(archforge_er::ENTITY)).count()
    };
    let before = tables(&h);
    h.state_mut()
        .session
        .as_mut()
        .unwrap()
        .execute("er.entity.add", &serde_json::json!({ "schema": schema.to_string(), "name": "Invoice" }))
        .unwrap();
    // A long message, as a translation makes it, on the toast the removal would show.
    let text = "Orders entfernt; was es enthielt, wurde nach Commerce UML verschoben, mit allen Klassen und Beziehungen".to_string();
    let mut beside_minimap = Vec::new();
    // The report's 1505-point window among them; at the narrower ones the minimap folds away.
    for size in [egui::vec2(1600.0, 1000.0), egui::vec2(1505.0, 1040.0), egui::vec2(1420.0, 900.0), egui::vec2(980.0, 900.0)] {
        h.set_size(size);
        let undo_label = h.state().session.as_ref().unwrap().undo_label().map(str::to_string);
        let until = h.state().frames + 1000;
        h.state_mut().canvas.toast = Some(archforge_shell_api::canvas::Toast { text: text.clone(), until, undo_label });
        h.run();
        let toast = rect_of(&h, "workspace.toast");
        let undo = rect_of(&h, "workspace.toast.undo");
        assert!(toast.contains_rect(undo), "{size:?}: Undo {undo:?} lies within the toast {toast:?}");
        if ids(&h).iter().any(|i| i == "workspace.minimap") {
            let minimap = rect_of(&h, "workspace.minimap");
            assert!(!toast.intersects(minimap), "{size:?}: the toast {toast:?} is clear of the minimap {minimap:?}");
            beside_minimap.push(size);
        }
        assert!(painted(&h).iter().any(|t| t.starts_with("Orders entfernt") && t.ends_with("Beziehungen")), "{size:?}: the whole message shows");
    }
    // The minimap showed at the wider widths, so the check above ran there.
    assert!(beside_minimap.len() >= 2, "{beside_minimap:?}");
    // Its Undo, at the narrowest width, undoes what the toast is about.
    let at = rect_of(&h, "workspace.toast.undo").center();
    click(&mut h, at);
    assert_eq!(tables(&h), before, "Undo took the table back");
}

/// AR-790: a project saved before each notation answered its own open-time steps, holding both
/// shapes an opening puts right (a table box too short for its two columns, and two packages from
/// before the root package), opens to the very model, boxes and history the shell made of it
/// before: ER's boxes grown as the rows are read, then UML's root package as one step that can be
/// undone.
#[test]
fn a_project_saved_before_the_open_time_hooks_opens_to_the_same_model_and_boxes() {
    // Ids of their own while the older project is made, so none is given again as it opens.
    let ids = |at: u64| -> Box<dyn FnMut() -> archforge_mof::Uuid + Send> {
        let mut n: u64 = 0;
        Box::new(move || {
            n += 1;
            let mut r = [0u8; 10];
            r[2..].copy_from_slice(&n.to_be_bytes());
            archforge_mof::Uuid::v7(at, r)
        })
    };
    let (tables, junctions) = archforge_notations::storage();
    let snap = |s: &archforge_notations::Session| archforge_mof::rows::snapshot(s.doc(), &tables, &junctions).unwrap();
    let mut made = archforge_notations::new_project_session("Commerce platform", ids(1_761_670_700_000)).unwrap();
    let id = |v: serde_json::Value| v["id"].as_str().unwrap().to_string();
    let schema = id(made.execute("er.schema.add", &serde_json::json!({ "name": "shop" })).unwrap().value);
    let account = id(made.execute("er.entity.add", &serde_json::json!({ "schema": schema, "name": "Account" })).unwrap().value);
    for name in ["employee_id", "employee_id_2"] {
        made.execute("er.attribute.add", &serde_json::json!({ "entity": account, "name": name, "type": "UUID" })).unwrap();
    }
    let diagram =
        id(made.execute("diagram.add", &serde_json::json!({ "kind": archforge_er::DIAGRAM_KIND, "element": schema, "name": "Shop" })).unwrap().value);
    made.execute("diagram.place", &serde_json::json!({ "diagram": diagram, "element": account, "x": 0, "y": 0, "width": 240, "height": 72 }))
        .unwrap();
    // The packages as older projects saved them: at the top level, with no root package.
    let older = archforge_notations::session_from_rows_repaired(&snap(&made), ids(1_761_670_710_000), |st| {
        let model = st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_core::MODEL)).unwrap();
        for name in ["Commerce", "Billing"] {
            let p = st.create(archforge_uml::PACKAGE).unwrap();
            st.set(p, archforge_uml::named_element::NAME, archforge_mof::Value::str(name)).unwrap();
            st.push(model, archforge_core::model::MEMBER, archforge_mof::Value::Ref(p)).unwrap();
        }
    })
    .unwrap();
    let rows = snap(&older);
    // What the shell made of it before: the boxes fitted as the rows were read, then the root.
    // The shell's ids are the harness's, from the first.
    let shells = test_ids();
    let mut before = archforge_notations::session_from_rows_repaired(&rows, Box::new(move || shells()), er_place::fit_tables).unwrap();
    before.execute("uml.model.root", &serde_json::json!({})).unwrap();
    let opened = archforge_ui_egui::Opened { model: Some(std::sync::Arc::new(rows)), ..commerce() };
    let (h, _host) = saving_harness(Some(opened), None);
    let session = h.state().session.as_ref().expect("the older project opens");
    assert_eq!(snap(session), snap(&before), "the same model and the same boxes");
    assert_eq!((session.undo_label(), session.is_dirty()), (before.undo_label(), true), "the root package is one step, to be written");
    // Both steps were taken: the box shows both columns, and the packages sit in the root.
    let st = session.doc();
    let node = st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_diagram::NODE)).unwrap();
    assert_eq!(archforge_diagram::grid::bounds_of(st, node).height, er_place::content_height(2));
    let root = archforge_uml::commands::root_package(st).expect("the root is made on opening");
    let held: Vec<String> = archforge_uml::commands::subpackages(st, root).into_iter().map(|p| archforge_core::name_of(st, p)).collect();
    assert_eq!(held, ["Commerce", "Billing"]);
}

/// The Relationship tool armed with N-ary association draws by clicks on a class diagram only: not
/// on an ER diagram, whose own Relationship tool has the same key, and not with no diagram open
/// (AR-128).
#[test]
fn the_n_ary_tool_is_armed_on_a_class_diagram_only() {
    let (mut h, _) = related_sample_in(ThemeKind::Light, egui::vec2(1600.0, 1000.0));
    let class_diagram = h.state().canvas.active.expect("the class diagram is open");
    let er_diagram = {
        let s = h.state_mut().session.as_mut().unwrap();
        let shop = s.execute("er.schema.add", &serde_json::json!({ "name": "shop" })).unwrap().value["id"].as_str().unwrap().to_string();
        let d = s.execute("diagram.add", &serde_json::json!({ "kind": archforge_er::DIAGRAM_KIND, "element": shop, "name": "Shop" })).unwrap().value;
        archforge_mof::Uuid::parse(d["id"].as_str().unwrap()).unwrap()
    };
    let armed_on = |shell: &mut Shell, d: Option<archforge_mof::Uuid>| {
        shell.canvas.active = d;
        shell.canvas.tool = archforge_shell_api::canvas::Tool::Connect("relationship");
        uml_of_mut(shell).relation = uml_relate::Relate::NAry;
        uml_nary::armed(&archforge_ui_egui::testing::cx_ref(shell))
    };
    let shell = h.state_mut();
    assert!(armed_on(shell, Some(class_diagram)), "on the class diagram");
    assert!(!armed_on(shell, Some(er_diagram)), "not on an ER diagram");
    assert!(!armed_on(shell, None), "not with no diagram open");
}

/// The diagram's kind reads a press of the primary button first (AR-789): with UML's N-ary
/// association tool armed, a click on a card is the drawing's next end, a press that is no click
/// is still the drawing's (no line starts), and a click on empty canvas cancels it; with Select
/// while Add end waits, a click on a card gives the association its new end. With nothing of UML's
/// under way, and from ER's kind always, the press is left to the shared gestures. In light and
/// dark.
#[test]
fn the_kind_reads_a_press_first_and_ers_leaves_it() {
    use archforge_shell_api::kinds::DiagramKind;
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, ids) = nary_sample(theme, true);
        let shell = h.state_mut();
        let customer = node_here(shell, &ids["Customer"]);
        let ask = |shell: &Shell, kind: &dyn DiagramKind, node, clicked| {
            pressed_with(shell, node, None, clicked, |p| answered(kind.pointer(&archforge_ui_egui::testing::cx_ref(shell), p)))
        };
        assert_eq!(ask(shell, &uml_ui::UmlKind, Some(customer), true).0, "none", "{theme:?}: nothing under way");
        shell.canvas.tool = archforge_shell_api::canvas::Tool::Connect("relationship");
        uml_of_mut(shell).relation = uml_relate::Relate::NAry;
        assert_eq!(ask(shell, &er_kind::ErKind, Some(customer), true).0, "none", "{theme:?}: ER's kind leaves it");
        assert_eq!(ask(shell, &uml_ui::UmlKind, Some(customer), false).0, "nothing", "{theme:?}: no line starts");
        let (word, run) = ask(shell, &uml_ui::UmlKind, Some(customer), true);
        assert_eq!(word, "own", "{theme:?}");
        archforge_ui_egui::snapshot::cx(shell, |cx| run.unwrap()(cx));
        let ends = |shell: &Shell| match &uml_of(shell).nary {
            Some(uml_nary::Nary::Draft { ends, .. }) => ends.len(),
            _ => 0,
        };
        assert_eq!(ends(shell), 1, "{theme:?}: Customer is the first end");
        let (word, run) = ask(shell, &uml_ui::UmlKind, None, true);
        assert_eq!(word, "own", "{theme:?}");
        archforge_ui_egui::snapshot::cx(shell, |cx| run.unwrap()(cx));
        assert!(uml_of(shell).nary.is_none() && shell.canvas.tool == archforge_shell_api::canvas::Tool::Select, "{theme:?}: empty canvas cancels");
        // Add end waits on a class.
        let st = shell.session.as_ref().unwrap().doc();
        let association = st.alive().find(|o| archforge_uml::nary::is_nary(st, *o)).unwrap();
        let (association, diagram) = (st.uuid(association).unwrap(), shell.canvas.active.unwrap());
        uml_of_mut(shell).nary = Some(uml_nary::Nary::AddEnd { diagram, association });
        assert_eq!(ask(shell, &er_kind::ErKind, Some(customer), true).0, "none", "{theme:?}: ER's kind leaves it");
        assert_eq!(ask(shell, &uml_ui::UmlKind, Some(customer), false).0, "none", "{theme:?}: a drag may start");
        let (word, run) = ask(shell, &uml_ui::UmlKind, Some(customer), true);
        assert_eq!(word, "own", "{theme:?}");
        archforge_ui_egui::snapshot::cx(shell, |cx| run.unwrap()(cx));
        let st = shell.session.as_ref().unwrap().doc();
        let ends = archforge_uml::nary::ends(st, st.by_uuid(association).unwrap()).len();
        assert_eq!((ends, uml_of(shell).nary.is_none()), (4, true), "{theme:?}: a fourth end, at Customer");
    }
}

/// A right click means what the diagram's kind says first (AR-789): UML opens an n-ary
/// association's menu from its diamond, the association selected, and the set actions from a
/// generalization's line, the line selected first unless its generalization was; on a card it
/// leaves the click to the element menu, and ER's kind leaves every right click to it. In light
/// and dark.
#[test]
fn the_kind_reads_a_right_click_first_and_ers_leaves_it() {
    use archforge_shell_api::kinds::DiagramKind;
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, ids) = nary_sample(theme, true);
        let shell = h.state_mut();
        let st = shell.session.as_ref().unwrap().doc();
        let a = st.alive().find(|o| archforge_uml::nary::is_nary(st, *o)).unwrap();
        let a_id = st.uuid(a).unwrap();
        let diamond = node_here(shell, &a_id.to_string());
        let customer = node_here(shell, &ids["Customer"]);
        let ask = |shell: &Shell, kind: &dyn DiagramKind, node, line| {
            pressed_with(shell, node, line, false, |p| answered(kind.secondary_click(&archforge_ui_egui::testing::cx_ref(shell), p)))
        };
        assert_eq!(ask(shell, &er_kind::ErKind, Some(diamond), None).0, "none", "{theme:?}: ER's kind leaves it");
        assert_eq!(ask(shell, &uml_ui::UmlKind, Some(customer), None).0, "none", "{theme:?}: a card's menu is the element menu");
        let (word, run) = ask(shell, &uml_ui::UmlKind, Some(diamond), None);
        assert_eq!(word, "own", "{theme:?}");
        archforge_ui_egui::snapshot::cx(shell, |cx| run.unwrap()(cx));
        assert_eq!(uml_of(shell).nary_menu, Some((PRESSED_AT, a_id)), "{theme:?}: its menu, where pressed");
        let st = shell.session.as_ref().unwrap().doc();
        assert_eq!(shell.session.as_ref().unwrap().view().selection, vec![st.by_uuid(a_id).unwrap()], "{theme:?}: the association selected");
    }
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, _) = related_sample_in(theme, egui::vec2(1600.0, 1000.0));
        let (gens, _) = archforge_ui_egui::snapshot::cx(h.state_mut(), uml_ui::put_payment_kinds).unwrap();
        h.run();
        let shell = h.state_mut();
        let st = shell.session.as_ref().unwrap().doc();
        let g = st.by_uuid(archforge_mof::Uuid::parse(&gens[0]).unwrap()).unwrap();
        let line = archforge_diagram::views_of(st, g).1[0];
        let ask = |shell: &Shell, kind: &dyn DiagramKind| {
            pressed_with(shell, None, Some(line), false, |p| answered(kind.secondary_click(&archforge_ui_egui::testing::cx_ref(shell), p)))
        };
        assert_eq!(ask(shell, &er_kind::ErKind).0, "none", "{theme:?}: ER's kind leaves it");
        let (word, run) = ask(shell, &uml_ui::UmlKind);
        assert_eq!(word, "own", "{theme:?}");
        archforge_ui_egui::snapshot::cx(shell, |cx| run.unwrap()(cx));
        assert_eq!(uml_of(shell).line_menu, Some(PRESSED_AT), "{theme:?}: the set actions, where pressed");
        assert_eq!(shell.session.as_ref().unwrap().view().selection, vec![g], "{theme:?}: the line selected first");
        uml_of_mut(shell).line_menu = None;
        shell.session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": gens })).unwrap();
        let run = ask(shell, &uml_ui::UmlKind).1.unwrap();
        archforge_ui_egui::snapshot::cx(shell, |cx| run(cx));
        assert_eq!(shell.session.as_ref().unwrap().view().selection.len(), 3, "{theme:?}: a selection holding it is kept");
    }
}

/// A drawing under way on the diagram's kind takes its keys first (AR-789): UML's n-ary
/// association drawing takes Escape, which drops it; with nothing drawn UML leaves the keys to
/// the canvas, and ER's kind always does. In light and dark.
#[test]
fn the_kind_takes_its_drawings_keys_first_and_ers_takes_none() {
    use archforge_shell_api::kinds::DiagramKind;
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, ids) = nary_sample(theme, false);
        let shell = h.state_mut();
        let d = shell.session.as_ref().unwrap().doc().by_uuid(shell.canvas.active.unwrap()).unwrap();
        let escape = |shell: &mut Shell, kind: &dyn DiagramKind| {
            keyed_alone(shell, &[egui::Key::Escape], egui::Modifiers::NONE, |s, ui| archforge_ui_egui::snapshot::cx(s, |cx| kind.keys(cx, ui, d)))
        };
        assert!(!escape(shell, &uml_ui::UmlKind), "{theme:?}: nothing drawn");
        shell.canvas.tool = archforge_shell_api::canvas::Tool::Connect("relationship");
        uml_of_mut(shell).relation = uml_relate::Relate::NAry;
        let customer = node_here(shell, &ids["Customer"]);
        let node = shell.session.as_ref().unwrap().doc().uuid(customer).unwrap();
        archforge_ui_egui::snapshot::cx(shell, |cx| uml_nary::click(cx, d, node, false));
        assert!(uml_of(shell).nary.is_some(), "{theme:?}: a drawing under way");
        assert!(!escape(shell, &er_kind::ErKind), "{theme:?}: ER's kind takes none");
        assert!(uml_of(shell).nary.is_some(), "{theme:?}: and leaves the drawing");
        assert!(escape(shell, &uml_ui::UmlKind), "{theme:?}: UML takes Escape");
        assert!(uml_of(shell).nary.is_none(), "{theme:?}: the drawing dropped");
    }
}

/// A popover's menu holds the keys as the kind whose chevron it is says (AR-789): UML's
/// classifier kinds do, its relationship kinds do not, ER's relationship types do not; every kind
/// is asked, so the classifier kinds still hold the keys while an ER diagram shows, until the
/// toolbar closes them. In light and dark.
#[test]
fn a_popover_holds_the_keys_as_its_kind_says() {
    use archforge_shell_api::kinds::DiagramKind;
    let (uml, er) = (uml_ui::UmlKind, er_kind::ErKind);
    assert!(uml.popover_takes_keys(uml_classifier::KINDS));
    assert!(!uml.popover_takes_keys("relationship-kinds") && !uml.popover_takes_keys("relationship-types"));
    assert!(!er.popover_takes_keys("relationship-types") && !er.popover_takes_keys(uml_classifier::KINDS));
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, _, _) = diagram_harness_in(theme);
        let shell = h.state_mut();
        assert!(!archforge_shell_api::kinds::popover_takes_keys(archforge_ui_egui::testing::kinds(shell), &shell.canvas), "{theme:?}: none open");
        shell.canvas.popover = Some("relationship-types");
        assert!(!archforge_shell_api::kinds::popover_takes_keys(archforge_ui_egui::testing::kinds(shell), &shell.canvas), "{theme:?}");
        shell.canvas.popover = Some(uml_classifier::KINDS);
        assert!(
            archforge_shell_api::kinds::popover_takes_keys(archforge_ui_egui::testing::kinds(shell), &shell.canvas),
            "{theme:?}: UML's, over an ER diagram"
        );
    }
}

/// Select all takes in what the diagram's kind adds beside its boxes and lines (AR-789): UML the
/// generalization set its generalizations belong to, ER's kind nothing. In light and dark.
#[test]
fn select_all_takes_in_what_the_kind_adds() {
    use archforge_shell_api::kinds::DiagramKind;
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, _) = related_sample_in(theme, egui::vec2(1600.0, 1000.0));
        let (gens, _) = archforge_ui_egui::snapshot::cx(h.state_mut(), uml_ui::put_payment_kinds).unwrap();
        h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": gens })).unwrap();
        assert!(archforge_ui_egui::snapshot::cx(h.state_mut(), uml_sets::shortcut), "{theme:?}: a set made");
        let st = h.state().session.as_ref().unwrap().doc();
        let set = st.alive().find(|o| archforge_uml::generalization_sets::is_set(st, *o)).unwrap();
        let lines: Vec<archforge_mof::Obj> = gens.iter().map(|g| st.by_uuid(archforge_mof::Uuid::parse(g).unwrap()).unwrap()).collect();
        let class = st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_uml::CLASS)).unwrap();
        assert!(er_kind::ErKind.also_selects(st, &lines).is_empty(), "{theme:?}: ER's kind adds nothing");
        assert!(uml_ui::UmlKind.also_selects(st, &[class]).is_empty(), "{theme:?}: no generalization, no set");
        let added = uml_ui::UmlKind.also_selects(st, &lines);
        assert!(!added.is_empty() && added.iter().all(|o| *o == set), "{theme:?}: the set of its lines, {added:?}");
    }
}

/// Delete removes a notation's own selected element its own way, whatever the diagram (AR-789): ER
/// a relationship with its foreign key, without Shift; UML a generalization set, its
/// generalizations kept, Shift held or not; each leaves the other's element to it. In light and
/// dark.
#[test]
fn delete_removes_each_notations_own_selected_element_its_own_way() {
    use archforge_shell_api::notation_ui::NotationUi;
    let alive = |h: &Harness<'static, Shell>, o: archforge_mof::Obj| h.state().session.as_ref().unwrap().doc().is_alive(o);
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, _, _) = diagram_harness_in(theme);
        let (rel, fk) = {
            let s = h.state_mut().session.as_mut().unwrap();
            let st = s.doc();
            let rel = st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_er::RELATIONSHIP)).unwrap();
            let fk = st.referrers(rel).into_iter().find(|(_, p)| *p == archforge_er::foreign_key::RELATIONSHIP).map(|(f, _)| f).unwrap();
            let id = st.uuid(rel).unwrap().to_string();
            s.execute("view.select", &serde_json::json!({ "ids": [id] })).unwrap();
            (rel, fk)
        };
        assert!(!archforge_ui_egui::snapshot::cx(h.state_mut(), |cx| uml_ui::UmlUi.canvas_delete(cx, false)), "{theme:?}: UML leaves a relationship");
        assert!(
            !archforge_ui_egui::snapshot::cx(h.state_mut(), |cx| er_ui::ErUi.canvas_delete(cx, true)),
            "{theme:?}: Shift leaves it to the dialog"
        );
        assert!(alive(&h, rel), "{theme:?}");
        assert!(archforge_ui_egui::snapshot::cx(h.state_mut(), |cx| er_ui::ErUi.canvas_delete(cx, false)), "{theme:?}");
        assert!(!alive(&h, rel) && !alive(&h, fk), "{theme:?}: with its foreign key");
        assert_eq!(h.state().canvas.toast.as_ref().map(|t| t.text.as_str()), Some("places removed"), "{theme:?}");
    }
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, _) = related_sample_in(theme, egui::vec2(1600.0, 1000.0));
        let (gens, _) = archforge_ui_egui::snapshot::cx(h.state_mut(), uml_ui::put_payment_kinds).unwrap();
        h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": gens })).unwrap();
        assert!(archforge_ui_egui::snapshot::cx(h.state_mut(), uml_sets::shortcut), "{theme:?}: a set made, and selected");
        let st = h.state().session.as_ref().unwrap().doc();
        let set = st.alive().find(|o| archforge_uml::generalization_sets::is_set(st, *o)).unwrap();
        let g = st.by_uuid(archforge_mof::Uuid::parse(&gens[0]).unwrap()).unwrap();
        assert!(!archforge_ui_egui::snapshot::cx(h.state_mut(), |cx| er_ui::ErUi.canvas_delete(cx, false)), "{theme:?}: ER leaves a set");
        assert!(alive(&h, set), "{theme:?}");
        assert!(archforge_ui_egui::snapshot::cx(h.state_mut(), |cx| uml_ui::UmlUi.canvas_delete(cx, true)), "{theme:?}: Shift held too");
        assert!(!alive(&h, set) && alive(&h, g), "{theme:?}: its generalizations kept");
    }
}

/// A notation's chord on the selection is its own (AR-789): ⌥⌘G with generalizations selected
/// makes UML's set and takes the keys; ER's notation takes no chord; without the chord UML takes
/// nothing. In light and dark.
#[test]
fn a_notations_chord_on_the_selection_is_its_own() {
    use archforge_shell_api::notation_ui::NotationUi;
    let chord = egui::Modifiers::COMMAND | egui::Modifiers::ALT;
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, _) = related_sample_in(theme, egui::vec2(1600.0, 1000.0));
        let (gens, _) = archforge_ui_egui::snapshot::cx(h.state_mut(), uml_ui::put_payment_kinds).unwrap();
        h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": gens })).unwrap();
        let sets = |h: &Harness<'static, Shell>| {
            let st = h.state().session.as_ref().unwrap().doc();
            st.alive().filter(|o| archforge_uml::generalization_sets::is_set(st, *o)).count()
        };
        let shell = h.state_mut();
        assert!(
            !keyed_alone(shell, &[egui::Key::G], chord, |s, ui| archforge_ui_egui::snapshot::cx(s, |cx| er_ui::ErUi.canvas_keys(cx, ui))),
            "{theme:?}: ER takes none"
        );
        assert!(
            !keyed_alone(shell, &[egui::Key::G], egui::Modifiers::COMMAND, |s, ui| archforge_ui_egui::snapshot::cx(s, |cx| uml_ui::UmlUi
                .canvas_keys(cx, ui))),
            "{theme:?}"
        );
        assert_eq!(sets(&h), 0, "{theme:?}");
        assert!(
            keyed_alone(h.state_mut(), &[egui::Key::G], chord, |s, ui| archforge_ui_egui::snapshot::cx(s, |cx| uml_ui::UmlUi.canvas_keys(cx, ui))),
            "{theme:?}: ⌥⌘G"
        );
        assert_eq!(sets(&h), 1, "{theme:?}: a set made");
    }
}

/// What a press and Escape on the canvas close is each notation's own (AR-789): a press closes
/// UML's menus of generalization lines and of an n-ary association, Escape ER's mapping table
/// offer; neither closes the other's.
#[test]
fn a_press_and_escape_close_each_notations_own() {
    use archforge_shell_api::notation_ui::NotationState;
    let id = archforge_mof::Uuid([7; 16]);
    let mut uml = uml_state::UmlState { line_menu: Some(PRESSED_AT), nary_menu: Some((PRESSED_AT, id)), ..Default::default() };
    let offer = || Some(er_connect::MappingOffer { relationship: id, name: "CustomerOrder".into(), refused: None });
    let mut er = er_state::ErState { mapping: offer(), ..Default::default() };
    uml.canvas_escape();
    er.canvas_pressed();
    assert!(uml.line_menu.is_some() && uml.nary_menu.is_some() && er.mapping.is_some(), "neither closes the other's");
    uml.canvas_pressed();
    er.canvas_escape();
    assert!(uml.line_menu.is_none() && uml.nary_menu.is_none() && er.mapping.is_none());
}

/// The card typed on is drawn with its row as UML's state says (AR-789): none while nothing is
/// typed, the class, its attributes' compartment and a new row once a member is typed on Catalog's
/// card; ER's state has no row of its own; the shell's states answer as UML's does. In light and
/// dark.
#[test]
fn a_card_draft_is_the_notations_row_typed_in_place() {
    use archforge_shell_api::notation_ui::NotationState;
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, nodes) = related_sample_in(theme, egui::vec2(1600.0, 1000.0));
        let (class, catalog) = {
            let st = h.state().session.as_ref().unwrap().doc();
            let node = st.by_uuid(archforge_mof::Uuid::parse(&nodes["Catalog"]).unwrap()).unwrap();
            let class = st.get(node, archforge_diagram::node::ELEMENT).unwrap().as_ref().unwrap();
            (st.uuid(class).unwrap(), class)
        };
        let st = h.state().session.as_ref().unwrap().doc();
        assert_eq!(uml_of(h.state()).card_draft(st), None, "{theme:?}");
        assert!(
            archforge_ui_egui::snapshot::cx(h.state_mut(), |cx| uml_inline::open(cx, class, uml_panel::MemberKind::Attribute, None)),
            "{theme:?}"
        );
        let st = h.state().session.as_ref().unwrap().doc();
        assert_eq!(uml_of(h.state()).card_draft(st), Some((catalog, 0, None)), "{theme:?}: a new attribute on Catalog");
        assert_eq!(er_of(h.state()).card_draft(st), None, "{theme:?}: ER types no row on a card");
        assert_eq!(archforge_ui_egui::testing::notation_states_of(h.state()).card_draft(st), Some((catalog, 0, None)), "{theme:?}");
    }
}

/// A problem badge that offers a way out says and does what its element's notation answers
/// (AR-789): ER's many-to-many relationship offers its mapping table and a click opens the offer;
/// UML offers none. In light and dark.
#[test]
fn a_resolvable_badge_says_and_does_what_its_notation_answers() {
    use archforge_shell_api::notation_ui::NotationUi;
    assert_eq!(er_ui::ErUi.resolve_hint().as_deref(), Some("Many to many · add a mapping table"));
    assert_eq!(uml_ui::UmlUi.resolve_hint(), None);
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, _, _) = diagram_harness_in(theme);
        let rel = {
            let st = h.state().session.as_ref().unwrap().doc();
            st.uuid(st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_er::RELATIONSHIP)).unwrap()).unwrap()
        };
        archforge_ui_egui::snapshot::cx(h.state_mut(), |cx| uml_ui::UmlUi.resolve(cx, rel));
        assert!(er_of(h.state()).mapping.is_none(), "{theme:?}: UML opens nothing");
        archforge_ui_egui::snapshot::cx(h.state_mut(), |cx| er_ui::ErUi.resolve(cx, rel));
        assert_eq!(er_of(h.state()).mapping.as_ref().map(|m| m.relationship), Some(rel), "{theme:?}: ER opens the offer");
    }
}

/// The words each notation's figures say reach the catalogue tests through that notation's own
/// list (AR-789): ER's and UML's, and no longer the canvas's.
#[test]
fn each_notations_figure_words_are_its_own() {
    assert!(archforge_er::figures::WORDS.iter().all(|w| er_ui::texts().any(|t| t == *w)));
    assert!(archforge_uml::WORDS.iter().all(|w| uml_ui::texts().any(|t| t == *w)));
    assert!(canvas_words().all(|t| t != "Entity · {schema}" && t != "Class · {package}"), "the canvas names neither");
}
