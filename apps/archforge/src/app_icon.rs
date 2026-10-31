//! The window's and the Dock's icon: the Archforge beaver of `assets/app-icon/icon.png`, which the
//! shell builds in (`archforge_ui_egui::ux::app_icon_rgba`), set on the platform icon grid (its tile 824/1024 of the side, centred, a clear
//! margin round it). Without one the windowing layer shows its own default icon, even over a
//! packaged app's. A packaged macOS app keeps its bundle's icon: it is given the empty icon, which
//! leaves the system's alone. A picture that cannot be read leaves the empty icon too, and says so.

use std::sync::Arc;

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
    let tile = archforge_ui_egui::ux::app_icon_rgba(TILE).ok_or("the picture built in could not be read")?;
    let mut out = vec![0u8; SIDE * SIDE * 4];
    let at = (SIDE - TILE) / 2;
    for y in 0..TILE {
        let to = ((y + at) * SIDE + at) * 4;
        out[to..to + TILE * 4].copy_from_slice(&tile[y * TILE * 4..(y + 1) * TILE * 4]);
    }
    Ok(egui::IconData { rgba: out, width: SIDE as u32, height: SIDE as u32 })
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
        // The tile fills the grid's square, 206 pixels from 25, centred: opaque just inside each
        // edge's middle, clear just outside.
        let (lo, hi) = (25, 25 + TILE - 1);
        for (x, y) in [(lo + 3, 128), (hi - 3, 128), (128, lo + 3), (128, hi - 3)] {
            assert!(pixel(&icon, x, y)[3] > 200, "the tile reaches the grid at ({x}, {y}): {:?}", pixel(&icon, x, y));
        }
        for (x, y) in [(lo - 2, 128), (hi + 2, 128), (128, lo - 2), (128, hi + 2)] {
            assert_eq!(pixel(&icon, x, y)[3], 0, "and no further, at ({x}, {y})");
        }
        // The blue of the tile, high up at the left, where the beaver is not.
        let blue = pixel(&icon, 50, 60);
        assert!(blue[3] > 240, "the tile is opaque: {blue:?}");
        assert!(blue[2] > blue[0] + 40, "the tile's blue: {blue:?}");
    }
}
