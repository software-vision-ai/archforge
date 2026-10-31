//! The window's and the Dock's icon: the Archforge app icon of `assets/app-icon/icon.svg`, drawn
//! here from the same shapes, as the binary carries no SVG renderer. Without one the windowing
//! layer shows its own default icon, even over a packaged app's. A packaged macOS app keeps its
//! bundle's icon, which has the light and the dark appearance: it is given the empty icon, which
//! leaves the system's alone. Keep the shapes in step with the SVG.

use std::sync::Arc;

/// The side the icon is drawn at, and the samples per pixel along each axis.
const SIDE: usize = 256;
const SAMPLES: usize = 4;

/// The SVG's canvas; its tile's edges and corner radius; the tile's border, inset by half its
/// width; the grid's first line, spacing, count and half width.
const CANVAS: f32 = 1024.0;
const TILE: (f32, f32, f32) = (100.0, 924.0, 185.0);
const BORDER: (f32, f32, f32, f32) = (101.5, 922.5, 183.5, 1.5);
const GRID: (f32, f32, f32, f32) = (160.0, 64.0, 11.0, 1.0);
/// The tile's gradient, top to bottom; the glow's centre, radius and strength; the mark's
/// placement and scale, and the crossbar's teal.
const TOP: [f32; 3] = [0x3A as f32 / 255.0, 0x5F as f32 / 255.0, 0x86 as f32 / 255.0];
const BOTTOM: [f32; 3] = [0x22 as f32 / 255.0, 0x3B as f32 / 255.0, 0x55 as f32 / 255.0];
const GLOW: (f32, f32, f32, f32) = (347.2, 264.8, 741.6, 0.10);
const MARK_AT: (f32, f32) = (217.6, 225.6);
const MARK_SCALE: f32 = 9.2;
const TEAL: [f32; 3] = [0x9E as f32 / 255.0, 0xD8 as f32 / 255.0, 0xDE as f32 / 255.0];

/// The mark in its own box: the A's two legs, then the crossbar over them.
const LEGS: [[(f32, f32); 4]; 2] =
    [[(7.0, 54.0), (26.0, 10.0), (36.0, 10.0), (18.0, 54.0)], [(28.0, 10.0), (38.0, 10.0), (57.0, 54.0), (45.0, 54.0)]];
const BAR: [(f32, f32); 4] = [(24.0, 35.0), (40.0, 35.0), (43.0, 43.0), (21.0, 43.0)];

/// The icon for the window and the Dock: the drawn app icon, or the empty icon inside a packaged
/// macOS app.
pub fn icon() -> Arc<egui::IconData> {
    Arc::new(if in_bundle() { egui::IconData::default() } else { draw() })
}

/// Whether the app runs from a packaged macOS bundle, whose icon the system shows.
fn in_bundle() -> bool {
    cfg!(target_os = "macos")
        && std::env::current_exe().is_ok_and(|exe| exe.components().any(|c| c.as_os_str().to_str().is_some_and(|s| s.ends_with(".app"))))
}

/// The app icon at `SIDE` pixels, each the average of its samples, with straight alpha.
fn draw() -> egui::IconData {
    let mut rgba = Vec::with_capacity(SIDE * SIDE * 4);
    let unit = CANVAS / SIDE as f32;
    let n = (SAMPLES * SAMPLES) as f32;
    for py in 0..SIDE {
        for px in 0..SIDE {
            // Premultiplied sums, so the edge of the tile fades instead of darkening.
            let mut sum = [0.0f32; 4];
            for sy in 0..SAMPLES {
                for sx in 0..SAMPLES {
                    let x = (px as f32 + (sx as f32 + 0.5) / SAMPLES as f32) * unit;
                    let y = (py as f32 + (sy as f32 + 0.5) / SAMPLES as f32) * unit;
                    if let Some(c) = colour_at(x, y) {
                        for (s, v) in sum.iter_mut().zip(c) {
                            *s += v;
                        }
                        sum[3] += 1.0;
                    }
                }
            }
            let alpha = sum[3] / n;
            for v in &sum[..3] {
                rgba.push(if sum[3] > 0.0 { to_byte(v / sum[3]) } else { 0 });
            }
            rgba.push(to_byte(alpha));
        }
    }
    egui::IconData { rgba, width: SIDE as u32, height: SIDE as u32 }
}

fn to_byte(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// The colour at a point of the canvas, none outside the tile.
fn colour_at(x: f32, y: f32) -> Option<[f32; 3]> {
    let (lo, hi, radius) = TILE;
    if rounded_rect(x, y, lo, hi, radius) > 0.0 {
        return None;
    }
    let t = ((y - lo) / (hi - lo)).clamp(0.0, 1.0);
    let mut c: [f32; 3] = std::array::from_fn(|i| TOP[i] + (BOTTOM[i] - TOP[i]) * t);
    let (first, step, last, half) = GRID;
    let on_grid = |v: f32| {
        let k = ((v - first) / step).round();
        (0.0..=last).contains(&k) && (v - (first + k * step)).abs() <= half
    };
    if on_grid(x) || on_grid(y) {
        c = over(c, [1.0; 3], 0.06);
    }
    let (gx, gy, reach, strength) = GLOW;
    let d = ((x - gx).powi(2) + (y - gy).powi(2)).sqrt() / reach;
    c = over(c, [1.0; 3], strength * (1.0 - d.min(1.0)));
    let (blo, bhi, bradius, bhalf) = BORDER;
    if rounded_rect(x, y, blo, bhi, bradius).abs() <= bhalf {
        c = over(c, [1.0; 3], 0.20);
    }
    let (u, v) = ((x - MARK_AT.0) / MARK_SCALE, (y - MARK_AT.1) / MARK_SCALE);
    if LEGS.iter().any(|leg| inside(leg, u, v)) {
        c = [1.0; 3];
    }
    if inside(&BAR, u, v) {
        c = TEAL;
    }
    Some(c)
}

/// `top` laid over `base` at `alpha`.
fn over(base: [f32; 3], top: [f32; 3], alpha: f32) -> [f32; 3] {
    std::array::from_fn(|i| base[i] + (top[i] - base[i]) * alpha)
}

/// The signed distance from a point to a square rounded rectangle from `lo` to `hi` on both axes:
/// negative inside.
fn rounded_rect(x: f32, y: f32, lo: f32, hi: f32, radius: f32) -> f32 {
    let centre = (lo + hi) / 2.0;
    let half = (hi - lo) / 2.0 - radius;
    let (qx, qy) = ((x - centre).abs() - half, (y - centre).abs() - half);
    (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt() + qx.max(qy).min(0.0) - radius
}

/// Whether a point lies inside a convex polygon, whichever way round its corners run.
fn inside(polygon: &[(f32, f32)], x: f32, y: f32) -> bool {
    let mut sign = 0.0f32;
    for (i, &(ax, ay)) in polygon.iter().enumerate() {
        let (bx, by) = polygon[(i + 1) % polygon.len()];
        let cross = (bx - ax) * (y - ay) - (by - ay) * (x - ax);
        if cross != 0.0 {
            if sign != 0.0 && cross.signum() != sign {
                return false;
            }
            sign = cross.signum();
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pixel(icon: &egui::IconData, x: usize, y: usize) -> [u8; 4] {
        let i = (y * SIDE + x) * 4;
        [icon.rgba[i], icon.rgba[i + 1], icon.rgba[i + 2], icon.rgba[i + 3]]
    }

    /// A canvas point's pixel.
    fn at(icon: &egui::IconData, x: f32, y: f32) -> [u8; 4] {
        let k = SIDE as f32 / CANVAS;
        pixel(icon, (x * k) as usize, (y * k) as usize)
    }

    #[test]
    fn the_drawn_icon_is_the_slate_tile_with_the_white_a_and_its_teal_crossbar() {
        let icon = draw();
        assert_eq!((icon.width, icon.height, icon.rgba.len()), (256, 256, 256 * 256 * 4));
        assert_eq!(pixel(&icon, 0, 0)[3], 0, "the corner outside the tile is clear");
        assert_eq!(at(&icon, 105.0, 105.0)[3], 0, "the tile's rounded corner is clear");
        let tile = at(&icon, 512.0, 880.0);
        assert_eq!(tile[3], 255);
        assert!(tile[2] > tile[0] && tile[0] < 80, "slate blue near the foot: {tile:?}");
        // The crossbar's middle, in the mark's own units (32, 39), and a leg's (14, 40).
        let bar = at(&icon, MARK_AT.0 + 32.0 * MARK_SCALE, MARK_AT.1 + 39.0 * MARK_SCALE);
        assert_eq!(bar, [0x9E, 0xD8, 0xDE, 255]);
        let leg = at(&icon, MARK_AT.0 + 14.0 * MARK_SCALE, MARK_AT.1 + 40.0 * MARK_SCALE);
        assert_eq!(leg, [255, 255, 255, 255]);
    }

    #[test]
    fn a_point_is_inside_a_convex_polygon_either_way_round() {
        let square = [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)];
        let mut reversed = square;
        reversed.reverse();
        for s in [square, reversed] {
            assert!(inside(&s, 0.5, 0.5));
            assert!(!inside(&s, 1.5, 0.5));
        }
    }
}
