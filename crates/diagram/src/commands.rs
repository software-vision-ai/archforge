//! The diagram commands, once for every notation: generic over the notation's view type, whose
//! [`DiagramView`] says what a diagram of each kind may show. Each command validates what it is
//! given before it changes anything, keeps the diagram rules, and refuses with a message naming the
//! key. Ids in parameters are the elements' UUIDs; coordinates are numbers in the logical unit,
//! absolute in the diagram's space.

use std::collections::{BTreeMap, BTreeSet};

use archforge_engine::{CommandError, CommandSpec, Ctx, Facts, Params, always, cmd};
use archforge_mof::{ClassId, Invalid, Obj, PropId, Store, Uuid, Value};
use serde_json::{Value as Json, json};

use crate::rules::{self, MAX_DEPTH, MAX_LABELS, MAX_LINE, MAX_NAME, MAX_TEXT, MAX_WAYPOINTS, real, text};
use crate::{
    DIAGRAM, DiagramView, EDGE, LABEL, LabelPlace, NODE, POINT, Routing, STYLE, diagram, diagram_of, edge, label, node, node_tree, point, style,
};

type C<'a, V> = Ctx<'a, Store, V>;
type R = Result<Json, CommandError>;

/// Why a command is not available.
pub const NO_DIAGRAMS: &str = "There is no diagram yet.";
/// A note's size when the user names none.
pub const NOTE_SIZE: (f64, f64) = (160.0, 80.0);
/// The node kind of a note.
pub const NOTE: &str = "note";

fn has_diagrams<V: DiagramView>(f: &Facts<'_, Store, V>) -> Result<(), &'static str> {
    if f.doc.alive().any(|o| f.doc.class_of(o).ok() == Some(DIAGRAM)) { Ok(()) } else { Err(NO_DIAGRAMS) }
}

fn refuse(key: &str, why: impl Into<String>) -> CommandError {
    CommandError::about(key, why)
}

/// A store refusal, as the command's failure.
fn fail(e: Invalid) -> CommandError {
    CommandError::new(e.to_string())
}

fn str_of<'a>(p: &'a Params, key: &str) -> &'a str {
    p.str(key).unwrap_or_default()
}

fn num(p: &Params, key: &str) -> f64 {
    p.number(key).unwrap_or(0.0)
}

fn uuid_of(store: &Store, obj: Obj) -> Result<String, CommandError> {
    store.uuid(obj).map(|u| u.to_string()).map_err(fail)
}

fn uuid_json(store: &Store, obj: Obj) -> Json {
    store.uuid(obj).map(|u| Json::String(u.to_string())).unwrap_or(Json::Null)
}

/// The live element the id under `key` names.
fn any_of(store: &Store, p: &Params, key: &str) -> Result<Obj, CommandError> {
    let id = str_of(p, key);
    let Some(uuid) = Uuid::parse(id) else { return Err(refuse(key, format!("{id:?} is not an id"))) };
    store.by_uuid(uuid).ok_or_else(|| refuse(key, format!("there is no element {id}")))
}

/// The element the id under `key` names, of `class` (`what` names it in the refusal).
fn one_of(store: &Store, p: &Params, key: &str, class: ClassId, what: &str) -> Result<Obj, CommandError> {
    let obj = any_of(store, p, key)?;
    if store.class_of(obj).ok() != Some(class) {
        return Err(refuse(key, format!("{} is not {what}", str_of(p, key))));
    }
    Ok(obj)
}

/// The ids of an array parameter: strings naming live elements of `class`, none twice.
fn list_of(store: &Store, p: &Params, key: &str, classes: &[ClassId], what: &str) -> Result<Vec<Obj>, CommandError> {
    let items = p.get(key).and_then(Json::as_array).cloned().unwrap_or_default();
    if items.len() > archforge_mof::store::MAX_LIST {
        return Err(refuse(key, format!("at most {} ids", archforge_mof::store::MAX_LIST)));
    }
    let mut seen = BTreeSet::new();
    let mut out = Vec::with_capacity(items.len());
    for v in &items {
        let id = v.as_str().ok_or_else(|| refuse(key, "every id must be a string"))?;
        let uuid = Uuid::parse(id).ok_or_else(|| refuse(key, format!("{id:?} is not an id")))?;
        let obj = store.by_uuid(uuid).ok_or_else(|| refuse(key, format!("there is no element {id}")))?;
        if !store.class_of(obj).is_ok_and(|c| classes.contains(&c)) {
            return Err(refuse(key, format!("{id} is not {what}")));
        }
        if !seen.insert(obj) {
            return Err(refuse(key, format!("{id} is listed twice")));
        }
        out.push(obj);
    }
    Ok(out)
}

fn check(key: &str, problem: Option<String>, what: &str) -> Result<(), CommandError> {
    match problem {
        Some(p) => Err(refuse(key, format!("the {what} {p}"))),
        None => Ok(()),
    }
}

fn check_coord(key: &str, v: f64) -> Result<(), CommandError> {
    check(key, rules::coord_problem(v), key)
}

fn check_size(key: &str, v: f64) -> Result<(), CommandError> {
    check(key, rules::size_problem(v), key)
}

fn class_name(store: &Store, class: ClassId) -> String {
    store.universe().class_name(class).to_string()
}

fn kind_of(store: &Store, d: Obj) -> String {
    text(store, d, diagram::KIND)
}

fn element_of(store: &Store, obj: Obj, prop: PropId) -> Option<Obj> {
    store.get(obj, prop).ok().and_then(|v| v.as_ref())
}

fn set_text(store: &mut Store, obj: Obj, prop: PropId, s: &str) -> Result<(), CommandError> {
    store.set(obj, prop, if s.is_empty() { Value::Null } else { Value::str(s) }).map_err(fail)
}

/// A node of the diagram `d`, named under `key`.
fn node_in(store: &Store, p: &Params, key: &str, d: Obj) -> Result<Obj, CommandError> {
    let n = one_of(store, p, key, NODE, "a node")?;
    if diagram_of(store, n) != Some(d) {
        return Err(refuse(key, "the node is in another diagram"));
    }
    Ok(n)
}

/// The nodes of the diagram that show `element`, children included.
fn nodes_showing(store: &Store, d: Obj, element: Obj) -> Vec<Obj> {
    store.referrers(element).into_iter().filter(|(n, prop)| *prop == node::ELEMENT && diagram_of(store, *n) == Some(d)).map(|(n, _)| n).collect()
}

/// The edges of the diagram that show `element`.
fn edges_showing(store: &Store, d: Obj, element: Obj) -> Vec<Obj> {
    store.referrers(element).into_iter().filter(|(e, prop)| *prop == edge::ELEMENT && diagram_of(store, *e) == Some(d)).map(|(e, _)| e).collect()
}

fn select<V: DiagramView>(ctx: &mut C<'_, V>, objs: Vec<Obj>) {
    *ctx.view.selection_mut() = objs;
}

// Diagrams.

fn diagram_add<V: DiagramView>(ctx: &mut C<'_, V>, p: &Params) -> R {
    let kind = str_of(p, "kind");
    check("kind", rules::kind_problem(kind), "kind")?;
    let kinds = ctx.view.kinds();
    if kinds.is_empty() {
        return Err(refuse("kind", "the project's view holds no notation, so it knows no diagram kind"));
    }
    if !kinds.contains(&kind) {
        return Err(refuse("kind", format!("{kind:?} is not one of {}", kinds.join(", "))));
    }
    let name = str_of(p, "name");
    check("name", rules::text_problem(name, MAX_NAME), "name")?;
    let element = if str_of(p, "element").is_empty() {
        if ctx.view.needs_element(kind) {
            return Err(refuse("element", format!("a {kind} diagram is of an element; name it")));
        }
        None
    } else {
        let e = any_of(ctx.doc, p, "element")?;
        let class = ctx.doc.class_of(e).map_err(fail)?;
        if !ctx.view.is_of(kind, class) {
            return Err(refuse("element", format!("a {kind} diagram is not of a {}", class_name(ctx.doc, class).to_lowercase())));
        }
        Some(e)
    };
    let home = ctx.view.home(ctx.doc).map_err(CommandError::new)?;
    let d = ctx.doc.transaction(|st| {
        let d = st.create(DIAGRAM)?;
        st.set(d, diagram::KIND, Value::str(kind))?;
        if !name.is_empty() {
            st.set(d, diagram::NAME, Value::str(name))?;
        }
        if let Some(e) = element {
            st.set(d, diagram::ELEMENT, Value::Ref(e))?;
        }
        // A new diagram's lines run at right angles around the tables (owner decision);
        // one made earlier keeps straight lines.
        let look = st.create(STYLE)?;
        st.set(look, style::ROUTING, Value::Enum(Routing::RightAngled.index()))?;
        st.set(look, style::AVOID, Value::Bool(true))?;
        st.set(d, diagram::STYLE, Value::Ref(look))?;
        if let Some((owner, prop)) = home {
            st.push(owner, prop, Value::Ref(d))?;
        }
        Ok(d)
    });
    let d = d.map_err(fail)?;
    ctx.view.set_open_diagram(Some(d));
    select(ctx, vec![d]);
    Ok(json!({ "id": uuid_of(ctx.doc, d)? }))
}

fn diagram_rename<V: DiagramView>(ctx: &mut C<'_, V>, p: &Params) -> R {
    let d = one_of(ctx.doc, p, "diagram", DIAGRAM, "a diagram")?;
    let name = str_of(p, "name");
    check("name", rules::text_problem(name, MAX_NAME), "name")?;
    set_text(ctx.doc, d, diagram::NAME, name)?;
    Ok(json!({ "id": uuid_of(ctx.doc, d)? }))
}

fn diagram_remove<V: DiagramView>(ctx: &mut C<'_, V>, p: &Params) -> R {
    let d = one_of(ctx.doc, p, "diagram", DIAGRAM, "a diagram")?;
    let gone = ctx.doc.destroy(d).map_err(fail)?;
    Ok(json!({ "removed": gone }))
}

fn view_open_diagram<V: DiagramView>(ctx: &mut C<'_, V>, p: &Params) -> R {
    let d = one_of(ctx.doc, p, "diagram", DIAGRAM, "a diagram")?;
    ctx.view.set_open_diagram(Some(d));
    Ok(json!({ "id": uuid_of(ctx.doc, d)? }))
}

// Nodes.

/// The parent named under `parent`, a node of `d` with room for one more level, or the diagram.
fn parent_in(store: &Store, p: &Params, d: Obj) -> Result<(Obj, PropId), CommandError> {
    if str_of(p, "parent").is_empty() {
        return Ok((d, diagram::NODE));
    }
    let parent = node_in(store, p, "parent", d)?;
    let depth = rules::depth_of(store, parent).ok_or_else(|| refuse("parent", "the node belongs to no diagram"))?;
    if depth.saturating_add(1) > MAX_DEPTH {
        return Err(refuse("parent", format!("nodes nest at most {MAX_DEPTH} deep")));
    }
    Ok((parent, node::CHILD))
}

/// Creates a node at the parameters' place with the kind and size given, under `parent`.
fn new_node(store: &mut Store, p: &Params, parent: (Obj, PropId), element: Option<Obj>, kind: &str, size: (f64, f64)) -> Result<Obj, CommandError> {
    let (x, y) = (num(p, "x"), num(p, "y"));
    check_coord("x", x)?;
    check_coord("y", y)?;
    let width = if num(p, "width") == 0.0 { size.0 } else { num(p, "width") };
    let height = if num(p, "height") == 0.0 { size.1 } else { num(p, "height") };
    check_size("width", width)?;
    check_size("height", height)?;
    let text = str_of(p, "text");
    store
        .transaction(|st| {
            let n = st.create(NODE)?;
            if let Some(e) = element {
                st.set(n, node::ELEMENT, Value::Ref(e))?;
            } else if !text.is_empty() {
                st.set(n, node::TEXT, Value::str(text))?;
            }
            if !kind.is_empty() {
                st.set(n, node::KIND, Value::str(kind))?;
            }
            st.set(n, node::X, Value::Real(x))?;
            st.set(n, node::Y, Value::Real(y))?;
            st.set(n, node::WIDTH, Value::Real(width))?;
            st.set(n, node::HEIGHT, Value::Real(height))?;
            st.push(parent.0, parent.1, Value::Ref(n))?;
            Ok(n)
        })
        .map_err(fail)
}

fn diagram_place<V: DiagramView>(ctx: &mut C<'_, V>, p: &Params) -> R {
    let d = one_of(ctx.doc, p, "diagram", DIAGRAM, "a diagram")?;
    let kind = kind_of(ctx.doc, d);
    let e = any_of(ctx.doc, p, "element")?;
    let class = ctx.doc.class_of(e).map_err(fail)?;
    if !ctx.view.shows(&kind, class) {
        return Err(refuse("element", format!("a {kind} diagram does not show a {}", class_name(ctx.doc, class).to_lowercase())));
    }
    if !ctx.view.may_show(ctx.doc, d, e) {
        return Err(refuse("element", "the element is outside what this diagram shows"));
    }
    if !nodes_showing(ctx.doc, d, e).is_empty() {
        return Err(refuse("element", "the diagram shows this element already"));
    }
    let role = str_of(p, "kind");
    if !role.is_empty() {
        check("kind", rules::kind_problem(role), "kind")?;
    }
    let parent = parent_in(ctx.doc, p, d)?;
    let n = new_node(ctx.doc, p, parent, Some(e), role, ctx.view.default_size(class))?;
    select(ctx, vec![n]);
    Ok(json!({ "id": uuid_of(ctx.doc, n)? }))
}

fn diagram_note<V: DiagramView>(ctx: &mut C<'_, V>, p: &Params) -> R {
    let d = one_of(ctx.doc, p, "diagram", DIAGRAM, "a diagram")?;
    let text = str_of(p, "text");
    if text.is_empty() {
        return Err(refuse("text", "the text is empty"));
    }
    check("text", rules::text_problem(text, MAX_TEXT), "text")?;
    let parent = parent_in(ctx.doc, p, d)?;
    let n = new_node(ctx.doc, p, parent, None, NOTE, NOTE_SIZE)?;
    select(ctx, vec![n]);
    Ok(json!({ "id": uuid_of(ctx.doc, n)? }))
}

fn diagram_set_text<V: DiagramView>(ctx: &mut C<'_, V>, p: &Params) -> R {
    let n = one_of(ctx.doc, p, "node", NODE, "a node")?;
    if element_of(ctx.doc, n, node::ELEMENT).is_some() {
        return Err(refuse("node", "the node shows an element and draws its text"));
    }
    let text = str_of(p, "text");
    if text.is_empty() {
        return Err(refuse("text", "the text is empty"));
    }
    check("text", rules::text_problem(text, MAX_TEXT), "text")?;
    set_text(ctx.doc, n, node::TEXT, text)?;
    Ok(json!({ "id": uuid_of(ctx.doc, n)? }))
}

fn diagram_move<V: DiagramView>(ctx: &mut C<'_, V>, p: &Params) -> R {
    let nodes = list_of(ctx.doc, p, "nodes", &[NODE], "a node")?;
    if nodes.is_empty() {
        return Err(refuse("nodes", "nothing to move"));
    }
    let (dx, dy) = (num(p, "dx"), num(p, "dy"));
    let first = nodes.first().copied().ok_or_else(|| refuse("nodes", "nothing to move"))?;
    let d = diagram_of(ctx.doc, first).ok_or_else(|| refuse("nodes", "the node belongs to no diagram"))?;
    let mut moving: BTreeSet<Obj> = BTreeSet::new();
    for n in &nodes {
        if diagram_of(ctx.doc, *n) != Some(d) {
            return Err(refuse("nodes", "the nodes are in different diagrams"));
        }
        moving.extend(node_tree(ctx.doc, *n));
    }
    let mut points: BTreeSet<Obj> = BTreeSet::new();
    for n in &moving {
        for (e, prop) in ctx.doc.referrers(*n) {
            if prop == edge::SOURCE && element_of(ctx.doc, e, edge::TARGET).is_some_and(|t| moving.contains(&t)) {
                points.extend(ctx.doc.list(e, edge::WAYPOINT).unwrap_or_default().iter().filter_map(Value::as_ref));
            }
        }
    }
    // The offending value is the distance, so the refusal names it.
    for (o, xp, yp) in moving.iter().map(|n| (*n, node::X, node::Y)).chain(points.iter().map(|w| (*w, point::X, point::Y))) {
        if let Some(problem) = rules::coord_problem(real(ctx.doc, o, xp) + dx) {
            return Err(refuse("dx", format!("moves {o} to an x that {problem}")));
        }
        if let Some(problem) = rules::coord_problem(real(ctx.doc, o, yp) + dy) {
            return Err(refuse("dy", format!("moves {o} to a y that {problem}")));
        }
    }
    let moved = moving.len().saturating_add(points.len());
    ctx.doc
        .transaction(|st| {
            for n in &moving {
                let (x, y) = (real(st, *n, node::X), real(st, *n, node::Y));
                st.set(*n, node::X, Value::Real(x + dx))?;
                st.set(*n, node::Y, Value::Real(y + dy))?;
            }
            for w in &points {
                let (x, y) = (real(st, *w, point::X), real(st, *w, point::Y));
                st.set(*w, point::X, Value::Real(x + dx))?;
                st.set(*w, point::Y, Value::Real(y + dy))?;
            }
            Ok(())
        })
        .map_err(fail)?;
    Ok(json!({ "moved": moved }))
}

fn diagram_resize<V: DiagramView>(ctx: &mut C<'_, V>, p: &Params) -> R {
    let n = one_of(ctx.doc, p, "node", NODE, "a node")?;
    let (width, height) = (num(p, "width"), num(p, "height"));
    check_size("width", width)?;
    check_size("height", height)?;
    ctx.doc
        .transaction(|st| {
            st.set(n, node::WIDTH, Value::Real(width))?;
            st.set(n, node::HEIGHT, Value::Real(height))
        })
        .map_err(fail)?;
    Ok(json!({ "id": uuid_of(ctx.doc, n)? }))
}

fn diagram_unplace<V: DiagramView>(ctx: &mut C<'_, V>, p: &Params) -> R {
    let nodes = list_of(ctx.doc, p, "nodes", &[NODE], "a node")?;
    if nodes.is_empty() {
        return Err(refuse("nodes", "nothing to remove"));
    }
    let gone = ctx
        .doc
        .transaction(|st| {
            let mut gone = 0;
            for n in &nodes {
                if st.is_alive(*n) {
                    gone += crate::destroy_node_with_edges(st, *n)?;
                }
            }
            Ok(gone)
        })
        .map_err(fail)?;
    Ok(json!({ "removed": gone }))
}

// Edges.

/// Checks that an edge for `element` may go from `source` to `target` in the diagram of `kind`.
fn check_edge<V: DiagramView>(
    view: &V,
    store: &Store,
    d: Obj,
    kind: &str,
    element: Option<Obj>,
    source: Obj,
    target: Obj,
) -> Result<(), CommandError> {
    let Some(e) = element else {
        if element_of(store, source, node::ELEMENT).is_some() && element_of(store, target, node::ELEMENT).is_some() {
            return Err(refuse("element", "an edge between two elements shows a relationship; name it"));
        }
        return Ok(());
    };
    let class = store.class_of(e).map_err(fail)?;
    if !view.connects(kind, class) {
        return Err(refuse("element", format!("a {kind} diagram does not connect a {}", class_name(store, class).to_lowercase())));
    }
    let (from, to) = view.ends(store, e).ok_or_else(|| refuse("element", "the relationship lacks an end"))?;
    if element_of(store, source, node::ELEMENT) != Some(from) {
        return Err(refuse("source", "the node does not show the relationship's first end"));
    }
    if element_of(store, target, node::ELEMENT) != Some(to) {
        return Err(refuse("target", "the node does not show the relationship's second end"));
    }
    if !edges_showing(store, d, e).is_empty() {
        return Err(refuse("element", "the diagram shows this relationship already"));
    }
    Ok(())
}

fn new_edge(store: &mut Store, d: Obj, element: Option<Obj>, source: Obj, target: Obj) -> Result<Obj, Invalid> {
    store.transaction(|st| {
        let e = st.create(EDGE)?;
        if let Some(el) = element {
            st.set(e, edge::ELEMENT, Value::Ref(el))?;
        }
        st.set(e, edge::SOURCE, Value::Ref(source))?;
        st.set(e, edge::TARGET, Value::Ref(target))?;
        st.push(d, diagram::EDGE, Value::Ref(e))?;
        Ok(e)
    })
}

fn diagram_connect<V: DiagramView>(ctx: &mut C<'_, V>, p: &Params) -> R {
    let d = one_of(ctx.doc, p, "diagram", DIAGRAM, "a diagram")?;
    let kind = kind_of(ctx.doc, d);
    let source = node_in(ctx.doc, p, "source", d)?;
    let target = node_in(ctx.doc, p, "target", d)?;
    let element = if str_of(p, "element").is_empty() { None } else { Some(any_of(ctx.doc, p, "element")?) };
    check_edge(&*ctx.view, ctx.doc, d, &kind, element, source, target)?;
    let e = new_edge(ctx.doc, d, element, source, target).map_err(fail)?;
    select(ctx, vec![e]);
    Ok(json!({ "id": uuid_of(ctx.doc, e)? }))
}

/// Adds an edge for every relationship whose both ends the diagram shows and which it does not show
/// yet (decision: relationships appear where the user places edges; this adds the missing).
fn diagram_connect_all<V: DiagramView>(ctx: &mut C<'_, V>, p: &Params) -> R {
    let d = one_of(ctx.doc, p, "diagram", DIAGRAM, "a diagram")?;
    let kind = kind_of(ctx.doc, d);
    let mut shown: BTreeMap<Obj, Obj> = BTreeMap::new();
    for top in ctx.doc.list(d, diagram::NODE).map_err(fail)?.iter().filter_map(Value::as_ref) {
        for n in node_tree(ctx.doc, top) {
            if let Some(e) = element_of(ctx.doc, n, node::ELEMENT) {
                shown.entry(e).or_insert(n);
            }
        }
    }
    let mut missing: Vec<(Obj, Obj, Obj)> = Vec::new();
    for o in ctx.doc.alive() {
        let Ok(class) = ctx.doc.class_of(o) else { continue };
        if !ctx.view.connects(&kind, class) {
            continue;
        }
        let Some((from, to)) = ctx.view.ends(ctx.doc, o) else { continue };
        if let (Some(s), Some(t)) = (shown.get(&from), shown.get(&to))
            && edges_showing(ctx.doc, d, o).is_empty()
        {
            missing.push((o, *s, *t));
        }
    }
    let added = ctx
        .doc
        .transaction(|st| {
            let mut edges = Vec::with_capacity(missing.len());
            for (o, s, t) in &missing {
                edges.push(new_edge(st, d, Some(*o), *s, *t)?);
            }
            Ok(edges)
        })
        .map_err(fail)?;
    let n = added.len();
    select(ctx, added);
    Ok(json!({ "added": n }))
}

fn diagram_disconnect<V: DiagramView>(ctx: &mut C<'_, V>, p: &Params) -> R {
    let e = one_of(ctx.doc, p, "edge", EDGE, "an edge")?;
    let gone = crate::destroy_edge_with_links(ctx.doc, e).map_err(fail)?;
    Ok(json!({ "removed": gone }))
}

/// Links a note to a node, or to an edge that shows a relationship, in its diagram: an edge with no
/// element from the note (owner decision). It changes no model element.
fn diagram_link_note<V: DiagramView>(ctx: &mut C<'_, V>, p: &Params) -> R {
    let d = one_of(ctx.doc, p, "diagram", DIAGRAM, "a diagram")?;
    let note = node_in(ctx.doc, p, "note", d)?;
    if element_of(ctx.doc, note, node::ELEMENT).is_some() {
        return Err(refuse("note", "the node shows an element; only a note links"));
    }
    let target = any_of(ctx.doc, p, "target")?;
    if target == note {
        return Err(refuse("target", "a note does not link to itself"));
    }
    match ctx.doc.class_of(target).ok() {
        Some(c) if c == NODE => {
            if element_of(ctx.doc, target, node::ELEMENT).is_none() {
                return Err(refuse("target", "a note links to a table or a relationship, not to another note"));
            }
        }
        Some(c) if c == EDGE => {
            if element_of(ctx.doc, target, edge::ELEMENT).is_none() {
                return Err(refuse("target", "a note links to a relationship's line, not to another link"));
            }
        }
        _ => return Err(refuse("target", "a note links to a node or an edge")),
    }
    if diagram_of(ctx.doc, target) != Some(d) {
        return Err(refuse("target", "the target is in another diagram"));
    }
    // Already linked, either way round (an older anchor may run from the table to the note).
    let linked = ctx.doc.referrers(note).into_iter().any(|(e, prop)| {
        let other = match prop {
            p if p == edge::SOURCE => element_of(ctx.doc, e, edge::TARGET),
            p if p == edge::TARGET => element_of(ctx.doc, e, edge::SOURCE),
            _ => None,
        };
        ctx.doc.class_of(e).ok() == Some(EDGE) && element_of(ctx.doc, e, edge::ELEMENT).is_none() && other == Some(target)
    });
    if linked {
        return Err(refuse("target", "the note is linked to it already"));
    }
    let e = new_edge(ctx.doc, d, None, note, target).map_err(fail)?;
    select(ctx, vec![e]);
    Ok(json!({ "id": uuid_of(ctx.doc, e)? }))
}

fn diagram_route<V: DiagramView>(ctx: &mut C<'_, V>, p: &Params) -> R {
    let e = one_of(ctx.doc, p, "edge", EDGE, "an edge")?;
    if element_of(ctx.doc, e, edge::ELEMENT).is_none() {
        return Err(refuse("edge", "a note link is straight"));
    }
    let items = p.get("points").and_then(Json::as_array).cloned().unwrap_or_default();
    if items.len() > MAX_WAYPOINTS {
        return Err(refuse("points", format!("at most {MAX_WAYPOINTS} waypoints")));
    }
    let mut points = Vec::with_capacity(items.len());
    for item in &items {
        let coord = |k: &str| item.get(k).and_then(Json::as_f64).ok_or_else(|| refuse("points", format!("every point needs a number {k}")));
        let (x, y) = (coord("x")?, coord("y")?);
        check("points", rules::coord_problem(x), "x")?;
        check("points", rules::coord_problem(y), "y")?;
        points.push((x, y));
    }
    let old: Vec<Obj> = ctx.doc.list(e, edge::WAYPOINT).map_err(fail)?.iter().filter_map(Value::as_ref).collect();
    ctx.doc
        .transaction(|st| {
            for w in old {
                st.destroy(w)?;
            }
            for (x, y) in &points {
                let w = st.create(POINT)?;
                st.set(w, point::X, Value::Real(*x))?;
                st.set(w, point::Y, Value::Real(*y))?;
                st.push(e, edge::WAYPOINT, Value::Ref(w))?;
            }
            Ok(())
        })
        .map_err(fail)?;
    Ok(json!({ "id": uuid_of(ctx.doc, e)?, "points": points.len() }))
}

// Labels.

fn place(p: &Params) -> Result<LabelPlace, CommandError> {
    match str_of(p, "at") {
        "source" => Ok(LabelPlace::Source),
        "middle" | "" => Ok(LabelPlace::Middle),
        "target" => Ok(LabelPlace::Target),
        other => Err(refuse("at", format!("{other:?} is not source, middle or target"))),
    }
}

/// The place's name in parameters.
pub fn place_name(at: LabelPlace) -> &'static str {
    match at {
        LabelPlace::Source => "source",
        LabelPlace::Middle => "middle",
        LabelPlace::Target => "target",
    }
}

fn diagram_label<V: DiagramView>(ctx: &mut C<'_, V>, p: &Params) -> R {
    let e = one_of(ctx.doc, p, "edge", EDGE, "an edge")?;
    let element =
        element_of(ctx.doc, e, edge::ELEMENT).ok_or_else(|| refuse("edge", "the edge shows no element, so there is no text to label it with"))?;
    let class = ctx.doc.class_of(element).map_err(fail)?;
    let kind = str_of(p, "kind");
    check("kind", rules::kind_problem(kind), "kind")?;
    let slots = ctx.view.text_slots(class);
    if !slots.contains(&kind) {
        return Err(refuse(
            "kind",
            format!("a {} has no text slot {kind:?}; it has {}", class_name(ctx.doc, class).to_lowercase(), slots.join(", ")),
        ));
    }
    let labels = ctx.doc.list(e, edge::LABEL).map_err(fail)?;
    if labels.len() >= MAX_LABELS {
        return Err(refuse("edge", format!("the edge has {MAX_LABELS} labels already")));
    }
    if labels.iter().filter_map(Value::as_ref).any(|l| text(ctx.doc, l, label::KIND) == kind) {
        return Err(refuse("kind", format!("the edge has a {kind} label already")));
    }
    let at = place(p)?;
    let (dx, dy) = (num(p, "dx"), num(p, "dy"));
    check_coord("dx", dx)?;
    check_coord("dy", dy)?;
    let l = ctx
        .doc
        .transaction(|st| {
            let l = st.create(LABEL)?;
            st.set(l, label::KIND, Value::str(kind))?;
            st.set(l, label::AT, Value::Enum(at.index()))?;
            st.set(l, label::DX, Value::Real(dx))?;
            st.set(l, label::DY, Value::Real(dy))?;
            st.push(e, edge::LABEL, Value::Ref(l))?;
            Ok(l)
        })
        .map_err(fail)?;
    Ok(json!({ "id": uuid_of(ctx.doc, l)? }))
}

fn diagram_unlabel<V: DiagramView>(ctx: &mut C<'_, V>, p: &Params) -> R {
    let l = one_of(ctx.doc, p, "label", LABEL, "a label")?;
    let gone = ctx.doc.destroy(l).map_err(fail)?;
    Ok(json!({ "removed": gone }))
}

// Styles.

fn diagram_style<V: DiagramView>(ctx: &mut C<'_, V>, p: &Params) -> R {
    let ids = list_of(ctx.doc, p, "ids", &[DIAGRAM, NODE, EDGE], "a diagram, a node or an edge")?;
    if ids.is_empty() {
        return Err(refuse("ids", "nothing to style"));
    }
    let name = str_of(p, "name");
    if !name.is_empty() {
        check("name", rules::kind_problem(name), "style name")?;
    }
    let colours = [("fill", style::FILL), ("stroke", style::STROKE), ("text", style::TEXT)];
    for (key, _) in colours {
        let c = str_of(p, key);
        if !c.is_empty()
            && let Some(problem) = rules::colour_problem(c)
        {
            return Err(refuse(key, problem));
        }
    }
    let line = num(p, "line");
    if !line.is_finite() || !(0.0..=MAX_LINE).contains(&line) {
        return Err(refuse("line", format!("the line width is not between 0 and {MAX_LINE}")));
    }
    let routing_name = if str_of(p, "routing").is_empty() { ROUTINGS[0] } else { str_of(p, "routing") };
    let routing = ROUTINGS
        .iter()
        .position(|r| *r == routing_name)
        .ok_or_else(|| refuse("routing", format!("{routing_name:?} is not one of {}", ROUTINGS.join(", "))))?;
    let avoid = p.bool("avoid").unwrap_or(false);
    let clear = name.is_empty() && colours.iter().all(|(k, _)| str_of(p, k).is_empty()) && line == 0.0 && routing == 0 && !avoid;
    let cleared = ctx
        .doc
        .transaction(|st| {
            let mut cleared = 0usize;
            for o in &ids {
                let prop = match st.class_of(*o)? {
                    c if c == NODE => node::STYLE,
                    c if c == DIAGRAM => diagram::STYLE,
                    _ => edge::STYLE,
                };
                let existing = st.get(*o, prop)?.as_ref();
                if clear {
                    if let Some(s) = existing {
                        st.destroy(s)?;
                        cleared = cleared.saturating_add(1);
                    }
                    continue;
                }
                let s = match existing {
                    Some(s) => s,
                    None => {
                        let s = st.create(STYLE)?;
                        st.set(*o, prop, Value::Ref(s))?;
                        s
                    }
                };
                st.set(s, style::NAME, if name.is_empty() { Value::Null } else { Value::str(name) })?;
                for (key, sp) in colours {
                    let c = str_of(p, key);
                    st.set(s, sp, if c.is_empty() { Value::Null } else { Value::str(c) })?;
                }
                st.set(s, style::LINE, Value::Real(line))?;
                st.set(s, style::ROUTING, Value::Enum(routing as u32))?;
                st.set(s, style::AVOID, Value::Bool(avoid))?;
            }
            Ok(cleared)
        })
        .map_err(fail)?;
    Ok(json!({ "styled": if clear { 0 } else { ids.len() }, "cleared": cleared }))
}

/// Arranges the diagram's boxes in layers along their lines (crate::arrange), keeping its top-left
/// where it was; a box moves with what it contains, and the lines' waypoints go, since their
/// routes are worked out anew. One step.
fn diagram_arrange<V: DiagramView>(ctx: &mut C<'_, V>, p: &Params) -> R {
    let d = one_of(ctx.doc, p, "diagram", DIAGRAM, "a diagram")?;
    let st = &*ctx.doc;
    let tops: Vec<Obj> = st.list(d, diagram::NODE).map_err(fail)?.iter().filter_map(Value::as_ref).collect();
    if tops.is_empty() {
        return Err(refuse("diagram", "the diagram shows nothing to arrange"));
    }
    let index: BTreeMap<Obj, usize> = tops.iter().enumerate().map(|(i, n)| (*n, i)).collect();
    // A line between nested boxes counts between the boxes on the diagram they are in.
    let top_of = |mut n: Obj| -> Option<usize> {
        for _ in 0..=MAX_DEPTH {
            if let Some(i) = index.get(&n) {
                return Some(*i);
            }
            n = st.container(n).ok().flatten()?.0;
        }
        None
    };
    let rects: Vec<crate::Rect> = tops.iter().map(|n| crate::grid::bounds_of(st, *n)).collect();
    let edges: Vec<Obj> = st.list(d, diagram::EDGE).map_err(fail)?.iter().filter_map(Value::as_ref).collect();
    let lines: Vec<(usize, usize)> = edges
        .iter()
        .filter_map(|e| {
            let (s, t) = (element_of(st, *e, edge::SOURCE)?, element_of(st, *e, edge::TARGET)?);
            if st.class_of(t).ok() != Some(NODE) {
                return None;
            }
            Some((top_of(s)?, top_of(t)?))
        })
        .collect();
    let places = crate::arrange::arrange(&rects.iter().map(|r| (r.width, r.height)).collect::<Vec<_>>(), &lines);
    // Where the diagram started stays where it is.
    let left = rects.iter().map(|r| r.x).fold(f64::INFINITY, f64::min);
    let top = rects.iter().map(|r| r.y).fold(f64::INFINITY, f64::min);
    let moves: Vec<(Obj, f64, f64)> = tops.iter().zip(&places).zip(&rects).map(|((n, (x, y)), r)| (*n, left + x - r.x, top + y - r.y)).collect();
    let mut moving: Vec<(Obj, f64, f64)> = Vec::new();
    for (n, dx, dy) in &moves {
        for m in node_tree(st, *n) {
            let (x, y) = (real(st, m, node::X) + dx, real(st, m, node::Y) + dy);
            if let Some(problem) = rules::coord_problem(x).or_else(|| rules::coord_problem(y)) {
                return Err(refuse("diagram", format!("arranged, a box would land at a place that {problem}")));
            }
            moving.push((m, x, y));
        }
    }
    let points: Vec<Obj> = edges.iter().flat_map(|e| st.list(*e, edge::WAYPOINT).unwrap_or_default()).filter_map(|v| v.as_ref()).collect();
    ctx.doc
        .transaction(|st| {
            for (m, x, y) in &moving {
                st.set(*m, node::X, Value::Real(*x))?;
                st.set(*m, node::Y, Value::Real(*y))?;
            }
            for w in &points {
                st.destroy(*w)?;
            }
            Ok(())
        })
        .map_err(fail)?;
    Ok(json!({ "arranged": tops.len() }))
}

// Queries.

fn style_json(store: &Store, owner: Obj, prop: PropId) -> Json {
    match element_of(store, owner, prop) {
        None => Json::Null,
        Some(s) => {
            let opt = |p| store.get(s, p).ok().and_then(|v| v.as_str().map(str::to_string));
            let routing = match store.get(s, style::ROUTING) {
                Ok(Value::Enum(i)) => ROUTINGS.get(i as usize).copied().unwrap_or(ROUTINGS[0]),
                _ => ROUTINGS[0],
            };
            let avoid = matches!(store.get(s, style::AVOID), Ok(Value::Bool(true)));
            json!({ "name": opt(style::NAME), "fill": opt(style::FILL), "stroke": opt(style::STROKE), "text": opt(style::TEXT), "line": real(store, s, style::LINE), "routing": routing, "avoid": avoid })
        }
    }
}

fn node_json(store: &Store, n: Obj, depth: usize) -> Json {
    let children: Vec<Json> = if depth > MAX_DEPTH {
        Vec::new()
    } else {
        store
            .list(n, node::CHILD)
            .unwrap_or_default()
            .iter()
            .filter_map(Value::as_ref)
            .map(|c| node_json(store, c, depth.saturating_add(1)))
            .collect()
    };
    json!({
        "id": uuid_json(store, n),
        "element": element_of(store, n, node::ELEMENT).map(|e| uuid_json(store, e)).unwrap_or(Json::Null),
        "kind": store.get(n, node::KIND).ok().and_then(|v| v.as_str().map(str::to_string)),
        "text": store.get(n, node::TEXT).ok().and_then(|v| v.as_str().map(str::to_string)),
        "x": real(store, n, node::X),
        "y": real(store, n, node::Y),
        "width": real(store, n, node::WIDTH),
        "height": real(store, n, node::HEIGHT),
        "children": children,
        "style": style_json(store, n, node::STYLE),
    })
}

fn edge_json(store: &Store, e: Obj) -> Json {
    let end = |p| element_of(store, e, p).map(|n| uuid_json(store, n)).unwrap_or(Json::Null);
    let points: Vec<Json> = store
        .list(e, edge::WAYPOINT)
        .unwrap_or_default()
        .iter()
        .filter_map(Value::as_ref)
        .map(|w| json!({ "x": real(store, w, point::X), "y": real(store, w, point::Y) }))
        .collect();
    let labels: Vec<Json> = store
        .list(e, edge::LABEL)
        .unwrap_or_default()
        .iter()
        .filter_map(Value::as_ref)
        .map(|l| {
            let at = match store.get(l, label::AT) {
                Ok(Value::Enum(i)) => LabelPlace::from_index(i).map(place_name),
                _ => None,
            };
            json!({ "id": uuid_json(store, l), "kind": text(store, l, label::KIND), "at": at, "dx": real(store, l, label::DX), "dy": real(store, l, label::DY) })
        })
        .collect();
    json!({
        "id": uuid_json(store, e),
        "element": end(edge::ELEMENT),
        "source": end(edge::SOURCE),
        "target": end(edge::TARGET),
        "points": points,
        "labels": labels,
        "style": style_json(store, e, edge::STYLE),
    })
}

fn diagram_head(store: &Store, d: Obj) -> Json {
    json!({
        "id": uuid_json(store, d),
        "kind": kind_of(store, d),
        "name": store.get(d, diagram::NAME).ok().and_then(|v| v.as_str().map(str::to_string)),
        "element": element_of(store, d, diagram::ELEMENT).map(|e| uuid_json(store, e)).unwrap_or(Json::Null),
    })
}

fn diagram_list<V: DiagramView>(ctx: &mut C<'_, V>, p: &Params) -> R {
    let of = if str_of(p, "element").is_empty() { None } else { Some(any_of(ctx.doc, p, "element")?) };
    let list: Vec<Json> = ctx
        .doc
        .alive()
        .filter(|o| ctx.doc.class_of(*o).ok() == Some(DIAGRAM))
        .filter(|d| of.is_none() || element_of(ctx.doc, *d, diagram::ELEMENT) == of)
        .map(|d| diagram_head(ctx.doc, d))
        .collect();
    Ok(Json::Array(list))
}

fn diagram_get<V: DiagramView>(ctx: &mut C<'_, V>, p: &Params) -> R {
    let d = one_of(ctx.doc, p, "diagram", DIAGRAM, "a diagram")?;
    let mut out = diagram_head(ctx.doc, d);
    let nodes: Vec<Json> = ctx.doc.list(d, diagram::NODE).map_err(fail)?.iter().filter_map(Value::as_ref).map(|n| node_json(ctx.doc, n, 0)).collect();
    let edges: Vec<Json> = ctx.doc.list(d, diagram::EDGE).map_err(fail)?.iter().filter_map(Value::as_ref).map(|e| edge_json(ctx.doc, e)).collect();
    if let Some(o) = out.as_object_mut() {
        o.insert("style".into(), style_json(ctx.doc, d, diagram::STYLE));
        o.insert("nodes".into(), Json::Array(nodes));
        o.insert("edges".into(), Json::Array(edges));
    }
    Ok(out)
}

const PLACE_PARAMS: &str = "diagram: string; the diagram's id\nelement: string; the id of the element shown\nx: number = 0, -1000000..1000000; the left edge\ny: number = 0, -1000000..1000000; the top edge\nwidth: number = 0, 0..10000; the width, 0 for the kind's default, else 20 or more\nheight: number = 0, 0..10000; the height, 0 for the kind's default, else 20 or more\nparent: string = \"\"; the node it nests in, or nothing\nkind: string = \"\"; a role such as compartment or port, or nothing";
const NOTE_PARAMS: &str = "diagram: string; the diagram's id\ntext: string; the note's text\nx: number = 0, -1000000..1000000; the left edge\ny: number = 0, -1000000..1000000; the top edge\nwidth: number = 0, 0..10000; the width, 0 for the default, else 20 or more\nheight: number = 0, 0..10000; the height, 0 for the default, else 20 or more\nparent: string = \"\"; the node it nests in, or nothing";
const MOVE_PARAMS: &str = "nodes: array; the ids of the nodes moved, with what they contain\ndx: number, -2000000..2000000; how far right\ndy: number, -2000000..2000000; how far down";
const RESIZE_PARAMS: &str = "node: string; the node's id\nwidth: number, 20..10000; the new width\nheight: number, 20..10000; the new height";
const LABEL_PARAMS: &str = "edge: string; the edge's id\nkind: string; the text slot of the edge's element the label shows\nat: string = \"middle\", source|middle|target; where along the edge\ndx: number = 0, -1000000..1000000; the offset right of that place\ndy: number = 0, -1000000..1000000; the offset down";
const STYLE_PARAMS: &str = "ids: array; the ids of the diagrams, nodes and edges styled, whose style is set as a whole: a call naming nothing removes it; a diagram's style is the look its lines take unless they have their own\nname: string = \"\"; a style of the shipped sheet, or nothing\nfill: string = \"\"; a fill colour #rrggbb, or nothing\nstroke: string = \"\"; a stroke colour #rrggbb, or nothing\ntext: string = \"\"; a text colour #rrggbb, or nothing\nline: number = 0, 0..100; the stroke width, 0 for the sheet's\nrouting: string = \"as-diagram\", as-diagram|straight|right-angled|curved; how a line runs: as its diagram's lines, or its own way (a diagram as-diagram: straight)\navoid: bool = false; a right-angled line goes around the tables in its way";

/// The names of the routings, by literal.
const ROUTINGS: [&str; 4] = ["as-diagram", "straight", "right-angled", "curved"];

/// The diagram commands for a notation whose view is `V`, to add to the notation's command table.
pub fn specs<V: DiagramView>() -> Vec<CommandSpec<Store, V>> {
    vec![
        cmd!(
            "diagram.add",
            "Add diagram",
            &["Diagram"],
            None,
            "kind: string; the diagram's kind\nelement: string = \"\"; the id of what the diagram is of, or nothing\nname: string = \"\"; the diagram's name",
            always,
            diagram_add::<V>
        ),
        cmd!(
            "diagram.rename",
            "Rename diagram",
            &["Diagram"],
            None,
            "diagram: string; the diagram's id\nname: string = \"\"; the new name, or nothing",
            has_diagrams::<V>,
            diagram_rename::<V>
        ),
        cmd!("diagram.remove", "Remove diagram", &["Diagram"], None, "diagram: string; the diagram's id", has_diagrams::<V>, diagram_remove::<V>),
        cmd!("diagram.place", "Place element", &["Diagram"], None, PLACE_PARAMS, has_diagrams::<V>, diagram_place::<V>),
        cmd!("diagram.note", "Add note", &["Diagram"], None, NOTE_PARAMS, has_diagrams::<V>, diagram_note::<V>),
        cmd!(
            "diagram.set-text",
            "Set note text",
            &["Diagram"],
            None,
            "node: string; the note's id\ntext: string; the new text",
            has_diagrams::<V>,
            diagram_set_text::<V>
        ),
        cmd!("diagram.move", "Move", &["Diagram"], None, MOVE_PARAMS, has_diagrams::<V>, diagram_move::<V>),
        cmd!("diagram.resize", "Resize", &["Diagram"], None, RESIZE_PARAMS, has_diagrams::<V>, diagram_resize::<V>),
        cmd!(
            "diagram.unplace",
            "Remove from diagram",
            &["Diagram"],
            None,
            "nodes: array; the ids of the nodes removed, with the edges on them",
            has_diagrams::<V>,
            diagram_unplace::<V>
        ),
        cmd!(
            "diagram.connect",
            "Connect",
            &["Diagram"],
            None,
            "diagram: string; the diagram's id\nsource: string; the first node's id\ntarget: string; the second node's id\nelement: string = \"\"; the id of the relationship shown, or nothing for an anchor to a note",
            has_diagrams::<V>,
            diagram_connect::<V>
        ),
        cmd!(
            "diagram.connect-all",
            "Show all relationships",
            &["Diagram"],
            None,
            "diagram: string; the diagram's id",
            has_diagrams::<V>,
            diagram_connect_all::<V>
        ),
        cmd!("diagram.disconnect", "Disconnect", &["Diagram"], None, "edge: string; the edge's id", has_diagrams::<V>, diagram_disconnect::<V>),
        cmd!(
            "diagram.link-note",
            "Link note",
            &["Diagram"],
            None,
            "diagram: string; the diagram's id\nnote: string; the note's node\ntarget: string; the node, or the edge of a relationship, it points at",
            has_diagrams::<V>,
            diagram_link_note::<V>
        ),
        cmd!(
            "diagram.route",
            "Route edge",
            &["Diagram"],
            None,
            "edge: string; the edge's id\npoints: array = []; the waypoints as {x, y} objects, in order",
            has_diagrams::<V>,
            diagram_route::<V>
        ),
        cmd!("diagram.label", "Add label", &["Diagram"], None, LABEL_PARAMS, has_diagrams::<V>, diagram_label::<V>),
        cmd!("diagram.unlabel", "Remove label", &["Diagram"], None, "label: string; the label's id", has_diagrams::<V>, diagram_unlabel::<V>),
        cmd!("diagram.style", "Style", &["Diagram"], None, STYLE_PARAMS, has_diagrams::<V>, diagram_style::<V>),
        cmd!(
            "diagram.arrange",
            "Arrange diagram",
            &["Diagram"],
            None,
            "diagram: string; the diagram's id: its boxes are laid out in layers along their lines, the box a line starts at above the box it ends at",
            has_diagrams::<V>,
            diagram_arrange::<V>
        ),
        cmd!(noundo "view.open-diagram", "Open diagram", &["View"], None, "diagram: string; the diagram's id", has_diagrams::<V>, view_open_diagram::<V>),
        cmd!(query "diagram.list", "List diagrams", &["Diagram"], None, "element: string = \"\"; only the diagrams of this element, or all", always, diagram_list::<V>),
        cmd!(query "diagram.get", "Get diagram", &["Diagram"], None, "diagram: string; the diagram's id", has_diagrams::<V>, diagram_get::<V>),
    ]
}
