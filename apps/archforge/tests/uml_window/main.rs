//! UML's window tests: the whole app's shell, given ER's and UML's screens as the app registers
//! them, drawn in a harness in both appearances, its controls found and pressed by automation id
//! (ADR 0005: whole-app behaviour with real notations is tested in the app). They reach the shell
//! through `archforge_ui_egui::testing` and `archforge_ui_egui::snapshot`, and UML's screens
//! through `archforge_uml_ui::snapshot`. Here are their harnesses, samples and probes; the tests of
//! the canvas, its tools and tabs, and relationships are in `canvas`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, reason = "test helpers sit outside the #[test] code that clippy.toml exempts")]

use egui_kittest::Harness;

use archforge_ui_egui::testing::{click, ids};
use archforge_ui_egui::theme::ThemeKind;
use archforge_ui_egui::{Services, Shell};
// UML's screens' modules, as their crate's `snapshot` lists them for the tests outside it.
use archforge_uml_ui::snapshot::{
    uml_bindings, uml_classifier, uml_constraints, uml_inline, uml_nary, uml_nav_route, uml_package, uml_package_nav, uml_panel, uml_profiles,
    uml_relate, uml_sets, uml_state, uml_stereotypes, uml_ui,
};

/// The app's screens parts, ER's then UML's, as the app registers them: every shell the tests make
/// is given them.
static SCREENS: archforge_shell_api::screens::ScreensRegistry =
    archforge_shell_api::screens::ScreensRegistry::new(&[archforge_ui_egui::ER, archforge_uml_ui::UML]);

mod canvas;

/// UML's own state in `shell`, whose notations hold UML.
fn uml_of(shell: &Shell) -> &uml_state::UmlState {
    uml_state::uml(archforge_ui_egui::testing::notation_states_of(shell)).expect("the shell's notations hold UML")
}

/// UML's own state in `shell`, to change.
fn uml_of_mut(shell: &mut Shell) -> &mut uml_state::UmlState {
    uml_state::uml_mut(archforge_ui_egui::snapshot::notation_states(shell)).expect("the shell's notations hold UML")
}

/// The rows of the model shown, as a host reads them.
fn rows_now(h: &Harness<'static, Shell>) -> std::sync::Arc<archforge_mof::rows::Rows> {
    let (tables, junctions) = archforge_notations::storage();
    std::sync::Arc::new(archforge_mof::rows::snapshot(h.state().session.as_ref().unwrap().doc(), &tables, &junctions).unwrap())
}

/// The shell with a project open, in the harness at the design's frame size or another.
fn workspace_harness(theme: ThemeKind, size: egui::Vec2) -> Harness<'static, Shell> {
    archforge_ui_egui::i18n::set_language("en");
    let mut shell = Shell::new(Services { ids: Some(test_ids()), ..Services::default() }, &SCREENS);
    shell.state.theme = theme;
    shell.open_project(archforge_ui_egui::Opened {
        name: "Commerce platform".into(),
        path: "/tmp/Commerce platform.archforge".into(),
        ..Default::default()
    });
    // Frames a sixtieth of a second apart, as on screen, so two clicks in a row read as a double click;
    // enough of them to settle a package switch's fade (AR-95).
    let mut h = Harness::builder().with_size(size).with_step_dt(1.0 / 60.0).with_max_steps(32).build_ui_state(
        |ui, shell: &mut Shell| {
            shell.logic(ui.ctx());
            shell.ui(ui);
        },
        shell,
    );
    h.run();
    h
}

/// Ids for a test's model: version 7 from a counter, no clock.
fn test_ids() -> std::sync::Arc<dyn Fn() -> archforge_mof::Uuid + Send + Sync> {
    let n = std::sync::atomic::AtomicU64::new(0);
    std::sync::Arc::new(move || {
        let i = n.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
        let mut r = [0u8; 10];
        r[2..].copy_from_slice(&i.to_be_bytes());
        archforge_mof::Uuid::v7(1_761_670_800_000, r)
    })
}

/// The rectangle the last frame drew the control `id` in.
fn rect_of(h: &Harness<'static, Shell>, id: &str) -> egui::Rect {
    let w = h.state().widgets().iter().find(|w| w.id == id).unwrap_or_else(|| panic!("no control {id}"));
    egui::Rect::from_min_max(egui::pos2(w.rect[0], w.rect[1]), egui::pos2(w.rect[2], w.rect[3]))
}

/// A press and release of `key` with `modifiers`, and the frames it starts.
fn key(h: &mut Harness<'static, Shell>, key: egui::Key, modifiers: egui::Modifiers) {
    h.key_down_modifiers(modifiers, key);
    h.key_up_modifiers(modifiers, key);
    h.run();
}

/// The title the open delete dialog paints, `Delete … from the model?`; never its empty form
/// (AR-216).
fn delete_title(h: &Harness<'static, Shell>) -> String {
    let titles: Vec<String> = painted(h).into_iter().filter(|t| t.starts_with("Delete ") && t.ends_with(" from the model?")).collect();
    assert_eq!(titles.len(), 1, "one title: {titles:?}");
    assert_ne!(titles[0], "Delete  from the model?", "the title names the element");
    titles[0].clone()
}

/// A drag with the primary button from `from` to `to`, a frame at each step.
fn drag(h: &mut Harness<'static, Shell>, from: egui::Pos2, to: egui::Pos2) {
    h.event(egui::Event::PointerMoved(from));
    h.event(egui::Event::PointerButton { pos: from, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE });
    h.run();
    h.event(egui::Event::PointerMoved(to));
    h.run();
    h.event(egui::Event::PointerButton { pos: to, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE });
    h.run();
}

/// Lets package navigation's thresholds rest after a switch.
fn rest(h: &mut Harness<'static, Shell>) {
    for _ in 0..45 {
        h.step();
    }
    h.run();
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

/// The default diagram of the package `p`, the one its binding names; none when it has none yet or
/// its binding is unsound.
fn default_of(st: &archforge_mof::Store, p: archforge_mof::Obj) -> Option<archforge_mof::Obj> {
    match archforge_uml::defaults::resolve(st, p) {
        Ok(archforge_uml::defaults::DefaultDiagram::Bound(d)) => Some(d),
        _ => None,
    }
}

/// The packages, their diagrams and the default-diagram bindings in the open project, counted.
fn packages_diagrams_bindings(h: &Harness<'static, Shell>) -> [usize; 3] {
    let st = h.state().session.as_ref().unwrap().doc();
    let of = |class| st.alive().filter(|o| st.class_of(*o).ok() == Some(class)).count();
    [of(archforge_uml::PACKAGE), of(archforge_diagram::DIAGRAM), archforge_diagram::defaults::bindings(st).len()]
}

/// The root's diagram Top with the package Sales on it (240 × 148 at 200, 160), and Sales's own
/// diagram with Order and OrderLine side by side, joined by an association: the harness, Top,
/// Sales, its card's node, its own diagram and the two classes' nodes there (AR-113).
fn package_preview_harness(
    theme: ThemeKind,
) -> (Harness<'static, Shell>, archforge_mof::Uuid, archforge_mof::Uuid, archforge_mof::Uuid, archforge_mof::Uuid, [archforge_mof::Uuid; 2]) {
    let mut h = workspace_harness(theme, egui::vec2(1600.0, 1000.0));
    let id = |v: serde_json::Value| archforge_mof::Uuid::parse(v["id"].as_str().unwrap()).unwrap();
    let ids = {
        let s = h.state_mut().session.as_mut().unwrap();
        let mut run = |c: &str, p: serde_json::Value| id(s.execute(c, &p).unwrap().value);
        let root = run("uml.model.root", serde_json::json!({}));
        let top = run("diagram.add", serde_json::json!({ "kind": archforge_uml::STRUCTURE_DIAGRAM, "element": root.to_string(), "name": "Top" }));
        let sales = run("uml.package.add", serde_json::json!({ "name": "Sales" }));
        let card = run(
            "diagram.place",
            serde_json::json!({ "diagram": top.to_string(), "element": sales.to_string(), "x": 200, "y": 160, "width": 240, "height": 148 }),
        );
        let own = run("diagram.add", serde_json::json!({ "kind": archforge_uml::STRUCTURE_DIAGRAM, "element": sales.to_string(), "name": "Sales" }));
        let order = run("uml.class.add", serde_json::json!({ "package": sales.to_string(), "name": "Order" }));
        let line = run("uml.class.add", serde_json::json!({ "package": sales.to_string(), "name": "OrderLine" }));
        run("uml.association.add", serde_json::json!({ "from": order.to_string(), "to": line.to_string() }));
        let a = run("diagram.place", serde_json::json!({ "diagram": own.to_string(), "element": order.to_string(), "x": 0, "y": 0 }));
        let b = run("diagram.place", serde_json::json!({ "diagram": own.to_string(), "element": line.to_string(), "x": 400, "y": 0 }));
        (top, sales, card, own, [a, b])
    };
    let (top, sales, card, own, classes) = ids;
    // The association shows on Sales's diagram, whether placing drew it or not.
    if let Some(s) = h.state_mut().session.as_mut() {
        let _ = s.execute("diagram.connect-all", &serde_json::json!({ "diagram": own.to_string() }));
    }
    archforge_ui_egui::canvas::open(h.state_mut(), top);
    h.state_mut().canvas.fit_pending = false;
    h.state_mut().canvas.zoom = 1.0;
    h.state_mut().canvas.pan = egui::Vec2::ZERO;
    h.run();
    (h, top, sales, card, own, classes)
}

/// A project whose root's structure diagram is open at 100 % (AR-565), with the root's id and the
/// diagram's.
fn owning_path_harness(theme: ThemeKind) -> (Harness<'static, Shell>, String, String) {
    let mut h = workspace_harness(theme, egui::vec2(1600.0, 1000.0));
    let root = owning_path_made(&mut h, "uml.model.root", serde_json::json!({}));
    let kind = archforge_uml::STRUCTURE_DIAGRAM;
    let top = owning_path_made(&mut h, "diagram.add", serde_json::json!({ "kind": kind, "element": root, "name": "Commerce platform" }));
    archforge_ui_egui::canvas::open(h.state_mut(), archforge_mof::Uuid::parse(&top).unwrap());
    h.state_mut().canvas.fit_pending = false;
    h.state_mut().canvas.zoom = 1.0;
    h.state_mut().canvas.pan = egui::Vec2::ZERO;
    h.run();
    (h, root, top)
}

/// Runs `command` on the harness's project and gives the id it answers with.
fn owning_path_made(h: &mut Harness<'static, Shell>, command: &str, params: serde_json::Value) -> String {
    let value = h.state_mut().session.as_mut().unwrap().execute(command, &params).unwrap().value;
    value["id"].as_str().unwrap().to_string()
}

/// Where the last frame filled the rectangle `r`, in painting order.
fn filled_at(h: &Harness<'static, Shell>, r: egui::Rect) -> Option<usize> {
    /// Every shape `s` holds, nested lists opened, in painting order.
    fn walk<'a>(s: &'a egui::Shape, out: &mut Vec<&'a egui::Shape>) {
        match s {
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
            s => out.push(s),
        }
    }
    let mut flat = Vec::new();
    h.output().shapes.iter().for_each(|c| walk(&c.shape, &mut flat));
    flat.iter()
        .position(|s| matches!(s, egui::Shape::Rect(f) if f.fill.a() > 0 && f.rect.min.distance(r.min) < 0.5 && f.rect.max.distance(r.max) < 0.5))
}

/// One zoom-out gesture over the canvas of at most `ticks` frames, ending at the first switch when
/// `stop` is set; gives the diagrams it showed, in turn.
fn zoom_out_gesture(h: &mut Harness<'static, Shell>, ticks: usize, stop: bool) -> Vec<archforge_mof::Uuid> {
    h.event(egui::Event::PointerMoved(rect_of(h, "workspace.canvas").center()));
    let mut shown: Vec<_> = h.state().canvas.active.into_iter().collect();
    for _ in 0..ticks {
        h.event(egui::Event::Zoom(0.8));
        h.step();
        if let Some(now) = h.state().canvas.active.filter(|now| shown.last() != Some(now)) {
            shown.push(now);
            if stop {
                break;
            }
        }
    }
    h.run();
    shown
}

/// The (source, destination) of each step on the route of `tab`'s tab, oldest first.
fn route_of(h: &Harness<'static, Shell>, tab: archforge_mof::Uuid) -> Vec<(archforge_mof::Uuid, archforge_mof::Uuid)> {
    uml_of(h.state()).nav.routes.get(&tab).map(|r| r.steps().map(|s| (s.source, s.destination)).collect()).unwrap_or_default()
}

/// The root's class diagram First with the card of the package Sales, and Sales' own diagram:
/// `(root, first, sales, sales_d)`.
fn first_and_sales(h: &mut Harness<'static, Shell>) -> (String, archforge_mof::Uuid, archforge_mof::Uuid, archforge_mof::Uuid) {
    let id = |v: serde_json::Value| archforge_mof::Uuid::parse(v["id"].as_str().unwrap()).unwrap();
    let s = h.state_mut().session.as_mut().unwrap();
    let root = id(s.execute("uml.model.root", &serde_json::json!({})).unwrap().value).to_string();
    let first = id(s
        .execute("diagram.add", &serde_json::json!({ "kind": archforge_uml::STRUCTURE_DIAGRAM, "element": root, "name": "First" }))
        .unwrap()
        .value);
    let sales = id(s.execute("uml.package.add", &serde_json::json!({ "name": "Sales" })).unwrap().value);
    let p = serde_json::json!({ "diagram": first.to_string(), "element": sales.to_string(), "x": 200, "y": 160, "width": 240, "height": 148 });
    s.execute("diagram.place", &p).unwrap();
    let sales_d = own_diagram(s, sales);
    (root, first, sales, sales_d)
}

/// Zooms out one frame at a time and stops on the frame that switches to `to`, before `to` draws:
/// its landing is still pending.
fn zoom_out_to(h: &mut Harness<'static, Shell>, to: archforge_mof::Uuid) {
    h.event(egui::Event::PointerMoved(rect_of(h, "workspace.canvas").center()));
    for _ in 0..40 {
        if h.state().canvas.active == Some(to) {
            break;
        }
        h.event(egui::Event::Zoom(0.8));
        h.step();
    }
    assert_eq!(h.state().canvas.active, Some(to), "zoomed out to the diagram entered from");
    assert!(uml_of(h.state()).nav.reveal.is_some(), "not drawn yet: the landing still pending");
}

/// The automation ids of the cards the last frame drew on the canvas.
fn node_ids(h: &Harness<'static, Shell>) -> Vec<String> {
    ids(h).iter().filter(|i| i.starts_with("workspace.canvas.node.")).cloned().collect()
}

/// Where the node `node` sits on its diagram, in the diagram's units.
fn node_bounds(h: &Harness<'static, Shell>, node: &str) -> archforge_diagram::Rect {
    let st = h.state().session.as_ref().unwrap().doc();
    let n = st.by_uuid(archforge_mof::Uuid::parse(node).unwrap()).unwrap();
    archforge_diagram::grid::bounds_of(st, n)
}

/// A secondary click, for context menus.
fn right_click(h: &mut Harness<'static, Shell>, at: egui::Pos2) {
    h.event(egui::Event::PointerMoved(at));
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Secondary, pressed, modifiers: egui::Modifiers::NONE });
    }
    h.run();
}

/// Two clicks a frame apart: `run` would settle for longer than egui's double-click window.
fn double_click(h: &mut Harness<'static, Shell>, at: egui::Pos2) {
    h.event(egui::Event::PointerMoved(at));
    for _ in 0..2 {
        for pressed in [true, false] {
            h.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE });
        }
        h.step();
    }
    h.run();
}

/// The texts painted with their top left inside `r`, each with how many glyphs it was laid out in.
fn texts_in(h: &Harness<'static, Shell>, r: egui::Rect) -> Vec<(String, usize)> {
    /// Collects the texts of `s` inside `r` and of the shapes it holds into `out`.
    fn walk(s: &egui::Shape, r: egui::Rect, out: &mut Vec<(String, usize)>) {
        match s {
            egui::Shape::Text(t) if r.contains(t.pos) => {
                out.push((t.galley.text().to_string(), t.galley.rows.iter().map(|row| row.glyphs.len()).sum()))
            }
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, r, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    h.output().shapes.iter().for_each(|c| walk(&c.shape, r, &mut out));
    out
}

/// Every text painted in the last frame, in the order it was painted.
fn painted<S>(h: &Harness<'_, S>) -> Vec<String> {
    /// Collects the texts of `s` and of the shapes it holds into `out`.
    fn walk(s: &egui::Shape, out: &mut Vec<String>) {
        match s {
            egui::Shape::Text(t) => out.push(t.galley.text().to_string()),
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    h.output().shapes.iter().for_each(|c| walk(&c.shape, &mut out));
    out
}

/// The class diagram the canvas shows, and its classes' names on it.
fn class_diagram_shown(h: &Harness<'static, Shell>) -> (String, Vec<String>) {
    let st = h.state().session.as_ref().unwrap().doc();
    let d = st.by_uuid(h.state().canvas.active.expect("a diagram is open")).unwrap();
    let kind = st.get(d, archforge_diagram::diagram::KIND).unwrap().as_str().unwrap().to_string();
    let classes = st
        .list(d, archforge_diagram::diagram::NODE)
        .unwrap()
        .iter()
        .filter_map(archforge_mof::Value::as_ref)
        .filter_map(|n| st.get(n, archforge_diagram::node::ELEMENT).ok()?.as_ref())
        .map(|e| archforge_core::name_of(st, e))
        .collect();
    (kind, classes)
}

/// The sample class model open on its class diagram, and the edge of its association `name`.
fn read_class_diagram(name: &str) -> (Harness<'static, Shell>, archforge_mof::Obj, archforge_mof::Obj) {
    let mut h = workspace_harness(ThemeKind::Light, egui::vec2(1600.0, 1000.0));
    archforge_ui_egui::snapshot::cx(h.state_mut(), uml_ui::open_sample).unwrap();
    h.run();
    let st = h.state().session.as_ref().unwrap().doc();
    let d = st.by_uuid(h.state().canvas.active.unwrap()).unwrap();
    let a = st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_uml::ASSOCIATION) && archforge_core::name_of(st, *o) == name).unwrap();
    let e = archforge_diagram::views_of(st, a).1[0];
    (h, d, e)
}

/// Whether the last frame painted a path in the accent colour.
fn painted_accent(h: &Harness<'static, Shell>) -> bool {
    let accent = archforge_ui_egui::ux::palette(false).accent;
    /// Whether `s`, or a shape it holds, is a path stroked in `accent`.
    fn walk(s: &egui::Shape, accent: archforge_ui_egui::ux::Colour) -> bool {
        match s {
            egui::Shape::Path(p) => matches!(p.stroke.color, egui::epaint::ColorMode::Solid(c) if c == egui::Color32::from(accent)),
            egui::Shape::Vec(v) => v.iter().any(|s| walk(s, accent)),
            _ => false,
        }
    }
    h.output().shapes.iter().any(|c| walk(&c.shape, accent))
}

/// A class's attributes or operations as its card lists them.
fn member_rows(h: &Harness<'static, Shell>, class: archforge_mof::Uuid, operations: bool) -> Vec<String> {
    let st = h.state().session.as_ref().unwrap().doc();
    let c = st.by_uuid(class).unwrap();
    if operations {
        archforge_uml::members::operations(st, c).into_iter().map(|o| archforge_uml::figures::operation_text(st, o)).collect()
    } else {
        archforge_uml::members::attributes(st, c).into_iter().map(|a| archforge_uml::figures::attribute_text(st, a)).collect()
    }
}

/// Picks the type labelled `label` from the open editor's type select `id`.
fn pick_type(h: &mut Harness<'static, Shell>, id: &str, label: &str) {
    let at = rect_of(h, id).center();
    click(h, at);
    let i = uml_panel::type_choices(h.state().session.as_ref().unwrap().doc(), None).iter().position(|(l, _)| l == label).expect(label);
    let at = rect_of(h, &format!("{id}.{i}")).center();
    click(h, at);
}

/// Clicks the field `id` and types `text` into it.
fn type_into(h: &mut Harness<'static, Shell>, id: &str, text: &str) {
    let at = rect_of(h, id).center();
    click(h, at);
    h.event(egui::Event::Text(text.into()));
    h.run();
}

/// A class diagram of the package Orders, two deep under the root (root › Fulfilment › Orders), at
/// 100 %: the class Order (`number : Integer`, `total : Money`, `pay() : Boolean`) at (80, 80) and
/// the enumeration OrderStatus (Placed, Paid, Shipped) at (480, 80). Gives the harness, Order's id
/// and OrderStatus's id.
fn nested_orders(theme: ThemeKind) -> (Harness<'static, Shell>, archforge_mof::Uuid, archforge_mof::Uuid) {
    let mut h = workspace_harness(theme, egui::vec2(1600.0, 1000.0));
    let run = |h: &mut Harness<'static, Shell>, command: &str, params: serde_json::Value| {
        let out = h.state_mut().session.as_mut().unwrap().execute(command, &params).unwrap_or_else(|e| panic!("{command}: {e}"));
        out.value["id"].as_str().map(|i| archforge_mof::Uuid::parse(i).unwrap())
    };
    let fulfilment = run(&mut h, "uml.package.add", serde_json::json!({ "name": "Fulfilment" })).unwrap();
    let orders = run(&mut h, "uml.package.add", serde_json::json!({ "name": "Orders", "parent": fulfilment.to_string() })).unwrap();
    let d =
        run(&mut h, "diagram.add", serde_json::json!({ "kind": archforge_uml::STRUCTURE_DIAGRAM, "element": orders.to_string(), "name": "Orders" }))
            .unwrap();
    let money = run(&mut h, "uml.data-type.add", serde_json::json!({ "package": orders.to_string(), "name": "Money" })).unwrap();
    let order = run(&mut h, "uml.class.add", serde_json::json!({ "package": orders.to_string(), "name": "Order" })).unwrap();
    run(&mut h, "uml.attribute.add", serde_json::json!({ "class": order.to_string(), "name": "number", "type": "Integer" }));
    run(&mut h, "uml.attribute.add", serde_json::json!({ "class": order.to_string(), "name": "total", "type": money.to_string() }));
    run(&mut h, "uml.operation.add", serde_json::json!({ "class": order.to_string(), "name": "pay", "returns": "Boolean" }));
    let status = run(&mut h, "uml.enumeration.add", serde_json::json!({ "package": orders.to_string(), "name": "OrderStatus" })).unwrap();
    for (element, x, height) in [(order, 80, uml_ui::members_height(2, 1)), (status, 480, uml_ui::literals_height(3))] {
        let place = serde_json::json!({ "diagram": d.to_string(), "element": element.to_string(), "x": x, "y": 80, "width": 280, "height": height });
        run(&mut h, "diagram.place", place);
    }
    archforge_ui_egui::snapshot::cx(h.state_mut(), |cx| uml_ui::put_literals(cx, status, &["Placed", "Paid", "Shipped"])).unwrap();
    archforge_ui_egui::canvas::open(h.state_mut(), d);
    h.run();
    let shell = h.state_mut();
    shell.canvas.fit_pending = false;
    shell.canvas.zoom = 1.0;
    shell.canvas.pan = egui::Vec2::ZERO;
    h.run();
    (h, order, status)
}

/// Every text the last frame painted, with the box its galley took.
fn text_boxes(h: &Harness<'static, Shell>) -> Vec<(String, egui::Rect)> {
    /// Collects the texts of `s` and of the shapes it holds into `out`, each with its box.
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

/// Every rectangle the last frame painted, with its fill and its stroke's colour.
fn rect_shapes(h: &Harness<'static, Shell>) -> Vec<(egui::Rect, egui::Color32, egui::Color32)> {
    /// Collects every rectangle under `s` with its fill and its stroke's colour.
    fn walk(s: &egui::Shape, out: &mut Vec<(egui::Rect, egui::Color32, egui::Color32)>) {
        match s {
            egui::Shape::Rect(r) => out.push((r.rect, r.fill, r.stroke.color)),
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    h.output().shapes.iter().for_each(|c| walk(&c.shape, &mut out));
    out
}

/// Every level line segment the last frame painted: its left and right ends, its height, its
/// thickness and its colour.
fn level_lines(h: &Harness<'static, Shell>) -> Vec<(f32, f32, f32, f32, egui::Color32)> {
    /// Collects every level line segment under `s`.
    fn walk(s: &egui::Shape, out: &mut Vec<(f32, f32, f32, f32, egui::Color32)>) {
        match s {
            egui::Shape::LineSegment { points: [a, b], stroke } if (a.y - b.y).abs() < 0.01 => {
                out.push((a.x.min(b.x), a.x.max(b.x), a.y, stroke.width, stroke.color));
            }
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    h.output().shapes.iter().for_each(|c| walk(&c.shape, &mut out));
    out
}

/// The size of node `n`, width and height.
fn node_size(h: &Harness<'static, Shell>, n: archforge_mof::Uuid) -> (f64, f64) {
    let st = h.state().session.as_ref().unwrap().doc();
    let b = archforge_diagram::grid::bounds_of(st, st.by_uuid(n).unwrap());
    (b.width, b.height)
}

/// The sample class model open, with the node of each class on its diagram by name.
fn related_sample() -> (Harness<'static, Shell>, std::collections::BTreeMap<String, String>) {
    related_sample_in(ThemeKind::Light, egui::vec2(1600.0, 1000.0))
}

/// [`related_sample`] in `theme` in a window of `size`.
fn related_sample_in(theme: ThemeKind, size: egui::Vec2) -> (Harness<'static, Shell>, std::collections::BTreeMap<String, String>) {
    let mut h = workspace_harness(theme, size);
    archforge_ui_egui::snapshot::cx(h.state_mut(), uml_ui::open_sample).unwrap();
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

/// The relationships of the class named `name`, as the agent's tool lists them.
fn related(h: &mut Harness<'static, Shell>, name: &str) -> Vec<serde_json::Value> {
    let s = h.state_mut().session.as_mut().unwrap();
    let st = s.doc();
    let c = st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_uml::CLASS) && archforge_core::name_of(st, *o) == name).unwrap();
    let id = st.uuid(c).unwrap().to_string();
    s.execute("uml.relationship.list", &serde_json::json!({ "class": id })).unwrap().value["relationships"].as_array().unwrap().clone()
}

/// How many edges the open diagram holds.
fn edge_count(h: &Harness<'static, Shell>) -> usize {
    let st = h.state().session.as_ref().unwrap().doc();
    st.list(st.by_uuid(h.state().canvas.active.unwrap()).unwrap(), archforge_diagram::diagram::EDGE).unwrap().len()
}

/// The boxes of the icons the last frame painted in `color`: each a mesh tinted that colour.
fn icon_boxes(h: &Harness<'static, Shell>, color: archforge_ui_egui::ux::Colour) -> Vec<egui::Rect> {
    /// Collects the box of every mesh under `s` tinted `color` throughout.
    fn walk(s: &egui::Shape, color: archforge_ui_egui::ux::Colour, out: &mut Vec<egui::Rect>) {
        match s {
            egui::Shape::Mesh(m) if !m.vertices.is_empty() && m.vertices.iter().all(|v| v.color == egui::Color32::from(color)) => {
                out.push(egui::Rect::from_points(&m.vertices.iter().map(|v| v.pos).collect::<Vec<_>>()));
            }
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, color, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    h.output().shapes.iter().for_each(|c| walk(&c.shape, color, &mut out));
    out
}

/// The classifier tool's hover text, as the class diagram gives it.
fn classifier_hint(h: &Harness<'static, Shell>) -> String {
    use archforge_shell_api::kinds::DiagramKind;
    uml_ui::UmlKind.tool_hint(&archforge_ui_egui::testing::cx_ref(h.state()), uml_classifier::TOOL).expect("the classifier tool names its kind")
}

/// The id of the live element of `class` named `name`.
fn named(h: &Harness<'static, Shell>, class: archforge_mof::ClassId, name: &str) -> archforge_mof::Uuid {
    let st = h.state().session.as_ref().unwrap().doc();
    let o = st.alive().find(|o| st.class_of(*o).ok() == Some(class) && archforge_core::name_of(st, *o) == name).expect(name);
    st.uuid(o).unwrap()
}

/// A point of the sample class diagram no card or line covers: under OrderLine, beside Catalog.
fn empty_spot(h: &Harness<'static, Shell>, nodes: &std::collections::BTreeMap<String, String>) -> egui::Pos2 {
    let x = rect_of(h, &format!("workspace.canvas.node.{}", nodes["OrderLine"])).center().x;
    let y = rect_of(h, &format!("workspace.canvas.node.{}", nodes["Catalog"])).center().y;
    egui::pos2(x, y)
}

/// A screen point on the line of diagram edge `edge` as the canvas drew it, found by the diagram's
/// own hit test inside the line's drawn bounds.
fn point_on_edge(h: &Harness<'static, Shell>, edge: archforge_mof::Uuid) -> egui::Pos2 {
    let s = h.state();
    let st = s.session.as_ref().unwrap().doc();
    let d = st.by_uuid(s.canvas.active.unwrap()).unwrap();
    let e = st.by_uuid(edge).unwrap();
    let grid = archforge_diagram::Grid::build(st, d);
    let origin = rect_of(h, "workspace.canvas").min;
    let bounds = rect_of(h, &format!("workspace.canvas.edge.{edge}"));
    let (pan, zoom) = (s.canvas.pan, s.canvas.zoom);
    for i in 0..=40 {
        for j in 0..=40 {
            let at = bounds.min + egui::vec2(bounds.width() * i as f32 / 40.0, bounds.height() * j as f32 / 40.0);
            let c = (at - origin - pan) / zoom;
            if grid.hit(f64::from(c.x), f64::from(c.y)) == Some(archforge_diagram::Hit::Edge(e)) {
                return at;
            }
        }
    }
    panic!("no point of the line {edge} on screen")
}

/// The sample with its payment kinds, BankTransfer's generalization alone in the set Kinds and
/// named by the constraint settled on Payment beside Payment: the generalizations' ids and
/// Payment's.
fn sample_with_a_set_and_a_rule(theme: ThemeKind) -> (Harness<'static, Shell>, [String; 3], String) {
    let (mut h, _) = related_sample_in(theme, egui::vec2(1600.0, 1000.0));
    let (gens, payment) = archforge_ui_egui::snapshot::cx(h.state_mut(), uml_ui::put_payment_kinds).unwrap();
    let s = h.state_mut().session.as_mut().unwrap();
    s.execute("uml.generalization-set.add", &serde_json::json!({ "generalizations": [gens[1]], "name": "Kinds" })).unwrap();
    let rule = serde_json::json!({ "context": payment, "name": "settled", "expression": "x", "constrained": [gens[1], payment] });
    s.execute("uml.constraint.add", &rule).unwrap();
    h.run();
    (h, gens, payment)
}

/// The sample class model in `theme` with two interfaces, Payable and Refundable, placed in its
/// empty bottom row and the whole diagram in view: the harness, every card's node by name, and the
/// interfaces' nodes.
fn realization_sample(theme: ThemeKind) -> (Harness<'static, Shell>, std::collections::BTreeMap<String, String>, [String; 2]) {
    let (mut h, nodes) = related_sample_in(theme, egui::vec2(1600.0, 1000.0));
    let d = h.state().canvas.active.unwrap().to_string();
    let s = h.state_mut().session.as_mut().unwrap();
    let st = s.doc();
    let commerce =
        st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_uml::PACKAGE) && archforge_core::name_of(st, *o) == "Commerce").unwrap();
    let commerce = st.uuid(commerce).unwrap().to_string();
    let mut place = |name: &str, x: i32| {
        let id = s.execute("uml.interface.add", &serde_json::json!({ "package": commerce, "name": name })).unwrap().value["id"]
            .as_str()
            .unwrap()
            .to_string();
        let n =
            s.execute("diagram.place", &serde_json::json!({ "diagram": d, "element": id, "x": x, "y": 1000, "width": 290, "height": 136 })).unwrap();
        n.value["id"].as_str().unwrap().to_string()
    };
    let interfaces = [place("Payable", 560), place("Refundable", 1040)];
    h.state_mut().canvas.fit_pending = true;
    h.run();
    (h, nodes, interfaces)
}

/// The node that shows `element` on the open diagram.
fn node_of(h: &Harness<'static, Shell>, element: archforge_mof::Uuid) -> String {
    let st = h.state().session.as_ref().unwrap().doc();
    let n = archforge_diagram::views_of(st, st.by_uuid(element).unwrap()).2[0];
    st.uuid(n).unwrap().to_string()
}

/// The names of the enumeration `enumeration`'s literals, in their order.
fn literal_names(h: &Harness<'static, Shell>, enumeration: archforge_mof::Uuid) -> Vec<String> {
    let st = h.state().session.as_ref().unwrap().doc();
    archforge_uml::commands::literals(st, st.by_uuid(enumeration).unwrap()).into_iter().map(|l| archforge_core::name_of(st, l)).collect()
}

/// The id of the literal of `enumeration` named `name`.
fn literal_named(h: &Harness<'static, Shell>, enumeration: archforge_mof::Uuid, name: &str) -> archforge_mof::Uuid {
    let st = h.state().session.as_ref().unwrap().doc();
    let l = archforge_uml::commands::literals(st, st.by_uuid(enumeration).unwrap()).into_iter().find(|l| archforge_core::name_of(st, *l) == name);
    st.uuid(l.expect(name)).unwrap()
}

/// [`enumeration_sample`] with OrderStatus holding `literals` and selected: the harness, every
/// card's node by name, OrderStatus and its node.
fn literal_sample(
    theme: ThemeKind,
    literals: &[&str],
) -> (Harness<'static, Shell>, std::collections::BTreeMap<String, String>, archforge_mof::Uuid, String) {
    let (mut h, nodes, [status_node, _]) = enumeration_sample(theme);
    let status = named(&h, archforge_uml::ENUMERATION, "OrderStatus");
    if !literals.is_empty() {
        archforge_ui_egui::snapshot::cx(h.state_mut(), |cx| uml_ui::put_literals(cx, status, literals)).unwrap();
    }
    h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [status.to_string()] })).unwrap();
    h.run();
    assert!(h.state().canvas.zoom >= archforge_shell_api::canvas::DETAIL_ZOOM, "the cards show their rows");
    (h, nodes, status, status_node)
}

/// The middle of row `i` of the card of node `node`, as drawn.
fn card_row(h: &Harness<'static, Shell>, node: &str, i: usize) -> egui::Pos2 {
    let card = rect_of(h, &format!("workspace.canvas.node.{node}"));
    let z = h.state().canvas.zoom;
    let y =
        archforge_shell_api::canvas::CARD_FIRST_ROW + i as f32 * archforge_shell_api::canvas::CARD_ROW + archforge_shell_api::canvas::CARD_ROW / 2.0;
    egui::pos2(card.left() + 60.0 * z, card.top() + y * z)
}

/// The constraints of the class named `name`, each as its row reads.
fn constraint_rows(h: &Harness<'static, Shell>, name: &str) -> Vec<String> {
    let st = h.state().session.as_ref().unwrap().doc();
    let c = st.by_uuid(named(h, archforge_uml::CLASS, name)).unwrap();
    archforge_uml::constraints::constraints_of(st, c).into_iter().map(|k| uml_constraints::row_text(st, k)).collect()
}

/// The sample class model in `theme` with two enumerations, OrderStatus and ClosingStatus, placed in
/// its empty bottom row and the whole diagram in view: the harness, every card's node by name, and
/// the enumerations' nodes.
fn enumeration_sample(theme: ThemeKind) -> (Harness<'static, Shell>, std::collections::BTreeMap<String, String>, [String; 2]) {
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
    h.state_mut().canvas.fit_pending = true;
    h.run();
    (h, nodes, enumerations)
}

/// The sample class model in `theme` with two data types, Amount and Price, placed in its empty
/// bottom row and the whole diagram in view: the harness, every card's node by name, the data
/// types' ids and their nodes.
fn data_type_sample(
    theme: ThemeKind,
) -> (Harness<'static, Shell>, std::collections::BTreeMap<String, String>, [archforge_mof::Uuid; 2], [String; 2]) {
    let (mut h, nodes) = related_sample_in(theme, egui::vec2(1600.0, 1000.0));
    let d = h.state().canvas.active.unwrap().to_string();
    let s = h.state_mut().session.as_mut().unwrap();
    let st = s.doc();
    let commerce =
        st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_uml::PACKAGE) && archforge_core::name_of(st, *o) == "Commerce").unwrap();
    let commerce = st.uuid(commerce).unwrap().to_string();
    let mut place = |name: &str, x: i32| {
        let id = s.execute("uml.data-type.add", &serde_json::json!({ "package": commerce, "name": name })).unwrap().value["id"]
            .as_str()
            .unwrap()
            .to_string();
        let n =
            s.execute("diagram.place", &serde_json::json!({ "diagram": d, "element": id, "x": x, "y": 1000, "width": 290, "height": 136 })).unwrap();
        (archforge_mof::Uuid::parse(&id).unwrap(), n.value["id"].as_str().unwrap().to_string())
    };
    // Not Money: the sample keeps a primitive type of that name in Types.
    let (money, money_node) = place("Amount", 560);
    let (price, price_node) = place("Price", 1040);
    h.state_mut().canvas.fit_pending = true;
    h.run();
    (h, nodes, [money, price], [money_node, price_node])
}

/// The sample class model with two primitive types of the model's own in Commerce, Sku and Code,
/// each on its diagram as the tool places one; gives their ids and their nodes.
fn primitive_type_sample(
    theme: ThemeKind,
) -> (Harness<'static, Shell>, std::collections::BTreeMap<String, String>, [archforge_mof::Uuid; 2], [String; 2]) {
    let (mut h, nodes) = related_sample_in(theme, egui::vec2(1600.0, 1000.0));
    let d = h.state().canvas.active.unwrap().to_string();
    let s = h.state_mut().session.as_mut().unwrap();
    let st = s.doc();
    let commerce =
        st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_uml::PACKAGE) && archforge_core::name_of(st, *o) == "Commerce").unwrap();
    let commerce = st.uuid(commerce).unwrap().to_string();
    let (w, height) = archforge_uml::PRIMITIVE_TYPE_SIZE;
    let mut place = |name: &str, x: i32| {
        let id = s.execute("uml.primitive-type.add", &serde_json::json!({ "package": commerce, "name": name })).unwrap().value["id"]
            .as_str()
            .unwrap()
            .to_string();
        let n =
            s.execute("diagram.place", &serde_json::json!({ "diagram": d, "element": id, "x": x, "y": 1000, "width": w, "height": height })).unwrap();
        (archforge_mof::Uuid::parse(&id).unwrap(), n.value["id"].as_str().unwrap().to_string())
    };
    let (sku, sku_node) = place("Sku", 560);
    let (code, code_node) = place("Code", 1040);
    h.state_mut().canvas.fit_pending = true;
    h.run();
    (h, nodes, [sku, code], [sku_node, code_node])
}

/// The height of the card `node` on the open diagram.
fn node_height(h: &Harness<'static, Shell>, node: &str) -> f64 {
    let st = h.state().session.as_ref().unwrap().doc();
    archforge_diagram::grid::bounds_of(st, st.by_uuid(archforge_mof::Uuid::parse(node).unwrap()).unwrap()).height
}

/// A package Reviews on its own class diagram, open in `theme`: the classes Customer and Product
/// side by side, Store under Customer and the interface Rated under Product, each card a class's
/// size. What the association class tests read.
struct Reviews {
    h: Harness<'static, Shell>,
    package: String,
    diagram: String,
    /// The classifiers' ids by name, and their nodes by name.
    ids: std::collections::BTreeMap<&'static str, String>,
    nodes: std::collections::BTreeMap<&'static str, String>,
}

/// [`Reviews`] in `theme`, the whole diagram in view.
fn reviews(theme: ThemeKind) -> Reviews {
    let mut h = workspace_harness(theme, egui::vec2(1600.0, 1000.0));
    let s = h.state_mut().session.as_mut().unwrap();
    let mut run = |cmd: &str, p: serde_json::Value| s.execute(cmd, &p).unwrap().value["id"].as_str().unwrap_or_default().to_string();
    let package = run("uml.package.add", serde_json::json!({ "name": "Reviews" }));
    let diagram = run("diagram.add", serde_json::json!({ "kind": archforge_uml::STRUCTURE_DIAGRAM, "element": package, "name": "Reviews" }));
    let (mut ids, mut nodes) = (std::collections::BTreeMap::new(), std::collections::BTreeMap::new());
    for (name, cmd, x, y) in [
        ("Customer", "uml.class.add", 80, 520),
        ("Product", "uml.class.add", 760, 520),
        ("Store", "uml.class.add", 80, 860),
        ("Rated", "uml.interface.add", 760, 860),
    ] {
        let id = run(cmd, serde_json::json!({ "package": package, "name": name }));
        let node = run("diagram.place", serde_json::json!({ "diagram": diagram, "element": id, "x": x, "y": y, "width": 240, "height": 136 }));
        ids.insert(name, id);
        nodes.insert(name, node);
    }
    let d = archforge_mof::Uuid::parse(&diagram).unwrap();
    archforge_ui_egui::canvas::open(h.state_mut(), d);
    h.state_mut().canvas.fit_pending = true;
    h.run();
    Reviews { h, package, diagram, ids, nodes }
}

impl Reviews {
    /// The centre of the card of the classifier named `name`.
    fn at(&self, name: &str) -> egui::Pos2 {
        rect_of(&self.h, &format!("workspace.canvas.node.{}", self.nodes[name])).center()
    }

    /// The diagram's nodes and lines, counted.
    fn shown(&self) -> (usize, usize) {
        let st = self.h.state().session.as_ref().unwrap().doc();
        let d = st.by_uuid(archforge_mof::Uuid::parse(&self.diagram).unwrap()).unwrap();
        (st.list(d, archforge_diagram::diagram::NODE).unwrap().len(), st.list(d, archforge_diagram::diagram::EDGE).unwrap().len())
    }

    /// The live association classes, by name.
    fn association_classes(&self) -> Vec<String> {
        let st = self.h.state().session.as_ref().unwrap().doc();
        st.alive().filter(|o| st.class_of(*o).ok() == Some(archforge_uml::ASSOCIATION_CLASS)).map(|o| archforge_core::name_of(st, o)).collect()
    }

    /// Makes the association class Review from Customer to Product through the commands, its line
    /// and its card above the line's middle: its id, its line's id and its card's id.
    fn review(&mut self) -> (String, String, String) {
        let (customer, product) = (self.ids["Customer"].clone(), self.ids["Product"].clone());
        let (from, to, d) = (self.nodes["Customer"].clone(), self.nodes["Product"].clone(), self.diagram.clone());
        let s = self.h.state_mut().session.as_mut().unwrap();
        let mut run = |cmd: &str, p: serde_json::Value| s.execute(cmd, &p).unwrap().value["id"].as_str().unwrap_or_default().to_string();
        let ac = run("uml.association-class.add", serde_json::json!({ "from": customer, "to": product, "name": "Review" }));
        run("uml.attribute.add", serde_json::json!({ "class": ac, "name": "stars", "type": "Integer" }));
        let edge = run("diagram.connect", serde_json::json!({ "diagram": d, "element": ac, "source": from, "target": to }));
        let card = run("diagram.place", serde_json::json!({ "diagram": d, "element": ac, "x": 420, "y": 200, "width": 240, "height": 136 }));
        self.h.run();
        (ac, edge, card)
    }

    /// The middle of the line from Customer to Product, which runs level between their cards.
    fn line_middle(&self) -> egui::Pos2 {
        let (a, b) = (
            rect_of(&self.h, &format!("workspace.canvas.node.{}", self.nodes["Customer"])),
            rect_of(&self.h, &format!("workspace.canvas.node.{}", self.nodes["Product"])),
        );
        egui::pos2((a.right() + b.left()) / 2.0, a.center().y)
    }

    /// The current selection's ids.
    fn selection(&self) -> Vec<String> {
        let s = self.h.state().session.as_ref().unwrap();
        s.view().selection.iter().filter_map(|o| s.doc().uuid(*o).ok()).map(|u| u.to_string()).collect()
    }
}

/// The open Delete from the model dialog: the diagrams it says the element disappears from, and
/// whether it says so in those words. Cancelled after, so nothing changes.
fn asked_diagrams(h: &mut Harness<'static, Shell>) -> (usize, Vec<String>) {
    h.run();
    let asked = h.state().explorer.delete.clone().expect("the dialog asks");
    let said = painted(h).into_iter().filter(|t| t.starts_with("It disappears from")).collect();
    let at = rect_of(h, "workspace.explorer.delete.cancel").center();
    click(h, at);
    assert!(h.state().explorer.delete.is_none());
    (asked.diagrams, said)
}

/// The model's revision.
fn revision(h: &Harness<'static, Shell>) -> u64 {
    h.state().session.as_ref().unwrap().revision()
}

/// The labels of the Models tree's rows, each indented by its depth.
fn tree_labels(h: &Harness<'static, Shell>) -> Vec<String> {
    archforge_ui_egui::testing::models_rows(
        archforge_ui_egui::testing::notation_uis(h.state()),
        h.state().session.as_ref().unwrap().doc(),
        &h.state().explorer,
    )
    .into_iter()
    .map(|r| format!("{}{}{}", "  ".repeat(r.depth as usize), r.label, r.detail.map(|d| format!(" : {d}")).unwrap_or_default()))
    .collect()
}

/// Selects the element of `class` named `name` and gives its id.
fn select_named(h: &mut Harness<'static, Shell>, class: archforge_mof::ClassId, name: &str) -> archforge_mof::Uuid {
    let st = h.state().session.as_ref().unwrap().doc();
    let o = st.alive().find(|o| st.class_of(*o).ok() == Some(class) && archforge_core::name_of(st, *o) == name).unwrap();
    let id = st.uuid(o).unwrap();
    h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [id.to_string()] })).unwrap();
    h.run();
    id
}

/// A package Shop open on its diagram in `theme` (AR-126): the classes Item and Order, Order
/// specializing Item; the enumeration Status with Placed and Paid; Item's `parts : Item [*]`,
/// Order's `lines : Item [*]` and `status : Status`; an association from Order to Item whose
/// second end is `items [*]`. Order is selected with Properties showing it. Gives the harness and
/// the ids by name: Item, Order, Status, parts, lines, status and the association, `holds`.
fn modifiers_sample(theme: ThemeKind) -> (Harness<'static, Shell>, std::collections::BTreeMap<&'static str, String>) {
    let mut h = workspace_harness(theme, egui::vec2(1600.0, 1400.0));
    let mut ids = std::collections::BTreeMap::new();
    {
        let s = h.state_mut().session.as_mut().unwrap();
        let mut run = |cmd: &str, p: serde_json::Value| s.execute(cmd, &p).unwrap().value["id"].as_str().unwrap_or_default().to_string();
        let pkg = run("uml.package.add", serde_json::json!({ "name": "Shop" }));
        let d = run("diagram.add", serde_json::json!({ "kind": archforge_uml::STRUCTURE_DIAGRAM, "element": pkg, "name": "Shop" }));
        let item = run("uml.class.add", serde_json::json!({ "package": pkg, "name": "Item" }));
        let order = run("uml.class.add", serde_json::json!({ "package": pkg, "name": "Order" }));
        run("uml.generalization.add", serde_json::json!({ "specific": order, "general": item }));
        let status = run("uml.enumeration.add", serde_json::json!({ "package": pkg, "name": "Status" }));
        for l in ["Placed", "Paid"] {
            run("uml.literal.add", serde_json::json!({ "enumeration": status, "name": l }));
        }
        let parts = run("uml.attribute.add", serde_json::json!({ "class": item, "name": "parts", "type": item, "multiplicity": "*" }));
        let lines = run("uml.attribute.add", serde_json::json!({ "class": order, "name": "lines", "type": item, "multiplicity": "*" }));
        let st = run("uml.attribute.add", serde_json::json!({ "class": order, "name": "status", "type": status }));
        let holds = run("uml.association.add", serde_json::json!({ "from": order, "to": item }));
        run("uml.association.set-end", serde_json::json!({ "association": holds, "end": 1, "role": "items", "multiplicity": "*" }));
        for (element, x, y) in [(&item, 80, 80), (&order, 80, 420), (&status, 520, 80)] {
            run("diagram.place", serde_json::json!({ "diagram": d, "element": element, "x": x, "y": y, "width": 260, "height": 160 }));
        }
        run("diagram.connect-all", serde_json::json!({ "diagram": d }));
        for (k, v) in [
            ("Item", item),
            ("Order", order.clone()),
            ("Status", status),
            ("parts", parts),
            ("lines", lines),
            ("status", st),
            ("holds", holds),
            ("diagram", d),
        ] {
            ids.insert(k, v);
        }
    }
    archforge_ui_egui::canvas::open(h.state_mut(), archforge_mof::Uuid::parse(&ids["diagram"]).unwrap());
    h.run();
    h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [ids["Order"]] })).unwrap();
    h.state_mut().state.show_panel(archforge_shell_api::state::Panel::Properties);
    h.run();
    (h, ids)
}

/// Whether a text the last frame painted holds `text`.
fn painted_has(h: &Harness<'static, Shell>, text: &str) -> bool {
    painted(h).iter().any(|t| t.contains(text))
}

/// Order and OrderLine joined by `contains`, its OrderLine end `line` at 0..1, on two class
/// diagrams of the package Shop, the first open and the association selected with Properties
/// shown (AR-127). Gives the ids by name: Order, OrderLine, contains, diagram, second.
fn qualifiers_sample(theme: ThemeKind) -> (Harness<'static, Shell>, std::collections::BTreeMap<&'static str, String>) {
    let mut h = workspace_harness(theme, egui::vec2(1600.0, 1500.0));
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
    archforge_ui_egui::canvas::open(h.state_mut(), archforge_mof::Uuid::parse(&ids["diagram"]).unwrap());
    h.run();
    h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [ids["contains"]] })).unwrap();
    h.state_mut().state.show_panel(archforge_shell_api::state::Panel::Properties);
    h.run();
    (h, ids)
}

/// The qualifier box's rows the association's line draws at its two ends.
fn qualifier_rows(h: &Harness<'static, Shell>, a: &str) -> [Vec<String>; 2] {
    let st = h.state().session.as_ref().unwrap().doc();
    let l = archforge_uml::figures::line(st, st.by_uuid(archforge_mof::Uuid::parse(a).unwrap()).unwrap()).unwrap();
    [l.ends[0].qualifiers.clone(), l.ends[1].qualifiers.clone()]
}

/// The edge of `element` on diagram `d`.
fn edge_on(h: &Harness<'static, Shell>, element: &str, d: &str) -> String {
    let st = h.state().session.as_ref().unwrap().doc();
    let d = st.by_uuid(archforge_mof::Uuid::parse(d).unwrap()).unwrap();
    let e = archforge_diagram::views_of(st, st.by_uuid(archforge_mof::Uuid::parse(element).unwrap()).unwrap())
        .1
        .into_iter()
        .find(|e| archforge_diagram::diagram_of(st, *e) == Some(d))
        .unwrap();
    st.uuid(e).unwrap().to_string()
}

/// Whether the last frame stroked a path in the accent colour of `theme` inside `within`.
fn accent_path_in(h: &Harness<'static, Shell>, theme: ThemeKind, within: egui::Rect) -> bool {
    let accent = archforge_ui_egui::ux::palette(theme == ThemeKind::Dark).accent;
    /// Whether `s`, or a shape it holds, is a path stroked in `accent` whose points all lie in `within`.
    fn walk(s: &egui::Shape, accent: archforge_ui_egui::ux::Colour, within: egui::Rect) -> bool {
        match s {
            egui::Shape::Path(p) => {
                matches!(p.stroke.color, egui::epaint::ColorMode::Solid(c) if c == egui::Color32::from(accent))
                    && !p.points.is_empty()
                    && p.points.iter().all(|at| within.contains(*at))
            }
            egui::Shape::Vec(v) => v.iter().any(|s| walk(s, accent, within)),
            _ => false,
        }
    }
    h.output().shapes.iter().any(|c| walk(&c.shape, accent, within))
}

/// Repository, Order and Entity (Order specializes it) in the package Shop, on two class diagrams,
/// the first open and Repository selected with Properties shown (AR-130). Gives the ids by name:
/// Repository, Order, Entity, diagram, second.
fn templates_sample(theme: ThemeKind) -> (Harness<'static, Shell>, std::collections::BTreeMap<&'static str, String>) {
    let mut h = workspace_harness(theme, egui::vec2(1600.0, 1500.0));
    let mut made = std::collections::BTreeMap::new();
    {
        let s = h.state_mut().session.as_mut().unwrap();
        let mut run = |cmd: &str, p: serde_json::Value| s.execute(cmd, &p).unwrap().value["id"].as_str().unwrap_or_default().to_string();
        let pkg = run("uml.package.add", serde_json::json!({ "name": "Shop" }));
        let repo = run("uml.class.add", serde_json::json!({ "package": pkg, "name": "Repository" }));
        let order = run("uml.class.add", serde_json::json!({ "package": pkg, "name": "Order" }));
        let entity = run("uml.class.add", serde_json::json!({ "package": pkg, "name": "Entity" }));
        run("uml.generalization.add", serde_json::json!({ "specific": order, "general": entity }));
        let mut diagrams = Vec::new();
        for (name, x) in [("Shop", 200), ("Overview", 500)] {
            let d = run("diagram.add", serde_json::json!({ "kind": archforge_uml::STRUCTURE_DIAGRAM, "element": pkg, "name": name }));
            run("diagram.place", serde_json::json!({ "diagram": d, "element": repo, "x": x, "y": 200 }));
            diagrams.push(d);
        }
        for (k, v) in [("Repository", repo), ("Order", order), ("Entity", entity), ("diagram", diagrams[0].clone()), ("second", diagrams[1].clone())]
        {
            made.insert(k, v);
        }
    }
    archforge_ui_egui::canvas::open(h.state_mut(), archforge_mof::Uuid::parse(&made["diagram"]).unwrap());
    h.run();
    h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [made["Repository"]] })).unwrap();
    h.state_mut().state.show_panel(archforge_shell_api::state::Panel::Properties);
    h.run();
    (h, made)
}

/// The rows of Repository's signature, as its card describes them.
fn signature_of(h: &Harness<'static, Shell>, c: &str) -> Vec<String> {
    let st = h.state().session.as_ref().unwrap().doc();
    archforge_uml::templates::signature_rows(st, st.by_uuid(archforge_mof::Uuid::parse(c).unwrap()).unwrap())
}

/// The id of `element`'s card on diagram `d`.
fn node_on(h: &Harness<'static, Shell>, element: &str, d: &str) -> String {
    let st = h.state().session.as_ref().unwrap().doc();
    let d = st.by_uuid(archforge_mof::Uuid::parse(d).unwrap()).unwrap();
    let n = archforge_diagram::views_of(st, st.by_uuid(archforge_mof::Uuid::parse(element).unwrap()).unwrap())
        .2
        .into_iter()
        .find(|n| archforge_diagram::diagram_of(st, *n) == Some(d))
        .unwrap();
    st.uuid(n).unwrap().to_string()
}

/// Picks entry `i` of the menu the select `id` opens.
fn pick_entry(h: &mut Harness<'static, Shell>, id: &str, entry: &str) {
    let at = rect_of(h, id).center();
    click(h, at);
    let at = rect_of(h, &format!("{id}.{entry}")).center();
    click(h, at);
}

/// The template sample with T (no default) and Key (a data type, Integer by default) on
/// Repository, and the class OrderRepository placed on the diagram beside it: gives the harness,
/// the sample's ids, and the cards of OrderRepository and Repository.
fn binding_sample(
    theme: ThemeKind,
) -> (Harness<'static, Shell>, std::collections::BTreeMap<&'static str, String>, (archforge_mof::Uuid, archforge_mof::Uuid)) {
    let (mut h, mut made) = templates_sample(theme);
    let repo = made["Repository"].clone();
    let s = h.state_mut().session.as_mut().unwrap();
    s.execute("uml.template-parameter.add", &serde_json::json!({ "classifier": repo, "name": "T" })).unwrap();
    s.execute("uml.template-parameter.add", &serde_json::json!({ "classifier": repo, "name": "Key", "kind": "data-type", "default": "Integer" }))
        .unwrap();
    let st = s.doc();
    let pkg = archforge_uml::commands::package_of(st, st.by_uuid(archforge_mof::Uuid::parse(&repo).unwrap()).unwrap()).unwrap();
    let pkg = st.uuid(pkg).unwrap().to_string();
    let bound = s.execute("uml.class.add", &serde_json::json!({ "package": pkg, "name": "OrderRepository" })).unwrap().value["id"]
        .as_str()
        .unwrap()
        .to_string();
    s.execute("diagram.place", &serde_json::json!({ "diagram": made["diagram"], "element": bound, "x": 200, "y": 600 })).unwrap();
    s.execute("view.select", &serde_json::json!({ "ids": [] })).unwrap();
    made.insert("OrderRepository", bound.clone());
    h.run();
    let nodes = (
        archforge_mof::Uuid::parse(&node_on(&h, &bound, &made["diagram"])).unwrap(),
        archforge_mof::Uuid::parse(&node_on(&h, &repo, &made["diagram"])).unwrap(),
    );
    (h, made, nodes)
}

/// The bindings in the harness's model.
fn bindings_in(h: &Harness<'static, Shell>) -> Vec<archforge_mof::Obj> {
    let st = h.state().session.as_ref().unwrap().doc();
    st.alive().filter(|o| archforge_uml::bindings::is_binding(st, *o)).collect()
}

/// The sample class model's Recommendations diagram (AR-128): Customer and Product side by side,
/// Catalog under them; `drawn`, the n-ary association recommends between them. In `theme`, in a
/// window tall enough for an end's editor. Gives the ids by name, as `put_recommends` does.
fn nary_sample(theme: ThemeKind, drawn: bool) -> (Harness<'static, Shell>, std::collections::BTreeMap<&'static str, String>) {
    nary_sample_in(theme, drawn, egui::vec2(1600.0, 1200.0))
}

/// [`nary_sample`] in a window of `size`.
fn nary_sample_in(theme: ThemeKind, drawn: bool, size: egui::Vec2) -> (Harness<'static, Shell>, std::collections::BTreeMap<&'static str, String>) {
    let mut h = workspace_harness(theme, size);
    archforge_ui_egui::snapshot::cx(h.state_mut(), uml_ui::open_sample).unwrap();
    h.run();
    let ids = archforge_ui_egui::snapshot::cx(h.state_mut(), |cx| uml_nary::put_recommends(cx, drawn)).unwrap();
    h.state_mut().canvas.fit_pending = false;
    h.state_mut().canvas.zoom = 1.0;
    h.state_mut().canvas.pan = egui::vec2(20.0, 30.0);
    h.state_mut().state.show_panel(archforge_shell_api::state::Panel::Properties);
    h.run();
    (h, ids)
}

/// The centre of the card `node` on the canvas.
fn card(h: &Harness<'static, Shell>, node: &str) -> egui::Pos2 {
    rect_of(h, &format!("workspace.canvas.node.{node}")).center()
}

/// Arms the Relationship tool with N-ary association, from its chevron's menu.
fn arm_nary(h: &mut Harness<'static, Shell>) {
    let at = rect_of(h, "workspace.tools.relationship-kinds").center();
    click(h, at);
    let at = rect_of(h, "workspace.relate.n-ary").center();
    click(h, at);
}

/// The n-ary associations of the model, each with its ends as `role : Class [multiplicity]`.
fn naries(h: &Harness<'static, Shell>) -> Vec<(archforge_mof::Obj, Vec<String>)> {
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
fn legs_on(h: &Harness<'static, Shell>, d: Option<&str>) -> usize {
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
fn diamonds_here(h: &Harness<'static, Shell>) -> usize {
    let st = h.state().session.as_ref().unwrap().doc();
    let d = st.by_uuid(h.state().canvas.active.unwrap()).unwrap();
    st.list(d, archforge_diagram::diagram::NODE)
        .unwrap()
        .iter()
        .filter_map(archforge_mof::Value::as_ref)
        .filter(|n| st.get(*n, archforge_diagram::node::ELEMENT).ok().and_then(|v| v.as_ref()).is_some_and(|e| archforge_uml::nary::is_nary(st, e)))
        .count()
}

/// The elements selected in the model shown.
fn selection(h: &Harness<'static, Shell>) -> Vec<archforge_mof::Obj> {
    h.state().session.as_ref().unwrap().view().selection.clone()
}

/// The accent-coloured line segments `h` painted last frame, each with its two ends.
fn accent_segments(h: &Harness<'static, Shell>) -> Vec<[egui::Pos2; 2]> {
    /// Collects the accent segments of `s` and of the shapes it holds into `out`.
    fn walk(s: &egui::Shape, accent: egui::Color32, out: &mut Vec<[egui::Pos2; 2]>) {
        match s {
            egui::Shape::LineSegment { points, stroke } if stroke.color == accent => out.push(*points),
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, accent, out)),
            _ => {}
        }
    }
    let accent: egui::Color32 = archforge_ui_egui::ux::palette(h.state().state.theme == ThemeKind::Dark).accent.into();
    let mut out = Vec::new();
    h.output().shapes.iter().for_each(|c| walk(&c.shape, accent, &mut out));
    out
}

/// The ids of the controls the last frame registered.
fn ids_of(h: &Harness<'static, Shell>) -> Vec<String> {
    ids(h)
}

/// The root's diagram, Top, with a card of Sales `size` big at (200, 160), and of the classes
/// `far` names at those x, far apart; Sales' own diagram. Gives Top, Sales, Sales' card and
/// Sales' diagram.
fn top_with_sales(
    h: &mut Harness<'static, Shell>,
    size: (u32, u32),
    far: &[i64],
) -> (archforge_mof::Uuid, archforge_mof::Uuid, archforge_mof::Uuid, archforge_mof::Uuid) {
    let id = |v: serde_json::Value| archforge_mof::Uuid::parse(v["id"].as_str().unwrap()).unwrap();
    let s = h.state_mut().session.as_mut().unwrap();
    let root = id(s.execute("uml.model.root", &serde_json::json!({})).unwrap().value).to_string();
    let p = serde_json::json!({ "kind": archforge_uml::STRUCTURE_DIAGRAM, "element": root, "name": "Top" });
    let top = id(s.execute("diagram.add", &p).unwrap().value);
    let sales = id(s.execute("uml.package.add", &serde_json::json!({ "name": "Sales" })).unwrap().value);
    let p = serde_json::json!({ "diagram": top.to_string(), "element": sales.to_string(), "x": 200, "y": 160, "width": size.0, "height": size.1 });
    let card = id(s.execute("diagram.place", &p).unwrap().value);
    for (i, x) in far.iter().enumerate() {
        let c = id(s.execute("uml.class.add", &serde_json::json!({ "package": root, "name": format!("Far{i}") })).unwrap().value);
        let p = serde_json::json!({ "diagram": top.to_string(), "element": c.to_string(), "x": x, "y": 0, "width": 200, "height": 120 });
        s.execute("diagram.place", &p).unwrap();
    }
    let sales_d = own_diagram(s, sales);
    (top, sales, card, sales_d)
}

/// Whether the canvas shows the card `card` at 100 %, centred, by its pan and zoom.
fn centred_at_landing(h: &Harness<'static, Shell>, card: archforge_mof::Uuid) -> bool {
    let st = h.state().session.as_ref().unwrap().doc();
    let b = archforge_diagram::grid::bounds_of(st, st.by_uuid(card).unwrap());
    let c = &h.state().canvas;
    let centre = egui::vec2((b.x + b.width / 2.0) as f32, (b.y + b.height / 2.0) as f32);
    c.zoom == 1.0 && (c.pan + centre - c.size / 2.0).length() < 1.0
}
