//! The window's and the Dock's icon: the Archforge beaver of `assets/app-icon/icon.png`, built into
//! the binary and set on the platform icon grid (its tile 824/1024 of the side, centred, a clear
//! margin round it). Without one the windowing layer shows its own default icon, even over a
//! packaged app's. A packaged macOS app keeps its bundle's icon: it is given the empty icon, which
//! leaves the system's alone. A picture that cannot be read leaves the empty icon too, and says so.

use std::io::Cursor;
use std::sync::Arc;

/// The picture, the tile filling a 1024 pixel square.
const PICTURE: &[u8] = include_bytes!("../../../assets/app-icon/icon.png");

/// The side the icon is set at, and its tile's side on the grid.
const SIDE: usize = 256;
const TILE: usize = SIDE * 824 / 1024;

/// The icon for the window and the Dock: the beaver, or the empty icon inside a packaged macOS app
/// or when the picture cannot be read.
pub fn icon() -> Arc<egui::IconData> {
    if in_bundle() {
        return Arc::new(egui::IconData::default());
    }
    Arc::new(match picture() {
        Ok(icon) => icon,
        Err(e) => {
            log::warn!("the app icon could not be read: {e}");
            egui::IconData::default()
        }
    })
}

/// Whether the app runs from a packaged macOS bundle, whose icon the system shows.
fn in_bundle() -> bool {
    cfg!(target_os = "macos")
        && std::env::current_exe().is_ok_and(|exe| exe.components().any(|c| c.as_os_str().to_str().is_some_and(|s| s.ends_with(".app"))))
}

/// The picture at `SIDE` pixels on the grid, with straight alpha.
fn picture() -> Result<egui::IconData, String> {
    let (side, rgba) = decode(PICTURE)?;
    let tile = scale(&rgba, side, TILE);
    let mut out = vec![0u8; SIDE * SIDE * 4];
    let at = (SIDE - TILE) / 2;
    for y in 0..TILE {
        let to = ((y + at) * SIDE + at) * 4;
        out[to..to + TILE * 4].copy_from_slice(&tile[y * TILE * 4..(y + 1) * TILE * 4]);
    }
    Ok(egui::IconData { rgba: out, width: SIDE as u32, height: SIDE as u32 })
}

/// A square PNG's side and its pixels as 8-bit RGBA.
fn decode(bytes: &[u8]) -> Result<(usize, Vec<u8>), String> {
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
    let (w, h) = (reader.info().width, reader.info().height);
    if w != h || (w as usize) < SIDE {
        return Err(format!("a {w}x{h} picture; a square of at least {SIDE} pixels is needed"));
    }
    let mut buf = vec![0u8; reader.output_buffer_size().ok_or("too large to read")?];
    let frame = reader.next_frame(&mut buf).map_err(|e| e.to_string())?;
    let pixels = (w as usize) * (w as usize);
    let rgba = match frame.color_type {
        png::ColorType::Rgba => buf.get(..pixels * 4).ok_or("short picture")?.to_vec(),
        png::ColorType::Rgb => buf.get(..pixels * 3).ok_or("short picture")?.chunks_exact(3).flat_map(|p| [p[0], p[1], p[2], 255]).collect(),
        other => return Err(format!("{other:?} pictures are not read")),
    };
    Ok((w as usize, rgba))
}

/// `rgba`, `from` pixels a side, averaged down to `to` pixels a side, alpha premultiplied so a
/// clear edge stays clean.
fn scale(rgba: &[u8], from: usize, to: usize) -> Vec<u8> {
    let step = from as f32 / to as f32;
    let weights: Vec<Vec<(usize, f32)>> = (0..to)
        .map(|i| {
            let (lo, hi) = (i as f32 * step, (i + 1) as f32 * step);
            (lo.floor() as usize..(hi.ceil() as usize).min(from))
                .map(|j| (j, (hi.min(j as f32 + 1.0) - lo.max(j as f32)).max(0.0) / step))
                .filter(|(_, k)| *k > 0.0)
                .collect()
        })
        .collect();
    let mut across = vec![0f32; from * to * 4];
    for y in 0..from {
        for (x, ws) in weights.iter().enumerate() {
            let mut acc = [0f32; 4];
            for &(j, k) in ws {
                let p = &rgba[(y * from + j) * 4..(y * from + j) * 4 + 4];
                let a = f32::from(p[3]) / 255.0;
                for c in 0..3 {
                    acc[c] += f32::from(p[c]) * a * k;
                }
                acc[3] += a * k;
            }
            across[(y * to + x) * 4..(y * to + x) * 4 + 4].copy_from_slice(&acc);
        }
    }
    let mut out = vec![0u8; to * to * 4];
    for (y, ws) in weights.iter().enumerate() {
        for x in 0..to {
            let mut acc = [0f32; 4];
            for &(j, k) in ws {
                for c in 0..4 {
                    acc[c] += across[(j * to + x) * 4 + c] * k;
                }
            }
            let px = &mut out[(y * to + x) * 4..(y * to + x) * 4 + 4];
            for c in 0..3 {
                px[c] = if acc[3] > 0.0 { (acc[c] / acc[3]).round().clamp(0.0, 255.0) as u8 } else { 0 };
            }
            px[3] = (acc[3] * 255.0).round().clamp(0.0, 255.0) as u8;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pixel(icon: &egui::IconData, x: usize, y: usize) -> [u8; 4] {
        let i = (y * SIDE + x) * 4;
        [icon.rgba[i], icon.rgba[i + 1], icon.rgba[i + 2], icon.rgba[i + 3]]
    }

    #[test]
    fn the_icon_is_the_beaver_on_the_grid_with_a_clear_margin() {
        let icon = picture().unwrap();
        assert_eq!((icon.width, icon.height, icon.rgba.len()), (256, 256, 256 * 256 * 4));
        assert_eq!(pixel(&icon, 0, 0)[3], 0, "the margin is clear");
        assert_eq!(pixel(&icon, 128, 10)[3], 0, "above the tile too");
        // The blue of the tile, high up at the left, where the beaver is not.
        let blue = pixel(&icon, 50, 60);
        assert!(blue[3] > 240, "the tile is opaque: {blue:?}");
        assert!(blue[2] > blue[0] + 40, "the tile's blue: {blue:?}");
    }

    #[test]
    fn an_average_keeps_colour_and_coverage_and_a_clear_edge_stays_clean() {
        let src = [255, 0, 0, 255, 0, 0, 255, 255, 0, 255, 0, 0, 0, 255, 0, 0];
        assert_eq!(scale(&src, 2, 1), [128, 0, 128, 128]);
    }
}
