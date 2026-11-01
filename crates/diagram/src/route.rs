//! How a line runs between two boxes (owner decisions on archforge-4ym): straight, at right angles,
//! at right angles around the boxes in its way, or curved. Pure geometry in diagram coordinates;
//! routes are worked out each time they are drawn and never stored.
//!
//! A right-angled line leaves a box from the middle of a side and enters the other box the same
//! way. Around the boxes in its way, it searches the channels between them: the lines along every
//! box's edges, kept `MARGIN` away, and the middles between them, for the path with the shortest
//! length and the fewest bends; a box it cannot get around leaves it the plain right-angled route.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock, PoisonError};

use archforge_mof::{Obj, Store, Value};

use crate::{Rect, Routing, diagram, edge, style};

/// How far a line keeps from the boxes it goes around, and how far it runs out of a box before
/// it turns.
pub const MARGIN: f64 = 16.0;
/// What one bend costs, in length: a longer path with fewer bends reads better.
const BEND: f64 = 48.0;
/// How far around the two boxes the search looks for a way, beside their own span.
const REACH: f64 = 240.0;
/// The most channel lines a search uses in each direction, so a crowded diagram stays quick.
const MAX_LINES: usize = 400;
/// The points a curve is drawn with.
const CURVE_STEPS: usize = 24;
/// The most avoiding routes remembered, and the most points they hold in all; past either the
/// memory starts afresh.
const REMEMBERED: usize = 4096;
const REMEMBERED_POINTS: usize = 250_000;
/// The most boxes one search goes around: the ones nearest the line's two boxes.
const MAX_WALLS: usize = 64;
/// The most crossings one search looks at before it gives up and the plain route is drawn.
const MAX_STEPS: usize = 20_000;
/// How far from the origin a box may lie for a search: past it the plain route is drawn.
const FAR: f64 = 1.0e7;

/// A point in diagram coordinates.
pub type P = (f64, f64);

/// How a line runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Way {
    Straight,
    RightAngled,
    /// At right angles, around the boxes in the way.
    Avoiding,
    Curved,
}

/// The four sides of a box, as the direction a line leaves it by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Side {
    Left,
    Right,
    Top,
    Bottom,
}

const SIDES: [Side; 4] = [Side::Left, Side::Right, Side::Top, Side::Bottom];

impl Side {
    fn out(self) -> (f64, f64) {
        match self {
            Side::Left => (-1.0, 0.0),
            Side::Right => (1.0, 0.0),
            Side::Top => (0.0, -1.0),
            Side::Bottom => (0.0, 1.0),
        }
    }

    /// The point `at` along this side of `r`, from its left or top: 0.5 is its middle.
    fn mid(self, r: &Rect, at: f64) -> P {
        let (x, y) = (r.x + r.width * at, r.y + r.height * at);
        match self {
            Side::Left => (r.x, y),
            Side::Right => (r.x + r.width, y),
            Side::Top => (x, r.y),
            Side::Bottom => (x, r.y + r.height),
        }
    }

    /// The point `MARGIN` out of that point.
    fn stub(self, r: &Rect, at: f64) -> P {
        let (x, y) = self.mid(r, at);
        let (dx, dy) = self.out();
        (x + dx * MARGIN, y + dy * MARGIN)
    }
}

fn centre(r: &Rect) -> P {
    (r.x + r.width / 2.0, r.y + r.height / 2.0)
}

/// How the edge `e` runs: as its own style says, else as its diagram's says; straight when
/// neither says.
pub fn way_of(store: &Store, e: Obj) -> Way {
    let routing = |owner: Obj, prop| -> Option<(Routing, bool)> {
        let s = store.get(owner, prop).ok()?.as_ref()?;
        let r = match store.get(s, style::ROUTING).ok()? {
            Value::Enum(i) => Routing::from_index(i)?,
            _ => return None,
        };
        (r != Routing::AsDiagram).then(|| (r, matches!(store.get(s, style::AVOID), Ok(Value::Bool(true)))))
    };
    let own = routing(e, edge::STYLE);
    let of_diagram = || store.container(e).ok().flatten().and_then(|(d, _)| routing(d, diagram::STYLE));
    match own.or_else(of_diagram) {
        Some((Routing::RightAngled, true)) => Way::Avoiding,
        Some((Routing::RightAngled, false)) => Way::RightAngled,
        Some((Routing::Curved, _)) => Way::Curved,
        _ => Way::Straight,
    }
}

/// Where along its boxes' sides each end of every line of diagram `d` sits, for [`route`]: a box's
/// lines are grouped by the side that faces the box at their other end, and spread over the middle
/// three fifths of that side in the order of those boxes along it, so they leave side by side and
/// do not cross beside it. A lone line sits in the middle; a line missing here sits in the middle.
/// `rect` gives a box's rectangle (the store's, or a drag's preview).
pub fn spreads(store: &Store, d: Obj, rect: impl Fn(Obj) -> Rect) -> HashMap<Obj, (f64, f64)> {
    let ends = |x: Obj| {
        let at = |p| store.get(x, p).ok().and_then(|v| v.as_ref());
        at(edge::SOURCE).zip(at(edge::TARGET)).filter(|(a, b)| a != b && at(edge::ELEMENT).is_some())
    };
    // Each end: the box it is on, the side it leaves by, where its other box lies along that side.
    let mut groups: HashMap<(Obj, Side), Vec<(f64, Obj, usize)>> = HashMap::new();
    let mut rects: HashMap<Obj, Rect> = HashMap::new();
    let mut rect_of = |n: Obj| *rects.entry(n).or_insert_with(|| rect(n));
    for e in store.list(d, diagram::EDGE).unwrap_or_default().iter().filter_map(Value::as_ref) {
        let Some((s, t)) = ends(e) else { continue };
        let (rs, rt) = (rect_of(s), rect_of(t));
        for (end, own, other) in [(0, (s, rs), rt), (1, (t, rt), rs)] {
            if let Some((side, _)) = facing(&own.1, &other) {
                let (cx, cy) = centre(&other);
                let along = if matches!(side, Side::Top | Side::Bottom) { cx } else { cy };
                groups.entry((own.0, side)).or_default().push((along, e, end));
            }
        }
    }
    let mut out: HashMap<Obj, (f64, f64)> = HashMap::new();
    for mut group in groups.into_values() {
        group.sort_by(|p, q| p.0.total_cmp(&q.0).then(p.1.cmp(&q.1)).then(p.2.cmp(&q.2)));
        let n = group.len();
        for (k, (_, e, end)) in group.into_iter().enumerate() {
            let at = if n > 1 { 0.2 + 0.6 * k as f64 / (n - 1) as f64 } else { 0.5 };
            let slot = out.entry(e).or_insert((0.5, 0.5));
            if end == 0 {
                slot.0 = at;
            } else {
                slot.1 = at;
            }
        }
    }
    out
}

/// The points of a line from `from` to `to` through `via`, drawn as a polyline: its first point
/// on `from`'s border, its last on `to`'s. `at` says where along a side each end sits, 0.5 its
/// middle, so the lines of one box spread out ([`spreads`]). `boxes` are the other boxes on the diagram, which an
/// avoiding line goes around. A curve and an avoiding line take no waypoints: with any, they run
/// at right angles through them.
pub fn route(way: Way, from: &Rect, to: &Rect, at: (f64, f64), via: &[P], boxes: &[Rect]) -> Vec<P> {
    let at = (if at.0.is_finite() { at.0.clamp(0.0, 1.0) } else { 0.5 }, if at.1.is_finite() { at.1.clamp(0.0, 1.0) } else { 0.5 });
    let fine = |r: &Rect| [r.x, r.y, r.width, r.height].iter().all(|v| v.is_finite());
    if !fine(from) || !fine(to) || via.iter().any(|(x, y)| !x.is_finite() || !y.is_finite()) {
        return Vec::new();
    }
    match way {
        Way::Straight => straight(from, to, via),
        _ if !via.is_empty() => through(from, to, via),
        Way::RightAngled => right_angled(from, to, at),
        Way::Avoiding => avoiding(from, to, at, boxes).unwrap_or_else(|| right_angled(from, to, at)),
        Way::Curved => curved(from, to, at),
    }
}

/// Where the line from the middle of `r` towards `toward` leaves `r`.
pub fn exit(r: &Rect, toward: P) -> P {
    let (cx, cy) = centre(r);
    let (dx, dy) = (toward.0 - cx, toward.1 - cy);
    if (dx == 0.0 && dy == 0.0) || !dx.is_finite() || !dy.is_finite() {
        return (cx, cy);
    }
    let tx = if dx.abs() > f64::EPSILON { (r.width / 2.0) / dx.abs() } else { f64::INFINITY };
    let ty = if dy.abs() > f64::EPSILON { (r.height / 2.0) / dy.abs() } else { f64::INFINITY };
    let t = tx.min(ty);
    if t.is_finite() { (cx + dx * t, cy + dy * t) } else { (cx, cy) }
}

fn straight(from: &Rect, to: &Rect, via: &[P]) -> Vec<P> {
    let mut points = vec![centre(from)];
    points.extend_from_slice(via);
    points.push(centre(to));
    let n = points.len();
    points[0] = exit(from, points[1]);
    points[n - 1] = exit(to, points[n - 2]);
    points
}

/// The sides the line leaves `from` and enters `to` by: across the wider gap between them, left
/// and right or top and bottom; None when the boxes overlap both ways.
fn facing(from: &Rect, to: &Rect) -> Option<(Side, Side)> {
    let gap_x = (to.x - (from.x + from.width)).max(from.x - (to.x + to.width));
    let gap_y = (to.y - (from.y + from.height)).max(from.y - (to.y + to.height));
    if gap_x <= 0.0 && gap_y <= 0.0 {
        return None;
    }
    let (fc, tc) = (centre(from), centre(to));
    Some(if gap_x >= gap_y {
        if tc.0 >= fc.0 { (Side::Right, Side::Left) } else { (Side::Left, Side::Right) }
    } else if tc.1 >= fc.1 {
        (Side::Bottom, Side::Top)
    } else {
        (Side::Top, Side::Bottom)
    })
}

/// Out of one side, into the facing one: straight across when the boxes share a band, else with
/// two bends halfway.
fn right_angled(from: &Rect, to: &Rect, at: (f64, f64)) -> Vec<P> {
    let Some((out, into)) = facing(from, to) else { return straight(from, to, &[]) };
    let horizontal = matches!(out, Side::Left | Side::Right);
    let (a, b) = (out.mid(from, at.0), into.mid(to, at.1));
    let points = if horizontal {
        // The band both boxes span, top to bottom.
        let (top, bottom) = (from.y.max(to.y), (from.y + from.height).min(to.y + to.height));
        if bottom - top >= 8.0 {
            let y = (top + bottom) / 2.0;
            vec![(a.0, y), (b.0, y)]
        } else {
            let mx = (a.0 + b.0) / 2.0;
            vec![a, (mx, a.1), (mx, b.1), b]
        }
    } else {
        let (left, right) = (from.x.max(to.x), (from.x + from.width).min(to.x + to.width));
        if right - left >= 8.0 {
            let x = (left + right) / 2.0;
            vec![(x, a.1), (x, b.1)]
        } else {
            let my = (a.1 + b.1) / 2.0;
            vec![a, (a.0, my), (b.0, my), b]
        }
    };
    simplify(points)
}

/// At right angles through the waypoints: each leg turns once, horizontally first. The line
/// leaves a box straight towards its first waypoint when the box spans it across or down, and
/// enters the other box the same way, so a route whose corners were dragged keeps its right
/// angles at the boxes.
fn through(from: &Rect, to: &Rect, via: &[P]) -> Vec<P> {
    let mut points = vec![facing_point(from, via[0])];
    for &q in via {
        let last = points[points.len() - 1];
        points.push((q.0, last.1));
        points.push(q);
    }
    let last = points[points.len() - 1];
    let end = facing_point(to, last);
    if (end.0 - last.0).abs() > 1e-9 && (end.1 - last.1).abs() > 1e-9 {
        // Into the box along its side's normal: the last leg turns before it.
        if (end.0 - to.x).abs() < 1e-9 || (end.0 - (to.x + to.width)).abs() < 1e-9 {
            points.push((last.0, end.1));
        } else {
            points.push((end.0, last.1));
        }
    }
    points.push(end);
    simplify(points)
}

/// Where a line from `r` towards `q` meets `r`'s border at right angles: straight up or down when
/// `r` spans `q` across, left or right when it spans it down, else where the line to its middle
/// leaves it.
fn facing_point(r: &Rect, q: P) -> P {
    let inside_x = q.0 > r.x && q.0 < r.x + r.width;
    let inside_y = q.1 > r.y && q.1 < r.y + r.height;
    match (inside_x, inside_y) {
        (true, false) => (q.0, if q.1 < r.y { r.y } else { r.y + r.height }),
        (false, true) => (if q.0 < r.x { r.x } else { r.x + r.width }, q.1),
        _ => exit(r, q),
    }
}

/// A cubic curve out of one side and into the facing one, its handles along the sides' normals.
fn curved(from: &Rect, to: &Rect, at: (f64, f64)) -> Vec<P> {
    let Some((out, into)) = facing(from, to) else { return straight(from, to, &[]) };
    let (a, b) = (out.mid(from, at.0), into.mid(to, at.1));
    let span = ((b.0 - a.0).abs() + (b.1 - a.1).abs()) * 0.4;
    let k = span.max(40.0);
    let (o, i) = (out.out(), into.out());
    let c1 = (a.0 + o.0 * k, a.1 + o.1 * k);
    let c2 = (b.0 + i.0 * k, b.1 + i.1 * k);
    (0..=CURVE_STEPS)
        .map(|s| {
            let t = s as f64 / CURVE_STEPS as f64;
            let u = 1.0 - t;
            let (w0, w1, w2, w3) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
            (w0 * a.0 + w1 * c1.0 + w2 * c2.0 + w3 * b.0, w0 * a.1 + w1 * c1.1 + w2 * c2.1 + w3 * b.1)
        })
        .collect()
}

/// Drops repeated points and the middle of three in a row on one line.
fn simplify(points: Vec<P>) -> Vec<P> {
    let mut out: Vec<P> = Vec::with_capacity(points.len());
    for q in points {
        if out.last().is_some_and(|l| (l.0 - q.0).abs() < 1e-9 && (l.1 - q.1).abs() < 1e-9) {
            continue;
        }
        if out.len() >= 2 {
            let (a, b) = (out[out.len() - 2], out[out.len() - 1]);
            let collinear = ((a.0 - b.0).abs() < 1e-9 && (b.0 - q.0).abs() < 1e-9) || ((a.1 - b.1).abs() < 1e-9 && (b.1 - q.1).abs() < 1e-9);
            if collinear {
                out.pop();
            }
        }
        out.push(q);
    }
    out
}

/// A box grown by `m` on every side.
fn grown(r: &Rect, m: f64) -> Rect {
    Rect::new(r.x - m, r.y - m, r.width + 2.0 * m, r.height + 2.0 * m)
}

/// Whether `(x, y)` is inside `r`, its border excluded.
fn inside(r: &Rect, x: f64, y: f64) -> bool {
    x > r.x + 1e-9 && x < r.x + r.width - 1e-9 && y > r.y + 1e-9 && y < r.y + r.height - 1e-9
}

/// The channel lines of one axis: the given coordinates and the middles between neighbours,
/// sorted and without repeats, at most `MAX_LINES`.
fn lines(mut at: Vec<f64>) -> Vec<f64> {
    at.retain(|v| v.is_finite());
    at.sort_by(f64::total_cmp);
    at.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
    let mut out = Vec::with_capacity(at.len() * 2);
    for w in at.windows(2) {
        out.push(w[0]);
        out.push((w[0] + w[1]) / 2.0);
    }
    out.extend(at.last());
    if out.len() > MAX_LINES {
        // Keep the box edges, drop the middles; past the cap, the ones nearest the middle.
        out = at;
        if out.len() > MAX_LINES {
            let middle = (out[0] + out[out.len() - 1]) / 2.0;
            out.sort_by(|p, q| (p - middle).abs().total_cmp(&(q - middle).abs()));
            out.truncate(MAX_LINES);
            out.sort_by(f64::total_cmp);
        }
    }
    out
}

/// The shortest way with the fewest bends from a side of `from` to a side of `to` that keeps
/// `MARGIN` from every other box near them; None when there is none.
fn avoiding(from: &Rect, to: &Rect, at: (f64, f64), boxes: &[Rect]) -> Option<Vec<P>> {
    facing(from, to)?;
    let far = |r: &Rect| [r.x, r.y, r.x + r.width, r.y + r.height].iter().any(|v| v.abs() > FAR);
    if far(from) || far(to) {
        return None;
    }
    // The search looks around the two boxes, not over the whole diagram.
    let left = from.x.min(to.x) - REACH;
    let top = from.y.min(to.y) - REACH;
    let right = (from.x + from.width).max(to.x + to.width) + REACH;
    let bottom = (from.y + from.height).max(to.y + to.height) + REACH;
    let window = Rect::new(left, top, right - left, bottom - top);
    let near = |r: &Rect| r.x < window.x + window.width && r.x + r.width > window.x && r.y < window.y + window.height && r.y + r.height > window.y;
    let starts: Vec<(Side, P)> = SIDES.iter().map(|s| (*s, s.stub(from, at.0))).collect();
    let ends: Vec<(Side, P)> = SIDES.iter().map(|s| (*s, s.stub(to, at.1))).collect();
    // The boxes in the way, grown by the margin; one that covers an end's way out is left out, so
    // overlapping boxes still connect.
    let mut walls: Vec<Rect> = boxes
        .iter()
        .filter(|r| near(r) && !far(r))
        .map(|r| grown(r, MARGIN - 1.0))
        .filter(|w| !starts.iter().chain(&ends).any(|(_, q)| inside(w, q.0, q.1)))
        .collect();
    // In a crowded window, the boxes nearest the line's two count; the rest are far from its way.
    if walls.len() > MAX_WALLS {
        let ((ax, ay), (bx, by)) = (centre(from), centre(to));
        let (mx, my) = ((ax + bx) / 2.0, (ay + by) / 2.0);
        let distance = |w: &Rect| {
            let (cx, cy) = centre(w);
            (cx - mx).abs() + (cy - my).abs()
        };
        walls.sort_by(|p, q| distance(p).total_cmp(&distance(q)));
        walls.truncate(MAX_WALLS);
    }
    // The same boxes near the same two give the same route: every frame draws it, a drag moves
    // only some of them.
    let key = {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        at.0.to_bits().hash(&mut h);
        at.1.to_bits().hash(&mut h);
        for r in [from, to].into_iter().chain(&walls) {
            for v in [r.x, r.y, r.width, r.height] {
                v.to_bits().hash(&mut h);
            }
        }
        h.finish()
    };
    let memory = MEMORY.get_or_init(|| Mutex::new(Memory::default()));
    if let Some(known) = memory.lock().unwrap_or_else(PoisonError::into_inner).routes.get(&key).cloned() {
        return known;
    }
    let found = search(from, to, at, &walls, &starts, &ends, window);
    let mut m = memory.lock().unwrap_or_else(PoisonError::into_inner);
    let points = found.as_ref().map_or(0, Vec::len);
    if m.routes.len() >= REMEMBERED || m.points + points > REMEMBERED_POINTS {
        m.routes.clear();
        m.points = 0;
    }
    m.points += points;
    m.routes.insert(key, found.clone());
    found
}

/// The routes worked out already, by what they were worked out from, and the points they hold.
#[derive(Default)]
struct Memory {
    routes: HashMap<u64, Option<Vec<P>>>,
    points: usize,
}

static MEMORY: OnceLock<Mutex<Memory>> = OnceLock::new();
/// Whether a search that gave up has been logged: once is enough.
static GAVE_UP: AtomicBool = AtomicBool::new(false);

/// The search itself: over the channel lines around `walls`, from a stub of `from` to one of `to`.
fn search(from: &Rect, to: &Rect, at: (f64, f64), walls: &[Rect], starts: &[(Side, P)], ends: &[(Side, P)], window: Rect) -> Option<Vec<P>> {
    let (left, top, right, bottom) = (window.x, window.y, window.x + window.width, window.y + window.height);
    let mut xs = vec![left, right];
    let mut ys = vec![top, bottom];
    for w in walls.iter().chain([&grown(from, MARGIN), &grown(to, MARGIN)]) {
        xs.extend([w.x, w.x + w.width]);
        ys.extend([w.y, w.y + w.height]);
    }
    for (_, q) in starts.iter().chain(ends) {
        xs.push(q.0);
        ys.push(q.1);
    }
    let (xs, ys) = (lines(xs), lines(ys));
    let index = |v: f64, along: &[f64]| along.iter().position(|a| (a - v).abs() < 1e-6);
    // A line may not cross the two boxes either, past their stubs.
    let own = [grown(from, MARGIN - 1.0), grown(to, MARGIN - 1.0)];
    let blocked = |x: f64, y: f64| walls.iter().chain(&own).any(|w| inside(w, x, y));
    let open = |a: (usize, usize), b: (usize, usize)| {
        let (x, y) = ((xs[a.0] + xs[b.0]) / 2.0, (ys[a.1] + ys[b.1]) / 2.0);
        !blocked(xs[b.0], ys[b.1]) && !blocked(x, y)
    };
    // Costs in hundredths, so the queue orders integers.
    let cost = |v: f64| (v * 100.0).round() as i64;
    let goal: HashMap<(usize, usize), Side> = ends.iter().filter_map(|(s, q)| Some(((index(q.0, &xs)?, index(q.1, &ys)?), *s))).collect();
    let (gx, gy) = centre(to);
    let guess = |c: (usize, usize)| cost((xs[c.0] - gx).abs() + (ys[c.1] - gy).abs());
    // A state: a crossing and the direction it was reached in (0 left, 1 right, 2 up, 3 down).
    let mut best: HashMap<((usize, usize), u8), i64> = HashMap::new();
    let mut came: HashMap<((usize, usize), u8), ((usize, usize), u8)> = HashMap::new();
    let mut queue = BinaryHeap::new();
    for (side, q) in starts {
        let Some(c) = index(q.0, &xs).zip(index(q.1, &ys)) else { continue };
        if blocked(q.0, q.1) {
            continue;
        }
        let d = side_dir(*side);
        best.insert((c, d), cost(MARGIN));
        queue.push(Reverse((cost(MARGIN).saturating_add(guess(c)), cost(MARGIN), c, d)));
    }
    let mut found = None;
    let mut steps = 0usize;
    while let Some(Reverse((_, g, c, d))) = queue.pop() {
        steps += 1;
        if steps > MAX_STEPS {
            if !GAVE_UP.swap(true, Ordering::Relaxed) {
                log::info!("a line's way around the tables took too long to find; it is drawn plain");
            }
            return None;
        }
        if best.get(&(c, d)).is_some_and(|b| *b < g) {
            continue;
        }
        if let Some(side) = goal.get(&c)
            && !blocked(xs[c.0], ys[c.1])
        {
            // Into the box against its side's way out: a last bend when the line comes in sideways.
            let inward = opposite(side_dir(*side));
            if d == inward {
                found = Some((c, d, *side));
                break;
            }
        }
        for nd in 0..4u8 {
            let next = match nd {
                0 if c.0 > 0 => (c.0 - 1, c.1),
                1 if c.0 + 1 < xs.len() => (c.0 + 1, c.1),
                2 if c.1 > 0 => (c.0, c.1 - 1),
                3 if c.1 + 1 < ys.len() => (c.0, c.1 + 1),
                _ => continue,
            };
            if nd == opposite(d) || !open(c, next) {
                continue;
            }
            let step = (xs[next.0] - xs[c.0]).abs() + (ys[next.1] - ys[c.1]).abs();
            let ng = g.saturating_add(cost(step)).saturating_add(if nd == d { 0 } else { cost(BEND) });
            if best.get(&(next, nd)).is_none_or(|b| ng < *b) {
                best.insert((next, nd), ng);
                came.insert((next, nd), (c, d));
                queue.push(Reverse((ng.saturating_add(guess(next)), ng, next, nd)));
            }
        }
    }
    let (mut c, mut d, side) = found?;
    let mut path = vec![side.mid(to, at.1)];
    loop {
        path.push((xs[c.0], ys[c.1]));
        match came.get(&(c, d)) {
            Some(&(pc, pd)) => (c, d) = (pc, pd),
            None => break,
        }
    }
    // The first crossing is a stub: the line starts on the side it leaves by.
    let start_side = starts.iter().find(|(_, q)| index(q.0, &xs) == Some(c.0) && index(q.1, &ys) == Some(c.1)).map(|(s, _)| *s)?;
    path.push(start_side.mid(from, at.0));
    path.reverse();
    Some(simplify(path))
}

fn side_dir(side: Side) -> u8 {
    match side {
        Side::Left => 0,
        Side::Right => 1,
        Side::Top => 2,
        Side::Bottom => 3,
    }
}

fn opposite(d: u8) -> u8 {
    match d {
        0 => 1,
        1 => 0,
        2 => 3,
        _ => 2,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(x: f64, y: f64, w: f64, h: f64) -> Rect {
        Rect::new(x, y, w, h)
    }

    fn right_angles(points: &[P]) -> bool {
        points.windows(2).all(|w| (w[0].0 - w[1].0).abs() < 1e-6 || (w[0].1 - w[1].1).abs() < 1e-6)
    }

    /// Whether a polyline passes through the inside of `b`.
    fn crosses(points: &[P], b: &Rect) -> bool {
        points.windows(2).any(|w| {
            (0..=50).any(|s| {
                let t = s as f64 / 50.0;
                inside(b, w[0].0 + (w[1].0 - w[0].0) * t, w[0].1 + (w[1].1 - w[0].1) * t)
            })
        })
    }

    #[test]
    fn a_straight_line_runs_from_border_to_border() {
        let line = route(Way::Straight, &r(0.0, 0.0, 100.0, 50.0), &r(300.0, 0.0, 100.0, 50.0), (0.5, 0.5), &[], &[]);
        assert_eq!(line, vec![(100.0, 25.0), (300.0, 25.0)]);
    }

    #[test]
    fn a_right_angled_line_goes_straight_across_a_shared_band_and_bends_twice_otherwise() {
        let across = route(Way::RightAngled, &r(0.0, 0.0, 100.0, 100.0), &r(300.0, 50.0, 100.0, 100.0), (0.5, 0.5), &[], &[]);
        assert_eq!(across, vec![(100.0, 75.0), (300.0, 75.0)]);
        let bent = route(Way::RightAngled, &r(0.0, 0.0, 100.0, 50.0), &r(300.0, 200.0, 100.0, 50.0), (0.5, 0.5), &[], &[]);
        assert!(right_angles(&bent) && bent.len() == 4, "{bent:?}");
        assert_eq!((bent[0], bent[3]), ((100.0, 25.0), (300.0, 225.0)));
        // One box under the other: out of the bottom, into the top.
        let down = route(Way::RightAngled, &r(0.0, 0.0, 100.0, 50.0), &r(0.0, 300.0, 100.0, 50.0), (0.5, 0.5), &[], &[]);
        assert_eq!(down, vec![(50.0, 50.0), (50.0, 300.0)]);
    }

    #[test]
    fn an_avoiding_line_goes_around_the_box_in_its_way() {
        let (a, b) = (r(0.0, 0.0, 100.0, 60.0), r(500.0, 0.0, 100.0, 60.0));
        let wall = r(220.0, -40.0, 120.0, 140.0);
        let plain = route(Way::RightAngled, &a, &b, (0.5, 0.5), &[], &[wall]);
        assert!(crosses(&plain, &wall), "the plain route runs through it");
        let around = route(Way::Avoiding, &a, &b, (0.5, 0.5), &[], &[wall]);
        assert!(right_angles(&around), "{around:?}");
        assert!(!crosses(&around, &grown(&wall, MARGIN - 2.0)), "{around:?}");
        assert!(!crosses(&around, &a) && !crosses(&around, &b), "{around:?}");
        assert!(SIDES.iter().any(|s| around.first() == Some(&s.mid(&a, 0.5))), "it starts on a side's middle: {around:?}");
        assert!(SIDES.iter().any(|s| around.last() == Some(&s.mid(&b, 0.5))), "it ends on a side's middle: {around:?}");
        // Nothing in the way: the same as the plain route.
        assert_eq!(route(Way::Avoiding, &a, &b, (0.5, 0.5), &[], &[]), route(Way::RightAngled, &a, &b, (0.5, 0.5), &[], &[]));
    }

    #[test]
    fn a_box_with_no_way_around_leaves_the_plain_route() {
        let (a, b) = (r(0.0, 0.0, 100.0, 60.0), r(500.0, 0.0, 100.0, 60.0));
        // Walls closing the two boxes in on every side within reach.
        let walls = [
            r(-400.0, -400.0, 1400.0, 300.0),
            r(-400.0, 160.0, 1400.0, 300.0),
            r(-400.0, -100.0, 300.0, 260.0),
            r(700.0, -100.0, 300.0, 260.0),
            r(250.0, -100.0, 100.0, 260.0),
        ];
        let line = route(Way::Avoiding, &a, &b, (0.5, 0.5), &[], &walls);
        assert_eq!(line, route(Way::RightAngled, &a, &b, (0.5, 0.5), &[], &[]));
    }

    #[test]
    fn a_crowded_or_far_off_diagram_gives_the_plain_route_quickly() {
        let (a, b) = (r(0.0, 0.0, 100.0, 60.0), r(4000.0, 3000.0, 100.0, 60.0));
        // Hundreds of small boxes between them: the search stays within its budget.
        let crowd: Vec<Rect> = (0..900).map(|i| r(200.0 + (i % 30) as f64 * 130.0, 100.0 + (i / 30) as f64 * 95.0, 60.0, 40.0)).collect();
        let started = std::time::Instant::now();
        let line = route(Way::Avoiding, &a, &b, (0.5, 0.5), &[], &crowd);
        assert!(line.len() >= 2 && started.elapsed() < std::time::Duration::from_secs(2), "{:?}", started.elapsed());
        let far = r(1.0e12, 0.0, 100.0, 60.0);
        assert_eq!(route(Way::Avoiding, &a, &far, (0.5, 0.5), &[], &[]), route(Way::RightAngled, &a, &far, (0.5, 0.5), &[], &[]));
    }

    #[test]
    fn a_curve_leaves_and_enters_along_the_sides_normals() {
        let line = route(Way::Curved, &r(0.0, 0.0, 100.0, 50.0), &r(300.0, 200.0, 100.0, 50.0), (0.5, 0.5), &[], &[]);
        assert_eq!(line.len(), CURVE_STEPS + 1);
        assert_eq!((line[0], line[CURVE_STEPS]), ((100.0, 25.0), (300.0, 225.0)));
        assert!(line[1].0 > line[0].0 && (line[1].1 - line[0].1).abs() < (line[1].0 - line[0].0), "leaves to the right");
    }

    #[test]
    fn waypoints_are_kept_and_nothing_breaks_on_odd_input() {
        let line = route(Way::Avoiding, &r(0.0, 0.0, 100.0, 50.0), &r(300.0, 200.0, 100.0, 50.0), (0.5, 0.5), &[(200.0, 100.0)], &[]);
        assert!(right_angles(&line) && line.contains(&(200.0, 100.0)), "{line:?}");
        assert!(route(Way::RightAngled, &r(f64::NAN, 0.0, 1.0, 1.0), &r(0.0, 0.0, 1.0, 1.0), (0.5, 0.5), &[], &[]).is_empty());
        // Overlapping boxes: a short straight line, never a panic.
        let overlap = route(Way::Avoiding, &r(0.0, 0.0, 100.0, 100.0), &r(50.0, 50.0, 100.0, 100.0), (0.5, 0.5), &[], &[]);
        assert_eq!(overlap.len(), 2);
    }
}
