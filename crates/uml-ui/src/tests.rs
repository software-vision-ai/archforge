//! UML's screens tested without the shell (ADR 0005, "Tests follow the plugin"): what UML's own
//! code draws or decides, driven through UML's interfaces over the shell's interfaces' test
//! context (`archforge_shell_api::testing`). A test opens a project's session in a context given
//! UML alone, runs commands through the narrow context as the shell's panels do, and asks UML's
//! hooks or draws UML's Properties body in a harness, as the shell draws it under the heading of
//! the element selected.
//!
//! What the shell does around UML's screens (its canvas, Models, menus, Chat, the Properties
//! panel's scroll and keys) is tested with the whole window in the app.

use archforge_mof::{ClassId, Uuid};
use archforge_shell_api::cx::Ctx;
use archforge_shell_api::i18n::set_language;
use archforge_shell_api::notation_ui::{NotationUi, Subject};
use archforge_shell_api::testing::{self, TestContext, click};
use egui_kittest::Harness;

use crate::uml_state::UmlState;
use crate::uml_ui::UmlUi;

/// UML alone, as a test context is given it.
static UIS: &[&dyn NotationUi] = &[&UmlUi];

/// The width of the Properties panel, as the shell first gives it.
const PANEL_WIDTH: f32 = 280.0;

/// The two appearances a screen is drawn in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Appearance {
    Light,
    Dark,
}

/// Ids for a test's model: version 7 from a counter, no clock.
fn test_ids() -> Box<dyn FnMut() -> Uuid + Send> {
    let mut n: u64 = 0;
    Box::new(move || {
        n += 1;
        let mut r = [0u8; 10];
        r[2..].copy_from_slice(&n.to_be_bytes());
        Uuid::v7(1_761_670_800_000, r)
    })
}

/// A test context given UML, in English, with a new project's session open, as the shell opens
/// one.
fn context() -> TestContext {
    set_language("en");
    let mut tc = TestContext::new(UIS).unwrap();
    tc.session = Some(archforge_notations::new_project_session("Commerce platform", test_ids()).unwrap());
    tc
}

/// Runs the command `cmd` through the narrow context, as the shell's panels do: a refusal is said
/// as the panel says it.
fn run(tc: &mut TestContext, cmd: &str, params: serde_json::Value) -> Result<serde_json::Value, String> {
    tc.cx(|cx: &mut Ctx| archforge_shell_api::panels::run_cx(cx, cmd, params))
}

/// The id of the live element of `class` named `name`.
fn named(tc: &TestContext, class: ClassId, name: &str) -> Uuid {
    let st = tc.session.as_ref().unwrap().doc();
    let o = st.alive().find(|o| st.class_of(*o).ok() == Some(class) && archforge_core::name_of(st, *o) == name).expect(name);
    st.uuid(o).unwrap()
}

/// The session's revision.
fn revision(tc: &TestContext) -> u64 {
    tc.session.as_ref().unwrap().revision()
}

/// UML's own state in the context.
fn uml(tc: &TestContext) -> &UmlState {
    crate::uml_state::uml(&tc.notation_states).expect("the context's notations hold UML")
}

/// UML's own state in the context, to change.
fn uml_mut(tc: &mut TestContext) -> &mut UmlState {
    crate::uml_state::uml_mut(&mut tc.notation_states).expect("the context's notations hold UML")
}

/// Opens the diagram `d` through the narrow context, as the shell's canvas opens one.
fn open(tc: &mut TestContext, d: Uuid) {
    tc.cx(|cx| archforge_shell_api::canvas::open_cx(cx, d));
}

/// Selects the element `id`, as a click on it in Models or on the canvas selects it.
fn select(tc: &mut TestContext, id: &str) {
    tc.session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [id] })).unwrap();
}

/// UML's Properties for the element selected in `tc`, drawn in a harness of `size` in English and
/// the appearance `theme`, as the shell draws it: each frame UML is told of the selection, then
/// draws its own panel in place of the selection's when it has one, else the body of the element
/// selected, in a column of the panel's width at the window's right, from its top. The shell's
/// heading, description and scroll around the body are not drawn: the harness is tall enough for
/// the whole body.
fn properties(tc: TestContext, theme: Appearance, size: egui::Vec2) -> Harness<'static, TestContext> {
    let mut h = testing::harness(tc, "en", theme == Appearance::Dark, size, |cx, ui, p| {
        let full = ui.max_rect();
        let r = egui::Rect::from_min_max(egui::pos2(full.right() - PANEL_WIDTH, full.top()), full.max);
        let selected = cx.session.as_ref().and_then(|s| s.view().selection.first().copied().filter(|o| s.doc().is_alive(*o)));
        let id = selected.and_then(|o| cx.session.as_ref().and_then(|s| s.doc().uuid(o).ok()));
        UmlUi.selection_moved(cx, id);
        if UmlUi.properties_in_place(cx, ui, p, r, 0.0) {
            return;
        }
        let subject = cx.session.as_ref().zip(selected).and_then(|(s, o)| {
            let st = s.doc();
            Some(Subject::of(o, st.uuid(o).ok(), st.class_of(o).ok(), UmlUi.words(st, o)?))
        });
        if let Some(subject) = subject {
            UmlUi.body(cx, ui, p, r, 0.0, &subject);
        }
    })
    .unwrap();
    h.run();
    h
}

/// The automation ids of the controls the last frame drew, in the order drawn.
fn ids(h: &Harness<'static, TestContext>) -> Vec<String> {
    h.state().registry.entries().iter().map(|e| e.id.clone()).collect()
}

/// Where the last frame drew the control `id`.
fn rect_of(h: &Harness<'static, TestContext>, id: &str) -> egui::Rect {
    let e = h.state().registry.entries().iter().find(|e| e.id == id).unwrap_or_else(|| panic!("no control {id}"));
    egui::Rect::from_min_max(egui::pos2(e.rect[0], e.rect[1]), egui::pos2(e.rect[2], e.rect[3]))
}

/// A press and release of `key` with `modifiers`, and the frames they start.
fn key(h: &mut Harness<'static, TestContext>, key: egui::Key, modifiers: egui::Modifiers) {
    h.key_down_modifiers(modifiers, key);
    h.key_up_modifiers(modifiers, key);
    h.run();
}

/// Every text the last frame painted, with the box its galley took, in the order painted.
fn text_boxes(h: &Harness<'static, TestContext>) -> Vec<(String, egui::Rect)> {
    /// The texts of the shape `s` and of the shapes it holds.
    fn walk(s: &egui::Shape, out: &mut Vec<(String, egui::Rect)>) {
        match s {
            egui::Shape::Text(t) => out.push((t.galley.text().to_string(), egui::Rect::from_min_size(t.pos, t.galley.size()))),
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    h.output().shapes.iter().for_each(|c| walk(&c.shape, &mut out));
    out
}

/// Every text the last frame painted, in the order painted.
fn painted(h: &Harness<'static, TestContext>) -> Vec<String> {
    text_boxes(h).into_iter().map(|(t, _)| t).collect()
}

/// Clicks the field `id`, selects what it holds and types `text` over it.
fn retype(h: &mut Harness<'static, TestContext>, id: &str, text: &str) {
    let at = rect_of(h, id).center();
    click(h, at);
    h.event(egui::Event::Key { key: egui::Key::A, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::COMMAND });
    h.run();
    if text.is_empty() {
        key(h, egui::Key::Backspace, egui::Modifiers::NONE);
    } else {
        h.event(egui::Event::Text(text.into()));
        h.run();
    }
}

/// Picks the type labelled `label` from the open editor's type select `id`.
fn pick_type(h: &mut Harness<'static, TestContext>, id: &str, label: &str) {
    let at = rect_of(h, id).center();
    click(h, at);
    let i = crate::uml_panel::type_choices(h.state().session.as_ref().unwrap().doc(), None).iter().position(|(l, _)| l == label).expect(label);
    let at = rect_of(h, &format!("{id}.{i}")).center();
    click(h, at);
}

/// Selects the live element of `class` named `name`, and gives its id.
fn select_named(h: &mut Harness<'static, TestContext>, class: ClassId, name: &str) -> Uuid {
    let id = named(h.state(), class, name);
    select(h.state_mut(), &id.to_string());
    h.run();
    id
}

/// A new project's session in a test context given UML, its Properties drawn in a harness of
/// `size` in the appearance `theme`.
fn project_harness(theme: Appearance, size: egui::Vec2) -> Harness<'static, TestContext> {
    properties(context(), theme, size)
}

/// The sample class model open, with the node of each class on its diagram by name.
fn related_sample() -> (Harness<'static, TestContext>, std::collections::BTreeMap<String, String>) {
    related_sample_in(Appearance::Light, egui::vec2(1600.0, 1000.0))
}

/// [`related_sample`] in `theme` in a window of `size`.
fn related_sample_in(theme: Appearance, size: egui::Vec2) -> (Harness<'static, TestContext>, std::collections::BTreeMap<String, String>) {
    let mut h = project_harness(theme, size);
    h.state_mut().cx(crate::uml_ui::open_sample).unwrap();
    h.run();
    let st = h.state().session.as_ref().unwrap().doc();
    let d = st.by_uuid(h.state().canvas.active.unwrap()).unwrap();
    let nodes = st
        .list(d, archforge_diagram::diagram::NODE)
        .unwrap()
        .iter()
        .filter_map(archforge_mof::Value::as_ref)
        .filter_map(|n| {
            let c = st.get(n, archforge_diagram::node::ELEMENT).ok()?.as_ref()?;
            Some((archforge_core::name_of(st, c), st.uuid(n).ok()?.to_string()))
        })
        .collect();
    (h, nodes)
}

/// The sample class model in `theme` with two enumerations, OrderStatus and ClosingStatus, placed in
/// its empty bottom row: the harness, every card's node by name, and the enumerations' nodes.
fn enumeration_sample(theme: Appearance) -> (Harness<'static, TestContext>, std::collections::BTreeMap<String, String>, [String; 2]) {
    let (mut h, nodes) = related_sample_in(theme, egui::vec2(1600.0, 1000.0));
    let d = h.state().canvas.active.unwrap().to_string();
    let s = h.state_mut().session.as_mut().unwrap();
    let st = s.doc();
    let commerce =
        st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_uml::PACKAGE) && archforge_core::name_of(st, *o) == "Commerce").unwrap();
    let commerce = st.uuid(commerce).unwrap().to_string();
    let mut place = |name: &str, x: i32| {
        let id = s.execute("uml.enumeration.add", &serde_json::json!({ "package": commerce, "name": name })).unwrap().value["id"]
            .as_str()
            .unwrap()
            .to_string();
        let n =
            s.execute("diagram.place", &serde_json::json!({ "diagram": d, "element": id, "x": x, "y": 1000, "width": 290, "height": 136 })).unwrap();
        n.value["id"].as_str().unwrap().to_string()
    };
    let enumerations = [place("OrderStatus", 560), place("ClosingStatus", 1040)];
    h.run();
    (h, nodes, enumerations)
}

/// The id of the literal of `enumeration` named `name`.
fn literal_named(h: &Harness<'static, TestContext>, enumeration: archforge_mof::Uuid, name: &str) -> archforge_mof::Uuid {
    let st = h.state().session.as_ref().unwrap().doc();
    let l = archforge_uml::commands::literals(st, st.by_uuid(enumeration).unwrap()).into_iter().find(|l| archforge_core::name_of(st, *l) == name);
    st.uuid(l.expect(name)).unwrap()
}

/// The names of the enumeration `enumeration`'s literals, in their order.
fn literal_names(h: &Harness<'static, TestContext>, enumeration: archforge_mof::Uuid) -> Vec<String> {
    let st = h.state().session.as_ref().unwrap().doc();
    archforge_uml::commands::literals(st, st.by_uuid(enumeration).unwrap()).into_iter().map(|l| archforge_core::name_of(st, l)).collect()
}

/// [`enumeration_sample`] with OrderStatus holding `literals` and selected: the harness, every
/// card's node by name, OrderStatus and its node.
fn literal_sample(
    theme: Appearance,
    literals: &[&str],
) -> (Harness<'static, TestContext>, std::collections::BTreeMap<String, String>, archforge_mof::Uuid, String) {
    let (mut h, nodes, [status_node, _]) = enumeration_sample(theme);
    let status = named(h.state(), archforge_uml::ENUMERATION, "OrderStatus");
    if !literals.is_empty() {
        h.state_mut().cx(|cx| crate::uml_ui::put_literals(cx, status, literals)).unwrap();
    }
    h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [status.to_string()] })).unwrap();
    h.run();
    (h, nodes, status, status_node)
}

/// The multiplicity of each end of `a` in the model, and as its line writes it at each end.
fn end_multiplicities(h: &Harness<'static, TestContext>, a: archforge_mof::Uuid) -> ([String; 2], [String; 2]) {
    let st = h.state().session.as_ref().unwrap().doc();
    let a = st.by_uuid(a).unwrap();
    let model = archforge_uml::figures::member_ends(st, a).unwrap().map(|e| archforge_uml::figures::multiplicity(st, e));
    let line = archforge_uml::figures::line(st, a).unwrap();
    (model, line.ends.map(|e| e.text.unwrap_or_default()))
}

/// The sample class model in `theme` and a window of `size`, its association "paid by" (Order to
/// Payment) selected and its Properties shown; its id.
fn paid_by(theme: Appearance, size: egui::Vec2) -> (Harness<'static, TestContext>, archforge_mof::Uuid) {
    let (mut h, _) = related_sample_in(theme, size);
    let paid = {
        let st = h.state().session.as_ref().unwrap().doc();
        let a =
            st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_uml::ASSOCIATION) && archforge_core::name_of(st, *o) == "paid by").unwrap();
        st.uuid(a).unwrap()
    };
    h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [paid.to_string()] })).unwrap();
    h.run();
    (h, paid)
}

/// The multiplicity field of end `i` as typed.
fn typed_multiplicity(h: &Harness<'static, TestContext>, i: usize) -> String {
    uml(h.state()).ends.as_ref().unwrap().multiplicity[i].clone()
}

/// The qualifier box's rows the association's line draws at its two ends.
fn qualifier_rows(h: &Harness<'static, TestContext>, a: &str) -> [Vec<String>; 2] {
    let st = h.state().session.as_ref().unwrap().doc();
    let l = archforge_uml::figures::line(st, st.by_uuid(archforge_mof::Uuid::parse(a).unwrap()).unwrap()).unwrap();
    [l.ends[0].qualifiers.clone(), l.ends[1].qualifiers.clone()]
}

/// Order and OrderLine joined by `contains`, its OrderLine end `line` at 0..1, on two class
/// diagrams of the package Shop, the first open and the association selected with Properties
/// shown (AR-127). Gives the ids by name: Order, OrderLine, contains, diagram, second.
fn qualifiers_sample(theme: Appearance) -> (Harness<'static, TestContext>, std::collections::BTreeMap<&'static str, String>) {
    let mut h = project_harness(theme, egui::vec2(1600.0, 1500.0));
    let mut ids = std::collections::BTreeMap::new();
    {
        let s = h.state_mut().session.as_mut().unwrap();
        let mut run = |cmd: &str, p: serde_json::Value| s.execute(cmd, &p).unwrap().value["id"].as_str().unwrap_or_default().to_string();
        let pkg = run("uml.package.add", serde_json::json!({ "name": "Shop" }));
        let order = run("uml.class.add", serde_json::json!({ "package": pkg, "name": "Order" }));
        let line = run("uml.class.add", serde_json::json!({ "package": pkg, "name": "OrderLine" }));
        let contains = run("uml.association.add", serde_json::json!({ "from": order, "to": line, "name": "contains" }));
        run("uml.association.set-end", serde_json::json!({ "association": contains, "end": 0, "role": "order", "multiplicity": "1" }));
        run("uml.association.set-end", serde_json::json!({ "association": contains, "end": 1, "role": "line", "multiplicity": "0..1" }));
        let mut diagrams = Vec::new();
        for (name, x) in [("Shop", 600), ("Overview", 120)] {
            let d = run("diagram.add", serde_json::json!({ "kind": archforge_uml::STRUCTURE_DIAGRAM, "element": pkg, "name": name }));
            for (element, cx) in [(&order, 80), (&line, x)] {
                run("diagram.place", serde_json::json!({ "diagram": d, "element": element, "x": cx, "y": if cx == 80 { 120 } else { 420 } }));
            }
            run("diagram.connect-all", serde_json::json!({ "diagram": d }));
            diagrams.push(d);
        }
        for (k, v) in
            [("Order", order), ("OrderLine", line), ("contains", contains.clone()), ("diagram", diagrams[0].clone()), ("second", diagrams[1].clone())]
        {
            ids.insert(k, v);
        }
    }
    open(h.state_mut(), archforge_mof::Uuid::parse(&ids["diagram"]).unwrap());
    h.run();
    h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [ids["contains"]] })).unwrap();
    h.run();
    (h, ids)
}

/// The sample class model's Recommendations diagram (AR-128): Customer and Product side by side,
/// Catalog under them; `drawn`, the n-ary association recommends between them. In `theme`, in a
/// window tall enough for an end's editor. Gives the ids by name, as `put_recommends` does.
fn nary_sample(theme: Appearance, drawn: bool) -> (Harness<'static, TestContext>, std::collections::BTreeMap<&'static str, String>) {
    nary_sample_in(theme, drawn, egui::vec2(1600.0, 1200.0))
}

/// [`nary_sample`] in a window of `size`.
fn nary_sample_in(
    theme: Appearance,
    drawn: bool,
    size: egui::Vec2,
) -> (Harness<'static, TestContext>, std::collections::BTreeMap<&'static str, String>) {
    let mut h = project_harness(theme, size);
    h.state_mut().cx(crate::uml_ui::open_sample).unwrap();
    h.run();
    let ids = h.state_mut().cx(|cx| crate::uml_nary::put_recommends(cx, drawn)).unwrap();
    h.run();
    (h, ids)
}

/// The n-ary associations of the model, each with its ends as `role : Class [multiplicity]`.
fn naries(h: &Harness<'static, TestContext>) -> Vec<(archforge_mof::Obj, Vec<String>)> {
    let st = h.state().session.as_ref().unwrap().doc();
    st.alive()
        .filter(|o| archforge_uml::nary::is_nary(st, *o))
        .map(|a| {
            let ends = archforge_uml::nary::ends(st, a)
                .into_iter()
                .map(|e| {
                    let ty = archforge_uml::nary::end_type(st, e).map(|t| archforge_core::name_of(st, t)).unwrap_or_default();
                    format!("{} : {ty} [{}]", archforge_core::name_of(st, e), archforge_uml::figures::multiplicity(st, e))
                })
                .collect();
            (a, ends)
        })
        .collect()
}

/// The legs of n-ary association ends on diagram `d` (the active one when none is given).
fn legs_on(h: &Harness<'static, TestContext>, d: Option<&str>) -> usize {
    let st = h.state().session.as_ref().unwrap().doc();
    let d = match d {
        Some(d) => st.by_uuid(archforge_mof::Uuid::parse(d).unwrap()).unwrap(),
        None => st.by_uuid(h.state().canvas.active.unwrap()).unwrap(),
    };
    st.list(d, archforge_diagram::diagram::EDGE)
        .unwrap()
        .iter()
        .filter_map(archforge_mof::Value::as_ref)
        .filter(|e| {
            st.get(*e, archforge_diagram::edge::ELEMENT)
                .ok()
                .and_then(|v| v.as_ref())
                .is_some_and(|x| archforge_uml::nary::association_of_end(st, x).is_some())
        })
        .count()
}

/// The diamonds on the active diagram.
fn diamonds_here(h: &Harness<'static, TestContext>) -> usize {
    let st = h.state().session.as_ref().unwrap().doc();
    let d = st.by_uuid(h.state().canvas.active.unwrap()).unwrap();
    st.list(d, archforge_diagram::diagram::NODE)
        .unwrap()
        .iter()
        .filter_map(archforge_mof::Value::as_ref)
        .filter(|n| st.get(*n, archforge_diagram::node::ELEMENT).ok().and_then(|v| v.as_ref()).is_some_and(|e| archforge_uml::nary::is_nary(st, e)))
        .count()
}

/// A package's own class diagram, made as a workflow makes one (AR-581): by `diagram.add`, named
/// like the package and its default from the start, the selection left as it was.
fn own_diagram(s: &mut archforge_notations::Session, package: archforge_mof::Uuid) -> archforge_mof::Uuid {
    let st = s.doc();
    let name = archforge_core::name_of(st, st.by_uuid(package).unwrap());
    let kept: Vec<String> = s.view().selection.iter().map(|o| st.uuid(*o).unwrap().to_string()).collect();
    let p = serde_json::json!({ "kind": archforge_uml::STRUCTURE_DIAGRAM, "element": package.to_string(), "name": name });
    let d = archforge_mof::Uuid::parse(s.execute("diagram.add", &p).unwrap().value["id"].as_str().unwrap()).unwrap();
    s.execute("view.select", &serde_json::json!({ "ids": kept })).unwrap();
    d
}

/// A relationship rule broken is said in the chosen language in the canvas's words, never in the
/// engine's English: a generalization's kinds, a final general one by its name, a repeat and a
/// cycle, a realization's ends, an association's and an n-ary one's end at an enumeration or a
/// primitive type, an association class's end at an interface and at itself, and a role another
/// end has (AR-436).
#[test]
fn a_relationship_rule_broken_is_said_in_the_chosen_language() {
    use archforge_shell_api::i18n::{fmt, tr, tr_ctx};
    use serde_json::json;
    let tc = &mut context();
    set_language("de");
    let mut id = |cmd: &str, p: serde_json::Value| run(tc, cmd, p).unwrap()["id"].as_str().unwrap().to_string();
    let sales = id("uml.package.add", json!({ "name": "Sales" }));
    let [order, invoice] = ["Order", "Invoice"].map(|n| id("uml.class.add", json!({ "package": sales, "name": n })));
    let priced = id("uml.interface.add", json!({ "package": sales, "name": "Priced" }));
    let tier = id("uml.enumeration.add", json!({ "package": sales, "name": "Tier" }));
    let sku = id("uml.primitive-type.add", json!({ "package": sales, "name": "Sku" }));
    let sealed = id("uml.class.add", json!({ "package": sales, "name": "Sealed" }));
    id("uml.classifier.set", json!({ "classifier": sealed, "final": true }));
    id("uml.generalization.add", json!({ "specific": invoice, "general": order }));
    let quote = id("uml.association-class.add", json!({ "from": order, "to": invoice, "name": "Quote" }));
    let line = id("uml.association.add", json!({ "from": order, "to": invoice }));
    id("uml.association.set-end", json!({ "association": line, "end": 0, "role": "buyer" }));
    let named = |key: &str, name: &str| fmt(&tr("There is {what} named {name} already."), &[("what", &tr_ctx("refusal", key)), ("name", name)]);
    let final_words = fmt(&tr("{name} is final: nothing may specialise it. Clear \"No subclasses\" in its Properties first."), &[("name", "Sealed")]);
    for (cmd, params, said) in [
        ("uml.generalization.add", json!({ "specific": priced, "general": order }), tr("An interface specializes only an interface")),
        ("uml.generalization.add", json!({ "specific": order, "general": tier }), tr("Only an enumeration specializes an enumeration")),
        ("uml.generalization.add", json!({ "specific": order, "general": priced }), tr("A class cannot specialize an interface")),
        ("uml.generalization.add", json!({ "specific": order, "general": sealed }), final_words),
        ("uml.generalization.add", json!({ "specific": invoice, "general": order }), tr("This one specializes that one already")),
        (
            "uml.generalization.add",
            json!({ "specific": order, "general": invoice }),
            tr("That one specializes this one already, which would make a cycle"),
        ),
        ("uml.realization.add", json!({ "class": priced, "interface": priced }), tr("Only a class implements an interface")),
        ("uml.realization.add", json!({ "class": order, "interface": invoice }), tr("A class realizes only an interface")),
        ("uml.association.add", json!({ "from": order, "to": tier }), tr("An enumeration is a data type and joins no association")),
        ("uml.association.add-nary", json!({ "classes": [order, invoice, sku] }), tr("A primitive type joins no association")),
        ("uml.association-class.add", json!({ "from": priced, "to": order }), tr("An association class joins only classes")),
        (
            "uml.relationship.reconnect",
            json!({ "relationship": quote, "end": "target", "class": quote }),
            tr("An association class does not join itself"),
        ),
        ("uml.association.set-end", json!({ "association": line, "end": 1, "role": "BUYER" }), named("an association end", "buyer")),
    ] {
        let refused = run(tc, cmd, params).err().unwrap_or_default();
        assert_eq!(refused, said, "{cmd}: in German");
        assert!(!refused.is_empty() && !refused.contains(" only ") && !refused.contains(" already"), "{cmd}: not English: {refused}");
    }
    set_language("en");
}

/// A template binding rule broken is said in the chosen language in the canvas's words, never in
/// the engine's English: an enumeration binding, a class with no parameter as the template, a data
/// type binding a class template, a template binding itself, a second binding and a cycle; on the
/// canvas in the same words (AR-436).
#[test]
fn a_binding_rule_broken_is_said_in_the_chosen_language() {
    use archforge_shell_api::i18n::tr;
    use serde_json::json;
    let tc = &mut context();
    set_language("de");
    let mut id = |cmd: &str, p: serde_json::Value| run(tc, cmd, p).unwrap()["id"].as_str().unwrap().to_string();
    let sales = id("uml.package.add", json!({ "name": "Sales" }));
    let [repository, order, entity] = ["Repository", "Order", "Entity"].map(|n| id("uml.class.add", json!({ "package": sales, "name": n })));
    let status = id("uml.enumeration.add", json!({ "package": sales, "name": "Status" }));
    let money = id("uml.data-type.add", json!({ "package": sales, "name": "Money" }));
    id("uml.template-parameter.add", json!({ "classifier": repository, "name": "T" }));
    id("uml.template-parameter.add", json!({ "classifier": order, "name": "U" }));
    let bind = |bound: &str, template: &str| json!({ "bound": bound, "template": template, "arguments": { "T": entity } });
    id("uml.template-binding.add", bind(&order, &repository));
    for (params, said) in [
        (bind(&status, &repository), tr("Only a class, an interface or a data type binds a template")),
        (bind(&entity, &money), tr("A binding ends at a template: a class, an interface or a data type with template parameters")),
        (bind(&money, &repository), tr("A template is bound by a classifier of its own kind")),
        (bind(&repository, &repository), tr("A template does not bind itself")),
        (bind(&order, &repository), tr("This one binds that template already")),
        (bind(&repository, &order), tr("That template binds this one already, which would make a cycle")),
    ] {
        let st = tc.session.as_ref().unwrap().doc();
        let at = |key: &str| st.by_uuid(archforge_mof::Uuid::parse(params[key].as_str().unwrap()).unwrap()).unwrap();
        let drawn = crate::uml_bindings::refusal(tc.notation_uis, st, at("bound"), at("template"), None);
        assert_eq!(drawn.as_ref(), Some(&said), "the canvas says it as the engine's refusal is said");
        let refused = run(tc, "uml.template-binding.add", params).err().unwrap_or_default();
        assert_eq!(refused, said, "in German");
        assert!(!refused.is_empty() && !refused.contains(" template") && !refused.contains(" already"), "not English: {refused}");
    }
    set_language("en");
}

/// A binding argument rule broken is said in the chosen language in the binding editor's words,
/// never in the engine's English: an argument that does not fit, one missing and two missing, both
/// named with their kinds; and, as said, a constrained parameter's misfit and a built-in type's
/// name taken (AR-442).
#[test]
fn an_argument_rule_broken_is_said_in_the_chosen_language() {
    use archforge_engine::{Parameter, Reason};
    use archforge_shell_api::i18n::{fmt, tr, tr_ctx};
    use serde_json::json;
    let tc = &mut context();
    set_language("de");
    let mut id = |cmd: &str, p: serde_json::Value| run(tc, cmd, p).unwrap()["id"].as_str().unwrap().to_string();
    let sales = id("uml.package.add", json!({ "name": "Sales" }));
    let [repository, entity] = ["Repository", "Entity"].map(|n| id("uml.class.add", json!({ "package": sales, "name": n })));
    let money = id("uml.data-type.add", json!({ "package": sales, "name": "Money" }));
    id("uml.template-parameter.add", json!({ "classifier": repository, "name": "T" }));
    id("uml.template-parameter.add", json!({ "classifier": repository, "name": "K", "kind": "data-type" }));
    let bind = |arguments: serde_json::Value| json!({ "bound": entity, "template": repository, "arguments": arguments });
    let named = |name: &str, kind: &str| fmt(&tr("{name} ({kind})"), &[("name", name), ("kind", &tr(kind))]);
    let both = fmt(&tr("{names} and {last}"), &[("names", &named("T", "class")), ("last", &named("K", "data type"))]);
    let each = "{names} have no default, so each needs an argument.";
    assert_ne!(tr(each), each, "the two missing are said in German");
    for (params, said) in [
        (bind(json!({ "T": money, "K": money })), fmt(&tr("Arguments for {name} must be classes."), &[("name", "T")])),
        (bind(json!({ "K": money })), fmt(&tr("{name} needs an argument: pick a class."), &[("name", "T")])),
        (bind(json!({})), fmt(&tr(each), &[("names", &both)])),
    ] {
        let refused = run(tc, "uml.template-binding.add", params).err().unwrap_or_default();
        assert_eq!(refused, said, "in German");
    }
    let taken = Reason::BuiltInTaken { kind: "class", name: "Integer".into() };
    let words = &tr("The package {types} holds {what} named {name}, where that built-in type is kept: rename it to use the built-in type.");
    assert_ne!(words, "The package {types} holds {what} named {name}, where that built-in type is kept: rename it to use the built-in type.");
    let what = tr_ctx("refusal", "a class");
    assert_eq!(archforge_shell_api::panels::said(UIS, &taken), Some(fmt(words, &[("types", "Types"), ("what", &what), ("name", "Integer")])));
    let constrained = Parameter { name: "T".into(), kind: "class", constraint: Some("Entity".into()), ..Parameter::default() };
    let words = fmt(&tr("Arguments for {name} must be {constraint} or specialise it."), &[("name", "T"), ("constraint", "Entity")]);
    assert_eq!(archforge_shell_api::panels::said(UIS, &Reason::Misfit(Box::new(constrained))), Some(words));
    set_language("en");
}

/// A generalization set's grouping rule, an association end's aggregation rule and an n-ary
/// association's count of ends broken are said in the chosen language, never in the engine's
/// English: a set of no member, of too many, of generalizations to different classes, an end
/// aggregating beside one that does, a composition's whole allowing more than one or made
/// navigable, an n-ary end aggregating, and too few or too many ends (AR-442).
#[test]
fn a_set_or_an_association_end_rule_broken_is_said_in_the_chosen_language() {
    use archforge_engine::Reason;
    use archforge_shell_api::i18n::{fmt, tr, trn};
    use serde_json::json;
    let tc = &mut context();
    set_language("de");
    let id = |tc: &mut TestContext, cmd: &str, p: serde_json::Value| run(tc, cmd, p).unwrap()["id"].as_str().unwrap().to_string();
    let sales = id(tc, "uml.package.add", json!({ "name": "Sales" }));
    let [payment, card, product, voucher, order, line] =
        ["Payment", "Card", "Product", "Voucher", "Order", "Line"].map(|n| id(tc, "uml.class.add", json!({ "package": sales, "name": n })));
    let to_payment = id(tc, "uml.generalization.add", json!({ "specific": card, "general": payment }));
    let to_product = id(tc, "uml.generalization.add", json!({ "specific": voucher, "general": product }));
    let binary = id(tc, "uml.association.add", json!({ "from": order, "to": line }));
    let nary = id(tc, "uml.association.add-nary", json!({ "classes": [order, line, card] }));
    let end = |e: u8, field: &str, value: serde_json::Value| json!({ "association": binary, "end": e, field: value });
    let set_end = "uml.association.set-end";
    let refused = |tc: &mut TestContext, cmd: &str, p: serde_json::Value| run(tc, cmd, p).err().unwrap_or_default();
    let set = |gens: Vec<&String>| json!({ "generalizations": gens, "name": "Kinds" });
    assert_eq!(refused(tc, "uml.generalization-set.add", set(vec![])), tr("A set groups at least one generalization."));
    let pair = |specific: &str, general: &str| fmt(&tr("{specific} specialises {general}"), &[("specific", specific), ("general", general)]);
    let both = fmt(&tr("{names} and {last}"), &[("names", &pair("Card", "Payment")), ("last", &pair("Voucher", "Product"))]);
    let differ = format!("{both}. {}", tr("A set groups generalizations of one class."));
    let said = refused(tc, "uml.generalization-set.add", set(vec![&to_payment, &to_product]));
    assert_eq!(said, differ, "in German");
    id(tc, set_end, end(0, "multiplicity", json!("*")));
    let whole_many = tr("A composition's whole allows one at most: set the other end's multiplicity to 1 or 0..1 first.");
    assert_eq!(refused(tc, set_end, end(1, "aggregation", json!("composite"))), whole_many);
    id(tc, set_end, end(0, "multiplicity", json!("1")));
    id(tc, set_end, end(1, "aggregation", json!("composite")));
    for (cmd, p, said) in [
        (set_end, end(0, "aggregation", json!("shared")), tr("The other end has an aggregation already: only the part's end has one.")),
        (set_end, end(0, "multiplicity", json!("*")), tr("This end is a composition's whole, which allows one at most.")),
        (set_end, end(0, "navigable", json!(true)), tr("The whole's end of an aggregation or composition is not navigable.")),
        (
            set_end,
            json!({ "association": nary, "end": 0, "aggregation": "shared" }),
            tr("An n-ary association's ends are not aggregations: only a binary association has a whole and a part."),
        ),
        ("uml.association.add-nary", json!({ "classes": [order, line] }), tr("An n-ary association needs three ends or more.")),
        ("uml.association.add-nary", json!({ "classes": vec![&order; 33] }), fmt(&tr("An association takes at most {n} ends"), &[("n", "32")])),
        ("uml.association.remove-end", json!({ "association": nary, "end": 0 }), tr("An n-ary association needs three ends or more.")),
    ] {
        assert_eq!(refused(tc, cmd, p), said, "in German");
    }
    // A set's most members, said with the plural form its count takes: one and many differ.
    let most = |max: usize| archforge_shell_api::panels::said(UIS, &Reason::TooMany { kind: "generalization", max });
    let form = |n| trn("A set groups at most {n} generalization.", "A set groups at most {n} generalizations.", n);
    assert_eq!((most(256), most(1)), (Some(form(256)), Some(form(1))));
    assert_ne!(form(1), form(256).replace("256", "1"), "German has a singular");
    set_language("en");
}

/// A profile's and a stereotype's own rules, refused by the engine, are said in the chosen language:
/// a profile applied twice or not applied, the metaclasses a stereotype may not extend, an
/// extension end, a property whose values are set, and a full profile (AR-445).
#[test]
fn a_profile_rule_refused_is_said_in_the_chosen_language() {
    use archforge_engine::Reason;
    use archforge_shell_api::i18n::{fmt, tr, trn};
    use serde_json::json;
    let tc = &mut context();
    set_language("de");
    let id = |tc: &mut TestContext, cmd: &str, p: serde_json::Value| run(tc, cmd, p).unwrap()["id"].as_str().unwrap().to_string();
    let refused = |tc: &mut TestContext, cmd: &str, p: serde_json::Value| run(tc, cmd, p).err().unwrap_or_default();
    let [commerce, tools] = ["Commerce", "Tools"].map(|n| id(tc, "uml.package.add", json!({ "name": n })));
    let [order, customer] = ["Order", "Customer"].map(|n| id(tc, "uml.class.add", json!({ "package": commerce, "name": n })));
    let persistence = id(tc, "uml.profile.add", json!({ "name": "Persistence" }));
    let entity = id(tc, "uml.stereotype.add", json!({ "profile": persistence, "name": "Entity", "extends": ["Class"] }));
    let table = id(tc, "uml.attribute.add", json!({ "class": entity, "name": "table", "type": "String" }));
    id(tc, "uml.profile.apply", json!({ "package": commerce, "profile": persistence }));
    let pair = |package: &str| [("package", package.to_string()), ("profile", "Persistence".to_string())];
    let filled = |template: String, args: &[(&str, String)]| fmt(&template, &args.iter().map(|(k, v)| (*k, v.as_str())).collect::<Vec<_>>());
    let already = filled(tr("{package} applies {profile} already."), &pair("Commerce"));
    assert_eq!(already, "Commerce wendet Persistence bereits an.", "the language is German");
    assert_eq!(refused(tc, "uml.profile.apply", json!({ "package": commerce, "profile": persistence })), already);
    let not_applied = filled(tr("{package} does not apply {profile}."), &pair("Tools"));
    assert_eq!(refused(tc, "uml.profile.unapply", json!({ "package": tools, "profile": persistence })), not_applied);
    // The metaclasses a stereotype may not extend, with the ones it may.
    let offered = "Class, Interface, Enumeration, DataType, Property, Operation, Association, Generalization, Package".to_string();
    let extends = |names: serde_json::Value| json!({ "stereotype": entity, "extends": names });
    let named = |name: &str| [("name", name.to_string()), ("metaclasses", offered.clone())];
    let not_drawn = filled(tr("{name} is a UML metaclass Archforge does not draw yet. A stereotype extends {metaclasses}."), &named("Actor"));
    assert_eq!(refused(tc, "uml.stereotype.set", extends(json!(["Actor"]))), not_drawn);
    let none = filled(tr("{name} is not a metaclass Archforge draws. A stereotype extends {metaclasses}."), &named("Table"));
    assert_eq!(refused(tc, "uml.stereotype.set", extends(json!(["Table"]))), none);
    let twice = fmt(&tr("{name} is named twice. Name each metaclass once."), &[("name", "Class")]);
    assert_eq!(refused(tc, "uml.stereotype.set", extends(json!(["Class", "Class"]))), twice);
    let most = fmt(&tr("A stereotype extends at most {max} metaclasses."), &[("max", "9")]);
    assert_eq!(refused(tc, "uml.stereotype.set", extends(json!(vec!["Class"; 10]))), most);
    // An extension's end, and a property whose values are set on one element, then on two.
    let base = {
        let st = tc.session.as_ref().unwrap().doc();
        let stereotype = st.by_uuid(archforge_mof::Uuid::parse(&entity).unwrap()).unwrap();
        st.uuid(archforge_uml::profiles::extensions(st, stereotype)[0].base).unwrap().to_string()
    };
    let end = tr("{name} is a stereotype's extension end. Drop the metaclass from what the stereotype extends instead.");
    assert_eq!(refused(tc, "element.remove", json!({ "ids": [base] })), fmt(&end, &[("name", "base_Class")]));
    for (class, n) in [(&order, 1), (&customer, 2)] {
        id(tc, "uml.stereotype.apply", json!({ "element": class, "stereotype": entity }));
        id(tc, "uml.stereotype.set-value", json!({ "element": class, "stereotype": entity, "property": "table", "values": ["t"] }));
        let set = trn("{name} has values on {n} element. Remove those first.", "{name} has values on {n} elements. Remove those first.", n);
        let set = fmt(&set, &[("name", "table")]);
        assert!(set.starts_with(&format!("table hat Werte an {n} Element")), "{set}");
        assert_eq!(refused(tc, "element.remove", json!({ "ids": [table] })), set);
    }
    // A full profile, which takes 256 stereotypes to reach, is said from its reason.
    let full = archforge_shell_api::panels::said(UIS, &Reason::TooMany { kind: "stereotype", max: 256 });
    assert_eq!(full, Some(fmt(&tr("A profile holds at most {max} stereotypes."), &[("max", "256")])));
    assert_eq!(full.as_deref(), Some("Ein Profil fasst höchstens 256 Stereotypen."));
    set_language("en");
}

/// An element that takes no stereotype, and a stereotype that may not be applied to one, refused by
/// the engine, are said in the chosen language, in the picker's own words where it has them
/// (AR-446).
#[test]
fn an_application_refused_is_said_in_the_chosen_language() {
    use archforge_engine::Reason;
    use archforge_shell_api::i18n::{fmt, tr};
    use serde_json::json;
    let tc = &mut context();
    set_language("de");
    let id = |tc: &mut TestContext, cmd: &str, p: serde_json::Value| run(tc, cmd, p).unwrap()["id"].as_str().unwrap().to_string();
    let refused = |tc: &mut TestContext, element: &str, stereotype: &str| {
        run(tc, "uml.stereotype.apply", json!({ "element": element, "stereotype": stereotype })).err().unwrap_or_default()
    };
    let commerce = id(tc, "uml.package.add", json!({ "name": "Commerce" }));
    let order = id(tc, "uml.class.add", json!({ "package": commerce, "name": "Order" }));
    let status = id(tc, "uml.enumeration.add", json!({ "package": commerce, "name": "Status" }));
    let open = id(tc, "uml.literal.add", json!({ "enumeration": status, "name": "open" }));
    let [persistence, logging] = ["Persistence", "Logging"].map(|n| id(tc, "uml.profile.add", json!({ "name": n })));
    let made = |tc: &mut TestContext, profile: &str, name: &str, extends: &[&str]| {
        id(tc, "uml.stereotype.add", json!({ "profile": profile, "name": name, "extends": extends }))
    };
    let entity = made(tc, &persistence, "Entity", &["Class"]);
    let key = made(tc, &persistence, "Id", &["Property", "Operation"]);
    let bare = made(tc, &persistence, "Bare", &[]);
    let tracked = made(tc, &logging, "Tracked", &["Class"]);
    id(tc, "uml.profile.apply", json!({ "package": commerce, "profile": persistence }));
    let instance = id(tc, "uml.stereotype.apply", json!({ "element": order, "stereotype": entity }));
    let already = fmt(&tr("«{stereotype}» is applied to {name} already."), &[("stereotype", "Entity"), ("name", "Order")]);
    assert_eq!(already, "«Entity» ist bereits auf Order angewendet.", "the language is German");
    assert_eq!(refused(tc, &order, &entity), already);
    let other = tr("«{stereotype}» does not extend {metaclass}: it extends {metaclasses}.");
    let other = fmt(&other, &[("stereotype", "Id"), ("metaclass", "Class"), ("metaclasses", "Property, Operation")]);
    assert_eq!(refused(tc, &order, &key), other);
    assert_eq!(refused(tc, &order, &bare), fmt(&tr("«{stereotype}» extends no metaclass yet."), &[("stereotype", "Bare")]));
    let out = tr("Its profile {profile} is not applied to {package} or a package around it.");
    assert_eq!(refused(tc, &order, &tracked), fmt(&out, &[("profile", "Logging"), ("package", "Commerce")]));
    // What takes none: a profile's own element, a literal and an application.
    let own = fmt(&tr("{name} belongs to a profile, which defines stereotypes and takes none."), &[("name", "Entity")]);
    assert_eq!(refused(tc, &entity, &entity), own);
    let offered = "Class, Interface, Enumeration, DataType, Property, Operation, Association, Generalization, Package";
    let none = fmt(&tr("{name} takes no stereotype. A stereotype extends {metaclasses}."), &[("name", "open"), ("metaclasses", offered)]);
    assert_eq!(refused(tc, &open, &entity), none);
    let application = tr("A stereotype application takes no stereotype. Stereotypes apply to the model's elements.");
    assert_eq!(refused(tc, &instance, &entity), application);
    // The model itself, and an element that carries the most, are said from their reasons.
    let outside = archforge_shell_api::panels::said(UIS, &Reason::NotInModel);
    assert_eq!(outside, Some(tr("This is no element of the model, so no stereotype applies to it.")));
    let most = archforge_shell_api::panels::said(UIS, &Reason::TooMany { kind: "applied stereotype", max: 32 });
    assert_eq!(most, Some(fmt(&tr("An element carries at most {max} stereotypes."), &[("max", "32")])));
    assert_eq!(most.as_deref(), Some("Ein Element trägt höchstens 32 Stereotypen."));
    set_language("en");
}

/// Values that do not fit their stereotype property, a property reshaped so the values set would
/// not fit, and a literal a value names removed, refused by the engine, are said in the chosen
/// language, the values in their field's own words (AR-458).
#[test]
fn a_value_refused_is_said_in_the_chosen_language() {
    use archforge_engine::{Accepted, Reason, ValueMisfit, Values};
    use archforge_shell_api::i18n::{fmt, tr, trn};
    use serde_json::json;
    let tc = &mut context();
    set_language("de");
    let id = |tc: &mut TestContext, cmd: &str, p: serde_json::Value| run(tc, cmd, p).unwrap()["id"].as_str().unwrap().to_string();
    let refused = |tc: &mut TestContext, cmd: &str, p: serde_json::Value| run(tc, cmd, p).err().unwrap_or_default();
    let commerce = id(tc, "uml.package.add", json!({ "name": "Commerce" }));
    let order = id(tc, "uml.class.add", json!({ "package": commerce, "name": "Order" }));
    let status = id(tc, "uml.enumeration.add", json!({ "package": commerce, "name": "Status" }));
    let open = id(tc, "uml.literal.add", json!({ "enumeration": status, "name": "open" }));
    id(tc, "uml.literal.add", json!({ "enumeration": status, "name": "shut" }));
    let persistence = id(tc, "uml.profile.add", json!({ "name": "Persistence" }));
    let entity = id(tc, "uml.stereotype.add", json!({ "profile": persistence, "name": "Entity", "extends": ["Class"] }));
    let tags = id(tc, "uml.attribute.add", json!({ "class": entity, "name": "tags", "type": "String", "multiplicity": "0..2" }));
    id(tc, "uml.attribute.add", json!({ "class": entity, "name": "state", "type": status }));
    let phase = id(tc, "uml.enumeration.add", json!({ "package": commerce, "name": "Phase" }));
    id(tc, "uml.attribute.add", json!({ "class": entity, "name": "phase", "type": phase }));
    id(tc, "uml.profile.apply", json!({ "package": commerce, "profile": persistence }));
    id(tc, "uml.stereotype.apply", json!({ "element": order, "stereotype": entity }));
    let set = |property: &str, values: serde_json::Value| {
        ("uml.stereotype.set-value", json!({ "element": order, "stereotype": entity, "property": property, "values": values }))
    };
    let (cmd, p) = set("tags", json!(["a", "a"]));
    let twice = fmt(&tr("{name} holds each value once, and {value} is there twice."), &[("name", "tags"), ("value", "a")]);
    assert_eq!(twice, "tags fasst jeden Wert einmal, und a steht zweimal da.", "the language is German");
    assert_eq!(refused(tc, cmd, p), twice);
    let (cmd, p) = set("tags", json!(["a", "b", "c"]));
    let count = fmt(
        &trn("{name} holds [{bounds}] values; {n} is given.", "{name} holds [{bounds}] values; {n} are given.", 3),
        &[("name", "tags"), ("bounds", "0..2")],
    );
    assert_eq!(refused(tc, cmd, p), count);
    let (cmd, p) = set("tags", json!([" "]));
    assert_eq!(refused(tc, cmd, p), tr("A value is empty. Clear the field to show the default."));
    let (cmd, p) = set("tags", json!(["x".repeat(1001)]));
    assert_eq!(refused(tc, cmd, p), fmt(&tr("A value is longer than {n} characters."), &[("n", "1000")]));
    let (cmd, p) = set("state", json!(["closed"]));
    let unread = fmt(
        &tr("{value} is no value of {name}. {accepted}"),
        &[("value", "closed"), ("name", "state"), ("accepted", &fmt(&tr("One of {literals}."), &[("literals", "open, shut")]))],
    );
    assert_eq!(refused(tc, cmd, p), unread);
    let (cmd, p) = set("phase", json!(["x"]));
    assert_eq!(refused(tc, cmd, p), "x ist kein Wert von phase. Die Aufzählung Phase hat noch keine Literale.");
    // What each type reads, said from its reason.
    let types = [
        (Accepted::Integer, tr("A whole number, such as 42 or -7.")),
        (Accepted::Real, tr("A number, such as 2.5 or -1e3.")),
        (Accepted::Boolean, tr("true or false.")),
        (Accepted::UnlimitedNatural, tr("A whole number of 0 or more, or *.")),
        (Accepted::Text, tr("Any text.")),
    ];
    for (accepted, words) in types {
        let misfit = ValueMisfit::Unread { value: "x".into(), accepted };
        let said = archforge_shell_api::panels::said(UIS, &Reason::Values(Box::new(Values { property: "n".into(), misfit })));
        let unread = fmt(&tr("{value} is no value of {name}. {accepted}"), &[("value", "x"), ("name", "n"), ("accepted", &words)]);
        assert_eq!(said, Some(unread));
    }
    let most = Reason::Values(Box::new(Values { property: "tags".into(), misfit: ValueMisfit::TooMany { max: 256 } }));
    assert_eq!(
        archforge_shell_api::panels::said(UIS, &most),
        Some(fmt(&tr("{name} holds at most {n} values."), &[("name", "tags"), ("n", "256")])),
        ""
    );
    // A property reshaped so the values set would not fit, and a literal a value names.
    for (property, values) in [("tags", json!(["a", "b"])), ("state", json!(["open"]))] {
        let (cmd, p) = set(property, values);
        run(tc, cmd, p).unwrap();
    }
    let retyped = fmt(
        &trn("{name} has values on {n} element. Remove those first.", "{name} has values on {n} elements. Remove those first.", 1),
        &[("name", "tags")],
    );
    assert_eq!(refused(tc, "uml.attribute.set", json!({ "attribute": tags, "type": "Integer" })), retyped);
    let narrower = fmt(
        &trn("Values on {n} element don't fit {name} [{bounds}].", "Values on {n} elements don't fit {name} [{bounds}].", 1),
        &[("name", "tags"), ("bounds", "0..1")],
    );
    assert_eq!(refused(tc, "uml.attribute.set", json!({ "attribute": tags, "multiplicity": "0..1" })), narrower);
    let valued = refused(tc, "element.remove", json!({ "ids": [open] }));
    assert_eq!(valued, "Das Literal open ist der Wert von state in «Entity» auf Order. Setzen Sie dort zuerst einen anderen Wert.");
    set_language("en");
}

/// The applications' parameters refused by the engine are said in the chosen language: an id
/// missing, as what the parameter takes, or of no element, an element of the wrong kind, a template parameter's classifier, a
/// stereotype not applied, a property it lacks, with or without others, and values that are no list
/// of texts or too many; so is an element gone (AR-473).
#[test]
fn an_application_parameter_refused_is_said_in_the_chosen_language() {
    use archforge_shell_api::i18n::{fmt, tr};
    use serde_json::json;
    let tc = &mut context();
    set_language("de");
    let ok = |tc: &mut TestContext, cmd: &str, p: serde_json::Value| run(tc, cmd, p).unwrap();
    let id = |tc: &mut TestContext, cmd: &str, p: serde_json::Value| ok(tc, cmd, p)["id"].as_str().unwrap().to_string();
    let refused = |tc: &mut TestContext, cmd: &str, p: serde_json::Value| run(tc, cmd, p).err().unwrap_or_default();
    let commerce = id(tc, "uml.package.add", json!({ "name": "Commerce" }));
    let order = id(tc, "uml.class.add", json!({ "package": commerce, "name": "Order" }));
    let count = id(tc, "uml.attribute.add", json!({ "class": order, "name": "count", "type": "Integer" }));
    let persistence = id(tc, "uml.profile.add", json!({ "name": "Persistence" }));
    let entity = id(tc, "uml.stereotype.add", json!({ "profile": persistence, "name": "Entity", "extends": ["Class"] }));
    id(tc, "uml.attribute.add", json!({ "class": entity, "name": "table", "type": "String" }));
    let key = id(tc, "uml.stereotype.add", json!({ "profile": persistence, "name": "Key", "extends": ["Property"] }));
    id(tc, "uml.profile.apply", json!({ "package": commerce, "profile": persistence }));
    id(tc, "uml.stereotype.apply", json!({ "element": count, "stereotype": key }));
    let apply = |element: &str, stereotype: &str| ("uml.stereotype.apply", json!({ "element": element, "stereotype": stereotype }));
    let (cmd, p) = apply("", &entity);
    assert_eq!(refused(tc, cmd, p), "Es ist kein Element angegeben. Geben Sie seine ID an.", "the language is German");
    let (cmd, p) = apply(&order, "");
    assert_eq!(refused(tc, cmd, p), tr("No stereotype is given. Give its id."));
    let unnamed = refused(tc, "uml.stereotype.show-values", json!({ "diagram": "", "show": true }));
    assert_eq!(unnamed, tr("No class diagram is given. Give its id."));
    let (cmd, p) = apply("nope", &entity);
    assert_eq!(refused(tc, cmd, p), fmt(&tr("There is no element with the id {id}."), &[("id", "nope")]));
    let (cmd, p) = apply(&order, &order);
    assert_eq!(refused(tc, cmd, p), fmt(&tr("{name} is not a stereotype."), &[("name", "Order")]));
    let shown = refused(tc, "uml.stereotype.show-values", json!({ "diagram": order, "show": true }));
    assert_eq!(shown, fmt(&tr("{name} is not a class diagram."), &[("name", "Order")]));
    let t = ok(tc, "uml.template-parameter.add", json!({ "classifier": order, "name": "T" }))["element"].as_str().unwrap().to_string();
    let (cmd, p) = apply(&t, &entity);
    let parameter = fmt(&tr("{name} is a template parameter of {template} and types only its members."), &[("name", "T"), ("template", "Order")]);
    assert_eq!(refused(tc, cmd, p), parameter);
    // Values set where the stereotype is not applied, on a property it lacks, and values no list.
    let set = |element: &str, stereotype: &str, property: &str, values: serde_json::Value| {
        ("uml.stereotype.set-value", json!({ "element": element, "stereotype": stereotype, "property": property, "values": values }))
    };
    let (cmd, p) = set(&order, &entity, "table", json!(["x"]));
    let unapplied = fmt(&tr("«{stereotype}» is not applied to {name}."), &[("stereotype", "Entity"), ("name", "Order")]);
    assert_eq!(refused(tc, cmd, p), unapplied);
    id(tc, "uml.stereotype.apply", json!({ "element": order, "stereotype": entity }));
    let (cmd, p) = set(&order, &entity, "colour", json!(["x"]));
    let lacks = fmt(
        &tr("«{stereotype}» has no property {name}. It has {properties}."),
        &[("stereotype", "Entity"), ("name", "colour"), ("properties", "table")],
    );
    assert_eq!(refused(tc, cmd, p), lacks);
    let (cmd, p) = set(&order, &entity, "", json!(["x"]));
    assert_eq!(refused(tc, cmd, p), tr("No property is given. Give its name or id."));
    let (cmd, p) = set(&count, &key, "x", json!(["x"]));
    let none = fmt(&tr("«{stereotype}» has no property {name}. It has no properties yet."), &[("stereotype", "Key"), ("name", "x")]);
    assert_eq!(refused(tc, cmd, p), none);
    let (cmd, p) = set(&order, &entity, "table", json!([1]));
    assert_eq!(refused(tc, cmd, p), tr("Give the values as a list of texts."));
    let (cmd, p) = set(&order, &entity, "table", json!(vec!["x"; 257]));
    assert_eq!(refused(tc, cmd, p), fmt(&tr("{name} holds at most {n} values."), &[("name", "table"), ("n", "256")]));
    assert_eq!(archforge_shell_api::panels::said(UIS, &archforge_engine::Reason::Gone), Some(tr("This element is gone.")));
    set_language("en");
}

/// A package moved back by Properties into a package whose diagram shows it already keeps that one
/// card, and the move is not refused (AR-561); moved where it is not shown, it is not placed there,
/// nor is a diagram made for its new owner (AR-568).
#[test]
fn a_package_moved_back_where_it_is_shown_keeps_its_one_card() {
    for theme in [Appearance::Light, Appearance::Dark] {
        let mut h = project_harness(theme, egui::vec2(1600.0, 1000.0));
        let id = |v: serde_json::Value| archforge_mof::Uuid::parse(v["id"].as_str().unwrap()).unwrap();
        // Sales on the root's diagram, then moved into Billing: the move keeps its card there (AR-567).
        let (top, sales, billing, card) = {
            let s = h.state_mut().session.as_mut().unwrap();
            let root = id(s.execute("uml.model.root", &serde_json::json!({})).unwrap().value).to_string();
            let top = own_diagram(s, archforge_mof::Uuid::parse(&root).unwrap());
            let sales = id(s.execute("uml.package.add", &serde_json::json!({ "name": "Sales" })).unwrap().value);
            let billing = id(s.execute("uml.package.add", &serde_json::json!({ "name": "Billing" })).unwrap().value);
            let at = serde_json::json!({ "diagram": top.to_string(), "element": sales.to_string(), "x": 80, "y": 80, "width": 240, "height": 148 });
            let card = id(s.execute("diagram.place", &at).unwrap().value);
            s.execute("uml.package.move", &serde_json::json!({ "package": sales.to_string(), "to": billing.to_string() })).unwrap();
            (top, sales, billing, card)
        };
        h.run();
        // Where Sales is shown: the uuids of the diagrams of its cards, and of the cards.
        let shown = |h: &Harness<'static, TestContext>| {
            let st = h.state().session.as_ref().unwrap().doc();
            let nodes = archforge_diagram::views_of(st, st.by_uuid(sales).unwrap()).2;
            let mut on: Vec<_> =
                nodes.into_iter().map(|n| (st.uuid(archforge_diagram::diagram_of(st, n).unwrap()).unwrap(), st.uuid(n).unwrap())).collect();
            on.sort();
            on
        };
        let parent = |h: &Harness<'static, TestContext>| {
            let st = h.state().session.as_ref().unwrap().doc();
            archforge_uml::commands::package_of(st, st.by_uuid(sales).unwrap()).map(|o| archforge_core::name_of(st, o))
        };
        assert_eq!(shown(&h), vec![(top, card)], "{theme:?}: shown on the root's diagram alone");
        let pick = |h: &mut Harness<'static, TestContext>, to: &str| {
            h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [sales.to_string()] })).unwrap();
            h.run();
            let at = rect_of(h, "workspace.properties.package.parent").center();
            click(h, at);
            let at = rect_of(h, &format!("workspace.properties.package.parent.{to}")).center();
            click(h, at);
        };
        // Back into the root by Properties: moved, not refused, its one card kept.
        pick(&mut h, "0");
        assert_eq!(h.state().panels.notice, None, "{theme:?}: not refused");
        assert_eq!(parent(&h).as_deref(), Some("Commerce platform"), "{theme:?}: back in the root");
        assert_eq!(shown(&h), vec![(top, card)], "{theme:?}: the one card it had");
        // One step: Undo takes Sales back into Billing, its card kept; Redo moves it again.
        h.state_mut().session.as_mut().unwrap().undo().unwrap();
        assert_eq!((parent(&h).as_deref(), shown(&h)), (Some("Billing"), vec![(top, card)]), "{theme:?}: undone");
        h.state_mut().session.as_mut().unwrap().redo().unwrap();
        assert_eq!(parent(&h).as_deref(), Some("Commerce platform"), "{theme:?}: redone");
        // Into Billing, which has no diagram: moved, its one card kept, and Billing still has none.
        pick(&mut h, "1");
        assert_eq!(h.state().panels.notice, None, "{theme:?}: not refused");
        assert_eq!(parent(&h).as_deref(), Some("Billing"), "{theme:?}: in Billing");
        assert_eq!(shown(&h), vec![(top, card)], "{theme:?}: the one card it had, none placed");
        let st = h.state().session.as_ref().unwrap().doc();
        let none_yet = Ok(archforge_uml::defaults::DefaultDiagram::NoDiagramYet);
        assert_eq!(archforge_uml::defaults::resolve(st, st.by_uuid(billing).unwrap()), none_yet, "{theme:?}: no diagram made");
    }
}

/// A package's parent menu longer than the window keeps within it: the wheel scrolls it to its last
/// choice, which a click picks; from the keyboard the arrows bring a choice out of sight into view,
/// and Enter picks it.
#[test]
fn a_parent_package_menu_longer_than_the_window_scrolls_to_its_last_choice() {
    let mut h = project_harness(Appearance::Light, egui::vec2(1600.0, 700.0));
    let id = |v: serde_json::Value| archforge_mof::Uuid::parse(v["id"].as_str().unwrap()).unwrap();
    let mover = {
        let s = h.state_mut().session.as_mut().unwrap();
        for i in 0..40 {
            s.execute("uml.package.add", &serde_json::json!({ "name": format!("Package {i:02}") })).unwrap();
        }
        id(s.execute("uml.package.add", &serde_json::json!({ "name": "Mover" })).unwrap().value)
    };
    h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [mover.to_string()] })).unwrap();
    h.run();
    let at = rect_of(&h, "workspace.properties.package.parent").center();
    click(&mut h, at);
    let window = h.ctx.content_rect();
    let choices =
        |h: &Harness<'static, TestContext>| ids(h).into_iter().filter(|i| i.starts_with("workspace.properties.package.parent.")).collect::<Vec<_>>();
    // The root and the forty packages: 41 choices, not all in sight at once.
    let last = "workspace.properties.package.parent.40".to_string();
    assert!(!choices(&h).contains(&last), "the last choice starts out of sight");
    let first = rect_of(&h, "workspace.properties.package.parent.0");
    assert!(window.contains_rect(first), "the menu keeps within the window");
    for _ in 0..30 {
        h.event(egui::Event::PointerMoved(first.center()));
        h.event(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, -200.0),
            modifiers: egui::Modifiers::NONE,
            phase: egui::TouchPhase::Move,
            source: egui::MouseWheelSource::Wheel,
        });
        h.run_steps(10);
        if choices(&h).contains(&last) {
            break;
        }
    }
    assert!(choices(&h).contains(&last), "scrolled to the last choice");
    let at = rect_of(&h, &last);
    assert!(window.contains_rect(at), "in the window");
    click(&mut h, at.center());
    let st = h.state().session.as_ref().unwrap().doc();
    let parent = archforge_uml::commands::package_of(st, st.by_uuid(mover).unwrap()).map(|p| archforge_core::name_of(st, p));
    assert_eq!(parent.as_deref(), Some("Package 39"), "the last choice, picked");
    // From the keyboard: the arrow keys reach a choice out of sight, scrolled into view; Enter picks it.
    h.run();
    let at = rect_of(&h, "workspace.properties.package.parent").center();
    click(&mut h, at);
    for _ in 0..41 {
        key(&mut h, egui::Key::ArrowDown, egui::Modifiers::NONE);
    }
    assert!(choices(&h).contains(&last), "the last choice brought into sight");
    // The last choice is Package 39 (the root comes first); one row up is Package 38.
    key(&mut h, egui::Key::ArrowUp, egui::Modifiers::NONE);
    key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
    let st = h.state().session.as_ref().unwrap().doc();
    let parent = archforge_uml::commands::package_of(st, st.by_uuid(mover).unwrap()).map(|p| archforge_core::name_of(st, p));
    assert_eq!(parent.as_deref(), Some("Package 38"), "picked from the keyboard");
}

/// Properties lists an enumeration's literals under `Literals · n`, each with its pencil: `+` and
/// the pencil open the literal editor (Name; Remove, Move up and Move down on the left, Cancel and
/// Apply on the right), Apply or Enter writes one undo step and grows the card to show a literal
/// added, Cancel or Escape discards, a clashing name is refused in the editor and changes nothing.
/// In light and dark.
#[test]
fn an_enumeration_s_literals_are_added_edited_moved_and_removed_in_properties() {
    use crate::uml_panel::MemberKind;
    for theme in [Appearance::Light, Appearance::Dark] {
        let (mut h, _, status, node) = literal_sample(theme, &["Placed", "Paid", "Shipped"]);
        h.run();
        assert!(painted(&h).iter().any(|t| t == "Literals · 3"));
        let found = ids(&h);
        for name in ["Placed", "Paid", "Shipped"] {
            let id = format!("workspace.properties.literal.{}.edit", literal_named(&h, status, name));
            assert!(found.contains(&id), "{id}");
        }
        // + opens the editor: its name alone, the name taking the keys; Enter writes it.
        let at = rect_of(&h, "workspace.properties.literals.add").center();
        click(&mut h, at);
        let edit = uml(h.state()).member.clone().expect("the editor");
        assert_eq!((edit.kind, edit.target), (MemberKind::Literal, None));
        let found = ids(&h);
        assert!(found.iter().any(|i| i == "workspace.properties.member.name"));
        for none in ["workspace.properties.member.visibility", "workspace.properties.member.type", "workspace.properties.member.remove"] {
            assert!(!found.iter().any(|i| i == none), "{none} in a new literal's editor");
        }
        h.event(egui::Event::Text("Cancelled".into()));
        h.run();
        key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
        assert!(uml(h.state()).member.is_none(), "{:?}", uml(h.state()).member);
        assert_eq!(literal_names(&h, status), ["Placed", "Paid", "Shipped", "Cancelled"]);
        assert_eq!(h.state().session.as_ref().unwrap().undo_label(), Some("Add literal"), "one undo step");
        // Its card grew to show it, in the same step.
        let st = h.state().session.as_ref().unwrap().doc();
        let height = archforge_diagram::grid::bounds_of(st, st.by_uuid(archforge_mof::Uuid::parse(&node).unwrap()).unwrap()).height;
        assert!(height >= crate::uml_ui::literals_height(4), "{height}");
        assert!(painted(&h).iter().any(|t| t == "Cancelled") && painted(&h).iter().any(|t| t == "Literals · 4"));
        // The pencil opens the editor in the row's place; a name another has is refused, nothing written.
        let paid = literal_named(&h, status, "Paid");
        let at = rect_of(&h, &format!("workspace.properties.literal.{paid}.edit")).center();
        click(&mut h, at);
        assert_eq!(uml(h.state()).member.as_ref().map(|e| (e.target, e.name.clone())), Some((Some(paid), "Paid".to_string())));
        let found = ids(&h);
        for id in ["remove", "up", "down", "cancel", "apply"] {
            assert!(found.iter().any(|i| *i == format!("workspace.properties.member.{id}")), "{id}");
        }
        // Remove and the moves on the left, Cancel and Apply on the right.
        let (remove, cancel, apply) = (
            rect_of(&h, "workspace.properties.member.remove"),
            rect_of(&h, "workspace.properties.member.cancel"),
            rect_of(&h, "workspace.properties.member.apply"),
        );
        assert!(
            remove.right() < rect_of(&h, "workspace.properties.member.down").left() + 1.0
                && rect_of(&h, "workspace.properties.member.down").right() < cancel.left()
        );
        assert!(cancel.right() < apply.left() && (remove.top() - apply.top()).abs() < 1.0);
        let revision = h.state().session.as_ref().unwrap().revision();
        uml_mut(h.state_mut()).member.as_mut().unwrap().name = "shipped".into();
        let at = rect_of(&h, "workspace.properties.member.apply").center();
        click(&mut h, at);
        let refused = uml(h.state()).member.as_ref().and_then(|e| e.refused.clone()).expect("refused");
        assert_eq!(refused, "There is a literal named Shipped already.");
        assert!(painted(&h).contains(&refused), "said in the editor");
        assert_eq!(h.state().session.as_ref().unwrap().revision(), revision, "nothing written");
        // Apply renames it: the same literal, one undo step.
        uml_mut(h.state_mut()).member.as_mut().unwrap().name = "Settled".into();
        let at = rect_of(&h, "workspace.properties.member.apply").center();
        click(&mut h, at);
        assert_eq!(literal_names(&h, status), ["Placed", "Settled", "Shipped", "Cancelled"]);
        assert_eq!(literal_named(&h, status, "Settled"), paid);
        assert_eq!(h.state().session.as_ref().unwrap().undo_label(), Some("Rename literal"));
        // Move up and Move down, each one step, the editor staying open on it.
        let shipped = literal_named(&h, status, "Shipped");
        let at = rect_of(&h, &format!("workspace.properties.literal.{shipped}.edit")).center();
        click(&mut h, at);
        let at = rect_of(&h, "workspace.properties.member.up").center();
        click(&mut h, at);
        assert_eq!(literal_names(&h, status), ["Placed", "Shipped", "Settled", "Cancelled"]);
        assert_eq!(h.state().session.as_ref().unwrap().undo_label(), Some("Move literal"));
        let at = rect_of(&h, "workspace.properties.member.down").center();
        click(&mut h, at);
        let at = rect_of(&h, "workspace.properties.member.down").center();
        click(&mut h, at);
        assert_eq!(literal_names(&h, status), ["Placed", "Settled", "Cancelled", "Shipped"]);
        assert_eq!(literal_named(&h, status, "Shipped"), shipped, "it keeps its identity");
        // Cancel and Escape discard what was typed.
        uml_mut(h.state_mut()).member.as_mut().unwrap().name = "Lost".into();
        let at = rect_of(&h, "workspace.properties.member.cancel").center();
        click(&mut h, at);
        assert!(uml(h.state()).member.is_none());
        let at = rect_of(&h, &format!("workspace.properties.literal.{shipped}.edit")).center();
        click(&mut h, at);
        uml_mut(h.state_mut()).member.as_mut().unwrap().name = "Lost".into();
        key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
        assert!(uml(h.state()).member.is_none());
        assert_eq!(literal_names(&h, status), ["Placed", "Settled", "Cancelled", "Shipped"]);
        // Remove takes it away, one step; undo brings it back in its place.
        let at = rect_of(&h, &format!("workspace.properties.literal.{shipped}.edit")).center();
        click(&mut h, at);
        let at = rect_of(&h, "workspace.properties.member.remove").center();
        click(&mut h, at);
        assert_eq!(literal_names(&h, status), ["Placed", "Settled", "Cancelled"]);
        assert!(painted(&h).iter().any(|t| t == "Literals · 3"));
        h.state_mut().session.as_mut().unwrap().undo().unwrap();
        assert_eq!(literal_names(&h, status), ["Placed", "Settled", "Cancelled", "Shipped"]);
    }
}

/// An end's Multiplicity takes a preset or typing, each written as one undo step that its line
/// follows; its menu offers none to many once, as `*`, which `0..*` typed reads as and ticks.
#[test]
fn an_association_end_multiplicity_is_picked_or_typed_one_undo_step_each_its_line_following_and_kept_on_reopen() {
    let size = egui::vec2(1600.0, 1000.0);
    for theme in [Appearance::Light, Appearance::Dark] {
        let (h, _) = paid_by(theme, size);
        let found = ids(&h);
        for i in 0..2 {
            for id in [format!("workspace.properties.end.{i}.multiplicity"), format!("workspace.properties.end.{i}.multiplicity.menu")] {
                assert!(found.contains(&id), "{theme:?}: {id}");
            }
        }
    }
    let (mut h, paid) = paid_by(Appearance::Light, size);
    let s = |a: [&str; 2]| a.map(String::from);
    assert_eq!(end_multiplicities(&h, paid), (s(["1", "0..1"]), s(["1", "0..1"])));
    // The chevron opens the presets, the hint and Clear; a pick writes at once, one undo step.
    let at = rect_of(&h, "workspace.properties.end.0.multiplicity.menu").center();
    click(&mut h, at);
    assert_eq!(h.state().panels.menu, Some(crate::uml_state::menus::END_MULTIPLICITY.at(0)));
    let found = ids(&h);
    for item in ["0", "1", "2", "3", "clear"] {
        assert!(found.contains(&format!("workspace.properties.end.0.multiplicity.menu.{item}")), "{item} in {found:?}");
    }
    assert!(!found.contains(&"workspace.properties.end.0.multiplicity.menu.4".to_string()), "none to many is offered once");
    assert!(painted(&h).iter().any(|t| t == "Or type a number or range"));
    let at = rect_of(&h, "workspace.properties.end.0.multiplicity.menu.3").center();
    click(&mut h, at);
    assert_eq!(h.state().panels.menu, None, "a pick closes the menu");
    assert_eq!(end_multiplicities(&h, paid), (s(["1..*", "0..1"]), s(["1..*", "0..1"])));
    h.state_mut().session.as_mut().unwrap().undo().unwrap();
    h.run();
    assert_eq!(end_multiplicities(&h, paid).0, s(["1", "0..1"]));
    assert_eq!(typed_multiplicity(&h, 0), "1", "the field follows the undo");
    h.state_mut().session.as_mut().unwrap().redo().unwrap();
    h.run();
    assert_eq!((end_multiplicities(&h, paid).0, typed_multiplicity(&h, 0)), (s(["1..*", "0..1"]), "1..*".to_string()));
    // Typed on the other end and Enter: written there, the first end keeping its own.
    retype(&mut h, "workspace.properties.end.1.multiplicity", "2..5");
    assert_eq!(end_multiplicities(&h, paid).0[1], "0..1", "typing writes nothing until it is committed");
    key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
    assert_eq!(end_multiplicities(&h, paid), (s(["1..*", "2..5"]), s(["1..*", "2..5"])));
    // A reversed range is refused under the field and stays there to correct; nothing is written.
    let before = revision(h.state());
    retype(&mut h, "workspace.properties.end.1.multiplicity", "5..2");
    key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
    assert!(painted(&h).iter().any(|t| t == "The upper bound must be at least 5."), "{:?}", painted(&h));
    assert_eq!(typed_multiplicity(&h, 1), "5..2");
    assert_eq!(end_multiplicities(&h, paid), (s(["1..*", "2..5"]), s(["1..*", "2..5"])));
    assert_eq!(revision(h.state()), before);
    // Escape brings the written value back and the reason goes.
    key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
    assert_eq!(typed_multiplicity(&h, 1), "2..5");
    assert!(!painted(&h).iter().any(|t| t == "The upper bound must be at least 5."));
    assert_eq!(revision(h.state()), before);
    // Each malformed text is refused with its reason, never coerced.
    for (bad, why) in [
        ("-1", "Type a whole number, a range such as 2..5, or *."),
        ("1...*", "Type a whole number, a range such as 2..5, or *."),
        ("99999999999", "A bound can be at most 1000000."),
        ("*..1", "The lower bound cannot be *."),
    ] {
        retype(&mut h, "workspace.properties.end.1.multiplicity", bad);
        key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
        assert_eq!(uml(h.state()).ends.as_ref().unwrap().refused[1].as_deref(), Some(why), "{bad}");
        key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
    }
    assert_eq!(revision(h.state()), before);
    // Saved and opened again, as the project's rows: the ends keep their multiplicities.
    let (tables, junctions) = archforge_notations::storage();
    let rows = archforge_mof::rows::snapshot(h.state().session.as_ref().unwrap().doc(), &tables, &junctions).unwrap();
    let back = archforge_notations::session_from_rows(&rows, Box::new(|| archforge_mof::Uuid::v7(1_761_670_900_000, [9; 10]))).unwrap();
    let st = back.doc();
    let ends = archforge_uml::figures::member_ends(st, st.by_uuid(paid).unwrap()).unwrap();
    assert_eq!(ends.map(|e| archforge_uml::figures::multiplicity(st, e)), s(["1..*", "2..5"]));
    // Clear takes it away, as empty text does; each one undo step.
    let at = rect_of(&h, "workspace.properties.end.1.multiplicity.menu").center();
    click(&mut h, at);
    let at = rect_of(&h, "workspace.properties.end.1.multiplicity.menu.clear").center();
    click(&mut h, at);
    assert_eq!(end_multiplicities(&h, paid), (s(["1..*", ""]), s(["1..*", ""])));
    h.state_mut().session.as_mut().unwrap().undo().unwrap();
    h.run();
    assert_eq!(end_multiplicities(&h, paid).0, s(["1..*", "2..5"]));
    retype(&mut h, "workspace.properties.end.0.multiplicity", "");
    key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
    assert_eq!(end_multiplicities(&h, paid), (s(["", "2..5"]), s(["", "2..5"])));
    assert!(painted(&h).iter().any(|t| t == "—"), "an empty field shows the placeholder");
}

/// An association end's multiplicity presets open from the field with Alt+Down at the current one,
/// follow the arrows and pick with Enter; Escape closes the menu before it cancels what was typed,
/// leaving the field writes it, and the menu offers none to many once, as `*`.
#[test]
fn an_end_multiplicity_menu_follows_the_keys_and_escape_closes_it_before_it_cancels_the_edit() {
    let (mut h, paid) = paid_by(Appearance::Dark, egui::vec2(1600.0, 1000.0));
    let open = |h: &Harness<'static, TestContext>| h.state().panels.menu == Some(crate::uml_state::menus::END_MULTIPLICITY.at(1));
    // Alt+Down in the field opens the presets at the current one; the arrows move; Enter picks.
    let at = rect_of(&h, "workspace.properties.end.1.multiplicity").center();
    click(&mut h, at);
    key(&mut h, egui::Key::ArrowDown, egui::Modifiers::ALT);
    assert!(open(&h));
    assert_eq!(h.state().panels.highlight, Some(0), "0..1 is the current one");
    key(&mut h, egui::Key::ArrowDown, egui::Modifiers::NONE);
    key(&mut h, egui::Key::ArrowDown, egui::Modifiers::NONE);
    key(&mut h, egui::Key::ArrowUp, egui::Modifiers::NONE);
    assert_eq!(h.state().panels.highlight, Some(1));
    key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
    assert!(!open(&h));
    assert_eq!(end_multiplicities(&h, paid).0[1], "1");
    // Typed but not committed: Escape closes the menu first, then brings the written value back.
    let before = revision(h.state());
    retype(&mut h, "workspace.properties.end.1.multiplicity", "3");
    key(&mut h, egui::Key::ArrowDown, egui::Modifiers::ALT);
    assert!(open(&h));
    key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
    assert!(!open(&h));
    assert_eq!(typed_multiplicity(&h, 1), "3", "closing the menu keeps the typing");
    key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
    assert_eq!(typed_multiplicity(&h, 1), "1");
    assert_eq!(revision(h.state()), before, "nothing was written");
    // Leaving the field writes what was typed: `0..*` reads as `*`.
    retype(&mut h, "workspace.properties.end.1.multiplicity", "0..*");
    let at = rect_of(&h, "workspace.properties.end.1.role").center();
    click(&mut h, at);
    assert_eq!(end_multiplicities(&h, paid).0[1], "*");
    assert_eq!(typed_multiplicity(&h, 1), "*");
    // Its menu offers none to many once, as `*`, and ticks it (AR-191).
    {
        use egui_kittest::kittest::{NodeT, Queryable};
        let at = rect_of(&h, "workspace.properties.end.1.multiplicity.menu").center();
        click(&mut h, at);
        assert!(h.query_all_by_label("0..*").next().is_none(), "no 0..* row beside *");
        let star = archforge_shell_api::widgets::egui_id("workspace.properties.end.1.multiplicity.menu.2").accesskit_id();
        let field = h.get_by_label("End at Payment · Multiplicity").accesskit_node().data().clone();
        assert_eq!(field.active_descendant(), Some(star), "the menu opens on its `*` row");
        let ticked = h.query_all_by_label("*").filter(|n| n.accesskit_node().data().is_selected() == Some(true)).count();
        assert_eq!(ticked, 1, "the `*` row is ticked");
        key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
    }
    // The keys reach the end chosen: the Order end kept its own.
    assert_eq!(end_multiplicities(&h, paid).0[0], "1");
}

/// A refused multiplicity's reason stays while its text stays unwritten, through writes to another
/// field of the end, and goes once the field holds what the model has; a refusal reads in full.
#[test]
fn a_multiplicity_refusal_stays_until_its_own_field_is_written_and_reads_in_full() {
    let (mut h, paid) = paid_by(Appearance::Light, egui::vec2(1600.0, 1000.0));
    let refused = |h: &Harness<'static, TestContext>, i: usize| uml(h.state()).ends.as_ref().unwrap().refused[i].clone();
    // Writing another field of the end keeps the reason while the refused text stays.
    retype(&mut h, "workspace.properties.end.1.multiplicity", "abc");
    key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
    assert!(refused(&h, 1).is_some());
    // Out of the field first, so the Navigable click writes on its own.
    let at = rect_of(&h, "workspace.properties.relationship.from").center();
    click(&mut h, at);
    assert!(refused(&h, 1).is_some());
    let at = rect_of(&h, "workspace.properties.end.1.navigable").center();
    click(&mut h, at);
    let navigable = |h: &Harness<'static, TestContext>| {
        let st = h.state().session.as_ref().unwrap().doc();
        let a = st.by_uuid(paid).unwrap();
        archforge_uml::figures::navigable(st, a, archforge_uml::figures::member_ends(st, a).unwrap()[1])
    };
    assert!(!navigable(&h), "Navigable was written");
    assert_eq!(end_multiplicities(&h, paid).0[1], "0..1");
    assert_eq!(typed_multiplicity(&h, 1), "abc");
    assert!(refused(&h, 1).is_some(), "the field still holds text that was never written");
    // Picking the value the model has already takes the reason away, though nothing is written.
    let at = rect_of(&h, "workspace.properties.end.1.multiplicity.menu").center();
    click(&mut h, at);
    let at = rect_of(&h, "workspace.properties.end.1.multiplicity.menu.0").center();
    click(&mut h, at);
    assert_eq!((typed_multiplicity(&h, 1), refused(&h, 1)), ("0..1".to_string(), None));
    // A composition's whole allows one at most: the model's refusal on Enter keeps the keyboard
    // in the field and shows its whole reason under it.
    let at = rect_of(&h, "workspace.properties.end.1.aggregation").center();
    click(&mut h, at);
    let at = rect_of(&h, "workspace.properties.end.1.aggregation.2").center();
    click(&mut h, at);
    retype(&mut h, "workspace.properties.end.0.multiplicity", "2");
    key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
    let why = refused(&h, 0).expect("the model refuses");
    assert!(why.contains("composition's whole"), "{why}");
    assert_eq!(end_multiplicities(&h, paid).0[0], "1");
    let id = archforge_shell_api::widgets::egui_id("workspace.properties.end.0.multiplicity");
    assert!(h.ctx.memory(|m| m.has_focus(id)), "the field keeps the keyboard");
    /// The galleys of the shape `s` and of the shapes it holds.
    fn galleys(s: &egui::Shape, out: &mut Vec<std::sync::Arc<egui::Galley>>) {
        match s {
            egui::Shape::Text(t) => out.push(t.galley.clone()),
            egui::Shape::Vec(v) => v.iter().for_each(|s| galleys(s, out)),
            _ => {}
        }
    }
    let mut all = Vec::new();
    for clipped in h.output().shapes.iter() {
        galleys(&clipped.shape, &mut all);
    }
    let said = all.iter().find(|g| g.text() == why).expect("the reason is drawn");
    assert!(!said.elided && said.rows.len() > 1, "the reason wraps instead of being cut: {:?} rows", said.rows.len());
    key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
    assert_eq!((typed_multiplicity(&h, 0), refused(&h, 0)), ("1".to_string(), None));
}

/// A multiplicity typed over the written one stays a draft while the chevron's menu is open:
/// nothing is written and the field keeps the keyboard; Escape closes the menu, then brings the
/// written one back; a preset picked after typing is written as one step, which one undo takes back.
#[test]
fn the_chevron_keeps_a_typed_multiplicity_a_draft_so_a_pick_is_one_step_and_escape_still_cancels() {
    let (mut h, paid) = paid_by(Appearance::Light, egui::vec2(1600.0, 1000.0));
    let id = archforge_shell_api::widgets::egui_id("workspace.properties.end.0.multiplicity");
    // Typed over the written 1, then the chevron: nothing is written and the field keeps the keyboard.
    let before = revision(h.state());
    retype(&mut h, "workspace.properties.end.0.multiplicity", "3");
    let at = rect_of(&h, "workspace.properties.end.0.multiplicity.menu").center();
    click(&mut h, at);
    assert_eq!(h.state().panels.menu, Some(crate::uml_state::menus::END_MULTIPLICITY.at(0)));
    assert_eq!((end_multiplicities(&h, paid).0[0].as_str(), typed_multiplicity(&h, 0)), ("1", "3".to_string()));
    assert_eq!(revision(h.state()), before);
    assert!(h.ctx.memory(|m| m.has_focus(id)));
    // Escape closes the menu, then brings the 1 back; still nothing written.
    key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
    key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
    assert_eq!((typed_multiplicity(&h, 0), revision(h.state())), ("1".to_string(), before));
    // Typed, then a preset picked: the preset is written as one step, which one undo takes back.
    retype(&mut h, "workspace.properties.end.0.multiplicity", "3");
    let at = rect_of(&h, "workspace.properties.end.0.multiplicity.menu").center();
    click(&mut h, at);
    let at = rect_of(&h, "workspace.properties.end.0.multiplicity.menu.3").center();
    click(&mut h, at);
    assert_eq!(end_multiplicities(&h, paid).0[0], "1..*");
    h.state_mut().session.as_mut().unwrap().undo().unwrap();
    h.run();
    assert_eq!(end_multiplicities(&h, paid).0[0], "1", "one undo goes back past the pick to the 1, not to a 3");
}

/// Leaving a multiplicity field closes its menu; its rows, its ticked preset, the row the arrows
/// reach and its refusal reach assistive technology.
#[test]
fn leaving_a_multiplicity_closes_its_menu_and_its_rows_and_refusal_reach_assistive_technology() {
    use egui_kittest::kittest::{NodeT, Queryable};
    let (mut h, paid) = paid_by(Appearance::Light, egui::vec2(1600.0, 1000.0));
    let open = |h: &Harness<'static, TestContext>| h.state().panels.menu == Some(crate::uml_state::menus::END_MULTIPLICITY.at(1));
    // The menu's rows and the chevron carry their words.
    let at = rect_of(&h, "workspace.properties.end.1.multiplicity.menu").center();
    click(&mut h, at);
    assert!(open(&h));
    assert!(h.query_by_label("End at Payment · Pick a multiplicity").is_some());
    assert!(h.query_by_label("End at Order · Pick a multiplicity").is_some(), "each chevron names its end");
    for word in ["0..1", "1", "*", "1..*", "Clear"] {
        assert!(!h.query_all_by_label(word).collect::<Vec<_>>().is_empty(), "{word} has an accessible name");
    }
    // The ticked row is selected; the open field is expanded, the row the keys reach its active
    // descendant, so a screen reader follows the arrows.
    let field_node = |h: &Harness<'static, TestContext>| h.get_by_label("End at Payment · Multiplicity").accesskit_node().data().clone();
    assert_eq!(h.get_by_label("0..1").accesskit_node().data().is_selected(), Some(true));
    assert_eq!(h.get_by_label("1..*").accesskit_node().data().is_selected(), Some(false));
    assert_eq!(field_node(&h).is_expanded(), Some(true));
    let row = |k: &str| archforge_shell_api::widgets::egui_id(&format!("workspace.properties.end.1.multiplicity.menu.{k}")).accesskit_id();
    assert_eq!(field_node(&h).active_descendant(), Some(row("0")));
    // Tab out of the field with the menu open and a row reached: the menu closes, and Enter
    // afterwards picks nothing.
    key(&mut h, egui::Key::ArrowDown, egui::Modifiers::NONE);
    key(&mut h, egui::Key::ArrowDown, egui::Modifiers::NONE);
    assert_eq!(h.state().panels.highlight, Some(2), "from the current 0..1 down to *");
    assert_eq!(field_node(&h).active_descendant(), Some(row("2")));
    for _ in 0..4 {
        key(&mut h, egui::Key::ArrowDown, egui::Modifiers::NONE);
    }
    assert_eq!(field_node(&h).active_descendant(), Some(row("clear")), "the arrows stop at Clear");
    key(&mut h, egui::Key::Tab, egui::Modifiers::NONE);
    assert!(!open(&h), "leaving the field closes its menu");
    assert_eq!((field_node(&h).is_expanded(), field_node(&h).active_descendant()), (Some(false), None));
    assert_eq!(end_multiplicities(&h, paid).0[1], "0..1", "nothing was picked");
    // Tab reached the chevron, whose Enter opens the menu again; Escape closes it.
    let chevron = archforge_shell_api::widgets::egui_id("workspace.properties.end.1.multiplicity.menu");
    assert!(h.ctx.memory(|m| m.has_focus(chevron)));
    key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
    assert!(open(&h));
    key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
    assert!(!open(&h));
    // Typing with the menu open takes over from the row reached: Enter writes the typing.
    let at = rect_of(&h, "workspace.properties.end.1.multiplicity").center();
    click(&mut h, at);
    key(&mut h, egui::Key::ArrowDown, egui::Modifiers::ALT);
    assert_eq!((open(&h), h.state().panels.highlight), (true, Some(0)));
    h.event(egui::Event::Key { key: egui::Key::A, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::COMMAND });
    h.event(egui::Event::Text("2..*".into()));
    h.run();
    assert_eq!(h.state().panels.highlight, None);
    key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
    assert_eq!(end_multiplicities(&h, paid).0[1], "2..*");
    // A refusal is the field's description, and the field says it is invalid.
    retype(&mut h, "workspace.properties.end.1.multiplicity", "5..2");
    key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
    let field = h.get_by_label("End at Payment · Multiplicity");
    assert_eq!(field.accesskit_node().description().as_deref(), Some("The upper bound must be at least 5."));
    assert_eq!(field.accesskit_node().data().invalid(), Some(egui::accesskit::Invalid::True));
    key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
    let field = h.get_by_label("End at Payment · Multiplicity");
    assert_eq!((field.accesskit_node().description(), field.accesskit_node().data().invalid()), (None, None));
}

/// A new association's ends have no multiplicity in the model or on its line, and each end's field
/// shows the placeholder, never a value.
#[test]
fn a_new_association_shows_both_ends_without_a_multiplicity() {
    let (mut h, _) = related_sample();
    let class = |h: &Harness<'static, TestContext>, name: &str| {
        let st = h.state().session.as_ref().unwrap().doc();
        let c = st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_uml::CLASS) && archforge_core::name_of(st, *o) == name).unwrap();
        st.uuid(c).unwrap().to_string()
    };
    let (from, to) = (class(&h, "Catalog"), class(&h, "Product"));
    let s = h.state_mut().session.as_mut().unwrap();
    let a = s.execute("uml.association.add", &serde_json::json!({ "from": from, "to": to })).unwrap().value["id"].as_str().unwrap().to_string();
    s.execute("view.select", &serde_json::json!({ "ids": [a.clone()] })).unwrap();
    h.run();
    let a = archforge_mof::Uuid::parse(&a).unwrap();
    let none = [String::new(), String::new()];
    assert_eq!(end_multiplicities(&h, a), (none.clone(), none));
    assert_eq!(painted(&h).iter().filter(|t| *t == "—").count(), 2, "each end shows the placeholder, never a value");
}

/// A second qualifier named like the end's first, whatever the case, is refused in place (design
/// frame A6): the reason under the fields, the editor open, the model and its history unchanged;
/// corrected, it is written. In light and dark.
#[test]
fn a_duplicate_qualifier_name_is_refused_in_place_and_nothing_is_written() {
    for theme in [Appearance::Light, Appearance::Dark] {
        let (mut h, made) = qualifiers_sample(theme);
        let a = made["contains"].clone();
        h.state_mut()
            .session
            .as_mut()
            .unwrap()
            .execute("uml.qualifier.add", &serde_json::json!({ "association": a, "end": 0, "name": "lineNo", "type": "Integer" }))
            .unwrap();
        h.run();
        let before = (qualifier_rows(&h, &a), h.state().session.as_ref().unwrap().undo_label().map(str::to_string));
        let at = rect_of(&h, "workspace.properties.end.0.qualifiers.add").center();
        click(&mut h, at);
        h.event(egui::Event::Text("LineNo".into()));
        h.run();
        pick_type(&mut h, "workspace.properties.end.0.qualifier.1.type", "String");
        let at = rect_of(&h, "workspace.properties.end.0.qualifier.1.apply").center();
        click(&mut h, at);
        let edit = uml(h.state()).qualifier.clone().expect("the editor stays open");
        assert_eq!(edit.refused_at, Some("name"));
        assert_eq!(edit.refused.as_deref(), Some("This end already has a qualifier named lineNo. Pick another name."));
        assert!(painted(&h).iter().any(|t| t == "This end already has a qualifier named lineNo. Pick another name."), "said in place");
        assert_eq!((qualifier_rows(&h, &a), h.state().session.as_ref().unwrap().undo_label().map(str::to_string)), before, "nothing written");
        // Typing again takes the reason away; another name is written.
        let at = rect_of(&h, "workspace.properties.end.0.qualifier.1.name").center();
        click(&mut h, at);
        key(&mut h, egui::Key::A, egui::Modifiers::COMMAND);
        h.event(egui::Event::Text("kind".into()));
        h.run();
        assert!(uml(h.state()).qualifier.as_ref().is_some_and(|e| e.refused.is_none()));
        let at = rect_of(&h, "workspace.properties.end.0.qualifier.1.apply").center();
        click(&mut h, at);
        assert_eq!(qualifier_rows(&h, &a)[0], ["lineNo : Integer", "kind : String"]);
    }
}

/// An end's editor writes its role, multiplicity, navigability, ordering and uniqueness as one
/// undo step; a role another end has, whatever the case, is refused in place, the editor open and
/// nothing written. In light and dark.
#[test]
fn an_n_ary_end_s_editor_applies_as_one_step_and_refuses_a_duplicate_role_in_place() {
    for theme in [Appearance::Light, Appearance::Dark] {
        let (mut h, made) = nary_sample(theme, true);
        h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [made["recommends"]] })).unwrap();
        h.run();
        let at = rect_of(&h, "workspace.properties.end.2.edit").center();
        click(&mut h, at);
        for id in ["role", "type", "multiplicity", "navigable", "ordered", "unique", "remove", "cancel", "apply"] {
            assert!(ids(&h).iter().any(|i| *i == format!("workspace.properties.end.2.{id}")), "{theme:?}: {id}");
        }
        assert!(!ids(&h).iter().any(|i| i.starts_with("workspace.properties.end.2.aggregation")), "no aggregation");
        retype(&mut h, "workspace.properties.end.2.role", "source");
        retype(&mut h, "workspace.properties.end.2.multiplicity", "0..1");
        for b in ["navigable", "ordered", "unique"] {
            let at = rect_of(&h, &format!("workspace.properties.end.2.{b}")).center();
            click(&mut h, at);
        }
        let at = rect_of(&h, "workspace.properties.end.2.apply").center();
        click(&mut h, at);
        assert!(uml(h.state()).nary_end.is_none(), "{theme:?}: applied, the editor closes");
        assert_eq!(naries(&h)[0].1[2], "source : Catalog [0..1]");
        let (a, end) = {
            let st = h.state().session.as_ref().unwrap().doc();
            let a = st.by_uuid(archforge_mof::Uuid::parse(&made["recommends"]).unwrap()).unwrap();
            (a, archforge_uml::nary::ends(st, a)[2])
        };
        let st = h.state().session.as_ref().unwrap().doc();
        let (_, _, ordered, unique) = archforge_uml::modifiers::flags(st, end);
        assert!(archforge_uml::figures::navigable(st, a, end) && ordered && !unique, "{theme:?}: the checks written");
        assert_eq!(h.state().session.as_ref().unwrap().undo_label(), Some("Set association end"));
        h.state_mut().session.as_mut().unwrap().undo().unwrap();
        assert_eq!(naries(&h)[0].1[2], "catalog : Catalog [1]", "{theme:?}: one step");
        h.run();
        // A duplicate role, refused in place.
        let before = revision(h.state());
        let at = rect_of(&h, "workspace.properties.end.1.edit").center();
        click(&mut h, at);
        retype(&mut h, "workspace.properties.end.1.role", "CUSTOMER");
        let at = rect_of(&h, "workspace.properties.end.1.apply").center();
        click(&mut h, at);
        let said = "Another end has the role customer already. Pick another role.";
        assert!(uml(h.state()).nary_end.as_ref().is_some_and(|e| e.refused.as_ref().is_some_and(|(at, why)| *at == Some("role") && why == said)));
        assert!(painted(&h).iter().any(|t| t == said), "{theme:?}: said under the field");
        assert_eq!(revision(h.state()), before, "{theme:?}: nothing written");
        key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
        assert!(uml(h.state()).nary_end.is_none(), "Escape cancels");
    }
}

/// The bin of one of four ends removes it at once, one undo step said by a toast; of three it asks
/// (A5), Keep 3 ends focused: Keep and Escape change nothing, Make it binary turns it into a plain
/// association between the two classes left, its diamond giving way to a line, one undo step. In
/// light and dark.
#[test]
fn removing_an_end_of_four_is_one_step_and_one_of_three_asks_to_keep_or_make_it_binary() {
    for theme in [Appearance::Light, Appearance::Dark] {
        let (mut h, ids) = nary_sample(theme, true);
        let a = ids["recommends"].clone();
        h.state_mut()
            .session
            .as_mut()
            .unwrap()
            .execute("uml.association.add-end", &serde_json::json!({ "association": a, "class": ids["Customer"] }))
            .unwrap();
        h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [a] })).unwrap();
        h.run();
        let at = rect_of(&h, "workspace.properties.end.3.edit").center();
        click(&mut h, at);
        let at = rect_of(&h, "workspace.properties.end.3.remove").center();
        click(&mut h, at);
        assert_eq!(naries(&h)[0].1.len(), 3, "{theme:?}: removed at once");
        assert_eq!(h.state().canvas.toast.as_ref().map(|t| t.text.as_str()), Some("End customer2 removed"));
        assert_eq!(legs_on(&h, None), 3);
        // One of three asks.
        let before = revision(h.state());
        let ask = |h: &mut Harness<'static, TestContext>| {
            let at = rect_of(h, "workspace.properties.end.2.edit").center();
            click(h, at);
            let at = rect_of(h, "workspace.properties.end.2.remove").center();
            click(h, at);
            assert!(uml(h.state()).nary_remove.is_some(), "asked");
            let shown = painted(h);
            for said in ["Two ends would remain", "Keep 3 ends", "Make it binary"] {
                assert!(shown.iter().any(|t| t == said), "{said} in {shown:?}");
            }
        };
        ask(&mut h);
        let keep = "workspace.properties.association.ends.removeConfirm.keep";
        assert!(h.ctx.memory(|m| m.has_focus(archforge_shell_api::widgets::egui_id(keep))), "{theme:?}: Keep 3 ends focused");
        let at = rect_of(&h, keep).center();
        click(&mut h, at);
        assert!(
            uml(h.state()).nary_remove.is_none() && revision(h.state()) == before && naries(&h)[0].1.len() == 3,
            "{theme:?}: Keep changes nothing"
        );
        ask(&mut h);
        key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
        assert!(uml(h.state()).nary_remove.is_none() && revision(h.state()) == before, "{theme:?}: Escape keeps");
        ask(&mut h);
        let at = rect_of(&h, "workspace.properties.association.ends.removeConfirm.binary").center();
        click(&mut h, at);
        assert!(naries(&h).is_empty(), "{theme:?}: binary now");
        assert_eq!(diamonds_here(&h), 0, "its diamond gave way");
        let st = h.state().session.as_ref().unwrap().doc();
        let ao = st.by_uuid(archforge_mof::Uuid::parse(&a).unwrap()).unwrap();
        assert!(st.is_alive(ao) && archforge_uml::figures::member_ends(st, ao).is_some(), "a binary association");
        assert_eq!(archforge_diagram::views_of(st, ao).1.len(), 1, "a line in its place");
        assert_eq!(h.state().session.as_ref().unwrap().undo_label(), Some("Remove association end"));
        h.state_mut().session.as_mut().unwrap().undo().unwrap();
        h.run();
        assert_eq!((naries(&h).len(), diamonds_here(&h), legs_on(&h, None)), (1, 1, 3), "{theme:?}: one undo step");
    }
}

/// A binary association keeps its two ends' sections, and its Delete still removes it.
#[test]
fn a_binary_association_keeps_its_ends_sections_beside_n_ary_ones() {
    let (mut h, _) = nary_sample(Appearance::Light, true);
    let places = {
        let st = h.state().session.as_ref().unwrap().doc();
        let a = st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_uml::ASSOCIATION) && archforge_core::name_of(st, *o) == "places").unwrap();
        st.uuid(a).unwrap().to_string()
    };
    h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [places] })).unwrap();
    h.run();
    let shown = painted(&h);
    assert!(shown.iter().any(|t| t == "End at Customer") && shown.iter().any(|t| t == "End at Order"), "{shown:?}");
    assert!(!shown.iter().any(|t| t.starts_with("Ends · ")), "no n-ary ends section");
    assert!(ids(&h).iter().any(|i| i == "workspace.properties.end.0.aggregation"), "aggregation offered");
}

/// An association end's part of Properties has a Stereotypes section of its own, beside the
/// association's, its ids its own: its picker applies a stereotype extending Property to that end
/// alone, one undo step (AR-135).
#[test]
fn an_association_end_takes_stereotypes_in_its_part_of_properties() {
    for theme in [Appearance::Light, Appearance::Dark] {
        let (mut h, _) = related_sample_in(theme, egui::vec2(1600.0, 2000.0));
        h.state_mut().cx(|cx| crate::uml_profiles::put_profiles(cx, false)).unwrap();
        let places = select_named(&mut h, archforge_uml::ASSOCIATION, "places");
        h.run();
        let ids_now = ids(&h);
        assert!(ids_now.iter().any(|i| i == "workspace.properties.stereotypes.add"), "{theme:?}: the association's own");
        assert!(ids_now.iter().any(|i| i == "workspace.properties.end.0.stereotypes.add"), "{theme:?}: {ids_now:?}");
        let id_stereotype = {
            let st = h.state().session.as_ref().unwrap().doc();
            let s = st.alive().find(|o| archforge_uml::profiles::is_stereotype(st, *o) && archforge_core::name_of(st, *o) == "Id").unwrap();
            st.uuid(s).unwrap()
        };
        let at = rect_of(&h, "workspace.properties.end.0.stereotypes.add").center();
        click(&mut h, at);
        assert!(!ids(&h).iter().any(|i| i.starts_with("workspace.properties.stereotypes.add.")), "{theme:?}: only the end's picker opens");
        let at = rect_of(&h, &format!("workspace.properties.end.0.stereotypes.add.{id_stereotype}")).center();
        click(&mut h, at);
        assert_eq!(h.state().session.as_ref().unwrap().undo_label(), Some("Apply stereotype"));
        let st = h.state().session.as_ref().unwrap().doc();
        let a = st.by_uuid(places).unwrap();
        let ends = archforge_uml::figures::member_ends(st, a).unwrap();
        assert_eq!(archforge_uml::applications::keywords(st, ends[0]).as_deref(), Some("«Id»"), "{theme:?}");
        assert_eq!(archforge_uml::applications::keywords(st, ends[1]), None);
        assert_eq!(archforge_uml::applications::keywords(st, a), None);
    }
}

/// A UML card's menu as UML answers it: its new row after Open in Models, named as its second
/// adorner names it (a class's attribute, an enumeration's literal), in the platform's keys; a
/// primitive type's adds none.
#[test]
fn a_uml_cards_menu_adds_the_row_its_adorner_names() {
    use archforge_shell_api::kinds::DiagramKind;
    let mut st = archforge_notations::store(test_ids()).unwrap();
    let [class, enumeration, primitive] =
        [archforge_uml::CLASS, archforge_uml::ENUMERATION, archforge_uml::PRIMITIVE_TYPE].map(|c| st.create(c).unwrap());
    let rows =
        |e, mac| crate::uml_ui::UmlKind.menu(&st, e, mac).unwrap().iter().map(|r| (r.key, r.label, r.keys, r.live, r.first)).collect::<Vec<_>>();
    assert_eq!(rows(class, true), [("add-row", "Add attribute", "⌘↩", true, false)]);
    assert_eq!(rows(class, false), [("add-row", "Add attribute", "Ctrl Enter", true, false)]);
    assert_eq!(rows(enumeration, false), [("add-row", "Add literal", "Ctrl Enter", true, false)]);
    assert!(rows(primitive, false).is_empty(), "a primitive type's menu adds nothing");
}

/// UML starts an n-ary association's leg at its diamond's outline rather than where the route
/// meets the diamond's node, and leaves a class card's line where its route meets the card, without
/// asking where the card is (AR-788). ER's default is held by
/// `the_canvas_starts_n_ary_legs_where_umls_kind_says`. In light and dark.
#[test]
fn umls_legs_start_at_the_diamond() {
    for theme in [Appearance::Light, Appearance::Dark] {
        let (h, ids) = nary_sample(theme, true);
        let st = h.state().session.as_ref().unwrap().doc();
        let node = |k: &str| st.by_uuid(archforge_mof::Uuid::parse(&ids[k]).unwrap()).unwrap();
        let (diamond, customer) = (node("diamond"), node("customer"));
        // A route leaving the node's left side at a quarter of its height: the diamond's outline is
        // further in, halfway between the side and the middle.
        let rect = egui::Rect::from_min_size(egui::pos2(100.0, 100.0), egui::vec2(40.0, 40.0));
        let (tip, next) = (egui::pos2(100.0, 110.0), egui::pos2(60.0, 110.0));
        let at = crate::uml_nary::leg_start(st, diamond, || rect, tip, next);
        assert!(at.is_some_and(|p| (p - egui::pos2(110.0, 110.0)).length() < 1e-3), "{theme:?}: on the diamond's outline: {at:?}");
        let card = crate::uml_nary::leg_start(st, customer, || panic!("a class card's place is not asked"), tip, next);
        assert_eq!(card, None, "{theme:?}: a class's line starts at its card");
    }
}
