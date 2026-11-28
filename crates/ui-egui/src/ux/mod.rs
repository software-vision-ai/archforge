//! The interface's design under the shell's old path: its colours, its type in Inter, its icons
//! drawn in code and the helpers that draw them come from the shell's interfaces
//! (`archforge_shell_api::ux`), so a notation's screens draw with them without the shell. What
//! needs the shell stays here: the app icon built in for the window and the bar's brand mark, and
//! the design's fonts, installed from the fonts input. `crate::theme` gives the toolkit's own
//! visuals the same palette.

pub use archforge_shell_api::ux::*;

use egui::FontFamily;

/// The app icon's picture (`assets/app-icon/icon.png`, the beaver's tile filling the square), built
/// in for the window icon and the bar's brand mark.
const APP_ICON: &[u8] = include_bytes!("../../../../assets/app-icon/icon.png");

/// The app icon's picture as 8-bit RGBA and its side, read once; none when it cannot be read
/// (said once in the log).
fn app_icon_pixels() -> Option<&'static (usize, Vec<u8>)> {
    static PIXELS: std::sync::OnceLock<Option<(usize, Vec<u8>)>> = std::sync::OnceLock::new();
    PIXELS
        .get_or_init(|| match decode_png(APP_ICON) {
            Ok(image) => Some((image.size[0], image.pixels.iter().flat_map(|c| c.to_srgba_unmultiplied()).collect())),
            Err(e) => {
                log::warn!("the app icon could not be read: {e}");
                None
            }
        })
        .as_ref()
}

/// The app icon averaged down to `side` pixels a side, with straight alpha: for the window icon
/// and the bar's brand mark. None when the picture cannot be read.
pub fn app_icon_rgba(side: usize) -> Option<Vec<u8>> {
    let (from, rgba) = app_icon_pixels()?;
    (side >= 1 && side <= *from).then(|| scale_rgba(rgba, *from, side))
}

/// The app icon as a texture `side` pixels a side, made once per context and size so it is drawn
/// at the screen's own pixels; none when the picture cannot be read.
pub(crate) fn app_icon(ctx: &egui::Context, side: usize) -> Option<egui::TextureHandle> {
    let id = crate::widgets::egui_id(&format!("archforge.app-icon.{side}"));
    if let Some(made) = ctx.data(|d| d.get_temp::<Option<egui::TextureHandle>>(id)) {
        return made;
    }
    let made = app_icon_rgba(side).map(|rgba| {
        let image = egui::ColorImage::from_rgba_unmultiplied([side, side], &rgba);
        ctx.load_texture(format!("app-icon-{side}"), image, egui::TextureOptions::LINEAR)
    });
    ctx.data_mut(|d| d.insert_temp(id, made.clone()));
    made
}

/// `rgba`, `from` pixels a side, averaged down to `to` pixels a side: each pixel the source pixels
/// it covers, alpha premultiplied so a clear edge stays clean. The same average as `cargo xtask
/// resize` (xtask/src/resize.rs), which the packaging sizes come from; keep the two in step.
pub fn scale_rgba(rgba: &[u8], from: usize, to: usize) -> Vec<u8> {
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
                let Some(p) = rgba.get((y * from + j) * 4..(y * from + j) * 4 + 4) else { continue };
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

/// A PNG as an image the toolkit draws.
fn decode_png(bytes: &[u8]) -> Result<egui::ColorImage, String> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
    let (w, h) = (reader.info().width as usize, reader.info().height as usize);
    let mut buf = vec![0u8; reader.output_buffer_size().ok_or("too large to read")?];
    let frame = reader.next_frame(&mut buf).map_err(|e| e.to_string())?;
    let rgba: Vec<u8> = match frame.color_type {
        png::ColorType::Rgba => buf.get(..w * h * 4).ok_or("short picture")?.to_vec(),
        png::ColorType::Rgb => {
            buf.get(..w * h * 3).ok_or("short picture")?.as_chunks::<3>().0.iter().flat_map(|[r, g, b]| [*r, *g, *b, 255]).collect()
        }
        other => return Err(format!("{other:?} pictures are not read")),
    };
    Ok(egui::ColorImage::from_rgba_unmultiplied([w, h], &rgba))
}

/// A face from the optional fonts input: empty unless built with `CRAFT_FONTS_DIR`.
pub struct Face {
    pub family: &'static str,
    pub style: &'static str,
    pub bytes: &'static [u8],
}

include!(concat!(env!("OUT_DIR"), "/faces.rs"));

/// Adds the design's two families to `defs`: each is its Inter face from the fonts input, then the
/// proportional faces for what Inter lacks; without the input, the proportional faces alone. A
/// face egui could not load is left out and said in the log.
pub(crate) fn install(defs: &mut egui::FontDefinitions) {
    let proportional = defs.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    for (family, style) in [(regular(), "Regular"), (semibold(), "SemiBold")] {
        let mut chain = proportional.clone();
        if let Some(face) = FACES.iter().find(|f| f.family == "Inter" && f.style == style) {
            if crate::fonts::readable(face.bytes, 0) {
                let key = format!("{}-{}", face.family, face.style);
                defs.font_data.insert(key.clone(), std::sync::Arc::new(egui::FontData::from_static(face.bytes)));
                chain.insert(0, key);
            } else {
                log::warn!("the fonts input's {} {} is not a font egui can draw; the toolkit's face stands in", face.family, face.style);
            }
        }
        defs.families.insert(family, chain);
    }
}
