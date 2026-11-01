//! Arrange a diagram (owner request of 2026-11-01): boxes in layers along their lines, the box a
//! line starts at above the box it ends at, each layer ordered so lines cross little; groups of
//! boxes with no line between them side by side, and boxes with no line at all in rows below.
//! Pure geometry over sizes and lines; the command moves the nodes.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// The space between boxes in a layer, and between layers, where the lines run.
const GAP_X: f64 = 80.0;
const GAP_Y: f64 = 120.0;
/// The space between groups, and how wide a row of groups or lone boxes grows before it wraps.
const GAP_GROUP: f64 = 160.0;
const ROW: f64 = 2400.0;
/// Every place lands on this grid.
const SNAP: f64 = 20.0;
/// The ordering sweeps: enough to settle a diagram's layers, few enough to stay quick.
const SWEEPS: usize = 8;

/// The top-left place of each box, by its index in `sizes`, for boxes of `sizes` (width, height)
/// and lines `lines` between them (from, to); a line naming a box not there is left out. The
/// arrangement starts at (0, 0).
pub fn arrange(sizes: &[(f64, f64)], lines: &[(usize, usize)]) -> Vec<(f64, f64)> {
    let n = sizes.len();
    let size = |i: usize| {
        let (w, h) = sizes[i];
        (if w.is_finite() { w.max(0.0) } else { 0.0 }, if h.is_finite() { h.max(0.0) } else { 0.0 })
    };
    let lines: BTreeSet<(usize, usize)> = lines.iter().copied().filter(|(a, b)| a != b && *a < n && *b < n).collect();
    let mut near: Vec<BTreeSet<usize>> = vec![BTreeSet::new(); n];
    for (a, b) in &lines {
        near[*a].insert(*b);
        near[*b].insert(*a);
    }
    // The groups: boxes joined by lines, largest first; lone boxes apart.
    let mut seen = vec![false; n];
    let mut groups: Vec<Vec<usize>> = Vec::new();
    let mut lone: Vec<usize> = Vec::new();
    for start in 0..n {
        if seen[start] {
            continue;
        }
        seen[start] = true;
        let mut group = vec![start];
        let mut queue = VecDeque::from([start]);
        while let Some(i) = queue.pop_front() {
            for &j in &near[i] {
                if !seen[j] {
                    seen[j] = true;
                    group.push(j);
                    queue.push_back(j);
                }
            }
        }
        if group.len() == 1 {
            lone.push(start);
        } else {
            groups.push(group);
        }
    }
    groups.sort_by_key(|g| std::cmp::Reverse(g.len()));
    // Each group's lines, sorted out once: a line joins two boxes of one group.
    let mut group_of = vec![usize::MAX; n];
    for (g, group) in groups.iter().enumerate() {
        for i in group {
            group_of[*i] = g;
        }
    }
    let mut inner: Vec<Vec<(usize, usize)>> = vec![Vec::new(); groups.len()];
    for (a, b) in &lines {
        if let Some(bucket) = inner.get_mut(group_of[*a]) {
            bucket.push((*a, *b));
        }
    }
    let mut out = vec![(0.0, 0.0); n];
    // Groups in rows, left to right; each laid out in layers.
    let (mut x, mut y, mut row_height) = (0.0_f64, 0.0_f64, 0.0_f64);
    for (group, inner) in groups.iter().zip(&inner) {
        let (places, width, height) = layered(group, inner, &size);
        if x > 0.0 && x + width > ROW {
            (x, y, row_height) = (0.0, y + row_height + GAP_GROUP, 0.0);
        }
        for (i, (px, py)) in places {
            out[i] = (x + px, y + py);
        }
        x += width + GAP_GROUP;
        row_height = row_height.max(height);
    }
    // Lone boxes in rows under the groups.
    if !lone.is_empty() {
        let mut lx = 0.0_f64;
        let mut ly = if groups.is_empty() { 0.0 } else { y + row_height + GAP_GROUP };
        let mut lh = 0.0_f64;
        for i in lone {
            let (w, h) = size(i);
            if lx > 0.0 && lx + w > ROW {
                (lx, ly, lh) = (0.0, ly + lh + GAP_Y, 0.0);
            }
            out[i] = (lx, ly);
            lx += w + GAP_X;
            lh = lh.max(h);
        }
    }
    out.into_iter().map(|(x, y)| ((x / SNAP).round() * SNAP, (y / SNAP).round() * SNAP)).collect()
}

/// One group in layers: places relative to its top-left, and its width and height.
fn layered(group: &[usize], inner: &[(usize, usize)], size: &impl Fn(usize) -> (f64, f64)) -> (Vec<(usize, (f64, f64))>, f64, f64) {
    // A line that closes a circle is read backwards, so the lines run one way.
    let acyclic = break_cycles(group, inner);
    let mut into: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    let mut out_of: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for (a, b) in &acyclic {
        out_of.entry(*a).or_default().push(*b);
        into.entry(*b).or_default().push(*a);
    }
    // Layers: the longest way down from a box nothing points at.
    let mut layer: BTreeMap<usize, usize> = BTreeMap::new();
    let mut waiting: BTreeMap<usize, usize> = group.iter().map(|i| (*i, into.get(i).map_or(0, Vec::len))).collect();
    let mut ready: VecDeque<usize> = group.iter().copied().filter(|i| waiting[i] == 0).collect();
    while let Some(i) = ready.pop_front() {
        let here = into.get(&i).map_or(0, |from| from.iter().filter_map(|f| layer.get(f)).map(|l| l + 1).max().unwrap_or(0));
        layer.insert(i, here);
        for j in out_of.get(&i).into_iter().flatten() {
            if let Some(w) = waiting.get_mut(j) {
                *w = w.saturating_sub(1);
                if *w == 0 {
                    ready.push_back(*j);
                }
            }
        }
    }
    let depth = layer.values().max().map_or(0, |m| m + 1);
    let mut layers: Vec<Vec<usize>> = vec![Vec::new(); depth];
    for i in group {
        if let Some(row) = layers.get_mut(layer.get(i).copied().unwrap_or(0)) {
            row.push(*i);
        }
    }
    // Order each layer by where its neighbours stand in the layer above, then below, and again.
    let mut at: BTreeMap<usize, f64> = BTreeMap::new();
    let place = |layers: &Vec<Vec<usize>>, at: &mut BTreeMap<usize, f64>| {
        for row in layers {
            for (k, i) in row.iter().enumerate() {
                at.insert(*i, k as f64);
            }
        }
    };
    place(&layers, &mut at);
    for sweep in 0..SWEEPS {
        let down = sweep % 2 == 0;
        let order: Vec<usize> = if down { (1..depth).collect() } else { (0..depth.saturating_sub(1)).rev().collect() };
        for l in order {
            let mut keyed: Vec<(f64, usize)> = layers[l]
                .iter()
                .map(|i| {
                    let them = if down { into.get(i) } else { out_of.get(i) };
                    let marks: Vec<f64> = them.into_iter().flatten().filter_map(|j| at.get(j).copied()).collect();
                    let key = if marks.is_empty() { at[i] } else { marks.iter().sum::<f64>() / marks.len() as f64 };
                    (key, *i)
                })
                .collect();
            keyed.sort_by(|p, q| p.0.total_cmp(&q.0).then(at[&p.1].total_cmp(&at[&q.1])));
            layers[l] = keyed.into_iter().map(|(_, i)| i).collect();
            for (k, i) in layers[l].iter().enumerate() {
                at.insert(*i, k as f64);
            }
        }
    }
    // Places: each layer centred under the widest, layers one under the other.
    let widths: Vec<f64> =
        layers.iter().map(|row| row.iter().map(|i| size(*i).0).sum::<f64>() + GAP_X * row.len().saturating_sub(1) as f64).collect();
    let width = widths.iter().copied().fold(0.0, f64::max);
    let mut places = Vec::with_capacity(group.len());
    let mut y = 0.0;
    for (row, row_width) in layers.iter().zip(&widths) {
        let mut x = (width - row_width) / 2.0;
        let mut tallest = 0.0_f64;
        for i in row {
            let (w, h) = size(*i);
            places.push((*i, (x, y)));
            x += w + GAP_X;
            tallest = tallest.max(h);
        }
        y += tallest + GAP_Y;
    }
    (places, width, (y - GAP_Y).max(0.0))
}

/// The lines with those that close a circle turned round: a depth-first walk from each box in
/// turn, a line back to a box still being walked is the one that closes it.
fn break_cycles(group: &[usize], lines: &[(usize, usize)]) -> Vec<(usize, usize)> {
    let mut out_of: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for (a, b) in lines {
        out_of.entry(*a).or_default().push(*b);
    }
    // 0 not walked, 1 being walked, 2 done.
    let mut state: BTreeMap<usize, u8> = group.iter().map(|i| (*i, 0)).collect();
    let mut back: BTreeSet<(usize, usize)> = BTreeSet::new();
    for &start in group {
        if state[&start] != 0 {
            continue;
        }
        // An explicit stack, so a long chain never overflows the call stack.
        let mut stack: Vec<(usize, usize)> = vec![(start, 0)];
        state.insert(start, 1);
        while let Some((i, k)) = stack.pop() {
            let next = out_of.get(&i).and_then(|v| v.get(k)).copied();
            match next {
                Some(j) => {
                    stack.push((i, k + 1));
                    match state.get(&j).copied().unwrap_or(2) {
                        0 => {
                            state.insert(j, 1);
                            stack.push((j, 0));
                        }
                        1 => {
                            back.insert((i, j));
                        }
                        _ => {}
                    }
                }
                None => {
                    state.insert(i, 2);
                }
            }
        }
    }
    lines.iter().map(|&(a, b)| if back.contains(&(a, b)) { (b, a) } else { (a, b) }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn overlap(a: (f64, f64), sa: (f64, f64), b: (f64, f64), sb: (f64, f64)) -> bool {
        a.0 < b.0 + sb.0 && b.0 < a.0 + sa.0 && a.1 < b.1 + sb.1 && b.1 < a.1 + sa.1
    }

    #[test]
    fn a_line_runs_down_from_the_box_it_starts_at_and_nothing_overlaps() {
        // Customer → Order → Line, Customer → Invoice, Product → Line; Note alone.
        let sizes = vec![(240.0, 160.0); 6];
        let lines = [(0, 1), (1, 2), (0, 3), (4, 2)];
        let at = arrange(&sizes, &lines);
        for (a, b) in lines {
            assert!(at[a].1 < at[b].1, "{a} above {b}: {at:?}");
        }
        for i in 0..6 {
            for j in i + 1..6 {
                assert!(!overlap(at[i], sizes[i], at[j], sizes[j]), "{i} and {j} overlap: {at:?}");
            }
        }
        assert!(at[5].1 > at[2].1, "the lone box goes under the group");
        assert!(at.iter().all(|(x, y)| x % SNAP == 0.0 && y % SNAP == 0.0), "on the grid");
    }

    #[test]
    fn circles_long_chains_and_odd_input_never_break_it() {
        let sizes = vec![(100.0, 50.0); 3];
        let at = arrange(&sizes, &[(0, 1), (1, 2), (2, 0), (0, 0), (7, 1)]);
        assert_eq!(at.len(), 3);
        let chain: Vec<(usize, usize)> = (0..5000).map(|i| (i, i + 1)).collect();
        let at = arrange(&vec![(10.0, 10.0); 5001], &chain);
        assert!(at[5000].1 > at[0].1);
        assert_eq!(arrange(&[(f64::NAN, f64::INFINITY)], &[]), vec![(0.0, 0.0)]);
        assert!(arrange(&[], &[]).is_empty());
    }

    #[test]
    fn many_groups_are_arranged_quickly() {
        // Thirty thousand pairs: each group's lines are sorted out once, not looked for in all.
        let pairs: Vec<(usize, usize)> = (0..30_000).map(|i| (2 * i, 2 * i + 1)).collect();
        let started = std::time::Instant::now();
        let at = arrange(&vec![(100.0, 50.0); 60_000], &pairs);
        assert_eq!(at.len(), 60_000);
        assert!(started.elapsed() < std::time::Duration::from_secs(5), "{:?}", started.elapsed());
    }
}
