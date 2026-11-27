//! The canvas's geometry and state, which name no shell: the view's mapping between diagram units
//! and the screen, where a line leaves a card, a self relationship's loop and the line that ties a
//! nested card to its owner's, a point along a line, the outline of a line's end mark, where a
//! qualifier, a template's signature, a selected line's end handles and its end texts sit; and what
//! the centre keeps between frames (`State`: the tabs, the view, the tool, the toggles, the editing
//! under way and what the last frame drew that takes a press). The shell draws the canvas with it
//! (`archforge_ui_egui::canvas` re-exports it under its old paths), and so will a notation's
//! screens, without the shell.

use std::collections::HashMap;

use archforge_diagram::figure::EndMark;
use archforge_mof::Uuid;
use egui::{Pos2, Rect, Vec2, pos2, vec2};

use crate::kinds::ExtraLabel;
use crate::ux;

/// The view's mapping between diagram units and the screen.
#[derive(Debug, Clone, Copy)]
pub struct View {
    pub origin: Pos2,
    pub pan: Vec2,
    pub zoom: f32,
}

impl View {
    /// The point on the screen of the diagram point (`x`, `y`).
    pub fn to_screen(self, x: f64, y: f64) -> Pos2 {
        self.origin + self.pan + vec2(x as f32, y as f32) * self.zoom
    }
    /// The rectangle on the screen of the diagram rectangle `b`.
    pub fn rect(self, b: &archforge_diagram::Rect) -> Rect {
        Rect::from_min_size(self.to_screen(b.x, b.y), vec2(b.width as f32, b.height as f32) * self.zoom)
    }
    /// The diagram point under the screen point `at`.
    pub fn to_canvas(self, at: Pos2) -> (f64, f64) {
        let v = (at - self.origin - self.pan) / self.zoom;
        (v.x as f64, v.y as f64)
    }
}

/// The route of the line that ties a nested classifier's card `nested` to its owner's card
/// `owner` (AR-132): straight where the cards face each other across a gap, else leaving the owner
/// at its side and turning once into the nested card, the elbow crossing fewer of the `others`
/// cards. Gives the points from the
/// ⊕'s rim to the nested card, and the ⊕'s centre, `radius` out from the owner's side; None when
/// the cards overlap, since a card over another is no ownership of its own and draws no line.
pub fn nesting_route(owner: Rect, nested: Rect, radius: f32, others: &[Rect]) -> Option<(Vec<Pos2>, Pos2)> {
    if owner.intersects(nested) || !owner.is_finite() || !nested.is_finite() {
        return None;
    }
    let (x_lo, x_hi) = (owner.left().max(nested.left()), owner.right().min(nested.right()));
    let (y_lo, y_hi) = (owner.top().max(nested.top()), owner.bottom().min(nested.bottom()));
    let below = nested.top() >= owner.bottom();
    let right = nested.left() >= owner.right();
    // Leaves the owner at `at` going `dir`: the ⊕ sits just outside, the line starts at its rim.
    let ring = |at: Pos2, dir: Vec2| (at + dir * radius, at + dir * 2.0 * radius);
    if x_hi - x_lo >= 2.0 * radius {
        let x = (x_lo + x_hi) / 2.0;
        let (from, to, dir) = if below {
            (pos2(x, owner.bottom()), pos2(x, nested.top()), vec2(0.0, 1.0))
        } else {
            (pos2(x, owner.top()), pos2(x, nested.bottom()), vec2(0.0, -1.0))
        };
        let (centre, start) = ring(from, dir);
        return Some((vec![start, to], centre));
    }
    if y_hi - y_lo >= 2.0 * radius {
        let y = (y_lo + y_hi) / 2.0;
        let (from, to, dir) = if right {
            (pos2(owner.right(), y), pos2(nested.left(), y), vec2(1.0, 0.0))
        } else {
            (pos2(owner.left(), y), pos2(nested.right(), y), vec2(-1.0, 0.0))
        };
        let (centre, start) = ring(from, dir);
        return Some((vec![start, to], centre));
    }
    // Apart both ways: an elbow, out of the owner's side then down or up into the nested card, or
    // out of its top or bottom then across into the nested card's side; the one crossing fewer
    // other cards.
    let (x, y) = (nested.center().x, nested.center().y);
    let toward_x = if x >= owner.center().x { 1.0 } else { -1.0 };
    let toward_y = if y >= owner.center().y { 1.0 } else { -1.0 };
    let side = pos2(if toward_x > 0.0 { owner.right() } else { owner.left() }, owner.center().y);
    let end_v = pos2(x, if toward_y > 0.0 { nested.top() } else { nested.bottom() });
    let (c1, s1) = ring(side, vec2(toward_x, 0.0));
    let across_first = vec![s1, pos2(x, side.y), end_v];
    let foot = pos2(owner.center().x, if toward_y > 0.0 { owner.bottom() } else { owner.top() });
    let end_h = pos2(if toward_x > 0.0 { nested.left() } else { nested.right() }, y);
    let (c2, s2) = ring(foot, vec2(0.0, toward_y));
    let down_first = vec![s2, pos2(foot.x, y), end_h];
    let crossings = |points: &[Pos2]| points.windows(2).map(|w| others.iter().filter(|o| segment_crosses(**o, w[0], w[1])).count()).sum::<usize>();
    if crossings(&down_first) < crossings(&across_first) { Some((down_first, c2)) } else { Some((across_first, c1)) }
}

/// Whether the level or upright segment from `a` to `b` passes through `r`, shrunk a little so a
/// segment along its edge does not count.
fn segment_crosses(r: Rect, a: Pos2, b: Pos2) -> bool {
    let inner = r.shrink(2.0);
    let seg = Rect::from_two_pos(a, b);
    inner.intersects(seg.expand(0.5))
}

/// Where a line from a rectangle's centre towards `toward` leaves the rectangle.
pub fn exit_point(rect: Rect, toward: Pos2) -> Pos2 {
    let c = rect.center();
    let d = toward - c;
    if d == Vec2::ZERO || !d.is_finite() {
        return c;
    }
    let tx = if d.x.abs() > f32::EPSILON { (rect.width() / 2.0) / d.x.abs() } else { f32::INFINITY };
    let ty = if d.y.abs() > f32::EPSILON { (rect.height() / 2.0) / d.y.abs() } else { f32::INFINITY };
    let t = tx.min(ty);
    if t.is_finite() { c + d * t } else { c }
}

/// How far a self relationship's loop reaches out of its entity.
/// The diagram's self-loop, on screen (the grid hit-tests the same shape).
const LOOP_REACH: f32 = archforge_diagram::grid::LOOP_REACH as f32;
const LOOP_HALF: f32 = archforge_diagram::grid::LOOP_HALF as f32;
/// The points of a self relationship's loop on the entity's right side.
pub fn self_loop(rect: Rect, z: f32) -> Vec<Pos2> {
    let cy = rect.center().y;
    vec![
        pos2(rect.right(), cy - LOOP_HALF * z),
        pos2(rect.right() + LOOP_REACH * z, cy - LOOP_HALF * z),
        pos2(rect.right() + LOOP_REACH * z, cy + LOOP_HALF * z),
        pos2(rect.right(), cy + LOOP_HALF * z),
    ]
}

/// A point along a polyline at `t` of its length, 0 at its start and 1 at its end.
pub fn along(points: &[Pos2], t: f32) -> Pos2 {
    let total: f32 = points.windows(2).map(|w| (w[1] - w[0]).length()).sum();
    if points.is_empty() {
        return Pos2::ZERO;
    }
    if total <= 0.0 || points.len() == 1 {
        return points[0];
    }
    let mut left = (total * t.clamp(0.0, 1.0)).max(0.0);
    for w in points.windows(2) {
        let len = (w[1] - w[0]).length();
        if left <= len {
            return w[0] + (w[1] - w[0]) * if len > 0.0 { left / len } else { 0.0 };
        }
        left -= len;
    }
    points[points.len() - 1]
}

/// Where a mark the line stops short of sits at `tip`, coming from `back`: the line's direction,
/// the mark's base, half its width across, and its length; a triangle stops the line at its base,
/// a diamond at its far corner.
pub fn mark_shape(mark: EndMark, tip: Pos2, back: Pos2, z: f32) -> (Vec2, Pos2, Vec2, f32) {
    let dir = if (tip - back).length() > f32::EPSILON { (tip - back).normalized() } else { vec2(0.0, -1.0) };
    let s = z.max(0.6);
    let (len, half) = match mark {
        EndMark::Triangle | EndMark::FilledTriangle => (12.0 * s, 7.5 * s),
        EndMark::HollowDiamond | EndMark::FilledDiamond => (20.0 * s, 6.0 * s),
        _ => (0.0, 6.0 * s),
    };
    (dir, tip - dir * len, vec2(-dir.y, dir.x) * half, len)
}

/// The corners of a line's mark as it is drawn at `tip`, coming from `back`: a triangle's tip and
/// base corners, a diamond's four corners, an open arrow's tip and the ends of its two strokes;
/// none for any other mark.
pub fn mark_outline(mark: EndMark, tip: Pos2, back: Pos2, z: f32) -> Vec<Pos2> {
    let (dir, base, side, len) = mark_shape(mark, tip, back, z);
    match mark {
        EndMark::Triangle | EndMark::FilledTriangle => vec![tip, base + side, base - side],
        EndMark::HollowDiamond | EndMark::FilledDiamond => {
            let middle = tip - dir * (len / 2.0);
            vec![tip, middle + side, base, middle - side]
        }
        EndMark::OpenArrow => {
            let back = tip - dir * 12.0 * z.max(0.6);
            vec![tip, back + side, back - side]
        }
        _ => Vec::new(),
    }
}

/// Where a qualifier box of `size` sits against `node` for a line leaving it at `tip`: on the side
/// `tip` is nearest, centred on it, and the point on its outer side the line starts from.
pub fn qualifier_place(node: Rect, tip: Pos2, size: Vec2) -> (Rect, Pos2) {
    let gaps = [(tip.x - node.left()).abs(), (tip.x - node.right()).abs(), (tip.y - node.top()).abs(), (tip.y - node.bottom()).abs()];
    let side = (0..4).min_by(|a, b| gaps[*a].total_cmp(&gaps[*b])).unwrap_or(1);
    let (w, h) = (size.x, size.y);
    match side {
        0 => (Rect::from_min_size(pos2(node.left() - w, tip.y - h / 2.0), size), pos2(node.left() - w, tip.y)),
        1 => (Rect::from_min_size(pos2(node.right(), tip.y - h / 2.0), size), pos2(node.right() + w, tip.y)),
        2 => (Rect::from_min_size(pos2(tip.x - w / 2.0, node.top() - h), size), pos2(tip.x, node.top() - h)),
        _ => (Rect::from_min_size(pos2(tip.x - w / 2.0, node.bottom()), size), pos2(tip.x, node.bottom() + h)),
    }
}

/// How far a template's signature box reaches past its card's right side and above its top, in
/// diagram units, as the design draws it (page 08, C1).
const SIGNATURE_OUT: Vec2 = vec2(28.0, 24.0);

/// Where a template signature box of `size` sits on `card`: over its top-right corner, reaching
/// [`SIGNATURE_OUT`] past it, scaled by `z`.
pub fn signature_place(card: Rect, size: Vec2, z: f32) -> Rect {
    Rect::from_min_size(pos2(card.right() + SIGNATURE_OUT.x * z - size.x, card.top() - SIGNATURE_OUT.y * z), size)
}

/// The radius of a selected line's end handle, and the room between it and the end's mark.
pub const END_HANDLE: f32 = 5.0;
const END_HANDLE_GAP: f32 = 3.0;

/// A selected line's end handle's hit box across, which two handles must not share.
pub(crate) const END_HANDLE_HIT: f32 = 16.0;
/// The step a handle's place is looked for in along the line, and the most steps taken.
const END_HANDLE_STEP: f32 = 0.5;
const END_HANDLE_STEPS: usize = 4096;

/// Where a selected line's end handle sits when its end's mark (an arrowhead, a triangle, a
/// diamond) would hide under it, `points` running from that end along the line: the first place
/// along the line a few points clear of the mark as drawn ([`mark_outline`]), measured from the
/// mark itself, so a bend close to the end never brings the handle onto it. It goes no further
/// than half a hit box short of the line's middle, so the two ends' handles stay apart and both
/// take a press on a short line. None, the handle on the end itself, for an end without such a
/// mark or a line of no length.
pub(crate) fn end_handle_at(points: &[Pos2], mark: EndMark, z: f32) -> Option<Pos2> {
    let (&tip, &next) = (points.first()?, points.get(1)?);
    let outline = mark_outline(mark, tip, next, z);
    let total: f32 = points.windows(2).map(|w| (w[1] - w[0]).length()).sum();
    if outline.is_empty() || total <= 0.0 || !total.is_finite() {
        return None;
    }
    let limit = (total / 2.0 - END_HANDLE_HIT / 2.0).max(0.0);
    let clear = END_HANDLE + END_HANDLE_GAP;
    // The steps short of `limit`, at most END_HANDLE_STEPS of them, then `limit` itself once, so
    // the farthest place is looked at too and none is looked at twice.
    let steps = (limit / END_HANDLE_STEP).ceil().clamp(0.0, END_HANDLE_STEPS as f32) as usize;
    let at = (0..steps)
        .map(|i| i as f32 * END_HANDLE_STEP)
        .filter(|d| *d < limit)
        .chain(std::iter::once(limit))
        .map(|d| at_length(points, d))
        .find(|at| outside_by(&outline, *at) >= clear);
    Some(at.unwrap_or_else(|| at_length(points, limit)))
}

/// The most places along each half of a line the two end handles are tried at together.
const END_HANDLE_TRIES: usize = 64;

/// Where a selected line's two end handles sit, `points` running from its first end to its
/// second, `marks` its ends' marks: each as [`end_handle_at`] places it (None, on its end), unless
/// the two hit boxes would meet on screen, as on a short line bent back on itself. Then the pair
/// is chosen together, each on its own half of the line, so the boxes stay apart: one that clears
/// both marks if there is one, else one that keeps both pressable, falling short of the marks'
/// clearance by as little as it can; the nearest to where they were placed alone. Where no two
/// places are apart, the pair furthest apart is taken.
pub fn end_handles_at(points: &[Pos2], marks: [EndMark; 2], z: f32) -> [Option<Pos2>; 2] {
    let backwards: Vec<Pos2> = points.iter().rev().copied().collect();
    let alone = [end_handle_at(points, marks[0], z), end_handle_at(&backwards, marks[1], z)];
    let (Some(&start), Some(&end)) = (points.first(), points.last()) else { return alone };
    let placed = [alone[0].unwrap_or(start), alone[1].unwrap_or(end)];
    let total: f32 = points.windows(2).map(|w| (w[1] - w[0]).length()).sum();
    if handles_apart(placed[0], placed[1]) || points.len() < 2 || total <= 0.0 || !total.is_finite() {
        return alone;
    }
    // Each end's places on its own half, with how far each falls short of clearing its mark.
    let half = total / 2.0;
    let step = half / END_HANDLE_TRIES as f32;
    let clear = END_HANDLE + END_HANDLE_GAP;
    let places = |route: &[Pos2], mark: EndMark| -> Vec<(f32, Pos2, f32)> {
        let outline = route.get(1).map(|next| mark_outline(mark, route[0], *next, z)).unwrap_or_default();
        (0..=END_HANDLE_TRIES)
            .map(|i| {
                let d = i as f32 * step;
                let at = at_length(route, d);
                let short = if outline.is_empty() { 0.0 } else { (clear - outside_by(&outline, at)).max(0.0) };
                (d, at, short)
            })
            .collect()
    };
    let (first, second) = (places(points, marks[0]), places(&backwards, marks[1]));
    let mut best: Option<(f32, [(f32, Pos2); 2])> = None;
    let mut widest: Option<(f32, [(f32, Pos2); 2])> = None;
    for &(da, a, sa) in &first {
        for &(db, b, sb) in &second {
            let gap = (a.x - b.x).abs().max((a.y - b.y).abs());
            if widest.is_none_or(|(w, _)| gap > w) {
                widest = Some((gap, [(da, a), (db, b)]));
            }
            if handles_apart(a, b) {
                // Falling short of a mark's clearance weighs far more than moving a handle.
                let cost = (sa + sb) * 1000.0 + (a - placed[0]).length() + (b - placed[1]).length();
                if best.is_none_or(|(c, _)| cost < c) {
                    best = Some((cost, [(da, a), (db, b)]));
                }
            }
        }
    }
    match best.or(widest) {
        Some((_, pair)) => pair.map(|(d, at)| (d > 0.0).then_some(at)),
        None => alone,
    }
}

/// Whether two end handles at `a` and `b` keep their hit boxes apart on screen, with a point
/// between them.
fn handles_apart(a: Pos2, b: Pos2) -> bool {
    (a.x - b.x).abs().max((a.y - b.y).abs()) >= END_HANDLE_HIT + 1.0
}

/// The point `d` along the polyline `points` from its start; its end when it is shorter.
fn at_length(points: &[Pos2], d: f32) -> Pos2 {
    let mut left = d.max(0.0);
    for w in points.windows(2) {
        let len = (w[1] - w[0]).length();
        if left <= len {
            return w[0] + (w[1] - w[0]) * if len > 0.0 { left / len } else { 0.0 };
        }
        left -= len;
    }
    points.last().copied().unwrap_or(Pos2::ZERO)
}

/// How far `at` is outside the convex `outline`: its distance to the nearest edge, none inside.
fn outside_by(outline: &[Pos2], at: Pos2) -> f32 {
    let n = outline.len();
    let edges = || (0..n).filter_map(|i| Some((*outline.get(i)?, *outline.get((i + 1) % n)?)));
    let cross = |(a, b): (Pos2, Pos2)| (b - a).x * (at - a).y - (b - a).y * (at - a).x;
    let inside = n >= 3 && (edges().all(|e| cross(e) >= 0.0) || edges().all(|e| cross(e) <= 0.0));
    if inside {
        return 0.0;
    }
    edges()
        .map(|(a, b)| {
            let ab = b - a;
            let t = if ab.length_sq() > 0.0 { ((at - a).dot(ab) / ab.length_sq()).clamp(0.0, 1.0) } else { 0.0 };
            (at - (a + ab * t)).length()
        })
        .fold(f32::INFINITY, f32::min)
}

/// How far a line's end mark reaches back along the line from its tip, and how far it spreads to
/// either side, at zoom `z`: a triangle, a diamond or an open arrow; nothing for any other mark.
fn mark_extent(mark: EndMark, z: f32) -> (f32, f32) {
    let s = z.max(0.6);
    match mark {
        EndMark::Triangle | EndMark::FilledTriangle => (12.0 * s, 7.5 * s),
        EndMark::HollowDiamond | EndMark::FilledDiamond => (20.0 * s, 6.0 * s),
        EndMark::OpenArrow => (12.0 * s, 6.0 * s),
        _ => (0.0, 0.0),
    }
}

/// Where a cardinality text sits by default: a little along the line from its end, and further
/// for a text longer than that reach, beside the line rather than on it: under a level line (a
/// line's name sits above it), right of an upright one, off a slanted one along its true normal;
/// clear of the end's `mark` (an arrowhead, a diamond or a triangle), along the line and to its
/// side.
pub fn beside(end: Pos2, towards: Pos2, size: Vec2, z: f32, mark: EndMark) -> Pos2 {
    match beside_parts(end, towards, size, z, mark) {
        Some((d, n, along, reach)) => end + d * along + n * reach,
        None => end,
    }
}

/// [`beside`]'s parts: the line's direction from the end, the unit normal the text sits on, how
/// far along and how far off the line its centre goes. None for an end with no direction.
fn beside_parts(end: Pos2, towards: Pos2, size: Vec2, z: f32, mark: EndMark) -> Option<(Vec2, Vec2, f32, f32)> {
    let d = towards - end;
    if d.length() <= f32::EPSILON || !d.is_finite() {
        return None;
    }
    let d = d.normalized();
    // The true normal, on the side a level line's text has (below) or an upright one's (right).
    let normal = vec2(-d.y, d.x);
    let n = if d.x.abs() >= d.y.abs() {
        if normal.y >= 0.0 { normal } else { -normal }
    } else if normal.x >= 0.0 {
        normal
    } else {
        -normal
    };
    let (mark_len, mark_half) = mark_extent(mark, z);
    // Far enough off the line that the text's box, projected on the normal, clears it, and the
    // mark's wings, whichever way the line runs.
    let reach = n.x.abs() * size.x / 2.0 + n.y.abs() * size.y / 2.0 + (6.0 * z).max(mark_half + 4.0 * z);
    // And far enough along it that a long text, such as an end's modifiers (AR-126), clears the
    // card the line leaves and the mark at its end; a short one keeps its usual place.
    let along = (d.x.abs() * size.x / 2.0 + d.y.abs() * size.y / 2.0 + mark_len + 8.0 * z).max(30.0 * z);
    Some((d, n, along, reach))
}

/// The default centres of a line's two end texts, `ends` each its end, the point the line leaves
/// it towards, the text's size and the end's mark, as [`beside`] places them; when the two would
/// overlap (a short line, a long text, a diamond's reach), each keeps to its own half of the span
/// between the ends, and if they still meet the second goes to the other side of the line.
pub fn beside_pair(ends: [(Pos2, Pos2, Vec2, EndMark); 2], z: f32) -> [Pos2; 2] {
    let centres = ends.map(|(end, towards, size, mark)| beside(end, towards, size, z, mark));
    let rect = |c: Pos2, size: Vec2| Rect::from_center_size(c, size).expand(2.0 * z);
    let meet = |c: [Pos2; 2]| rect(c[0], ends[0].2).intersects(rect(c[1], ends[1].2));
    if !meet(centres) {
        return centres;
    }
    let span = (ends[1].0 - ends[0].0).length();
    let capped = ends.map(|(end, towards, size, mark)| match beside_parts(end, towards, size, z, mark) {
        Some((d, n, along, reach)) => {
            let half = d.x.abs() * size.x / 2.0 + d.y.abs() * size.y / 2.0;
            let room = (span / 2.0 - half - 4.0 * z).max(half);
            (end + d * along.min(room) + n * reach, end + d * along.min(room) - n * reach)
        }
        None => (end, end),
    });
    let first = [capped[0].0, capped[1].0];
    if !meet(first) {
        return first;
    }
    [capped[0].0, capped[1].1]
}

/// The canvas tools.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tool {
    #[default]
    Select,
    Pan,
    /// A diagram kind's tool that places an element where the canvas is clicked (ER's Entity,
    /// slice E1), by its key.
    Place(&'static str),
    /// A diagram kind's tool that drags a line from one element to another (ER's Relationship,
    /// slice E2), by its key.
    Connect(&'static str),
    /// Places a note where the canvas is clicked (slice E4).
    Note,
    /// Drags a link from a note to a table or a relationship's line.
    Link,
}

/// A drag of the minimap's viewport: the minimap's mapping frozen as the drag began (the part of
/// the diagram it shows, its scale and where that sits on screen), so the box does not rescale under
/// the pointer, and where on the viewport the pointer took hold of it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MinimapDrag {
    pub shown: Rect,
    pub scale: f32,
    pub origin: Pos2,
    pub grab: Vec2,
}

/// A line of text at the canvas's foot for a while, with Undo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Toast {
    pub text: String,
    pub until: u64,
    /// The undo step the toast speaks of: once another step tops the history, the toast goes.
    pub undo_label: Option<String>,
}

/// What the centre keeps between frames: the open tabs, the view and the toggles.
#[derive(Debug, Clone, PartialEq)]
pub struct State {
    /// The diagrams with a tab, in tab order.
    pub tabs: Vec<Uuid>,
    /// The tab whose diagram the canvas draws.
    pub active: Option<Uuid>,
    pub tool: Tool,
    /// Where diagram unit (0, 0) is, in logical pixels from the canvas's top left.
    pub pan: Vec2,
    pub zoom: f32,
    /// The canvas's size as last drawn, which the View menu's zooms keep their centre in.
    pub size: Vec2,
    /// True until the diagram just opened has been fitted into the canvas once.
    pub fit_pending: bool,
    /// A node the next frame brings to the canvas's centre (what the search found).
    pub centre_on: Option<Uuid>,
    /// True when the active tab changed without the view: the next frame opens it through the
    /// engine, so the view's open diagram and the canvas agree.
    pub reopen_pending: bool,
    pub minimap_open: bool,
    /// The minimap's viewport being dragged: its mapping as it was when the drag began.
    pub minimap_drag: Option<MinimapDrag>,
    pub grid: bool,
    pub key_status: bool,
    /// The last refusal, shown at the top left of the centre, whatever it holds, until the next
    /// action; a refused step of versions shows as the toast instead (`refused_step`).
    pub notice: Option<String>,
    /// The words of `notice` when they say why a step between versions or a way out of current
    /// work did not go, or what a step passed. Only ever read against `notice`, so whatever
    /// clears or replaces that ends the toast as well.
    pub refused_step: Option<String>,
    /// Editing (slice E1): a name typed in a node, the pointer gesture under way, the entity menu,
    /// and whether the canvas has the keyboard. What a notation types, draws and opens on the
    /// canvas is its own (the shell's `notation_ui::NotationState`).
    pub rename: Option<crate::editing::NodeRename>,
    pub gesture: Option<crate::editing::Gesture>,
    pub menu: Option<crate::editing::EntityMenu>,
    pub focused: bool,
    /// Relationships (slice E2): the chevron whose popover is open (the cardinality popover, a menu
    /// of kinds) and the toast with Undo; the ends a new line gets and the mapping table offer are
    /// ER's own (the shell's `er_state::ErState`).
    pub popover: Option<&'static str>,
    /// The row of the open popover's menu the arrow keys are on, when they have moved.
    pub menu_focus: Option<usize>,
    /// A press closed the classifier's menu of kinds: the canvas ignores that press until the
    /// button lets go, so it places, selects and drags nothing.
    pub dismissing: bool,
    pub toast: Option<Toast>,
    /// A note's text being typed in place (slice E4).
    pub note_edit: Option<crate::editing::NoteEdit>,
    /// The texts of the skills with a tab, as typed.
    pub skills: Vec<crate::skill_tab::Doc>,
    /// The cardinality texts the last frame drew, where a press grabs one.
    pub ends: Vec<EndText>,
    /// The packages' tabs the last frame drew, which take a press as their cards do (AR-113).
    pub package_tabs: Vec<PackageTab>,
    /// The owning paths the last frame drew under the packages' cards (AR-565).
    pub path_badges: Vec<PathBadge>,
    /// The labels of the figures of no one node or line the last frame drew, which take a press
    /// (a UML generalization set's, AR-129).
    pub extra_labels: Vec<ExtraLabel>,
    /// The line ends the last frame moved out to a qualifier box's outer side (AR-127): where the
    /// route ended on its node, and where the line now starts, on screen; a selected line's end
    /// handle follows.
    pub qualified_ends: Vec<(Pos2, Pos2)>,
    /// The view each diagram was last left with, by diagram, kept for some ways back to it, not
    /// for every one: a diagram's kind may restore it as it moves between diagrams (UML's package
    /// navigation), and a tab taking over before the view of the tab left was drawn shows it
    /// (the shell's `canvas::settle_view`).
    pub views: HashMap<Uuid, (Vec2, f32)>,
    /// The tab whose view the canvas has drawn since it was opened: none while the view is still
    /// the one of the tab before, to be fitted or centred on the next frame.
    pub shown: Option<Uuid>,
    /// How many times a tab has taken the active one's place (the shell's `canvas::settle_view`):
    /// what a diagram's kind sets up for the tab it opened holds only until the next (UML's landing
    /// after a switch).
    pub settled: u64,
    /// The zoom the user made since the diagram's kind last ran before the canvas drew, as a
    /// factor; zero for none. The kind follows it there (the shell's
    /// `kinds::DiagramKind::before_draw`: UML's package navigation across a package's threshold),
    /// and the canvas clears it right after, so a zoom made later in a frame (the minimap's
    /// buttons) reaches the next one.
    pub zoom_intent: f32,
    /// When a diagram's kind last marked a switch: the canvas fades in from then over the shell's
    /// `canvas::SWITCH_FADE`. Not proof that the diagram shown changed: UML marks one too when a
    /// zoom out at the top package goes no further, so the gesture rests and the canvas fades on
    /// the same diagram.
    pub switched: Option<f64>,
}

/// A cardinality text as drawn: its line, which end (0 the line's start, 1 its end), where it
/// was and the end of the line it belongs to, on screen.
#[derive(Debug, Clone, PartialEq)]
pub struct EndText {
    pub edge: Uuid,
    pub end: u8,
    pub rect: Rect,
    pub anchor: Pos2,
}

/// A package's tab as drawn (AR-113): where it was on screen, its node, and the whole name when
/// the tab could show it only cut.
#[derive(Debug, Clone, PartialEq)]
pub struct PackageTab {
    pub rect: Rect,
    pub node: Uuid,
    pub cut: Option<String>,
}

/// A package's owning path as drawn under its card (AR-565): where it was on screen, its node, the
/// text shown, its first owners folded into `…` when the card is too narrow, and the whole
/// sentence, which its tooltip and a screen reader say.
#[derive(Debug, Clone, PartialEq)]
pub struct PathBadge {
    pub rect: Rect,
    pub node: Uuid,
    pub shown: String,
    pub whole: String,
}

impl Default for State {
    /// A canvas with no diagram open: Select, the grid and minimap on, no menu open, Class as the classifier kind.
    fn default() -> Self {
        Self {
            tabs: Vec::new(),
            active: None,
            tool: Tool::Select,
            pan: Vec2::ZERO,
            zoom: 1.0,
            size: Vec2::ZERO,
            fit_pending: false,
            centre_on: None,
            reopen_pending: false,
            minimap_open: true,
            minimap_drag: None,
            grid: true,
            key_status: true,
            notice: None,
            refused_step: None,
            rename: None,
            gesture: None,
            menu: None,
            focused: false,
            popover: None,
            menu_focus: None,
            dismissing: false,
            toast: None,
            note_edit: None,
            skills: Vec::new(),
            ends: Vec::new(),
            package_tabs: Vec::new(),
            path_badges: Vec::new(),
            extra_labels: Vec::new(),
            qualified_ends: Vec::new(),
            views: HashMap::new(),
            shown: None,
            settled: 0,
            zoom_intent: 0.0,
            switched: None,
        }
    }
}

impl State {
    /// `notice` becomes why a step between versions or a way out of current work did not go, or
    /// what a step passed, which the centre says as the toast, or none, which ends what was said
    /// before.
    pub fn say_refused_step(&mut self, words: Option<String>) {
        self.notice.clone_from(&words);
        self.refused_step = words;
    }

    /// Whether `notice` is still the words of a refused step, rather than something said since.
    pub fn notice_is_refused_step(&self) -> bool {
        self.notice.is_some() && self.notice == self.refused_step
    }
}

/// A colour as `#rrggbb`.
pub fn hex(c: ux::Colour) -> String {
    format!("#{:02X}{:02X}{:02X}", c.r(), c.g(), c.b())
}

/// A short end text, four characters at zoom 1, sits 30 points along a level or an upright line
/// from its end, as it always has, where the end has no mark; a long one goes further, clear of the
/// card and of an arrowhead, beside the line clear of the arrowhead's wings (AR-126).
#[cfg(test)]
#[test]
fn an_end_text_sits_thirty_along_its_line_and_a_long_one_clears_the_mark() {
    let four = vec2(26.0, 14.0);
    for (towards, d, n) in [(pos2(200.0, 0.0), vec2(1.0, 0.0), vec2(0.0, 1.0)), (pos2(0.0, 200.0), vec2(0.0, 1.0), vec2(1.0, 0.0))] {
        let at = beside(Pos2::ZERO, towards, four, 1.0, EndMark::None);
        assert_eq!((at.to_vec2() - n * (at.to_vec2().dot(n))).length(), 30.0, "{towards:?}: {at:?}");
        assert!((at.to_vec2().dot(d) - 30.0).abs() < f32::EPSILON);
    }
    let long = vec2(120.0, 14.0);
    let at = beside(Pos2::ZERO, pos2(400.0, 0.0), long, 1.0, EndMark::OpenArrow);
    let (len, half) = mark_extent(EndMark::OpenArrow, 1.0);
    assert!(at.x - long.x / 2.0 >= len + 8.0, "its near edge clears the arrowhead: {at:?}");
    assert!(at.y - long.y / 2.0 >= half + 4.0, "and its wings: {at:?}");
}

/// On a straight slanted route, direction (0.8, 0.6), an end text sits off the line along its true
/// normal: no corner of its box crosses the route, short or long, with or without a mark.
#[cfg(test)]
#[test]
fn an_end_text_on_a_slanted_line_keeps_its_box_off_the_route() {
    let towards = pos2(400.0, 300.0);
    let normal = vec2(-0.6, 0.8);
    for (size, mark) in [(vec2(26.0, 14.0), EndMark::None), (vec2(140.0, 14.0), EndMark::OpenArrow), (vec2(80.0, 14.0), EndMark::FilledDiamond)] {
        for z in [0.5, 1.0, 2.0] {
            let c = beside(Pos2::ZERO, towards, size * z, z, mark);
            let r = Rect::from_center_size(c, size * z);
            let sides: Vec<f32> = [r.left_top(), r.right_top(), r.left_bottom(), r.right_bottom()].iter().map(|p| p.to_vec2().dot(normal)).collect();
            assert!(sides.iter().all(|s| *s > 0.0) || sides.iter().all(|s| *s < 0.0), "{size:?} {mark:?} at {z}: {r:?} crosses the route");
        }
    }
}

/// A short line's two end texts never overlap, a diamond at one end included: each keeps to its
/// half of the line, or the second goes to the other side; texts that clear each other keep their
/// usual places.
#[cfg(test)]
#[test]
fn a_short_line_s_two_end_texts_keep_clear_of_each_other() {
    let gap = |c: [Pos2; 2], a: Vec2, b: Vec2| !Rect::from_center_size(c[0], a).intersects(Rect::from_center_size(c[1], b));
    for (len, a, b, marks) in [
        (110.0, vec2(70.0, 14.0), vec2(70.0, 14.0), [EndMark::FilledDiamond, EndMark::None]),
        (110.0, vec2(70.0, 14.0), vec2(70.0, 14.0), [EndMark::None, EndMark::HollowDiamond]),
        (60.0, vec2(90.0, 14.0), vec2(90.0, 14.0), [EndMark::FilledDiamond, EndMark::OpenArrow]),
    ] {
        let c = beside_pair([(Pos2::ZERO, pos2(len, 0.0), a, marks[0]), (pos2(len, 0.0), Pos2::ZERO, b, marks[1])], 1.0);
        assert!(gap(c, a, b), "{len} {marks:?}: {c:?}");
    }
    // Far apart, each stays where it alone would be.
    let far =
        [(Pos2::ZERO, pos2(600.0, 0.0), vec2(40.0, 14.0), EndMark::FilledDiamond), (pos2(600.0, 0.0), Pos2::ZERO, vec2(40.0, 14.0), EndMark::None)];
    let c = beside_pair(far, 1.0);
    assert_eq!(c, far.map(|(e, t, s, m)| beside(e, t, s, 1.0, m)));
}

/// A self relationship's loop leaves its entity's right side and comes back to it, around the
/// entity's centre.
#[cfg(test)]
#[test]
fn a_self_loop_leaves_and_returns_on_the_right_side_around_the_centre() {
    let rect = Rect::from_min_size(pos2(100.0, 100.0), vec2(240.0, 166.0));
    let points = self_loop(rect, 1.0);
    assert_eq!(points.len(), 4);
    assert!(points.iter().all(|p| p.is_finite()));
    assert_eq!(points[0].x, rect.right());
    assert_eq!(points[3].x, rect.right());
    assert_eq!(points[1].x, rect.right() + LOOP_REACH);
    assert!((points[0].y + points[3].y) / 2.0 - rect.center().y < 0.001);
}

/// An end handle is placed by its mark as drawn (AR-191): on a long straight line a few points past
/// the triangle's base, the arrow's strokes and the diamond's far corner, as before; where the line
/// bends close to the end it stays as clear of the mark; on a short line the two ends' handles stay
/// a hit box apart, never meeting in the middle; an end without such a mark keeps its handle.
#[cfg(test)]
#[test]
fn an_end_handle_is_placed_by_its_marks_drawn_outline_and_two_never_meet() {
    use crate::canvas::{END_HANDLE, END_HANDLE_HIT, end_handle_at, mark_outline};
    use archforge_diagram::figure::EndMark;
    // How far `at` is from the mark drawn at the route's start: from its nearest corner or edge.
    let clear_of = |route: &[egui::Pos2], mark: EndMark, at: egui::Pos2| {
        let outline = mark_outline(mark, route[0], route[1], 1.0);
        (0..outline.len())
            .map(|i| {
                let (a, b) = (outline[i], outline[(i + 1) % outline.len()]);
                let t = ((at - a).dot(b - a) / (b - a).length_sq()).clamp(0.0, 1.0);
                (at - (a + (b - a) * t)).length()
            })
            .fold(f32::INFINITY, f32::min)
    };
    let straight = [egui::pos2(0.0, 0.0), egui::pos2(400.0, 0.0)];
    for (mark, reach) in [(EndMark::Triangle, 12.0), (EndMark::OpenArrow, 12.0), (EndMark::FilledDiamond, 20.0), (EndMark::HollowDiamond, 20.0)] {
        let at = end_handle_at(&straight, mark, 1.0).expect("moved past its mark");
        assert!((at.x - (reach + 8.0)).abs() <= 0.5 && at.y == 0.0, "{mark:?}: {at:?}");
        assert!(clear_of(&straight, mark, at) >= END_HANDLE + 2.5, "{mark:?}");
    }
    for mark in [EndMark::None, EndMark::ExactlyOne, EndMark::ZeroOrMore] {
        assert_eq!(end_handle_at(&straight, mark, 1.0), None, "{mark:?} keeps its handle at the end");
    }
    // A bend 13 points from the end, inside the triangle's reach: the handle turns the corner and
    // goes on until it is as clear of the triangle's base corner as on a straight line.
    let bent = [egui::pos2(0.0, 0.0), egui::pos2(13.0, 0.0), egui::pos2(13.0, 300.0)];
    for mark in [EndMark::Triangle, EndMark::FilledDiamond, EndMark::OpenArrow] {
        let at = end_handle_at(&bent, mark, 1.0).unwrap();
        assert!(clear_of(&bent, mark, at) >= END_HANDLE + 2.5, "{mark:?} at {at:?} is clear of its mark");
        assert!(at.x == 13.0 && at.y > 0.0, "{mark:?}: on the line past the bend, {at:?}");
    }
    // A short line: each handle stops half a hit box short of the middle, so they never meet and
    // both take a press; a long one is placed as before.
    for length in [10.0, 30.0, 40.0, 60.0] {
        let line = [egui::pos2(0.0, 0.0), egui::pos2(length, 0.0)];
        let back = [line[1], line[0]];
        let (a, b) = (end_handle_at(&line, EndMark::Triangle, 1.0).unwrap(), end_handle_at(&back, EndMark::FilledDiamond, 1.0).unwrap());
        let apart = (b - a).length();
        assert!(apart >= END_HANDLE_HIT.min(length) - 0.01, "{length}: the handles {apart} apart");
        assert!(a.x <= b.x, "{length}: each on its own half");
    }
}

/// The two end handles of a line bent back on itself are placed together so their hit boxes never
/// meet on screen: a short U whose ends come back close, a diamond at one and an arrow at the
/// other, gets two handles apart, falling short of the marks' clearance only as far as it must;
/// a line whose handles are apart already keeps the places each takes alone (AR-191).
#[cfg(test)]
#[test]
fn a_bent_lines_two_end_handles_are_placed_together_so_their_hit_boxes_never_meet() {
    use crate::canvas::{END_HANDLE_HIT, end_handle_at, end_handles_at};
    use archforge_diagram::figure::EndMark;
    let hit = |at: egui::Pos2| egui::Rect::from_center_size(at, egui::Vec2::splat(END_HANDLE_HIT));
    let u = [egui::pos2(0.0, 0.0), egui::pos2(30.0, 0.0), egui::pos2(30.0, 12.0), egui::pos2(0.0, 12.0)];
    let back: Vec<egui::Pos2> = u.iter().rev().copied().collect();
    let marks = [EndMark::FilledDiamond, EndMark::OpenArrow];
    let alone = [end_handle_at(&u, marks[0], 0.5).unwrap(), end_handle_at(&back, marks[1], 0.5).unwrap()];
    assert!(hit(alone[0]).intersects(hit(alone[1])), "placed alone, their boxes meet: {alone:?}");
    let [a, b] = end_handles_at(&u, marks, 0.5);
    let (a, b) = (a.unwrap_or(u[0]), b.unwrap_or(u[3]));
    assert!(!hit(a).intersects(hit(b)), "placed together, apart: {a:?} {b:?}");
    // A long straight line keeps the places each end takes alone.
    let straight = [egui::pos2(0.0, 0.0), egui::pos2(400.0, 0.0)];
    let backwards = [straight[1], straight[0]];
    assert_eq!(
        end_handles_at(&straight, [EndMark::Triangle, EndMark::FilledDiamond], 1.0),
        [end_handle_at(&straight, EndMark::Triangle, 1.0), end_handle_at(&backwards, EndMark::FilledDiamond, 1.0)]
    );
}

/// The line that ties a nested card to its owner's (AR-132): straight where the cards face each
/// other, its ⊕ just outside the owner's side; an elbow where they are apart both ways, the one
/// crossing fewer other cards; none where the cards overlap, which is no ownership (D4).
#[cfg(test)]
#[test]
fn the_nesting_line_runs_from_the_owners_side_and_none_joins_overlapping_cards() {
    use crate::canvas::nesting_route;
    use egui::{Rect, pos2};
    let owner = Rect::from_min_size(pos2(0.0, 0.0), egui::vec2(200.0, 100.0));
    let below = Rect::from_min_size(pos2(50.0, 200.0), egui::vec2(200.0, 80.0));
    let (points, centre) = nesting_route(owner, below, 7.0, &[]).unwrap();
    assert_eq!(centre, pos2(125.0, 107.0), "the ⊕ under the owner's foot, in the overlap's middle");
    assert_eq!(points, [pos2(125.0, 114.0), pos2(125.0, 200.0)]);
    let right = Rect::from_min_size(pos2(400.0, 20.0), egui::vec2(100.0, 60.0));
    let (points, centre) = nesting_route(owner, right, 7.0, &[]).unwrap();
    assert_eq!((centre, points.len()), (pos2(207.0, 50.0), 2), "straight across from the owner's right side");
    let apart = Rect::from_min_size(pos2(400.0, 300.0), egui::vec2(100.0, 60.0));
    let (points, _) = nesting_route(owner, apart, 7.0, &[]).unwrap();
    assert_eq!(points.len(), 3, "one bend");
    // A card in the way of the first elbow sends the line the other way.
    let blocker = Rect::from_min_size(pos2(300.0, 20.0), egui::vec2(60.0, 60.0));
    let (points, centre) = nesting_route(owner, apart, 7.0, &[blocker]).unwrap();
    assert_eq!(centre, pos2(100.0, 107.0), "out of the owner's foot instead");
    assert_eq!(points.last().copied(), Some(pos2(400.0, 330.0)));
    let over = Rect::from_min_size(pos2(150.0, 60.0), egui::vec2(100.0, 80.0));
    assert!(nesting_route(owner, over, 7.0, &[]).is_none(), "overlapping cards draw no line");
}
