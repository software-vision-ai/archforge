//! ER's refusals in the chosen language that take a row of the shell's catalogue, which ER's
//! screens crate's test context does not register: said through the whole window.

use super::*;

/// An index column's direction or nulls order that is none of the words, or no word, is said in
/// the chosen language under the choice's label, with the words taken, one of only spaces as no
/// choice given, though not one whose word comes after 40 spaces; a word of another place is not ER's to say. In a window of the design's height, in light and dark (AR-491).
#[test]
fn an_er_index_column_word_refused_is_said_in_the_chosen_language() {
    use serde_json::json;
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let mut h = workspace_harness(theme, egui::vec2(1600.0, 1120.0));
        h.state_mut().state.language = "de".into();
        h.run();
        let mut id =
            |cmd: &str, p: serde_json::Value| archforge_ui_egui::snapshot::run(h.state_mut(), cmd, p).unwrap()["id"].as_str().unwrap().to_string();
        let schema = id("er.schema.add", json!({ "name": "Shop" }));
        let table = id("er.entity.add", json!({ "schema": schema, "name": "Kunde" }));
        let column = id("er.attribute.add", json!({ "entity": table, "name": "nummer", "type": "BIGINT" }));
        let mut refused = |field: &str, value: serde_json::Value| {
            let p = json!({ "entity": table, "columns": [{ "attribute": column, field: value }] });
            archforge_ui_egui::snapshot::run(h.state_mut(), "er.index.add", p).err().unwrap_or_default()
        };
        let up = "Richtung: up ist keine der Möglichkeiten. Möglich sind ascending, descending.";
        assert_eq!(refused("direction", json!("up")), up, "{theme:?}");
        let array = "Nullwerte: [1] ist keine der Möglichkeiten. Möglich sind default, first, last.";
        assert_eq!(refused("nulls", json!([1])), array, "{theme:?}");
        let none = "Richtung: Es ist keine Möglichkeit angegeben. Möglich sind ascending, descending.";
        assert_eq!(refused("direction", json!(" ")), none, "{theme:?}");
        // A word after more spaces than the typed word keeps is no blank.
        assert_eq!(refused("direction", json!(format!("{}up", " ".repeat(40)))), up, "{theme:?}");
        let diagram = archforge_engine::NotOneOf {
            what: archforge_engine::WordPlace::DiagramKind,
            word: "x".into(),
            known: vec![archforge_er::DIAGRAM_KIND.into()],
        };
        assert_eq!(er_said::said(SCREENS.notation_uis(), &archforge_engine::Reason::NotOneOf(Box::new(diagram))), None, "{theme:?}");
        h.run();
    }
    archforge_ui_egui::i18n::set_language("en");
}
