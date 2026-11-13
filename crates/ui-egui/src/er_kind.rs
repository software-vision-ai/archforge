//! The ER diagram's tools and gestures: the Entity tool places a table named in
//! place, the Relationship tool draws a relationship with the ends its popover sets, a line dropped
//! on empty canvas makes a new table there, a table renames in place, a double click on a column
//! opens its editor, the second adorner adds a column, and a right click opens the entity menu.

use archforge_mof::{Obj, Store, Uuid};
use egui::{Pos2, Rect};

use crate::Shell;
use crate::canvas::View;
use crate::i18n::tr;
use crate::kinds::{DiagramKind, KindTool, ToolUse};
use crate::ux::{Palette, icons};

/// The ER diagram kind in the shell.
pub(crate) struct ErKind;

const TOOLS: &[KindTool] = &[
    KindTool { key: "entity", icon: &icons::TOOL_ENTITY, hint: "Entity", use_: ToolUse::Place, chevron: None, wide: false },
    KindTool {
        key: "relationship",
        icon: &icons::TOOL_RELATIONSHIP,
        hint: "Relationship",
        use_: ToolUse::Connect,
        chevron: Some(("relationship-types", "Relationship types")),
        wide: true,
    },
];

impl DiagramKind for ErKind {
    fn kind(&self) -> &'static str {
        archforge_er::DIAGRAM_KIND
    }

    fn tools(&self) -> &'static [KindTool] {
        TOOLS
    }

    fn word(&self) -> String {
        tr("ER diagram")
    }

    fn group_word(&self) -> String {
        tr("ER diagrams")
    }

    fn description(&self) -> String {
        tr("Tables, their columns and keys, and the relationships between them.")
    }

    fn new_word(&self) -> String {
        tr("New ER diagram")
    }

    /// Two tables, Customer and Order, joined by a relationship with its many end.
    fn sketch(&self, paint: &egui::Painter, p: &crate::ux::Palette, r: egui::Rect) {
        use egui::{Stroke, StrokeKind, pos2, vec2};
        let at = |x: f32, y: f32| r.center() + vec2(x - 134.0, y - 50.0);
        // The sample tables' names, in the user's language, as the rest of the card is.
        for (x, name) in [(24.0, tr("Customer")), (160.0, tr("Order"))] {
            let table = egui::Rect::from_min_size(at(x, 20.0), vec2(84.0, 58.0));
            paint.rect_filled(table, 5.0, p.entity);
            paint.rect_stroke(table, 5.0, Stroke::new(1.0, p.entity_edge), StrokeKind::Inside);
            crate::ux::text(paint, table.min + vec2(8.0, 3.0), &name, crate::ux::types::TINY, p.text, Some(table.width() - 12.0));
            paint.line_segment([pos2(table.left(), table.top() + 18.0), pos2(table.right(), table.top() + 18.0)], Stroke::new(1.0, p.entity_line));
            for (i, w) in [52.0, 38.0].into_iter().enumerate() {
                paint.rect_filled(egui::Rect::from_min_size(table.min + vec2(8.0, 26.0 + i as f32 * 11.0), vec2(w, 4.0)), 2.0, p.entity_line);
            }
        }
        let (from, to) = (at(108.0, 49.0), at(160.0, 49.0));
        let ink = Stroke::new(1.2, p.text_secondary);
        paint.line_segment([from, to], ink);
        paint.line_segment([to + vec2(-6.0, -5.0), to], ink);
        paint.line_segment([to + vec2(-6.0, 5.0), to], ink);
    }

    fn owner_word(&self) -> String {
        tr("Schema")
    }

    fn new_owner(&self) -> String {
        tr("New schema")
    }

    fn owners(&self, store: &Store) -> Vec<Obj> {
        let Some(model) = archforge_core::skills::model_of(store) else { return Vec::new() };
        store
            .list(model, archforge_core::model::MEMBER)
            .unwrap_or_default()
            .iter()
            .filter_map(archforge_mof::Value::as_ref)
            .filter(|o| store.is_alive(*o) && store.class_of(*o).ok() == Some(archforge_er::SCHEMA))
            .collect()
    }

    fn owner_of(&self, store: &Store, selected: Obj) -> Option<Obj> {
        if store.class_of(selected).ok() == Some(archforge_er::SCHEMA) { Some(selected) } else { archforge_er::rules::schema_of(store, selected) }
    }

    /// A schema named `name` for a new ER diagram to be of; a refusal comes back in plain words
    /// ([`crate::panels::refusal`]).
    fn add_owner(&self, shell: &mut Shell, name: &str) -> Result<String, String> {
        crate::panels::run(shell, "er.schema.add", serde_json::json!({ "name": name }))
            .map(|v| v.get("id").and_then(|i| i.as_str()).unwrap_or_default().to_string())
    }

    fn status_marks(&self) -> bool {
        true
    }

    /// The relationship tool shows the ends it draws, as the cardinality popover sets them.
    fn tool_face(&self, shell: &Shell, paint: &egui::Painter, p: &Palette, key: &str, b: Rect, ink: egui::Color32, active: bool) -> bool {
        if key != "relationship" {
            return false;
        }
        let kind = shell.canvas.relationship;
        let fill = if active { p.accent_tint } else { p.surface };
        crate::connect::tool_line(paint, p, b.center(), kind.from, kind.to, ink, fill);
        true
    }

    /// The cardinality popover, ER's one chevron.
    fn popover(&self, shell: &mut Shell, ui: &egui::Ui, p: &Palette, r: Rect, chevron: Rect, _key: &str) {
        crate::connect::popover(shell, ui, p, r, chevron);
    }

    fn takes_part(&self, store: &Store, element: Obj) -> bool {
        store.class_of(element).ok() == Some(archforge_er::ENTITY)
    }

    fn place(&self, shell: &mut Shell, _tool: &'static str, d: Obj, x: f64, y: f64) {
        crate::editing::place_entity(shell, d, x, y);
    }

    fn connect(&self, shell: &mut Shell, _tool: &'static str, d: Obj, from_node: Uuid, to_node: Uuid) {
        crate::connect::connect(shell, d, from_node, to_node);
    }

    fn connect_to_new(&self, shell: &mut Shell, _tool: &'static str, d: Obj, from_node: Uuid, x: f64, y: f64) {
        crate::editing::connect_to_new(shell, d, from_node, x, y);
    }

    fn rename(&self, shell: &mut Shell, element: Uuid, name: &str) -> Result<(), String> {
        crate::editing::run(shell, "er.entity.rename", serde_json::json!({ "entity": element.to_string(), "name": name })).map(|_| ())
    }

    /// A column's editor, or a new column's past the last.
    fn open(&self, shell: &mut Shell, element: Uuid, row: Option<usize>) {
        let column = row.and_then(|row| {
            let store = shell.session.as_ref()?.doc();
            let e = store.by_uuid(element)?;
            let columns: Vec<Obj> = store
                .list(e, archforge_er::entity::ATTRIBUTE)
                .unwrap_or_default()
                .iter()
                .filter_map(archforge_mof::Value::as_ref)
                .filter(|c| store.is_alive(*c))
                .collect();
            store.uuid(*columns.get(row)?).ok()
        });
        crate::panels::open_column(shell, element, column);
        shell.state.show_panel(crate::state::Panel::Properties);
    }

    /// A table's second adorner adds a column.
    fn adorner(&self, _store: &Store, _element: Obj) -> Option<(&'static str, &'static str)> {
        Some(("add-column", "Add column"))
    }

    fn drop(&self, shell: &mut Shell, d: Obj, element: Uuid, at: (f64, f64)) {
        crate::editing::place_dropped(shell, d, element, at);
    }

    fn menu(&self) -> bool {
        true
    }

    fn live_line(&self, shell: &Shell, paint: &egui::Painter, p: &Palette, view: View, from_node: Uuid, pointer: Pos2, target: Option<Rect>) {
        crate::connect::live_line(shell, paint, p, view, from_node, pointer, target);
    }
}
