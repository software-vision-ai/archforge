//! The spatial index of one diagram, for culling and hit testing: nodes and edges binned into
//! square cells of the logical unit. Built from the store and queried without it; pure. The canvas
//! rebuilds it when the diagram changes (the spike measured the shape at ten thousand nodes).

use std::collections::{BTreeMap, BTreeSet};

use archforge_mof::{Obj, Store, Value};

use crate::rules::{MAX_COORD, MAX_SIZE, real};
use crate::{diagram, edge, node, node_tree, point};

/// A cell's side, in the logical unit.
pub const CELL: f64 = 256.0;
/// How far from an edge's line a point still hits it.
pub const EDGE_TOLERANCE: f64 = 4.0;
/// The most samples along one segment: a step is half a cell or a 1024th of the segment, whichever
/// is longer, so a long edge is sampled coarsely and a build stays bounded.
const MAX_STEPS: usize = 1024;
/// The most cells a viewport is walked through before `visible` scans the items instead.
const MAX_VIEW_CELLS: i64 = 4096;
/// The farthest cell either way: everything beyond the coordinate bound lands in the border cells.
const LIMIT: f64 = 2.0 * MAX_COORD / CELL;

/// An axis-aligned rectangle in the logical unit.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Rect {
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Rect {
        Rect { x, y, width, height }
    }

    pub fn contains(&self, px: f64, py: f64) -> bool {
        px >= self.x && px <= self.x + self.width && py >= self.y && py <= self.y + self.height
    }

    pub fn intersects(&self, o: &Rect) -> bool {
        self.x <= o.x + o.width && o.x <= self.x + self.width && self.y <= o.y + o.height && o.y <= self.y + self.height
    }

    pub fn center(&self) -> (f64, f64) {
        (self.x + self.width / 2.0, self.y + self.height / 2.0)
    }

    /// The smallest rectangle around both.
    fn union(&self, o: &Rect) -> Rect {
        let (x, y) = (self.x.min(o.x), self.y.min(o.y));
        Rect { x, y, width: (self.x + self.width).max(o.x + o.width) - x, height: (self.y + self.height).max(o.y + o.height) - y }
    }

    fn is_finite(&self) -> bool {
        self.x.is_finite() && self.y.is_finite() && self.width.is_finite() && self.height.is_finite() && self.width >= 0.0 && self.height >= 0.0
    }

    /// The corner cells the rectangle spans.
    fn span(&self) -> ((i64, i64), (i64, i64)) {
        ((cell(self.x), cell(self.y)), (cell(self.x + self.width), cell(self.y + self.height)))
    }

    /// The cells a node's rectangle covers, its sides capped at the largest node so a rectangle
    /// beyond the rules cannot flood the index.
    fn node_cells(&self) -> impl Iterator<Item = (i64, i64)> {
        let capped = Rect::new(self.x, self.y, self.width.clamp(0.0, MAX_SIZE), self.height.clamp(0.0, MAX_SIZE));
        let ((x0, y0), (x1, y1)) = capped.span();
        (x0..=x1).flat_map(move |cx| (y0..=y1).map(move |cy| (cx, cy)))
    }
}

/// The cell a coordinate falls in. The quotient is finite and clamped before it becomes an integer,
/// so the conversion is exact; a coordinate that is not a number lands in cell 0.
fn cell(v: f64) -> i64 {
    let c = (v / CELL).floor();
    if c.is_finite() { c.clamp(-LIMIT, LIMIT) as i64 } else { 0 }
}

/// What a point hits, the topmost first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hit {
    Node(Obj),
    Edge(Obj),
}

/// What a viewport shows, each list in drawing order.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Visible {
    pub nodes: Vec<Obj>,
    pub edges: Vec<Obj>,
}

struct NodeItem {
    obj: Obj,
    rect: Rect,
}

struct EdgeItem {
    obj: Obj,
    /// The line as drawn: from the source's edge through the waypoints to the target's edge; a
    /// note link straight to a node's edge or to a line's middle.
    points: Vec<(f64, f64)>,
    bounds: Rect,
}

/// The index of one diagram's nodes (children included, parents first) and edges (waypoints
/// included), in drawing order.
pub struct Grid {
    nodes: Vec<NodeItem>,
    edges: Vec<EdgeItem>,
    node_cells: BTreeMap<(i64, i64), Vec<usize>>,
    edge_cells: BTreeMap<(i64, i64), Vec<usize>>,
}

/// How far a node's own line loops out of its right side, and half its height there.
pub const LOOP_REACH: f64 = 40.0;
pub const LOOP_HALF: f64 = 20.0;

/// Where the line from `rect`'s centre towards `toward` leaves the rectangle; the centre for a
/// point at the centre.
pub fn exit_point(rect: &Rect, toward: (f64, f64)) -> (f64, f64) {
    let c = rect.center();
    let (dx, dy) = (toward.0 - c.0, toward.1 - c.1);
    if (dx == 0.0 && dy == 0.0) || !dx.is_finite() || !dy.is_finite() {
        return c;
    }
    let tx = if dx.abs() > f64::EPSILON { (rect.width / 2.0) / dx.abs() } else { f64::INFINITY };
    let ty = if dy.abs() > f64::EPSILON { (rect.height / 2.0) / dy.abs() } else { f64::INFINITY };
    let t = tx.min(ty);
    if t.is_finite() { (c.0 + dx * t, c.1 + dy * t) } else { c }
}

/// A node's own line: out of its right side and back, a loop of [`LOOP_REACH`] by twice
/// [`LOOP_HALF`].
pub fn self_loop(rect: &Rect) -> Vec<(f64, f64)> {
    let (right, cy) = (rect.x + rect.width, rect.y + rect.height / 2.0);
    vec![(right, cy - LOOP_HALF), (right + LOOP_REACH, cy - LOOP_HALF), (right + LOOP_REACH, cy + LOOP_HALF), (right, cy + LOOP_HALF)]
}

/// The point halfway along a polyline, by length; its first point when it has no length.
pub fn middle_of(points: &[(f64, f64)]) -> (f64, f64) {
    let len = |a: (f64, f64), b: (f64, f64)| ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
    let total: f64 = points.windows(2).map(|s| len(s[0], s[1])).sum();
    let mut left = total / 2.0;
    for s in points.windows(2) {
        let l = len(s[0], s[1]);
        if l > 0.0 && left <= l {
            let t = left / l;
            return (s[0].0 + (s[1].0 - s[0].0) * t, s[0].1 + (s[1].1 - s[0].1) * t);
        }
        left -= l;
    }
    points.first().copied().unwrap_or((0.0, 0.0))
}

/// A node's rectangle as stored.
pub fn bounds_of(store: &Store, n: Obj) -> Rect {
    Rect::new(real(store, n, node::X), real(store, n, node::Y), real(store, n, node::WIDTH), real(store, n, node::HEIGHT))
}

impl Grid {
    /// Indexes `diagram` as the store holds it now. An edge whose end is missing is left out; a
    /// note link that ends on an edge ends at that edge's middle, so it is indexed after the edges
    /// between nodes.
    pub fn build(store: &Store, diagram: Obj) -> Grid {
        let mut nodes = Vec::new();
        let mut node_cells: BTreeMap<(i64, i64), Vec<usize>> = BTreeMap::new();
        let mut index_of: BTreeMap<Obj, usize> = BTreeMap::new();
        let tops: Vec<(Obj, Rect)> =
            store.list(diagram, diagram::NODE).unwrap_or_default().iter().filter_map(Value::as_ref).map(|n| (n, bounds_of(store, n))).collect();
        for top in store.list(diagram, diagram::NODE).unwrap_or_default().iter().filter_map(Value::as_ref) {
            for n in node_tree(store, top) {
                let rect = bounds_of(store, n);
                let i = nodes.len();
                index_of.insert(n, i);
                for c in rect.node_cells() {
                    node_cells.entry(c).or_default().push(i);
                }
                nodes.push(NodeItem { obj: n, rect });
            }
        }
        let mut edges = Vec::new();
        let mut edge_cells: BTreeMap<(i64, i64), Vec<usize>> = BTreeMap::new();
        let all: Vec<Obj> = store.list(diagram, diagram::EDGE).unwrap_or_default().iter().filter_map(Value::as_ref).collect();
        let on_edge =
            |e: &Obj| store.get(*e, edge::TARGET).ok().and_then(|v| v.as_ref()).is_some_and(|t| store.class_of(t).ok() == Some(crate::EDGE));
        let mut middles: BTreeMap<Obj, (f64, f64)> = BTreeMap::new();
        // Where each line's ends sit along their boxes' sides, worked out once for the diagram.
        let spreads = crate::route::spreads(store, diagram, |n| bounds_of(store, n));
        for e in all.iter().filter(|e| !on_edge(e)).chain(all.iter().filter(|e| on_edge(e))).copied() {
            let end_obj = |prop| store.get(e, prop).ok().and_then(|v| v.as_ref());
            let rect_of = |o: Option<Obj>| o.and_then(|n| index_of.get(&n)).and_then(|i| nodes.get(*i)).map(|n| n.rect);
            let (source, target) = (end_obj(edge::SOURCE), end_obj(edge::TARGET));
            let Some(src) = rect_of(source) else { continue };
            let element = store.get(e, edge::ELEMENT).ok().and_then(|v| v.as_ref());
            // The line as the canvas draws it: cut at the nodes' edges, a node's own line looping
            // out of its right side; a note link straight, to a node's edge or a line's middle.
            let points = if element.is_none() {
                let dst = match rect_of(target) {
                    Some(t) => exit_point(&t, src.center()),
                    None => match target.and_then(|t| middles.get(&t).copied()) {
                        Some(m) => m,
                        None => continue,
                    },
                };
                vec![exit_point(&src, dst), dst]
            } else if source == target {
                self_loop(&src)
            } else {
                let Some(dst) = rect_of(target) else { continue };
                let via: Vec<(f64, f64)> = store
                    .list(e, edge::WAYPOINT)
                    .unwrap_or_default()
                    .iter()
                    .filter_map(Value::as_ref)
                    .map(|w| (real(store, w, point::X), real(store, w, point::Y)))
                    .collect();
                // The other boxes on the diagram, which an avoiding line goes around.
                let way = crate::route::way_of(store, e);
                let boxes: Vec<Rect> = if way == crate::Way::Avoiding {
                    tops.iter().filter(|(n, _)| Some(*n) != source && Some(*n) != target).map(|(_, r)| *r).collect()
                } else {
                    Vec::new()
                };
                let at = spreads.get(&e).copied().unwrap_or((0.5, 0.5));
                let points = crate::route(way, &src, &dst, at, &via, &boxes);
                if points.len() < 2 {
                    continue;
                }
                points
            };
            if points.iter().any(|(x, y)| !x.is_finite() || !y.is_finite()) {
                continue;
            }
            middles.insert(e, middle_of(&points));
            let first = points[0];
            let mut bounds = Rect::new(first.0, first.1, 0.0, 0.0);
            for p in &points {
                bounds = bounds.union(&Rect::new(p.0, p.1, 0.0, 0.0));
            }
            let i = edges.len();
            let mut cells = BTreeSet::new();
            for seg in points.windows(2) {
                if let [a, b] = seg {
                    walk(*a, *b, &mut cells);
                }
            }
            for c in cells {
                edge_cells.entry(c).or_default().push(i);
            }
            edges.push(EdgeItem { obj: e, points, bounds });
        }
        Grid { nodes, edges, node_cells, edge_cells }
    }

    /// How many nodes and edges are indexed.
    pub fn len(&self) -> (usize, usize) {
        (self.nodes.len(), self.edges.len())
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty() && self.edges.is_empty()
    }

    /// Where `n` comes in the drawing order, nested nodes included: later is drawn on top.
    pub fn drawn_at(&self, n: Obj) -> Option<usize> {
        self.nodes.iter().position(|item| item.obj == n)
    }

    /// Every line indexed with its points, in drawing order: one pass, where [`Grid::line`] looks
    /// up one.
    pub fn lines(&self) -> impl Iterator<Item = (Obj, &[(f64, f64)])> {
        self.edges.iter().map(|item| (item.obj, item.points.as_slice()))
    }

    /// The points of `e`'s line as indexed: as the canvas draws it from the store.
    pub fn line(&self, e: Obj) -> Option<&[(f64, f64)]> {
        self.edges.iter().find(|item| item.obj == e).map(|item| item.points.as_slice())
    }

    /// The nearest edge within [`EDGE_TOLERANCE`] of a point that `keep` takes, nodes aside: a note
    /// link dropped on a relationship's line looks past the links that end there.
    pub fn hit_edge(&self, x: f64, y: f64, keep: impl Fn(Obj) -> bool) -> Option<Obj> {
        if !x.is_finite() || !y.is_finite() {
            return None;
        }
        let mut best: Option<(f64, usize)> = None;
        for i in self.edge_cells.get(&(cell(x), cell(y))).into_iter().flatten() {
            let Some(e) = self.edges.get(*i).filter(|e| keep(e.obj)) else { continue };
            let d = e.points.windows(2).map(|s| if let [a, b] = s { distance(*a, *b, (x, y)) } else { f64::INFINITY }).fold(f64::INFINITY, f64::min);
            if d <= EDGE_TOLERANCE && best.is_none_or(|(bd, bi)| d < bd || (d == bd && *i > bi)) {
                best = Some((d, *i));
            }
        }
        best.and_then(|(_, i)| self.edges.get(i)).map(|e| e.obj)
    }

    /// The topmost node at a point, else the nearest edge within [`EDGE_TOLERANCE`].
    pub fn hit(&self, x: f64, y: f64) -> Option<Hit> {
        if !x.is_finite() || !y.is_finite() {
            return None;
        }
        let c = (cell(x), cell(y));
        if let Some(top) = self.node_cells.get(&c).into_iter().flatten().filter(|i| self.nodes.get(**i).is_some_and(|n| n.rect.contains(x, y))).max()
        {
            return self.nodes.get(*top).map(|n| Hit::Node(n.obj));
        }
        let mut best: Option<(f64, usize)> = None;
        for i in self.edge_cells.get(&c).into_iter().flatten() {
            let Some(e) = self.edges.get(*i) else { continue };
            let d = e.points.windows(2).map(|s| if let [a, b] = s { distance(*a, *b, (x, y)) } else { f64::INFINITY }).fold(f64::INFINITY, f64::min);
            if d <= EDGE_TOLERANCE && best.is_none_or(|(bd, bi)| d < bd || (d == bd && *i > bi)) {
                best = Some((d, *i));
            }
        }
        best.and_then(|(_, i)| self.edges.get(i)).map(|e| Hit::Edge(e.obj))
    }

    /// The nodes and edges a viewport shows, in drawing order; nothing for a viewport that is not a
    /// rectangle. A viewport wider than the cell budget is answered by scanning the items, so no
    /// zoom level loses what lies beyond the first cells.
    pub fn visible(&self, view: Rect) -> Visible {
        if !view.is_finite() {
            return Visible::default();
        }
        let mut nodes: BTreeSet<usize> = BTreeSet::new();
        let mut edges: BTreeSet<usize> = BTreeSet::new();
        let ((x0, y0), (x1, y1)) = view.span();
        let wide = (x1 - x0).saturating_add(1).saturating_mul((y1 - y0).saturating_add(1)) > MAX_VIEW_CELLS;
        if wide {
            nodes.extend(self.nodes.iter().enumerate().filter(|(_, n)| n.rect.intersects(&view)).map(|(i, _)| i));
            edges.extend(self.edges.iter().enumerate().filter(|(_, e)| e.bounds.intersects(&view)).map(|(i, _)| i));
        } else {
            for c in (x0..=x1).flat_map(|cx| (y0..=y1).map(move |cy| (cx, cy))) {
                nodes.extend(
                    self.node_cells.get(&c).into_iter().flatten().filter(|i| self.nodes.get(**i).is_some_and(|n| n.rect.intersects(&view))).copied(),
                );
                edges.extend(
                    self.edge_cells
                        .get(&c)
                        .into_iter()
                        .flatten()
                        .filter(|i| self.edges.get(**i).is_some_and(|e| e.bounds.intersects(&view)))
                        .copied(),
                );
            }
        }
        Visible {
            nodes: nodes.into_iter().filter_map(|i| self.nodes.get(i)).map(|n| n.obj).collect(),
            edges: edges.into_iter().filter_map(|i| self.edges.get(i)).map(|e| e.obj).collect(),
        }
    }
}

/// The cells a segment passes: sampled every half cell (coarser on a very long segment, up to
/// [`MAX_STEPS`] samples), with the eight cells around each sample, so a point within the tolerance
/// of the line and a viewport the line clips both share a cell with it.
fn walk(a: (f64, f64), b: (f64, f64), cells: &mut BTreeSet<(i64, i64)>) {
    let len = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
    // Finite and clamped to a constant before it becomes an integer, so the conversion is exact.
    let steps = if len.is_finite() { (len / (CELL / 2.0)).ceil().clamp(1.0, MAX_STEPS as f64) as usize } else { 1 };
    for s in 0..=steps {
        let t = s as f64 / steps as f64;
        let (x, y) = (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
        let (cx, cy) = (cell(x), cell(y));
        for dx in -1..=1 {
            for dy in -1..=1 {
                cells.insert((cx.saturating_add(dx), cy.saturating_add(dy)));
            }
        }
    }
}

/// The distance from `p` to the segment `a`–`b`.
fn distance(a: (f64, f64), b: (f64, f64), p: (f64, f64)) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len2 = dx * dx + dy * dy;
    let t = if len2 > 0.0 { (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / len2).clamp(0.0, 1.0) } else { 0.0 };
    let (cx, cy) = (a.0 + t * dx, a.1 + t * dy);
    ((p.0 - cx).powi(2) + (p.1 - cy).powi(2)).sqrt()
}
