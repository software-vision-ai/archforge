//! The diagram metamodel for every notation (decision gm2.21): a diagram of some element shows
//! model elements as nodes and relationships as edges, with labels that name a text slot of their
//! element and styles over the shipped style sheet; our own, in the spirit of OMG DI. The metamodel
//! is `diagram.xmi`, generated into `generated/` by `cargo xtask generate diagram`; the rules, the
//! generic commands, the cascades, the spatial index and the project's view are written here.
//! Knows no notation: the view says what each diagram shows through [`DiagramView`], asking the
//! [`Notation`] that owns the diagram's kind. Pure: builds for the portable target.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unreachable, clippy::todo, clippy::unimplemented)]

pub mod arrange;
pub mod commands;
mod generated;
pub mod grid;
pub mod project;
pub mod route;
pub mod rules;

use std::collections::BTreeSet;
use std::sync::Arc;

use archforge_mof::{ClassId, CompositeSlabs, Invalid, MetaError, Metamodel, Obj, PropId, Store, Universe};

pub use generated::classes::*;
pub use generated::meta::*;
pub use generated::schema::{JUNCTIONS, TABLES};
pub use generated::slabs::DiagramSlabs;
pub use grid::{Grid, Hit, Rect};
pub use project::{Notation, ProjectView};
pub use route::{Way, route};

/// What the view brings to the diagram commands: the open diagram and the selection, which
/// undo never touches, and the notation's policy, which the commands enforce so a diagram never
/// shows what its kind cannot draw.
pub trait DiagramView: archforge_engine::View<Store> {
    /// The diagram the canvas shows.
    fn open_diagram(&self) -> Option<Obj>;
    fn set_open_diagram(&mut self, diagram: Option<Obj>);
    /// The selection, which the commands set to what they made.
    fn selection_mut(&mut self) -> &mut Vec<Obj>;
    /// The diagram kinds the notations know, such as `er` or `uml.class`.
    fn kinds(&self) -> Vec<&'static str>;
    /// Where a new diagram goes: a container and its list property, None for a diagram without a
    /// container, or why no diagram can be added now.
    fn home(&self, store: &Store) -> Result<Option<(Obj, PropId)>, String>;
    /// Whether a diagram of `kind` may be of an element of `class`.
    fn is_of(&self, kind: &str, class: ClassId) -> bool;
    /// Whether a diagram of `kind` must be of an element.
    fn needs_element(&self, kind: &str) -> bool {
        let _ = kind;
        false
    }
    /// Whether a diagram of `kind` shows elements of `class` as nodes.
    fn shows(&self, kind: &str, class: ClassId) -> bool;
    /// Whether `diagram` may show `element`, beyond its class: a notation keeps a diagram to its own
    /// scope here, such as a schema's entities. Everything may by default.
    fn may_show(&self, store: &Store, diagram: Obj, element: Obj) -> bool {
        let _ = (store, diagram, element);
        true
    }
    /// Whether a diagram of `kind` shows elements of `class` as edges.
    fn connects(&self, kind: &str, class: ClassId) -> bool;
    /// The two ends of a relationship element, source first, or None when it is no relationship or
    /// lacks an end.
    fn ends(&self, store: &Store, element: Obj) -> Option<(Obj, Obj)>;
    /// The text slots of an element of `class` a label may show, such as `name`.
    fn text_slots(&self, class: ClassId) -> &'static [&'static str];
    /// A node's width and height for an element of `class` when the user names none.
    fn default_size(&self, class: ClassId) -> (f64, f64);
}

/// A universe of the diagram metamodel and the metamodels given.
pub fn universe_with(others: Vec<Arc<Metamodel>>) -> Result<Arc<Universe>, MetaError> {
    let mut all = vec![metamodel()?];
    all.extend(others);
    Ok(Arc::new(Universe::new(all)?))
}

/// The diagram slabs joined to `slabs`.
pub fn with_slabs(slabs: CompositeSlabs) -> Result<CompositeSlabs, String> {
    slabs.with(INDEX, Box::new(DiagramSlabs::default()))
}

/// The diagram a node, edge, point, label or style belongs to, through its containers.
pub fn diagram_of(store: &Store, obj: Obj) -> Option<Obj> {
    let mut cur = obj;
    for _ in 0..=rules::MAX_DEPTH.saturating_add(3) {
        if store.class_of(cur).ok() == Some(DIAGRAM) {
            return Some(cur);
        }
        cur = store.container(cur).ok().flatten()?.0;
    }
    None
}

/// A node with every node under it, parents first, bounded by the nesting depth.
pub fn node_tree(store: &Store, node: Obj) -> Vec<Obj> {
    let mut out = Vec::new();
    let mut stack = vec![(node, 0usize)];
    let mut seen = BTreeSet::new();
    while let Some((n, depth)) = stack.pop() {
        if !seen.insert(n) || depth > rules::MAX_DEPTH {
            continue;
        }
        out.push(n);
        let children: Vec<Obj> = store.list(n, node::CHILD).unwrap_or_default().iter().filter_map(archforge_mof::Value::as_ref).collect();
        for c in children.into_iter().rev() {
            stack.push((c, depth.saturating_add(1)));
        }
    }
    out
}

/// The edges with `node` or a node under it as an end, each once.
pub fn edges_on(store: &Store, node: Obj) -> Vec<Obj> {
    let mut out = Vec::new();
    let mut found = BTreeSet::new();
    for n in node_tree(store, node) {
        for (edge, prop) in store.referrers(n) {
            if (prop == edge::SOURCE || prop == edge::TARGET) && found.insert(edge) {
                out.push(edge);
            }
        }
    }
    out
}

/// The note links that end on `edge`, and those that end on them in turn (an edge's end may be
/// another edge: a note pointing at a relationship's line; a file may hold a chain or a loop of
/// them, which a command never makes), each once and `edge` itself left out.
pub fn links_on(store: &Store, edge: Obj) -> Vec<Obj> {
    let mut found = BTreeSet::from([edge]);
    let mut out = Vec::new();
    let mut queue = vec![edge];
    while let Some(on) = queue.pop() {
        for (e, p) in store.referrers(on) {
            if (p == self::edge::SOURCE || p == self::edge::TARGET) && found.insert(e) {
                out.push(e);
                queue.push(e);
            }
        }
    }
    out
}

/// Destroys an edge with the note links on it, all or nothing; gives how many objects went. The
/// MOF core alone refuses the edge while a link requires it as an end.
pub fn destroy_edge_with_links(store: &mut Store, edge: Obj) -> Result<usize, Invalid> {
    let links = links_on(store, edge);
    store.transaction(|st| {
        let mut gone = 0;
        for l in links {
            if st.is_alive(l) {
                gone += st.destroy(l)?;
            }
        }
        gone += st.destroy(edge)?;
        Ok(gone)
    })
}

/// Destroys a node with the edges on it or on the nodes under it, and the note links on those
/// edges, all or nothing; gives how many objects went. The MOF core alone refuses the node while
/// an edge requires it as an end.
pub fn destroy_node_with_edges(store: &mut Store, node: Obj) -> Result<usize, Invalid> {
    let edges = edges_on(store, node);
    store.transaction(|st| {
        let mut gone = 0;
        for e in edges {
            if st.is_alive(e) {
                gone += destroy_edge_with_links(st, e)?;
            }
        }
        gone += st.destroy(node)?;
        Ok(gone)
    })
}

/// The diagrams of `element`, the edges that show it and the nodes that show it, in that order.
pub fn views_of(store: &Store, element: Obj) -> (Vec<Obj>, Vec<Obj>, Vec<Obj>) {
    let (mut diagrams, mut edges, mut nodes) = (Vec::new(), Vec::new(), Vec::new());
    for (src, prop) in store.referrers(element) {
        if prop == diagram::ELEMENT {
            diagrams.push(src);
        } else if prop == edge::ELEMENT {
            edges.push(src);
        } else if prop == node::ELEMENT {
            nodes.push(src);
        }
    }
    (diagrams, edges, nodes)
}

/// Removes every view of `element` and of everything it contains: the diagrams of them, the edges
/// that show them, the nodes that show them with the edges on those nodes. A notation calls it
/// before it destroys a model element, so no node stays behind showing nothing (the MOF core alone
/// would drop the optional reference and leave the shape). All or nothing; gives how many objects
/// went.
pub fn remove_views_of(store: &mut Store, element: Obj) -> Result<usize, Invalid> {
    let mut elements = Vec::new();
    let mut stack = vec![element];
    let mut seen = BTreeSet::new();
    while let Some(o) = stack.pop() {
        if !seen.insert(o) {
            continue;
        }
        elements.push(o);
        stack.extend(store.owned_elements(o)?);
    }
    let (mut diagrams, mut edges, mut nodes) = (Vec::new(), Vec::new(), Vec::new());
    for e in elements {
        let (d, ed, n) = views_of(store, e);
        diagrams.extend(d);
        edges.extend(ed);
        nodes.extend(n);
    }
    store.transaction(|st| {
        let mut gone = 0;
        for d in diagrams {
            if st.is_alive(d) {
                gone += st.destroy(d)?;
            }
        }
        for e in edges {
            if st.is_alive(e) {
                gone += destroy_edge_with_links(st, e)?;
            }
        }
        for n in nodes {
            if st.is_alive(n) {
                gone += destroy_node_with_edges(st, n)?;
            }
        }
        Ok(gone)
    })
}
