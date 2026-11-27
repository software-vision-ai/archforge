//! ER's tables on its diagrams (from the design's frame "ER · Empty diagram · First entity named
//! in place"): the Entity tool places a table and opens its name for typing in the node's header,
//! a relationship dropped on empty canvas makes a new table there joined to the first, a table
//! dragged from Models lands with its lines to the tables the diagram shows, and a table added from
//! Models lands on an open diagram of its schema; a table's boxes grow to show its columns, with a
//! column command or a foreign key in the same undo step, and when a project opens. The shared
//! canvas reaches it through ER's kind ([`crate::er_kind`]) and ER's panels call it.

use archforge_diagram::grid::bounds_of;
use archforge_mof::{Obj, Store, Uuid};
use serde_json::json;

use crate::canvas::{ENTITY_FIRST_ROW, ENTITY_ROW, Tool};
use crate::cx::Ctx;
use crate::editing::{NodeRename, element_of, nodes_of, on_diagram, place_with_lines_cx, select_cx, unit_rect, uuid};
use crate::er_state::er;
use crate::i18n::tr;

/// A new entity's node: the design's width, and its height without columns.
pub const ENTITY_WIDTH: f64 = 240.0;
pub const ENTITY_EMPTY_HEIGHT: f64 = 72.0;

/// The table node `n` shows, when it shows one.
fn entity_of_node(store: &Store, n: Obj) -> Option<Obj> {
    element_of(store, n).filter(|e| store.class_of(*e).ok() == Some(archforge_er::ENTITY))
}

/// The columns an entity node shows, and the height that shows them all.
pub fn content_height(columns: usize) -> f64 {
    if columns == 0 { ENTITY_EMPTY_HEIGHT } else { (ENTITY_FIRST_ROW + 9.0) as f64 + columns as f64 * ENTITY_ROW as f64 }
}

/// The name a new entity gets: "Entity", then "Entity 2" and on, never one its schema has.
fn entity_name(store: &Store, schema: Obj) -> Option<String> {
    let taken: Vec<String> = store
        .list(schema, archforge_er::schema::ENTITY)
        .unwrap_or_default()
        .iter()
        .filter_map(archforge_mof::Value::as_ref)
        .map(|e| archforge_core::name_of(store, e).to_lowercase())
        .collect();
    let stem = tr("Entity");
    if !taken.contains(&stem.to_lowercase()) {
        return Some(stem);
    }
    (2..=10_000u32).map(|n| format!("{stem} {n}")).find(|name| !taken.contains(&name.to_lowercase()))
}

/// Adds a table to the diagram's schema and places it at `(x, y)`, its centre under the click,
/// as one undo step; then its name is open for typing.
pub(crate) fn place_entity(cx: &mut Ctx, d: Obj, x: f64, y: f64) {
    cx.canvas.notice = None;
    let Some(session) = cx.session.as_mut() else { return };
    let store = session.doc();
    let Some(schema) = crate::diagrams::schema_of(store, d) else {
        cx.canvas.notice = Some(tr("The diagram is of no schema"));
        return;
    };
    let (Some(schema_id), Some(diagram_id)) = (uuid(store, schema), uuid(store, d)) else { return };
    let Some(name) = entity_name(store, schema) else {
        cx.canvas.notice = Some(tr("No free name is left"));
        return;
    };
    let (px, py) = ((x - ENTITY_WIDTH / 2.0).round(), (y - ENTITY_EMPTY_HEIGHT / 2.0).round());
    let name_for_batch = name.clone();
    let out = session.batch(&tr("Add table"), |b| {
        let e = b.execute("er.entity.add", &json!({ "schema": schema_id.to_string(), "name": name_for_batch }))?;
        let entity = e.get("id").and_then(|i| i.as_str()).unwrap_or_default().to_string();
        b.execute(
            "diagram.place",
            &json!({ "diagram": diagram_id.to_string(), "element": entity, "x": px, "y": py, "width": ENTITY_WIDTH, "height": ENTITY_EMPTY_HEIGHT }),
        )?;
        Ok(())
    });
    match out {
        Ok(o) => {
            let id_at = |i: usize| o.value.get(i).and_then(|v| v.get("id")).and_then(|v| v.as_str()).and_then(Uuid::parse);
            if let (Some(entity), Some(node)) = (id_at(0), id_at(1)) {
                select_cx(cx, vec![entity]);
                cx.canvas.rename = Some(NodeRename { node, entity, text: name, opened: true, refused: None });
                cx.canvas.focused = true;
            }
            cx.canvas.tool = Tool::Select;
        }
        Err(e) => cx.canvas.notice = Some(crate::panels::refusal(cx.notation_uis, &e)),
    }
}

/// A connection dropped on empty canvas: a new table centred there, joined to the table of
/// `from_node` by the relationship the tool is set to, all one undo step; then the new table's name
/// is open for typing. The relationship's foreign key starts off (and so it is not identifying):
/// the new table is still to be shaped, and its keys come when the user turns the key on.
pub(crate) fn connect_to_new(cx: &mut Ctx, d: Obj, from_node: Uuid, x: f64, y: f64) {
    use crate::connect::cardinality_word;
    cx.canvas.notice = None;
    let kind = er(cx.notation_states).map(|er| er.relationship).unwrap_or_default();
    let Some(session) = cx.session.as_mut() else { return };
    let store = session.doc();
    let Some(schema) = crate::diagrams::schema_of(store, d) else {
        cx.canvas.notice = Some(tr("The diagram is of no schema"));
        return;
    };
    let Some(from) = store.by_uuid(from_node).and_then(|n| entity_of_node(store, n)) else {
        cx.canvas.notice = Some(tr("A relationship joins two tables"));
        return;
    };
    let (Some(schema_id), Some(diagram_id), Some(from_id)) = (uuid(store, schema), uuid(store, d), uuid(store, from)) else { return };
    let Some(name) = entity_name(store, schema) else {
        cx.canvas.notice = Some(tr("No free name is left"));
        return;
    };
    let (px, py) = ((x - ENTITY_WIDTH / 2.0).round(), (y - ENTITY_EMPTY_HEIGHT / 2.0).round());
    let name_for_batch = name.clone();
    let out = session.batch(&tr("Add table"), |b| {
        let e = b.execute("er.entity.add", &json!({ "schema": schema_id.to_string(), "name": name_for_batch }))?;
        let entity = e.get("id").and_then(|i| i.as_str()).unwrap_or_default().to_string();
        let n = b.execute(
            "diagram.place",
            &json!({ "diagram": diagram_id.to_string(), "element": &entity, "x": px, "y": py, "width": ENTITY_WIDTH, "height": ENTITY_EMPTY_HEIGHT }),
        )?;
        let node = n.get("id").and_then(|i| i.as_str()).unwrap_or_default().to_string();
        let r = b.execute(
            "er.relationship.add",
            &json!({
                "from": from_id.to_string(),
                "to": &entity,
                "from_end": cardinality_word(kind.from),
                "to_end": cardinality_word(kind.to),
                "identifying": false,
                "create_foreign_key": false,
            }),
        )?;
        let rel = r.get("id").and_then(|i| i.as_str()).unwrap_or_default().to_string();
        let edge = b.execute(
            "diagram.connect",
            &json!({ "diagram": diagram_id.to_string(), "element": rel, "source": from_node.to_string(), "target": node }),
        )?;
        let edge = edge.get("id").and_then(|i| i.as_str()).unwrap_or_default().to_string();
        b.execute("diagram.label", &json!({ "edge": edge, "kind": "name", "at": "middle" }))?;
        Ok(())
    });
    match out {
        Ok(o) => {
            let id_at = |i: usize| o.value.get(i).and_then(|v| v.get("id")).and_then(|v| v.as_str()).and_then(Uuid::parse);
            if let (Some(entity), Some(node)) = (id_at(0), id_at(1)) {
                select_cx(cx, vec![entity]);
                cx.canvas.rename = Some(NodeRename { node, entity, text: name, opened: true, refused: None });
                cx.canvas.focused = true;
            }
        }
        Err(e) => cx.canvas.notice = Some(crate::panels::refusal(cx.notation_uis, &e)),
    }
}

/// Places a table dragged from the Models explorer on diagram `d`, centred on `at`; a table of
/// another schema is welcome (slice S7), one already on the diagram is said so and left.
pub(crate) fn place_dropped(cx: &mut Ctx, d: Obj, entity: Uuid, at: (f64, f64)) {
    cx.canvas.notice = None;
    let Some(session) = cx.session.as_ref() else { return };
    let store = session.doc();
    let Some(e) = store.by_uuid(entity).filter(|e| store.class_of(*e).ok() == Some(archforge_er::ENTITY)) else { return };
    if on_diagram(store, d, e) {
        cx.canvas.notice = Some(tr("Already on this diagram"));
        return;
    }
    let columns = store.list(e, archforge_er::entity::ATTRIBUTE).map(|l| l.len()).unwrap_or(0);
    // A very wide table lands as tall as a node may be; its rows past that say how many more.
    let height = content_height(columns).min(archforge_diagram::rules::MAX_SIZE);
    // Its relationships and subtype lines to the tables the diagram shows draw with it.
    let links: Vec<Obj> = store
        .referrers(e)
        .into_iter()
        .filter(|(_, prop)| *prop == archforge_er::relationship::FROM || *prop == archforge_er::relationship::TO)
        .map(|(r, _)| r)
        .chain(archforge_er::rules::supertype_links(store, e))
        .chain(archforge_er::rules::subtype_links(store, e))
        .collect();
    place_with_lines_cx(cx, d, e, at, (ENTITY_WIDTH, height), &links, &archforge_er::ErNotation, &tr("Place table"));
}

/// Where a new table lands on the open diagram when the diagram is of `schema`: to the right of
/// the rightmost node, or near the top left of an empty diagram; none when no such diagram is open.
fn free_place(cx: &Ctx, schema: Obj) -> Option<(Uuid, f64, f64)> {
    let session = cx.session.as_ref()?;
    let store = session.doc();
    let d = cx.canvas.active.and_then(|id| store.by_uuid(id)).filter(|d| store.is_alive(*d))?;
    if crate::diagrams::schema_of(store, d) != Some(schema) {
        return None;
    }
    let nodes = nodes_of(store, d);
    let (x, y) = match nodes.iter().map(|n| unit_rect(store, *n)).filter(|r| r.is_finite()).max_by(|a, b| a.right().total_cmp(&b.right())) {
        Some(r) => ((r.right() + 40.0) as f64, r.top() as f64),
        None => (40.0, 40.0),
    };
    Some((uuid(store, d)?, x, y))
}

/// Adds a table to `schema` with `name`; on an open diagram of the schema it is placed as well,
/// in the same undo step. The table is selected either way. Gives the table's id.
pub(crate) fn add_entity(cx: &mut Ctx, schema: Uuid, name: &str) -> Result<Uuid, String> {
    let placed = cx.session.as_ref().and_then(|s| s.doc().by_uuid(schema)).and_then(|s| free_place(cx, s));
    let session = cx.session.as_mut().ok_or_else(|| tr("The model could not be opened"))?;
    let add = json!({ "schema": schema.to_string(), "name": name });
    let entity = match placed {
        None => session.execute("er.entity.add", &add).map(|o| o.value).map_err(|e| crate::panels::refusal(cx.notation_uis, &e))?,
        Some((diagram, x, y)) => {
            let out = session
                .batch(&tr("Add table"), |b| {
                    let e = b.execute("er.entity.add", &add)?;
                    let entity = e.get("id").and_then(|i| i.as_str()).unwrap_or_default().to_string();
                    b.execute(
                        "diagram.place",
                        &json!({ "diagram": diagram.to_string(), "element": entity, "x": x, "y": y, "width": ENTITY_WIDTH, "height": ENTITY_EMPTY_HEIGHT }),
                    )?;
                    Ok(())
                })
                .map_err(|e| crate::panels::refusal(cx.notation_uis, &e))?;
            out.value.get(0).cloned().unwrap_or(serde_json::Value::Null)
        }
    };
    let id = entity.get("id").and_then(|i| i.as_str()).and_then(Uuid::parse).ok_or_else(|| tr("The model could not be opened"))?;
    // Placing selected the node; the table is what the explorer and Properties follow.
    select_cx(cx, vec![id]);
    Ok(id)
}

/// Runs a column command and, in the same undo step, grows every node of `entity` whose height
/// would hide the columns it then has.
pub(crate) fn run_growing(cx: &mut Ctx, label: &str, cmd: &str, params: serde_json::Value, entity: Uuid) -> Result<serde_json::Value, String> {
    let Some(session) = cx.session.as_mut() else { return Err(tr("The model could not be opened")) };
    let store = session.doc();
    let Some(e) = store.by_uuid(entity) else { return Err(tr("The model could not be opened")) };
    let columns = store.list(e, archforge_er::entity::ATTRIBUTE).map(|l| l.len()).unwrap_or(0);
    let needed = content_height(columns.saturating_add(1));
    let (_, _, nodes) = archforge_diagram::views_of(store, e);
    let grow: Vec<(String, f64)> = nodes
        .iter()
        .filter_map(|n| {
            let b = bounds_of(store, *n);
            if b.height < needed { Some((uuid(store, *n)?.to_string(), b.width)) } else { None }
        })
        .collect();
    if grow.is_empty() {
        return session.execute(cmd, &params).map(|o| o.value).map_err(|e| crate::panels::refusal(cx.notation_uis, &e));
    }
    session
        .batch(label, |b| {
            b.execute(cmd, &params)?;
            for (n, width) in &grow {
                b.execute("diagram.resize", &json!({ "node": n, "width": width, "height": needed }))?;
            }
            Ok(())
        })
        .map(|o| o.value.get(0).cloned().unwrap_or(serde_json::Value::Null))
        .map_err(|e| crate::panels::refusal(cx.notation_uis, &e))
}

/// Grows every table box of the model read that is too short for its columns, before the project's
/// session starts: a box an earlier version left short (its key columns hidden below its edge)
/// shows them all when the project opens, without an undo step.
pub(crate) fn fit_tables(store: &mut Store) {
    let mut grow: Vec<(Obj, f64)> = Vec::new();
    for e in store.alive().filter(|o| store.class_of(*o).ok() == Some(archforge_er::ENTITY)) {
        let columns = store
            .list(e, archforge_er::entity::ATTRIBUTE)
            .unwrap_or_default()
            .iter()
            .filter_map(archforge_mof::Value::as_ref)
            .filter(|a| store.is_alive(*a))
            .count();
        let needed = content_height(columns);
        let (_, _, nodes) = archforge_diagram::views_of(store, e);
        grow.extend(nodes.into_iter().filter(|n| bounds_of(store, *n).height < needed).map(|n| (n, needed)));
    }
    for (n, height) in grow {
        if let Err(e) = store.set(n, archforge_diagram::node::HEIGHT, archforge_mof::Value::Real(height)) {
            log::warn!("a table box could not be grown to show its columns: {e}");
        }
    }
}

/// Grows, inside batch `b`, every node of the tables `entities` whose height would hide the
/// columns the table has now: a command that adds key columns (a foreign key, a shared subtype
/// key) keeps them in sight in the same undo step.
pub(crate) fn grow_to_fit(
    b: &mut archforge_engine::Batch<'_, Store, archforge_diagram::ProjectView>,
    entities: &[Uuid],
) -> Result<(), archforge_engine::EngineError> {
    let mut grow: Vec<(String, f64, f64)> = Vec::new();
    {
        let store = b.doc();
        for id in entities {
            let Some(e) = store.by_uuid(*id) else { continue };
            let columns = store
                .list(e, archforge_er::entity::ATTRIBUTE)
                .unwrap_or_default()
                .iter()
                .filter_map(archforge_mof::Value::as_ref)
                .filter(|a| store.is_alive(*a))
                .count();
            let needed = content_height(columns);
            let (_, _, nodes) = archforge_diagram::views_of(store, e);
            for n in nodes {
                let bounds = bounds_of(store, n);
                if bounds.height < needed
                    && let Some(node) = uuid(store, n)
                    && !grow.iter().any(|(g, _, _)| *g == node.to_string())
                {
                    grow.push((node.to_string(), bounds.width, needed));
                }
            }
        }
    }
    for (node, width, height) in grow {
        b.execute("diagram.resize", &json!({ "node": node, "width": width, "height": height }))?;
    }
    Ok(())
}

/// Runs `cmd` and grows the nodes of `entities` to show their columns, as one undo step named
/// `label`; gives the command's result.
pub(crate) fn run_fitting(cx: &mut Ctx, label: &str, cmd: &str, params: serde_json::Value, entities: &[Uuid]) -> Result<serde_json::Value, String> {
    let Some(session) = cx.session.as_mut() else { return Err(tr("The model could not be opened")) };
    session
        .batch(label, |b| {
            b.execute(cmd, &params)?;
            grow_to_fit(b, entities)
        })
        .map(|o| o.value.get(0).cloned().unwrap_or(serde_json::Value::Null))
        .map_err(|e| crate::panels::refusal(cx.notation_uis, &e))
}
