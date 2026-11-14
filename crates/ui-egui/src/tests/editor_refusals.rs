//! A refusal in a Properties editor brings the editor back into view, its refusal, Cancel and
//! Apply with it, once, as its opening does: no press needs the body scrolled by hand after it, and
//! a body scrolled away afterwards stays where it was put (AR-469). At the design's window height.

use serde_json::json;

use super::*;

/// The constraint editor's controls.
const APPLY: &str = "workspace.properties.constraint.apply";
const CANCEL: &str = "workspace.properties.constraint.cancel";

/// The sample class model in `theme` with Order's Properties shown; gives Order and its node.
fn order_properties(theme: ThemeKind) -> (Harness<'static, Shell>, String, String) {
    let (mut h, nodes) = related_sample_in(theme, egui::vec2(1600.0, 1000.0));
    let order = named(&h, archforge_uml::CLASS, "Order").to_string();
    crate::panels::run(h.state_mut(), "view.select", json!({ "ids": [order] })).unwrap();
    h.state_mut().state.show_panel(crate::state::Panel::Properties);
    h.run();
    (h, order, nodes["Order"].clone())
}

/// The centre of the Properties control `id` while it is inside the body as drawn, with no wheel.
fn in_view(h: &Harness<'static, Shell>, id: &str) -> Option<egui::Pos2> {
    let body = h.state().panels.body?;
    let w = h.state().widgets().iter().find(|w| w.id == id)?;
    Some(egui::pos2((w.rect[0] + w.rect[2]) / 2.0, (w.rect[1] + w.rect[3]) / 2.0)).filter(|at| body.contains(*at))
}

/// Whether the text `said` was painted inside the Properties body in the last frame.
fn said_in_view(h: &Harness<'static, Shell>, said: &str) -> bool {
    /// The rectangles of the texts `said` in the shape `s` and the shapes it holds.
    fn walk(s: &egui::Shape, said: &str, out: &mut Vec<egui::Rect>) {
        match s {
            egui::Shape::Text(t) if t.galley.text() == said => out.push(t.galley.rect.translate(t.pos.to_vec2())),
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, said, out)),
            _ => {}
        }
    }
    let Some(body) = h.state().panels.body else { return false };
    let mut rects = Vec::new();
    h.output().shapes.iter().for_each(|c| walk(&c.shape, said, &mut rects));
    rects.iter().any(|r| body.contains_rect(*r))
}

/// The body wheeled by `dy` points with the pointer over it, as a person scrolls it.
fn wheel(h: &mut Harness<'static, Shell>, dy: f32) {
    let body = h.state().panels.body.expect("the Properties body");
    h.event(egui::Event::PointerMoved(body.center()));
    h.event(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: egui::vec2(0.0, dy),
        modifiers: egui::Modifiers::NONE,
        phase: egui::TouchPhase::Move,
        source: egui::MouseWheelSource::Wheel,
    });
    h.run();
}

/// Order's constraint editor opened from its section, at the foot of the body, with braces picked
/// for a class the diagram no longer shows: Apply is refused above the buttons.
fn constraint_refused(theme: ThemeKind) -> Harness<'static, Shell> {
    let (mut h, _, node) = order_properties(theme);
    crate::panels::run(h.state_mut(), "diagram.unplace", json!({ "nodes": [node] })).unwrap();
    h.run();
    let at = reached_in_properties(&mut h, "workspace.properties.constraints.add");
    click(&mut h, at);
    let edit = h.state_mut().panels.constraint.as_mut().expect("the editor");
    (edit.expression, edit.form, edit.form_chosen) = ("total > 0".into(), archforge_uml::constraints::Form::Braces, true);
    h.run();
    let before = in_view(&h, APPLY).expect("the editor opened in view");
    click(&mut h, before);
    let edit = h.state().panels.constraint.clone().expect("the editor stays open");
    assert_eq!(edit.refused_at, Some("show"), "{theme:?}: refused above the buttons");
    h
}

/// A constraint's Apply refused above its buttons, then under them: each time the refusal and both
/// buttons are in view, and Apply and Cancel are pressed where they show, with no wheel.
#[test]
fn a_refused_constraint_keeps_its_buttons_in_view() {
    in_both_appearances(|theme| {
        let mut h = constraint_refused(theme);
        let braces = h.state().panels.constraint.as_ref().and_then(|e| e.refused.clone()).unwrap_or_default();
        assert!(said_in_view(&h, &braces), "{theme:?}: the refusal is in view");
        let at = in_view(&h, APPLY).expect("Apply in view after the refusal");
        assert!(in_view(&h, CANCEL).is_some(), "{theme:?}: Cancel in view after the refusal");
        // The diagram as the element: refused under the buttons, which stay in view with it.
        let shown = h.state().canvas.active.unwrap();
        h.state_mut().panels.constraint.as_mut().expect("the editor").elements = vec![shown];
        click(&mut h, at);
        let edit = h.state().panels.constraint.clone().expect("the editor stays open");
        let outside = crate::i18n::tr("A constraint names only elements of this model.");
        assert_eq!((edit.refused.as_deref(), edit.refused_at), (Some(outside.as_str()), None), "{theme:?}");
        assert!(said_in_view(&h, &outside), "{theme:?}: the refusal under the buttons is in view");
        assert!(in_view(&h, APPLY).is_some(), "{theme:?}: Apply in view after the second refusal");
        let at = in_view(&h, CANCEL).expect("Cancel in view after the second refusal");
        click(&mut h, at);
        assert!(h.state().panels.constraint.is_none(), "{theme:?}: Cancel closes the editor");
    });
}

/// The body scrolled away from a refused editor stays where it was put: the refusal brought the
/// editor into view once, not on every frame, and its going brings nothing into view either.
#[test]
fn a_body_scrolled_away_from_a_refusal_stays_put() {
    in_both_appearances(|theme| {
        let mut h = constraint_refused(theme);
        wheel(&mut h, 240.0);
        let put = h.state().panels.scroll;
        assert!(in_view(&h, APPLY).is_none(), "{theme:?}: Apply scrolled out of view");
        for _ in 0..3 {
            h.run();
        }
        assert_eq!(h.state().panels.scroll, put, "{theme:?}: nothing pulls the body back");
        // A linked note picked instead: the refusal goes, the card shrinks, and the body stays.
        let at = in_view(&h, "workspace.properties.constraint.show.note").expect("Linked note in view");
        click(&mut h, at);
        assert_eq!(h.state().panels.constraint.as_ref().and_then(|e| e.refused.clone()), None, "{theme:?}: the refusal went");
        assert!(in_view(&h, APPLY).is_none(), "{theme:?}: Apply is still out of view");
        assert_eq!(h.state().panels.scroll, put, "{theme:?}: the refusal's going moved nothing");
    });
}

/// A member's editor shares the card: an attribute added at the foot of a long list, refused for a
/// name another attribute has, keeps its refusal and buttons in view.
#[test]
fn a_refused_attribute_keeps_its_buttons_in_view() {
    in_both_appearances(|theme| {
        let (mut h, order, _) = order_properties(theme);
        for i in 0..16 {
            crate::panels::run(h.state_mut(), "uml.attribute.add", json!({ "class": order, "name": format!("note{i}") })).unwrap();
        }
        h.run();
        let at = reached_in_properties(&mut h, "workspace.properties.attributes.add");
        click(&mut h, at);
        h.state_mut().panels.member.as_mut().expect("the editor").name = "note0".into();
        h.run();
        let at = in_view(&h, "workspace.properties.member.apply").expect("the editor opened in view");
        click(&mut h, at);
        let why = h.state().panels.member.as_ref().and_then(|e| e.refused.clone()).expect("the name is refused");
        assert!(why.contains("note0"), "{theme:?}: {why}");
        assert!(said_in_view(&h, &why), "{theme:?}: the refusal is in view");
        assert!(in_view(&h, "workspace.properties.member.apply").is_some(), "{theme:?}: Apply in view");
        let at = in_view(&h, "workspace.properties.member.cancel").expect("Cancel in view");
        click(&mut h, at);
        assert!(h.state().panels.member.is_none(), "{theme:?}: Cancel closes the editor");
    });
}
