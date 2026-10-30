//! The interface's design: its colours, its type in Inter, its icons drawn in code, and the
//! helpers that draw them. Home draws through this module; `crate::theme` gives the toolkit's own
//! visuals the same palette. The design has two appearances, Light and Dark, chosen by the
//! shell's theme.

pub mod icons;

use egui::{Color32, FontFamily, FontId, Pos2, Rect, Stroke, Vec2};

/// The design's colours by role, one set per appearance, Light and Dark.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Palette {
    /// The page behind cards and panels; the search field.
    pub page: Color32,
    /// The bar, cards, buttons and fields.
    pub surface: Color32,
    /// The title strip that is the macOS title bar.
    pub title_strip: Color32,
    /// Dividers and the borders of cards, buttons and fields.
    pub border: Color32,
    /// A card's border under the pointer.
    pub border_hover: Color32,
    pub text: Color32,
    pub text_secondary: Color32,
    pub text_muted: Color32,
    /// The brand blue for text and icons.
    pub accent: Color32,
    /// The brand blue's tint: the avatar and icon tiles.
    pub accent_tint: Color32,
    /// The primary button's and the brand mark's fill.
    pub accent_fill: Color32,
    /// Text and icons on the accent fill.
    pub on_accent: Color32,
    pub green: Color32,
    pub green_tint: Color32,
    pub purple: Color32,
    pub purple_tint: Color32,
    pub amber: Color32,
    pub amber_tint: Color32,
    /// The veil over the window behind a dialog.
    pub scrim: Color32,
    /// A dialog's shadow.
    pub shadow: Color32,
    /// The ring of a field that has the keyboard.
    pub focus: Color32,
    /// A dialog's edge: none in Light, where its shadow sets it off, a hairline in Dark.
    pub dialog_edge: Color32,
    /// The canvas behind a diagram, a shade off the surface.
    pub canvas: Color32,
    /// A filled field in a panel.
    pub field: Color32,
}

const fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

pub const LIGHT: Palette = Palette {
    page: rgb(0xF5F7FA),
    surface: rgb(0xFFFFFF),
    title_strip: rgb(0xE9EDF2),
    border: rgb(0xDFE5ED),
    border_hover: rgb(0xB9C9F9),
    text: rgb(0x1B2B3C),
    text_secondary: rgb(0x657386),
    text_muted: rgb(0x8A96A7),
    accent: rgb(0x345EDB),
    accent_tint: rgb(0xEBF0FE),
    accent_fill: rgb(0x345EDB),
    on_accent: rgb(0xFFFFFF),
    green: rgb(0x27816D),
    green_tint: rgb(0xEAF5F0),
    purple: rgb(0x7A56B5),
    purple_tint: rgb(0xF2ECFA),
    amber: rgb(0xA47420),
    amber_tint: rgb(0xFBF3E4),
    // #172844 at 40 %, #152B3A at 20 %, premultiplied.
    scrim: Color32::from_rgba_premultiplied(9, 16, 27, 102),
    shadow: Color32::from_rgba_premultiplied(4, 9, 12, 51),
    focus: rgb(0x7999EF),
    dialog_edge: Color32::TRANSPARENT,
    canvas: rgb(0xFBFCFE),
    field: rgb(0xF2F4F7),
};

pub const DARK: Palette = Palette {
    page: rgb(0x12171E),
    surface: rgb(0x1A212B),
    title_strip: rgb(0x0D1117),
    border: rgb(0x2A3441),
    border_hover: rgb(0x3B5498),
    text: rgb(0xE6EBF2),
    text_secondary: rgb(0x9AA7B8),
    text_muted: rgb(0x7F8C9E),
    accent: rgb(0x7C9CF5),
    accent_tint: rgb(0x1F2A45),
    accent_fill: rgb(0x3A64DE),
    on_accent: rgb(0xFFFFFF),
    green: rgb(0x4FC3A1),
    green_tint: rgb(0x15302A),
    purple: rgb(0xB39AF0),
    purple_tint: rgb(0x2A2240),
    amber: rgb(0xE2AE55),
    amber_tint: rgb(0x362B16),
    // Black at 55 % and at 50 %.
    scrim: Color32::from_rgba_premultiplied(0, 0, 0, 140),
    shadow: Color32::from_rgba_premultiplied(0, 0, 0, 128),
    focus: rgb(0x5B7EE8),
    dialog_edge: rgb(0x2A3441),
    canvas: rgb(0x151B23),
    field: rgb(0x232C38),
};

/// The palette of an appearance.
pub fn palette(dark: bool) -> &'static Palette {
    if dark { &DARK } else { &LIGHT }
}

/// A text style of the design: Inter at a size, in a line of a height, regular or semibold, with
/// extra space between letters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Type {
    pub size: f32,
    pub line: f32,
    pub semibold: bool,
    pub spacing: f32,
}

impl Type {
    const fn regular(size: f32, line: f32) -> Self {
        Self { size, line, semibold: false, spacing: 0.0 }
    }
    const fn semibold(size: f32, line: f32) -> Self {
        Self { size, line, semibold: true, spacing: 0.0 }
    }
    pub fn font(self) -> FontId {
        FontId::new(self.size, if self.semibold { semibold() } else { regular() })
    }
}

/// The design's text styles.
pub mod types {
    use super::Type;

    pub const DISPLAY: Type = Type::semibold(36.0, 52.0);
    pub const SECTION: Type = Type::semibold(20.0, 29.0);
    pub const SUBSECTION: Type = Type::semibold(18.0, 26.0);
    pub const CARD_TITLE: Type = Type::semibold(17.0, 25.0);
    pub const LEAD: Type = Type::regular(16.0, 23.0);
    pub const LEAD_STRONG: Type = Type::semibold(16.0, 23.0);
    pub const BODY: Type = Type::regular(14.0, 20.0);
    pub const BODY_STRONG: Type = Type::semibold(14.0, 20.0);
    pub const UI: Type = Type::regular(13.0, 19.0);
    pub const UI_STRONG: Type = Type::semibold(13.0, 19.0);
    pub const SMALL: Type = Type::regular(12.0, 17.0);
    pub const CAPTION: Type = Type::regular(11.0, 16.0);
    pub const CAPTION_STRONG: Type = Type::semibold(11.0, 16.0);
    /// Small capitals above a heading, spaced out.
    pub const EYEBROW: Type = Type { size: 11.0, line: 16.0, semibold: true, spacing: 1.2 };
    pub const TINY: Type = Type::regular(10.0, 15.0);
}

/// A face from the optional fonts input: empty unless built with `CRAFT_FONTS_DIR`.
pub struct Face {
    pub family: &'static str,
    pub style: &'static str,
    pub bytes: &'static [u8],
}

include!(concat!(env!("OUT_DIR"), "/faces.rs"));

/// The family the design's regular text is drawn in.
pub fn regular() -> FontFamily {
    FontFamily::Name("ux-regular".into())
}

/// The family the design's semibold text is drawn in.
pub fn semibold() -> FontFamily {
    FontFamily::Name("ux-semibold".into())
}

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

/// Whether the design's families are installed in `ctx`. They arrive at the end of the first frame;
/// text in a family egui does not have stops the program, so a screen drawn in this design checks
/// this before its first text.
pub fn ready(ctx: &egui::Context) -> bool {
    ctx.fonts(|f| {
        let families = &f.definitions().families;
        families.contains_key(&regular()) && families.contains_key(&semibold())
    })
}

/// Lays `text` out in `ty`, on one line, cut with an ellipsis at `width` when given.
pub fn galley(painter: &egui::Painter, text: &str, ty: Type, color: Color32, width: Option<f32>) -> std::sync::Arc<egui::Galley> {
    galley_rows(painter, text, ty, color, width, 1)
}

/// Lays `text` out in `ty`, wrapped at `width` into at most `rows` lines, the last cut with an
/// ellipsis.
pub fn galley_rows(painter: &egui::Painter, text: &str, ty: Type, color: Color32, width: Option<f32>, rows: usize) -> std::sync::Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::single_section(
        text.to_owned(),
        egui::TextFormat {
            font_id: ty.font(),
            color,
            extra_letter_spacing: ty.spacing,
            line_height: Some(ty.line),
            valign: egui::Align::Center,
            ..Default::default()
        },
    );
    job.wrap = egui::text::TextWrapping {
        max_width: width.unwrap_or(f32::INFINITY),
        max_rows: rows.max(1),
        break_anywhere: false,
        overflow_character: Some('…'),
    };
    painter.layout_job(job)
}

/// Draws `text` with its line box's top-left corner at `at`, as the design places its text, and
/// gives the box it took.
pub fn text(painter: &egui::Painter, at: Pos2, text: &str, ty: Type, color: Color32, width: Option<f32>) -> Rect {
    let g = galley(painter, text, ty, color, width);
    let rect = Rect::from_min_size(at, g.size());
    painter.galley(at, g, color);
    rect
}

/// One stroke of an icon, in the icon's own box.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Seg {
    L([f32; 2], [f32; 2]),
    C([f32; 2], [f32; 2], [f32; 2], [f32; 2]),
}

/// An icon as the design draws it: strokes in a box of `size`, `stroke` wide, with round ends and
/// joins.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Icon {
    pub size: [f32; 2],
    pub stroke: f32,
    pub segments: &'static [Seg],
}

/// Draws `icon` with its box's top-left corner at `at`, `width` wide (its height in proportion),
/// in `color`, with the design's stroke weight whatever the size. The round ends and joins are
/// discs at every end point, so a stroke of no length draws a dot.
pub fn icon(painter: &egui::Painter, at: Pos2, icon: &Icon, width: f32, color: Color32) {
    icon_weighted(painter, at, icon, width, icon.stroke, color);
}

/// [`icon`] with a stroke weight of its own, where the design gives an instance one.
pub fn icon_weighted(painter: &egui::Painter, at: Pos2, icon: &Icon, width: f32, weight: f32, color: Color32) {
    let scale = if icon.size[0] > 0.0 { width / icon.size[0] } else { 1.0 };
    let stroke = Stroke::new(weight, color);
    let p = |q: [f32; 2]| at + Vec2::new(q[0], q[1]) * scale;
    for seg in icon.segments {
        let (a, b) = match *seg {
            Seg::L(a, b) => {
                painter.line_segment([p(a), p(b)], stroke);
                (a, b)
            }
            Seg::C(a, c1, c2, b) => {
                painter.add(egui::epaint::CubicBezierShape::from_points_stroke([p(a), p(c1), p(c2), p(b)], false, Color32::TRANSPARENT, stroke));
                (a, b)
            }
        };
        painter.circle_filled(p(a), weight / 2.0, color);
        painter.circle_filled(p(b), weight / 2.0, color);
    }
}

/// Draws `icon` at its design size centred in `rect`.
pub fn icon_in(painter: &egui::Painter, rect: Rect, icon_: &Icon, color: Color32) {
    let size = Vec2::new(icon_.size[0], icon_.size[1]);
    icon(painter, rect.center() - size / 2.0, icon_, icon_.size[0], color);
}
