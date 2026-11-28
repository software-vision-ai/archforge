//! A version's details over a UML diagram: its two sides draw the kinds' own figures.

use archforge_notations::differences::Area;

use super::*;

/// Three versions, newest first, each naming the one before it; the last is the first version.
fn history() -> Vec<archforge_ui_egui::VersionEntry> {
    let mut newest = version_row("c3", Some("Payment methods split out"), (Some("Dejan"), false), "2026-11-07T16:15:00Z");
    let mut middle = version_row("c2", Some("First classes"), (None, false), "2026-11-07T08:20:00Z");
    let first = version_row("c1", None, (None, false), "2026-11-06T15:05:00Z");
    (newest.previous, middle.previous) = (Some("c2".into()), Some("c1".into()));
    vec![newest, middle, first]
}

/// Clicks a control.
fn tap(h: &mut Harness<'static, Shell>, id: &str) {
    let at = rect_of(h, id).center();
    click(h, at);
}

/// Frames until `done`; a bound only against a hang.
fn frames_until(h: &mut Harness<'static, Shell>, done: impl Fn(&Harness<'static, Shell>) -> bool) {
    let until = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while !done(h) {
        assert!(std::time::Instant::now() < until, "still waiting");
        h.run_steps(1);
    }
    h.run_steps(1);
}

/// Frames until the working changes list has its answer: an edit's answer moves the rows below it.
fn list_still(h: &mut Harness<'static, Shell>) {
    h.run_steps(2);
    frames_until(h, |h| !archforge_ui_egui::testing::comparing(h.state()));
}

const BEFORE: &str = "workspace.change-view.before";

/// Runs the command `cmd` on the session; gives the id it answers with, if any.
fn edit(h: &mut Harness<'static, Shell>, cmd: &str, args: serde_json::Value) -> String {
    h.state_mut().session.as_mut().unwrap().execute(cmd, &args).unwrap().value["id"].as_str().unwrap_or_default().to_string()
}

/// A UML diagram's two sides draw its kind's own figures, as the canvas does (AR-788): with a
/// generalization set in the last version and a class placed since, both sides show the set's line
/// and label across its member's line.
#[test]
fn a_uml_diagrams_two_sides_draw_its_kinds_own_figures() {
    in_both_appearances(|theme| {
        let (mut h, host) = versions_panel(theme);
        let sales = edit(&mut h, "uml.package.add", serde_json::json!({ "name": "Sales" }));
        let class = |h: &mut _, name: &str| edit(h, "uml.class.add", serde_json::json!({ "package": sales, "name": name }));
        let (party, customer, fresh) = (class(&mut h, "Party"), class(&mut h, "Customer"), class(&mut h, "Fresh"));
        let diagram =
            edit(&mut h, "diagram.add", serde_json::json!({ "kind": archforge_uml::STRUCTURE_DIAGRAM, "element": sales, "name": "Billing" }));
        let place = |h: &mut _, e: &str, x: i32, y: i32| {
            edit(h, "diagram.place", serde_json::json!({ "diagram": diagram, "element": e, "x": x, "y": y, "width": 200, "height": 120 }))
        };
        place(&mut h, &party, 0, 0);
        place(&mut h, &customer, 0, 300);
        let g = edit(&mut h, "uml.generalization.add", serde_json::json!({ "specific": customer, "general": party }));
        edit(&mut h, "diagram.connect-all", serde_json::json!({ "diagram": diagram }));
        let set = edit(&mut h, "uml.generalization-set.add", serde_json::json!({ "generalizations": [g], "name": "Kinds" }));
        versions_answer(&mut h, &host, Ok(archforge_ui_egui::VersionsPage { versions: history(), more: false }));
        h.run_steps(2);
        let rows = rows_now(&h);
        answer_read(&mut h, &host, Ok(rows));
        super::compared(&mut h);
        let node = place(&mut h, &fresh, 400, 0);
        list_still(&mut h);
        let row = archforge_ui_egui::snapshot::changes(h.state()).iter().find(|r| r.area == Area::Diagram && r.id.to_string() == node).unwrap();
        let row = format!("workspace.versions.change.{}.{}", row.id, row.class.0);
        tap(&mut h, &row);
        frames_until(&mut h, |h| !archforge_ui_egui::testing::reading_element(h.state()) && has_id(h, BEFORE));
        for side in ["before", "after"] {
            for id in [
                format!("workspace.change-view.{side}.generalizationSet.{set}"),
                format!("workspace.change-view.{side}.generalizationSet.{set}.line"),
            ] {
                assert!(has_id(&h, &id), "{theme:?}: {id}");
            }
        }
    });
}
