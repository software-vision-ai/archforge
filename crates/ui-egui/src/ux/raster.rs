//! Icons drawn crisp: each icon is rasterised once per size and stroke weight into an alpha mask at
//! the screen's own pixels, then drawn as a tinted image whose corner sits on a pixel. A pixel's
//! ink is its distance to the nearest stroke, so ends and joins come out round without the double
//! ink of overlapping strokes; the weight is a whole number of pixels, and the ends of the strokes
//! sit on the pixel grid, centred on a pixel for an odd weight and between two for an even one, so
//! straight lines have hard edges. Masks are kept per context, a bounded number of them.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use egui::{Color32, ColorImage, Id, Pos2, Rect, TextureHandle, TextureOptions, Vec2, pos2, vec2};

use super::{Icon, Seg};

/// The most masks kept and the most bytes they may hold; past either, the least recently drawn
/// go (a zoomed canvas makes many sizes).
const MAX_CACHED: usize = 1024;
const MAX_BYTES: usize = 16 * 1024 * 1024;
/// The largest mask side in pixels and the heaviest stroke; a larger icon draws as vector strokes.
pub(crate) const MAX_SIDE: f32 = 256.0;
const MAX_WEIGHT: f32 = 32.0;
/// The pieces a cubic curve is cut into.
const CURVE_PIECES: usize = 24;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct Key {
    /// The icon's segments, by address, and its box: two icons never share a mask.
    icon: usize,
    size: [u32; 2],
    /// Pixels per design unit and the stroke weight in pixels, as bits.
    scale: u32,
    weight: u32,
}

/// A mask: its texture, its padding in pixels, its size in bytes and the pass it was last drawn in.
struct Mask {
    texture: TextureHandle,
    pad: f32,
    bytes: usize,
    used: u64,
}

#[derive(Default)]
struct Masks {
    by_key: HashMap<Key, Mask>,
    bytes: usize,
}

impl Masks {
    /// Drops the least recently drawn masks until `room` more bytes and one more mask fit.
    fn make_room(&mut self, room: usize) {
        while !self.by_key.is_empty() && (self.by_key.len() >= MAX_CACHED || self.bytes.saturating_add(room) > MAX_BYTES) {
            let Some(oldest) = self.by_key.iter().min_by_key(|(_, m)| m.used).map(|(k, _)| *k) else { break };
            if let Some(m) = self.by_key.remove(&oldest) {
                self.bytes = self.bytes.saturating_sub(m.bytes);
            }
        }
    }
}

#[derive(Clone, Default)]
struct Cache(Arc<Mutex<Masks>>);

/// A coordinate on the pixel grid for a stroke of `weight` whole pixels.
fn snap(v: f32, weight: u32) -> f32 {
    if weight % 2 == 1 { (v - 0.5).round() + 0.5 } else { v.round() }
}

/// The distance from `p` to the segment `a`-`b`.
fn distance(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let len = ab.length_sq();
    let t = if len > 0.0 { ((p - a).dot(ab) / len).clamp(0.0, 1.0) } else { 0.0 };
    (p - (a + ab * t)).length()
}

/// The point at `t` of a cubic curve.
fn cubic(a: Vec2, c1: Vec2, c2: Vec2, b: Vec2, t: f32) -> Vec2 {
    let u = 1.0 - t;
    a * (u * u * u) + c1 * (3.0 * u * u * t) + c2 * (3.0 * u * t * t) + b * (t * t * t)
}

/// The icon's strokes as straight pieces in pixels from the mask's corner, `pad` in.
fn pieces(icon: &Icon, scale: f32, weight: u32, pad: f32) -> Vec<(Vec2, Vec2)> {
    let at = |q: [f32; 2]| vec2(q[0] * scale + pad, q[1] * scale + pad);
    let snapped = |q: [f32; 2]| {
        let v = at(q);
        vec2(snap(v.x, weight), snap(v.y, weight))
    };
    let mut out = Vec::new();
    for seg in icon.segments {
        match *seg {
            Seg::L(a, b) => out.push((snapped(a), snapped(b))),
            Seg::C(a, c1, c2, b) => {
                // The ends sit on the grid like a line's, so curves meet lines; the control points
                // move with their ends.
                let (sa, sb) = (snapped(a), snapped(b));
                let (c1, c2) = (at(c1) + (sa - at(a)), at(c2) + (sb - at(b)));
                let mut prev = sa;
                for i in 1..=CURVE_PIECES {
                    let p = cubic(sa, c1, c2, sb, i as f32 / CURVE_PIECES as f32);
                    out.push((prev, p));
                    prev = p;
                }
            }
        }
    }
    out
}

/// The mask of `icon` at `scale` pixels per design unit with a stroke of `weight` pixels: white
/// with the ink as alpha, premultiplied, and the padding around the box in pixels.
fn mask(icon: &Icon, scale: f32, weight: u32) -> Option<(ColorImage, f32)> {
    let pad = (weight as f32 / 2.0).ceil() + 1.0;
    let side = |s: f32| {
        let v = (s * scale + 2.0 * pad).ceil();
        (v.is_finite() && (1.0..=MAX_SIDE + 2.0 * (MAX_WEIGHT + 1.0)).contains(&v)).then_some(v as usize)
    };
    let (w, h) = (side(icon.size[0].max(0.0))?, side(icon.size[1].max(0.0))?);
    let lines = pieces(icon, scale, weight, pad);
    let half = weight as f32 / 2.0;
    let mut rgba = vec![0u8; w.checked_mul(h)?.checked_mul(4)?];
    for y in 0..h {
        for x in 0..w {
            let p = vec2(x as f32 + 0.5, y as f32 + 0.5);
            let d = lines.iter().map(|(a, b)| distance(p, *a, *b)).fold(f32::INFINITY, f32::min);
            let ink = (half + 0.5 - d).clamp(0.0, 1.0);
            let a = (ink * 255.0).round() as u8;
            let i = (y * w + x) * 4;
            if let Some(px) = rgba.get_mut(i..i + 4) {
                px.copy_from_slice(&[a, a, a, a]);
            }
        }
    }
    Some((ColorImage::from_rgba_premultiplied([w, h], &rgba), pad))
}

/// Draws `icon` with its box's corner at `at`, `scale` points per design unit, a stroke `weight`
/// points wide, in `color`. False when it is too large for a mask; the caller then draws strokes.
pub(crate) fn draw(painter: &egui::Painter, at: Pos2, icon: &Icon, scale: f32, weight: f32, color: Color32) -> bool {
    let ppp = painter.pixels_per_point();
    let px_scale = scale * ppp;
    if !(px_scale.is_finite() && px_scale > 0.0) || icon.size[0] * px_scale > MAX_SIDE || icon.size[1] * px_scale > MAX_SIDE {
        return false;
    }
    // A stroke too heavy or not a number draws as strokes; a mask's weight is whole pixels.
    let weight_px = weight * ppp;
    if !weight_px.is_finite() || weight_px > MAX_WEIGHT {
        return false;
    }
    let weight_px = weight_px.round().max(1.0) as u32;
    // Quantised to a sixteenth of a pixel per unit, so near sizes share a mask.
    let px_scale = (px_scale * 16.0).round() / 16.0;
    let key = Key { icon: icon.segments.as_ptr() as usize, size: icon.size.map(f32::to_bits), scale: px_scale.to_bits(), weight: weight_px };
    let ctx = painter.ctx();
    let pass = ctx.cumulative_pass_nr();
    let cache = ctx.data_mut(|d| d.get_temp_mut_or_default::<Cache>(Id::unique("ux.icon.masks")).clone());
    let Ok(mut masks) = cache.0.lock() else { return false };
    let found = match masks.by_key.get_mut(&key) {
        Some(m) => {
            m.used = pass;
            Some((m.texture.clone(), m.pad))
        }
        None => None,
    };
    let (texture, pad) = match found {
        Some(f) => f,
        None => {
            let Some((image, pad)) = mask(icon, px_scale, weight_px) else { return false };
            let bytes = image.pixels.len() * 4;
            masks.make_room(bytes);
            let texture = ctx.load_texture("ux.icon", image, TextureOptions::NEAREST);
            masks.bytes = masks.bytes.saturating_add(bytes);
            masks.by_key.insert(key, Mask { texture: texture.clone(), pad, bytes, used: pass });
            (texture, pad)
        }
    };
    drop(masks);
    // The mask's corner on a pixel: the icon moves by less than half a pixel.
    let corner = pos2(((at.x * ppp).round() - pad) / ppp, ((at.y * ppp).round() - pad) / ppp);
    let [w, h] = texture.size();
    let rect = Rect::from_min_size(corner, vec2(w as f32 / ppp, h as f32 / ppp));
    painter.image(texture.id(), rect, Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0)), color);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_on_the_grid_has_hard_edges_and_joins_carry_no_double_ink() {
        // A horizontal line two pixels wide: full ink in two rows, none beside them.
        let line = Icon { size: [10.0, 4.0], stroke: 1.0, segments: &[Seg::L([0.0, 2.0], [10.0, 2.0])] };
        let (img, pad) = mask(&line, 1.0, 2).unwrap();
        let w = img.size[0];
        let alpha = |x: usize, y: usize| img.pixels[y * w + x].a();
        let row = (2.0 + pad) as usize;
        assert_eq!((alpha(5, row - 1), alpha(5, row)), (255, 255));
        assert_eq!((alpha(5, row - 2), alpha(5, row + 1)), (0, 0));
        // Two segments meeting at a corner: the corner is as inked as the lines, never more.
        let corner = Icon { size: [8.0, 8.0], stroke: 1.0, segments: &[Seg::L([0.0, 0.0], [8.0, 0.0]), Seg::L([8.0, 0.0], [8.0, 8.0])] };
        let (img, pad) = mask(&corner, 2.0, 2).unwrap();
        let w = img.size[0];
        let at = |x: f32, y: f32| img.pixels[(y + pad) as usize * w + (x + pad) as usize].a();
        assert_eq!((at(8.0, 0.0), at(15.0, 0.0), at(15.0, 8.0)), (255, 255, 255), "the lines and the corner are fully inked");
        assert_eq!(at(17.0, -2.0), 0, "nothing outside the round corner");
    }

    #[test]
    fn a_mask_never_grows_past_its_bounds() {
        let line = Icon { size: [10.0, 4.0], stroke: 1.0, segments: &[Seg::L([0.0, 2.0], [10.0, 2.0])] };
        assert!(mask(&line, f32::INFINITY, 2).is_none());
        assert!(mask(&line, 1.0e9, 2).is_none());
        assert!(mask(&line, 1.0, u32::MAX).is_none());
        let mut masks = Masks { bytes: MAX_BYTES, ..Masks::default() };
        masks.make_room(1);
        assert_eq!(masks.bytes, MAX_BYTES, "nothing to drop leaves the count alone");
    }

    #[test]
    fn odd_weights_centre_on_pixels_and_even_ones_between_them() {
        assert_eq!(snap(3.2, 1), 3.5);
        assert_eq!(snap(3.2, 2), 3.0);
        assert_eq!(snap(3.7, 3), 3.5);
    }
}
