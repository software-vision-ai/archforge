//! The interface's design: its colours, its type in Inter, its icons drawn in code, and the
//! helpers that draw them. The shell and the notations' screens draw through this module; the
//! shell's theme gives the toolkit's own visuals the same palette. The design has two appearances,
//! Light and Dark, chosen by the shell's theme. The shell keeps what needs it: the app icon and
//! the installing of the design's fonts (`archforge_ui_egui::ux`).

pub mod icons;
mod raster;

use egui::{Color32, FontFamily, FontId, Pos2, Rect, Stroke, Vec2};

/// A colour of the interface: a role of the [`Palette`], a named colour of this module
/// ([`CLEAR`], [`MARK`], …), or a colour the document holds ([`data_colour`]). Its value is private,
/// so only `ux` makes one; the rest of the interface takes them from here and hands them to egui,
/// which takes them wherever it takes its own colour (`impl Into<Color32>`, or `.into()`).
///
/// ```
/// // A colour of the design reaches egui as the colour it is.
/// let text: egui::Color32 = archforge_shell_api::ux::LIGHT.text.into();
/// assert_eq!(text, egui::Color32::from_rgb(0x1B, 0x2B, 0x3C));
/// ```
///
/// ```compile_fail
/// // Outside `ux` no colour is made from values: its value is private.
/// let red = archforge_shell_api::ux::Colour(egui::Color32::RED);
/// ```
///
/// ```compile_fail
/// // Nor from one of egui's colours.
/// let red: archforge_shell_api::ux::Colour = egui::Color32::RED.into();
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Colour(Color32);

impl From<Colour> for Color32 {
    /// The colour as egui draws it.
    fn from(c: Colour) -> Self {
        c.0
    }
}

impl Colour {
    /// The colour with its opacity (and its premultiplied channels) times `k`, in gamma space: a
    /// dimmed or faded form of it.
    pub fn gamma_multiply(self, k: f32) -> Self {
        Self(self.0.gamma_multiply(k))
    }

    /// Its red, 0 to 255, premultiplied by its opacity.
    pub fn r(self) -> u8 {
        self.0.r()
    }

    /// Its green, 0 to 255, premultiplied by its opacity.
    pub fn g(self) -> u8 {
        self.0.g()
    }

    /// Its blue, 0 to 255, premultiplied by its opacity.
    pub fn b(self) -> u8 {
        self.0.b()
    }

    /// Its opacity, 0 (clear) to 255 (opaque).
    pub fn a(self) -> u8 {
        self.0.a()
    }
}

/// The design's colours by role, one set per appearance, Light and Dark.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Palette {
    /// The page behind cards and panels; the search field.
    pub page: Colour,
    /// The bar, cards, buttons and fields.
    pub surface: Colour,
    /// The title strip that is the macOS title bar.
    pub title_strip: Colour,
    /// Dividers and the borders of cards, buttons and fields.
    pub border: Colour,
    /// A card's border under the pointer.
    pub border_hover: Colour,
    /// A scrolled panel's thumb.
    pub scroll_thumb: Colour,
    pub text: Colour,
    pub text_secondary: Colour,
    pub text_muted: Colour,
    /// The brand blue for text and icons.
    pub accent: Colour,
    /// The brand blue's tint: the avatar and icon tiles.
    pub accent_tint: Colour,
    /// The primary button's and the brand mark's fill.
    pub accent_fill: Colour,
    /// Text and icons on the accent fill.
    pub on_accent: Colour,
    /// The app icon's tile blue, for the wordmark's Forge: the tile's own colour in Light, a
    /// lighter one in Dark so it reads on the dark bar.
    pub brand: Colour,
    pub green: Colour,
    pub green_tint: Colour,
    pub purple: Colour,
    pub purple_tint: Colour,
    pub amber: Colour,
    pub amber_tint: Colour,
    /// The veil over the window behind a dialog.
    pub scrim: Colour,
    /// A dialog's shadow.
    pub shadow: Colour,
    /// The ring of a field that has the keyboard.
    pub focus: Colour,
    /// A dialog's edge: none in Light, where its shadow sets it off, a hairline in Dark.
    pub dialog_edge: Colour,
    /// The canvas behind a diagram, a shade off the surface.
    pub canvas: Colour,
    /// A filled field in a panel.
    pub field: Colour,
    /// A destructive button.
    pub danger: Colour,
    /// The danger colour's tint: behind a refusal said in a panel.
    pub danger_tint: Colour,
    /// A switch's track while it is off.
    pub control_off: Colour,
    /// An entity node on the canvas: its fill, its edge, its eyebrow and key badges, its header line.
    pub entity: Colour,
    pub entity_edge: Colour,
    pub entity_ink: Colour,
    pub entity_line: Colour,
    /// The foreign-key dot of the key status toggle.
    pub key_foreign: Colour,
}

/// The opaque colour of the code `0xRRGGBB`.
const fn rgb(hex: u32) -> Colour {
    Colour(Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8))
}

pub const LIGHT: Palette = Palette {
    page: rgb(0xF5F7FA),
    surface: rgb(0xFFFFFF),
    title_strip: rgb(0xE9EDF2),
    border: rgb(0xDFE5ED),
    border_hover: rgb(0xB9C9F9),
    scroll_thumb: rgb(0xC5CDD7),
    text: rgb(0x1B2B3C),
    text_secondary: rgb(0x657386),
    text_muted: rgb(0x8A96A7),
    accent: rgb(0x345EDB),
    accent_tint: rgb(0xEBF0FE),
    accent_fill: rgb(0x345EDB),
    on_accent: rgb(0xFFFFFF),
    brand: rgb(0x366A92),
    green: rgb(0x27816D),
    green_tint: rgb(0xEAF5F0),
    purple: rgb(0x7A56B5),
    purple_tint: rgb(0xF2ECFA),
    amber: rgb(0xA47420),
    amber_tint: rgb(0xFBF3E4),
    // #172844 at 40 %, #152B3A at 20 %, premultiplied.
    scrim: Colour(Color32::from_rgba_premultiplied(9, 16, 27, 102)),
    shadow: Colour(Color32::from_rgba_premultiplied(4, 9, 12, 51)),
    focus: rgb(0x7999EF),
    dialog_edge: CLEAR,
    canvas: rgb(0xFBFCFE),
    field: rgb(0xF2F4F7),
    danger: rgb(0xC0392B),
    danger_tint: rgb(0xFBEDEB),
    control_off: rgb(0xD5DCE6),
    entity: rgb(0xFFF7EE),
    entity_edge: rgb(0xF0B37A),
    entity_ink: rgb(0xC25E0A),
    entity_line: rgb(0xF6D6B5),
    key_foreign: rgb(0xE07B1A),
};

pub const DARK: Palette = Palette {
    page: rgb(0x12171E),
    surface: rgb(0x1A212B),
    title_strip: rgb(0x0D1117),
    border: rgb(0x2A3441),
    border_hover: rgb(0x3B5498),
    scroll_thumb: rgb(0x4A5466),
    text: rgb(0xE6EBF2),
    text_secondary: rgb(0x9AA7B8),
    text_muted: rgb(0x7F8C9E),
    accent: rgb(0x7C9CF5),
    accent_tint: rgb(0x1F2A45),
    accent_fill: rgb(0x3A64DE),
    on_accent: rgb(0xFFFFFF),
    brand: rgb(0x7FAFD4),
    green: rgb(0x4FC3A1),
    green_tint: rgb(0x15302A),
    purple: rgb(0xB39AF0),
    purple_tint: rgb(0x2A2240),
    amber: rgb(0xE2AE55),
    amber_tint: rgb(0x362B16),
    // Black at 55 % and at 50 %.
    scrim: Colour(Color32::from_rgba_premultiplied(0, 0, 0, 140)),
    shadow: Colour(Color32::from_rgba_premultiplied(0, 0, 0, 128)),
    focus: rgb(0x5B7EE8),
    dialog_edge: rgb(0x2A3441),
    canvas: rgb(0x151B23),
    field: rgb(0x232C38),
    danger: rgb(0xD9534F),
    danger_tint: rgb(0x3A1F1E),
    control_off: rgb(0x3A4554),
    entity: rgb(0x2A2119),
    entity_edge: rgb(0x7A5A2E),
    entity_ink: rgb(0xE8A05A),
    entity_line: rgb(0x4A3A28),
    key_foreign: rgb(0xF0A05A),
};

/// The palette of an appearance.
pub fn palette(dark: bool) -> &'static Palette {
    if dark { &DARK } else { &LIGHT }
}

/// No paint: a background or an edge that is not drawn.
pub const CLEAR: Colour = Colour(Color32::TRANSPARENT);

/// The tint that draws a picture in its own colours.
pub const UNTINTED: Colour = Colour(Color32::WHITE);

/// The colour of text laid out to be measured, or to be drawn in the colour given when it is
/// painted.
pub const PLACEHOLDER_INK: Colour = Colour(Color32::PLACEHOLDER);

/// The ring that marks the chosen point on the colour picker's field and hue bar: white in both
/// appearances, so it shows on every colour, with a thin dark edge ([`MARK_EDGE`]).
pub const MARK: Colour = Colour(Color32::WHITE);

/// The thin dark edge round the picker's [`MARK`]: black at 35 %.
pub const MARK_EDGE: Colour = Colour(Color32::from_black_alpha(90));

/// A colour the document holds (a node's own fill, line or text, chosen in the Appearance panel),
/// not one of the design's: the one way a colour read from data is made.
pub fn data_colour(r: u8, g: u8, b: u8) -> Colour {
    Colour(Color32::from_rgb(r, g, b))
}

/// The colour picker's colour of hue, saturation and brightness, each from 0 to 1, as the picker
/// shows them (in gamma space), fully opaque.
pub fn from_hsv(h: f32, s: f32, v: f32) -> Colour {
    Colour(egui::ecolor::HsvaGamma { h, s, v, a: 1.0 }.into())
}

/// `c` as the colour picker's hue, saturation and brightness ([`from_hsv`]).
pub fn to_hsv(c: Colour) -> [f32; 3] {
    let hsv = egui::ecolor::HsvaGamma::from(c.0);
    [hsv.h, hsv.s, hsv.v]
}

/// The own-colour swatch's colour at `t` (0 to 1) round its disc: every hue, a little soft.
pub fn swatch_hue(t: f32) -> Colour {
    Colour(egui::ecolor::Hsva::new(t, 0.75, 0.95, 1.0).into())
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
    /// A regular style at `size` in a line `line` tall, with no extra space between letters.
    const fn regular(size: f32, line: f32) -> Self {
        Self { size, line, semibold: false, spacing: 0.0 }
    }
    /// A semibold style at `size` in a line `line` tall, with no extra space between letters.
    const fn semibold(size: f32, line: f32) -> Self {
        Self { size, line, semibold: true, spacing: 0.0 }
    }
    /// The same style at `z` times the size, for text drawn on a zoomed canvas.
    pub fn scaled(self, z: f32) -> Self {
        let z = if z.is_finite() { z.clamp(0.05, 20.0) } else { 1.0 };
        Self { size: self.size * z, line: self.line * z, semibold: self.semibold, spacing: self.spacing * z }
    }

    /// The font the style is drawn in: Inter at its size, in the design's regular or semibold
    /// family.
    pub fn font(self) -> FontId {
        FontId::new(self.size, if self.semibold { semibold() } else { regular() })
    }
}

/// The design's text styles.
pub mod types {
    use super::Type;

    pub const DISPLAY: Type = Type::semibold(36.0, 52.0);
    /// The title of a page in the centre, such as a version's details.
    pub const PAGE_TITLE: Type = Type::semibold(26.0, 31.0);
    pub const SECTION: Type = Type::semibold(20.0, 29.0);
    pub const SUBSECTION: Type = Type::semibold(18.0, 26.0);
    pub const CARD_TITLE: Type = Type::semibold(17.0, 25.0);
    /// A small confirmation's title.
    pub const DIALOG_TITLE: Type = Type::semibold(15.0, 18.0);
    pub const LEAD: Type = Type::regular(16.0, 23.0);
    pub const LEAD_STRONG: Type = Type::semibold(16.0, 23.0);
    pub const BODY: Type = Type::regular(14.0, 20.0);
    pub const BODY_STRONG: Type = Type::semibold(14.0, 20.0);
    pub const UI: Type = Type::regular(13.0, 19.0);
    pub const UI_STRONG: Type = Type::semibold(13.0, 19.0);
    pub const SMALL: Type = Type::regular(12.0, 17.0);
    pub const SMALL_STRONG: Type = Type::semibold(12.0, 17.0);
    pub const CAPTION: Type = Type::regular(11.0, 16.0);
    pub const CAPTION_STRONG: Type = Type::semibold(11.0, 16.0);
    /// Small capitals above a heading, spaced out.
    pub const EYEBROW: Type = Type { size: 11.0, line: 16.0, semibold: true, spacing: 1.2 };
    /// A smaller eyebrow, over a section below a list.
    pub const EYEBROW_SMALL: Type = Type { size: 10.0, line: 12.0, semibold: true, spacing: 0.4 };
    pub const TINY: Type = Type::regular(10.0, 15.0);
}

/// The family the design's regular text is drawn in.
pub fn regular() -> FontFamily {
    FontFamily::Name("ux-regular".into())
}

/// The family the design's semibold text is drawn in.
pub fn semibold() -> FontFamily {
    FontFamily::Name("ux-semibold".into())
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
pub fn galley(painter: &egui::Painter, text: &str, ty: Type, color: Colour, width: Option<f32>) -> std::sync::Arc<egui::Galley> {
    galley_rows(painter, text, ty, color, width, 1)
}

/// Lays `text` out in `ty`, wrapped at `width` into at most `rows` lines, the last cut with an
/// ellipsis.
pub fn galley_rows(painter: &egui::Painter, text: &str, ty: Type, color: Colour, width: Option<f32>, rows: usize) -> std::sync::Arc<egui::Galley> {
    laid_out(painter, text, ty, color, width, rows, false)
}

/// [`text`], slanted (an abstract class's name).
pub fn text_slanted(painter: &egui::Painter, at: Pos2, text: &str, ty: Type, color: Colour, width: Option<f32>) -> Rect {
    text_marked(painter, at, text, ty, color, width, Marks { slanted: true, underlined: false })
}

/// How a member's text is set beside its plain form, either or both: slanted (abstract) and
/// underlined (static).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Marks {
    pub slanted: bool,
    pub underlined: bool,
}

/// [`text`] with its `marks`. The underline runs under the glyphs drawn, from the first to the end
/// of the last (the ellipsis of a cut text too), a little under their baseline, in the text's
/// colour and as thick as the text is large, so it reads in either appearance and at every zoom.
pub fn text_marked(painter: &egui::Painter, at: Pos2, text: &str, ty: Type, color: Colour, width: Option<f32>, marks: Marks) -> Rect {
    let g = laid_out(painter, text, ty, color, width, 1, marks.slanted);
    let rect = Rect::from_min_size(at, g.size());
    if marks.underlined {
        for row in &g.rows {
            let (Some(first), Some(last)) = (row.glyphs.first(), row.glyphs.last()) else { continue };
            underline(painter, at, row, first, last, ty, color);
        }
    }
    painter.galley(at, g, color.into());
    rect
}

/// Underlines the character at `index` of `g`, laid out to be drawn from `at`, as [`text_marked`]
/// underlines a text: a menu's access letter. Nothing when `g` was cut before that character.
pub fn underline_char(painter: &egui::Painter, at: Pos2, g: &egui::Galley, index: usize, ty: Type, color: Colour) {
    let mut glyphs = g.rows.iter().flat_map(|row| row.glyphs.iter().map(move |glyph| (row, glyph)));
    // A cut text holds the glyphs before the cut and then its ellipsis.
    if let Some((row, glyph)) = glyphs.nth(index).filter(|(_, glyph)| glyph.chr != '…') {
        underline(painter, at, row, glyph, glyph, ty, color);
    }
}

/// The underline from the glyph `first` to the end of `last` of `row`, a text drawn from `at`.
fn underline(
    painter: &egui::Painter,
    at: Pos2,
    row: &egui::epaint::text::PlacedRow,
    first: &egui::epaint::text::Glyph,
    last: &egui::epaint::text::Glyph,
    ty: Type,
    color: Colour,
) {
    let (left, right) = (at.x + row.pos.x + first.pos.x, at.x + row.pos.x + last.max_x());
    let thickness = (ty.size * UNDERLINE_WEIGHT).max(1.0);
    let y = painter.round_to_pixel_center(at.y + row.pos.y + first.pos.y + ty.size * UNDERLINE_DROP);
    painter.hline(left..=right, y, egui::Stroke::new(thickness, color));
}

/// An underline's thickness and its distance under the baseline, each a share of the text's size:
/// a point at the design's 13-point member text.
const UNDERLINE_WEIGHT: f32 = 0.075;
const UNDERLINE_DROP: f32 = 0.15;

/// Lays `text` out in `ty` and `color`, slanted when `italics`, wrapped at `width` into at most
/// `rows` lines (at least one), the last cut with an ellipsis.
fn laid_out(
    painter: &egui::Painter,
    text: &str,
    ty: Type,
    color: Colour,
    width: Option<f32>,
    rows: usize,
    italics: bool,
) -> std::sync::Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::single_section(
        text.to_owned(),
        egui::TextFormat {
            font_id: ty.font(),
            color: color.into(),
            extra_letter_spacing: ty.spacing,
            line_height: Some(ty.line),
            valign: egui::Align::Center,
            italics,
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
pub fn text(painter: &egui::Painter, at: Pos2, text: &str, ty: Type, color: Colour, width: Option<f32>) -> Rect {
    let g = galley(painter, text, ty, color, width);
    let rect = Rect::from_min_size(at, g.size());
    painter.galley(at, g, color.into());
    rect
}

/// Where a list of `size` opens from `anchor`, `left` its left edge: under the anchor, or above it
/// when the window has no room below and has room above, so no entry is cut off.
pub fn drop_rect(ui: &egui::Ui, anchor: Rect, left: f32, size: egui::Vec2) -> Rect {
    let window = ui.ctx().content_rect();
    let below = Rect::from_min_size(egui::pos2(left, anchor.bottom() + 4.0), size);
    let above = Rect::from_min_size(egui::pos2(left, anchor.top() - 4.0 - size.y), size);
    if below.bottom() > window.bottom() - 8.0 && above.top() >= window.top() + 8.0 { above } else { below }
}

/// Runs `add` on a layer above the window's content, covering `r`: a menu or a list drawn there is
/// never painted over by what the frame draws after it, and the clicks over it are its own. The
/// layer is told its size, so it shows and takes clicks on its first frame.
pub fn overlay<R>(ui: &egui::Ui, id: egui::Id, r: Rect, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let ctx = ui.ctx().clone();
    let layer = egui::LayerId::new(egui::Order::Foreground, id);
    let state = egui::AreaState {
        pivot_pos: Some(r.min),
        pivot: egui::Align2::LEFT_TOP,
        size: Some(r.size()),
        interactable: true,
        last_became_visible_at: None,
    };
    ctx.memory_mut(|m| m.areas_mut().set_state(layer, state));
    egui::Area::new(id)
        .order(egui::Order::Foreground)
        .fixed_pos(r.min)
        .constrain(false)
        .fade_in(false)
        .show(&ctx, |ui| {
            // The shadow reaches past the menu.
            ui.set_clip_rect(ctx.content_rect());
            ui.allocate_rect(r, egui::Sense::hover());
            add(ui)
        })
        .inner
}

/// A floating surface's soft shadow (menus, popovers, the toolbar, the minimap, dialogs), drawn
/// before the surface: blurred and offset down, never a solid ring.
pub fn shadow(painter: &egui::Painter, rect: Rect, radius: f32, color: Colour) {
    let shape = egui::epaint::Shadow { offset: [0, 4], blur: 16, spread: 0, color: color.gamma_multiply(0.7).into() }
        .as_shape(rect, egui::CornerRadius::same(radius.round().clamp(0.0, 255.0) as u8));
    painter.add(shape);
}

/// Draws `text` in the monospace face at `size`, starting at `x` and centred on the line `y`: a
/// command to type or a code to enter, set apart from the interface's words. Cut at `width`.
pub fn code_mid(painter: &egui::Painter, x: f32, y: f32, text: &str, size: f32, color: Colour, width: Option<f32>) -> Rect {
    let mut job = egui::text::LayoutJob::single_section(
        text.to_owned(),
        egui::TextFormat {
            font_id: FontId::new(size, FontFamily::Monospace),
            color: color.into(),
            valign: egui::Align::Center,
            ..Default::default()
        },
    );
    job.wrap =
        egui::text::TextWrapping { max_width: width.unwrap_or(f32::INFINITY), max_rows: 1, break_anywhere: true, overflow_character: Some('…') };
    let g = painter.layout_job(job);
    let at = Pos2::new(x, y - g.size().y / 2.0);
    let rect = Rect::from_min_size(at, g.size());
    painter.galley(at, g, color.into());
    rect
}

/// Draws `text` starting at `x`, centred on the line `y`, as every row centres its content.
pub fn text_mid(painter: &egui::Painter, x: f32, y: f32, text: &str, ty: Type, color: Colour, width: Option<f32>) -> Rect {
    let g = galley(painter, text, ty, color, width);
    let at = Pos2::new(x, y - g.size().y / 2.0);
    let rect = Rect::from_min_size(at, g.size());
    painter.galley(at, g, color.into());
    rect
}

/// A path of `parts` joined by `sep`, as it fits where `fits` says: whole; else its first parts
/// folded into `…` (`… › Orders`), so the last part, the one it names, stays; else that last part
/// alone, for the caller to cut at its end as any name is (`Commerce pl…`). The parts are given,
/// never found by splitting the text, so a name holding `sep` stays one part. Gives the text and
/// whether it was cut.
pub fn cut_front<S: AsRef<str>>(parts: &[S], sep: &str, fits: impl Fn(&str) -> bool) -> (String, bool) {
    let joined = |from: usize| parts.iter().skip(from).map(AsRef::as_ref).collect::<Vec<&str>>().join(sep);
    let whole = joined(0);
    if fits(&whole) {
        return (whole, false);
    }
    for k in 1..parts.len() {
        let folded = format!("…{sep}{}", joined(k));
        if fits(&folded) {
            return (folded, true);
        }
    }
    (parts.last().map(|p| p.as_ref().to_owned()).unwrap_or_default(), true)
}

/// Draws the path of `parts` joined by `sep` as [`text_mid`] does, cut from its front when it is
/// wider than `width` ([`cut_front`]), so its last part shows, that part cut at its end when it
/// alone is wider; gives the box it took and whether it was cut.
#[allow(clippy::too_many_arguments, reason = "a path is drawn as text_mid draws, from its parts and their separator")]
pub fn text_mid_front<S: AsRef<str>>(
    painter: &egui::Painter,
    x: f32,
    y: f32,
    parts: &[S],
    sep: &str,
    ty: Type,
    color: Colour,
    width: f32,
) -> (Rect, bool) {
    let fits = |t: &str| galley(painter, t, ty, color, None).size().x <= width;
    let (shown, cut) = cut_front(parts, sep, fits);
    (text_mid(painter, x, y, &shown, ty, color, Some(width.max(0.0))), cut)
}

/// Draws `icon` centred in a coloured tile: its longer side half the tile's, the interface's
/// stroke weight.
pub fn icon_in_tile(painter: &egui::Painter, tile: Rect, icon: &Icon, color: Colour) {
    let side = (tile.width().min(tile.height()) / 2.0).round();
    let k = side / icon.size[0].max(icon.size[1]).max(f32::EPSILON);
    let size = Vec2::new(icon.size[0], icon.size[1]) * k;
    icon_exact(painter, tile.center() - size / 2.0, icon, size.x, STROKE, color);
}

/// Draws `icon` centred on `centre` at its role's size.
pub fn icon_mid(painter: &egui::Painter, centre: Pos2, icon: &Icon, color: Colour) {
    icon_in(painter, Rect::from_center_size(centre, Vec2::new(icon.size[0], icon.size[1])), icon, color);
}

/// One stroke of an icon, in the icon's own box.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Seg {
    L([f32; 2], [f32; 2]),
    C([f32; 2], [f32; 2], [f32; 2], [f32; 2]),
}

/// An icon as the design draws it: strokes in a box of `size`, `stroke` wide, with round ends and
/// joins. `standard_size` is the longer side, in points, an interface icon is drawn at (chrome
/// 15-17, rows and fields 13-15, small actions 10-14, chevrons 8); none for the canvas's own
/// glyphs, the brand mark and illustrations, which keep the size they are given.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Icon {
    pub size: [f32; 2],
    pub stroke: f32,
    pub standard_size: Option<f32>,
    pub segments: &'static [Seg],
}

/// Draws `icon` with its box's top-left corner at `at`, `width` wide (its height in proportion),
/// in `color`, with the design's stroke weight whatever the size. The round ends and joins are
/// discs at every end point, so a stroke of no length draws a dot.
pub fn icon(painter: &egui::Painter, at: Pos2, icon: &Icon, width: f32, color: Colour) {
    icon_weighted(painter, at, icon, width, icon.stroke, color);
}

/// [`icon`] with a stroke weight of its own, where the design gives an instance one; an icon with
/// a standard size is drawn at that size instead, centred on the box asked for, with the
/// interface's stroke weight ([`STROKE`]).
pub fn icon_weighted(painter: &egui::Painter, at: Pos2, icon: &Icon, width: f32, weight: f32, color: Colour) {
    // An interface icon is drawn at its standard size, centred where the caller put its box, with
    // one stroke weight, so no screen draws one larger or heavier than another.
    let (at, width, weight) = match icon.standard_size {
        Some(side) => {
            let requested = Vec2::new(width, if icon.size[0] > 0.0 { width * icon.size[1] / icon.size[0] } else { width });
            let k = side / icon.size[0].max(icon.size[1]).max(f32::EPSILON);
            let drawn = Vec2::new(icon.size[0], icon.size[1]) * k;
            (at + requested / 2.0 - drawn / 2.0, drawn.x, STROKE)
        }
        None => (at, width, weight),
    };
    icon_exact(painter, at, icon, width, weight, color);
}

/// The stroke weight of every interface icon: two pixels on a retina screen, one on a standard
/// one.
pub const STROKE: f32 = 1.2;

/// [`icon_weighted`] at exactly the size and weight given: the canvas's zoomed glyphs and
/// illustrations.
pub fn icon_exact(painter: &egui::Painter, at: Pos2, icon: &Icon, width: f32, weight: f32, color: Colour) {
    let scale = if icon.size[0] > 0.0 { width / icon.size[0] } else { 1.0 };
    // Crisp at the screen's pixels as a mask; only an icon too large for one draws as strokes.
    if raster::draw(painter, at, icon, scale, weight, color.into()) {
        return;
    }
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
pub fn icon_in(painter: &egui::Painter, rect: Rect, icon_: &Icon, color: Colour) {
    let size = Vec2::new(icon_.size[0], icon_.size[1]);
    icon(painter, rect.center() - size / 2.0, icon_, icon_.size[0], color);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The boxes of what `draw` paints in one frame.
    fn painted(mut draw: impl FnMut(&egui::Painter)) -> Vec<Rect> {
        let mut output = egui::Context::default().run_ui(egui::RawInput::default(), |ui| draw(&ui.ctx().layer_painter(egui::LayerId::background())));
        // The masks' textures go nowhere without a renderer.
        output.textures_delta.clear();
        output.shapes.iter().map(|s| s.shape.visual_bounding_rect()).collect()
    }

    /// An icon with a standard size is drawn at it, centred on the box asked for, with the
    /// interface's stroke weight, whatever width and weight it is asked for; the same strokes
    /// without one are drawn at the width and weight asked for.
    #[test]
    fn an_icon_is_drawn_at_its_standard_size_or_as_asked_without_one() {
        assert_eq!(icons::PLUS.standard_size, Some(10.0));
        let asked = painted(|p| icon_weighted(p, Pos2::ZERO, &icons::PLUS, 18.0, 3.0, MARK));
        let standard = painted(|p| icon_exact(p, Pos2::new(4.0, 4.0), &icons::PLUS, 10.0, STROKE, MARK));
        assert_eq!(asked, standard, "ten points wide in the middle of eighteen, at the interface's weight");
        let plain = Icon { standard_size: None, ..icons::PLUS };
        let asked = painted(|p| icon_weighted(p, Pos2::ZERO, &plain, 18.0, 3.0, MARK));
        let as_given = painted(|p| icon_exact(p, Pos2::ZERO, &plain, 18.0, 3.0, MARK));
        assert_eq!(asked, as_given, "eighteen points wide at the weight asked for");
        assert_ne!(asked, standard);
    }
}
