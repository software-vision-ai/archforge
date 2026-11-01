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
    KindTool { key: "entity", icon: &icons::TOOL_ENTITY, hint: "Entity", use_: ToolUse::Place, chevron: None },
    KindTool {
        key: "relationship",
        icon: &icons::TOOL_RELATIONSHIP,
        hint: "Relationship",
        use_: ToolUse::Connect,
        chevron: Some(("relationship-types", "Relationship types")),
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

    fn add_owner(&self, shell: &mut Shell, name: &str) -> Result<String, String> {
        crate::editing::run(shell, "er.schema.add", serde_json::json!({ "name": name }))
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

    fn popover(&self, shell: &mut Shell, ui: &egui::Ui, p: &Palette, r: Rect, chevron: Rect) {
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

    fn adorner(&self) -> Option<(&'static str, &'static str)> {
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
