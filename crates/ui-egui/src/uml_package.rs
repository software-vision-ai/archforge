//! UML's package card on the canvas (AR-113, AR-565), which UML's diagram kind draws itself
//! (`DiagramKind::draw_card`): a card with a tab over its top left, as a folder's, that holds its
//! name, and the name typed in place there, the tab growing with it (`DiagramKind::name_field`);
//! the path of the packages that own it under the card, which a screen reader and the pointer read
//! whole; and inside the card a thumbnail of the package's own diagram, its cards and lines scaled
//! into it, or Empty diagram.

use archforge_diagram::figure::Tone;
use archforge_diagram::grid::bounds_of;
use archforge_diagram::{DIAGRAM, diagram, node};
use archforge_mof::{Obj, Store, Uuid};
use egui::{Pos2, Rect, Stroke, StrokeKind, pos2, vec2};

use crate::canvas::{
    CARD_FIRST_ROW, CARD_HEADER, CARD_INSET, CARD_ROW, COMPARTMENT_GAP, CardDrawn, CardLabel, CardPart, DETAIL_ZOOM, Drawing, card_palette,
    dashed_rect, element_of, name_from_afar,
};
use crate::i18n::{fmt, tr};
use crate::panels::PATH_SEP;
use crate::ux::{self, Palette, types};
use crate::widgets::{self, Registry};

/// Whether the element `e` is a package, whose card UML draws itself.
fn is_package(store: &Store, e: Obj) -> bool {
    store.class_of(e).ok() == Some(archforge_uml::PACKAGE)
}

/// Draws a package's card at `c.rect` (AR-113): its tab holding its name, the name as it is typed
/// while it is renamed, its card joined to the tab, dimmed and dashed for a package of another
/// scope, in the node's own look; the path of its owners under it (AR-565); and the thumbnail of
/// its own diagram inside it, or its name alone below [`DETAIL_ZOOM`]. Records the tab, the path
/// and the thumbnail in `registry`, the tab as a part of the card that takes a press and says the
/// whole name where it shows it cut, and the path as a label. False for any other card, which the
/// canvas draws.
pub(crate) fn draw_card(paint: &egui::Painter, d: &Drawing<'_>, c: &CardDrawn<'_>, registry: &mut Registry, ui: &egui::Ui) -> bool {
    if !is_package(d.store, c.element) {
        return false;
    }
    let (card, rect, z) = (c.card, c.rect, d.view.zoom);
    let id = d.store.uuid(c.node).ok();
    let renaming = id.is_some() && id == d.renaming;
    // Clipped to the card as drawn.
    let inner = paint.with_clip_rect(rect.intersect(paint.clip_rect()));
    let (p, border) = card_palette(d.p, d.store, c.node, card.elsewhere, true, z);
    let p = &p;
    // Its tab, over its top left, holding its name: the name as it is typed while it is renamed.
    let name = if renaming { d.rename_text.unwrap_or(&card.name) } else { card.name.as_str() };
    let (tab, shown, cut) = package_tab(paint, rect, name, z, p.text);
    paint.rect_filled(tab, 5.0 * z, p.card);
    paint.rect_stroke(tab, 5.0 * z, Stroke::new(border, if c.selected { p.accent } else { p.card_edge }), StrokeKind::Inside);
    paint.rect_filled(rect, 6.0 * z, p.card);
    if card.elsewhere {
        dashed_rect(paint, rect, Stroke::new(border, p.card_edge), 4.0 * z, 3.0 * z);
    } else {
        paint.rect_stroke(rect, 6.0 * z, Stroke::new(border, p.card_edge), StrokeKind::Inside);
    }
    // The tab joins the card: no border between them.
    paint.rect_filled(Rect::from_min_max(pos2(tab.left() + border, rect.top() - z), pos2(tab.right() - border, rect.top() + border)), 0.0, p.card);
    if z >= DETAIL_ZOOM && !renaming {
        paint.galley(tab.min + vec2(PACKAGE_TAB_PAD * z, (tab.height() - shown.size().y) / 2.0), shown, p.text.into());
    }
    if let Some(id) = id {
        widgets::record_visible(ui, registry, &format!("workspace.canvas.node.{id}.tab"), "tab", tab);
        d.parts.borrow_mut().push(CardPart { rect: tab, node: id, hover: cut.then(|| card.name.clone()) });
    }
    // Where the package lives, under its card; the root, which no package owns, has none.
    if z >= DETAIL_ZOOM
        && !card.owner_path.is_empty()
        && let Some(id) = id
    {
        let badge = path_badge(paint, rect, &card.owner_path, z, d.p.text_secondary, id);
        widgets::record_visible(ui, registry, &badge.id, "label", badge.rect);
        d.labels.borrow_mut().push(badge);
    }
    if z >= DETAIL_ZOOM {
        // The package's own diagram, scaled into its card.
        let area = draw_preview(&inner, d, c.element, rect, z);
        if let Some(id) = id {
            widgets::record_visible(ui, registry, &format!("workspace.canvas.node.{id}.preview"), "image", area);
        }
    } else {
        name_from_afar(&inner, &card.name, rect, z, p.text);
    }
    true
}

/// Where the name typed in place on the node `n`'s card at `rect` goes while it reads `text`: in
/// a package's tab, which grows with it, in the tab's style (AR-113); none for any other card.
pub(crate) fn name_field(paint: &egui::Painter, store: &Store, n: Obj, rect: Rect, text: &str, z: f32) -> Option<(Rect, ux::Type)> {
    element_of(store, n, node::ELEMENT).filter(|e| is_package(store, *e))?;
    // Laid out for its size alone: the field draws the text itself.
    let (tab, _, _) = package_tab(paint, rect, text, z, ux::CLEAR);
    Some((tab.shrink2(vec2(4.0, 3.0) * z), types::SMALL_STRONG.scaled(z)))
}

/// A package's tab: its height, the room either side of its name and its least width (AR-113).
const PACKAGE_TAB_HEIGHT: f32 = 22.0;
const PACKAGE_TAB_PAD: f32 = 12.0;
const PACKAGE_TAB_MIN: f32 = 56.0;
/// The margin between a package's card and the thumbnail of its diagram, and the room inside the
/// thumbnail around what it shows.
const PREVIEW_MARGIN: f32 = 10.0;
const PREVIEW_INSET: f32 = 8.0;
/// The most a thumbnail enlarges its diagram, so a few small cards still read as a thumbnail.
const PREVIEW_MOST: f32 = 0.5;

/// The tab over the top left of the package card at `rect` (AR-113): as wide as `name` and its
/// padding, never narrower than [`PACKAGE_TAB_MIN`] nor wider than the card, with the name laid out to
/// fit it (cut with an ellipsis where the card is narrower than the name), and whether it was cut.
fn package_tab(paint: &egui::Painter, rect: Rect, name: &str, z: f32, ink: ux::Colour) -> (Rect, std::sync::Arc<egui::Galley>, bool) {
    let ty = types::SMALL_STRONG.scaled(z);
    let pad = PACKAGE_TAB_PAD * z;
    let whole = ux::galley(paint, name, ty, ink, None);
    let room = (rect.width() - 2.0 * pad).max(0.0);
    let cut = whole.size().x > room;
    let shown = if cut { ux::galley(paint, name, ty, ink, Some(room)) } else { whole };
    let width = (shown.size().x + 2.0 * pad).max(PACKAGE_TAB_MIN * z).min(rect.width().max(0.0));
    let tab = Rect::from_min_size(pos2(rect.left(), rect.top() - (PACKAGE_TAB_HEIGHT - 1.0) * z), vec2(width, PACKAGE_TAB_HEIGHT * z));
    (tab, shown, cut)
}

/// How far a package's owning path is under its card.
const PATH_BADGE_GAP: f32 = 6.0;

/// Draws a package's owning path under its card (AR-565), `Owner: Commerce platform › Operations`, in the
/// caption style and the canvas's secondary ink: its words say what it is, never its colour alone.
/// It is as wide as the card at most: its first owners fold into `…` so the nearest stay, and the
/// nearest alone is cut at its end ([`ux::cut_front`]).
fn path_badge(paint: &egui::Painter, card: Rect, owners: &[String], z: f32, ink: ux::Colour, node: Uuid) -> CardLabel {
    let ty = types::CAPTION.scaled(z);
    let template = tr("Owner: {path}");
    let say = |path: &str| fmt(&template, &[("path", path)]);
    let width = card.width().max(0.0);
    let (shown, _) = ux::cut_front(owners, PATH_SEP, |t| ux::galley(paint, &say(t), ty, ink, None).size().x <= width);
    let shown = say(&shown);
    let rect = ux::text(paint, pos2(card.left(), card.bottom() + PATH_BADGE_GAP * z), &shown, ty, ink, Some(width));
    CardLabel { rect, node, id: format!("workspace.canvas.node.{node}.path"), shown, whole: say(&owners.join(PATH_SEP)) }
}

/// How a card of a thumbnail is drawn: as the card it stands for is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MiniLook {
    Plain,
    Tinted,
    Tabbed,
    Note,
    /// A constraint note and a constraint's braces (AR-133).
    Constraint,
    Braces,
    /// An n-ary association's diamond (AR-128).
    Diamond,
}

/// One card of a thumbnail, on screen: its look and how many rows each of its compartments has.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct MiniCard {
    pub rect: Rect,
    pub look: MiniLook,
    pub rows: Vec<usize>,
}

/// A diagram scaled into a thumbnail (AR-113): its cards and its lines as they are laid out, and
/// the scale it was drawn at.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Preview {
    pub cards: Vec<MiniCard>,
    pub lines: Vec<Vec<Pos2>>,
    pub scale: f32,
}

/// The diagram each element leads to, by the element: a package's own class diagram.
fn leads(store: &Store) -> std::collections::HashMap<Obj, Obj> {
    let mut out = std::collections::HashMap::new();
    for d in store.alive().filter(|o| store.class_of(*o).ok() == Some(DIAGRAM)) {
        if let Some(e) = element_of(store, d, diagram::ELEMENT) {
            out.entry(e).or_insert(d);
        }
    }
    out
}

/// `d` scaled to fit `area`, centred, enlarged at most [`PREVIEW_MOST`]: none when it shows
/// nothing.
pub(crate) fn preview(store: &Store, notations: &archforge_diagram::ProjectView, d: Obj, area: Rect) -> Option<Preview> {
    let grid = crate::canvas::grid(store, notations, d);
    let all = grid.visible(archforge_diagram::Rect::new(-1.0e9, -1.0e9, 2.0e9, 2.0e9));
    let mut boxes: Vec<(Rect, MiniLook, Vec<usize>)> = Vec::new();
    for n in &all.nodes {
        let b = bounds_of(store, *n);
        let r = Rect::from_min_size(pos2(b.x as f32, b.y as f32), vec2(b.width as f32, b.height as f32));
        if !r.is_finite() || r.width() <= 0.0 || r.height() <= 0.0 {
            continue;
        }
        let (look, rows) = match element_of(store, *n, node::ELEMENT) {
            None => (MiniLook::Note, Vec::new()),
            Some(e) => match notations.of_element(store, e).and_then(|nt| nt.card(store, e, d)) {
                Some(card) => {
                    let look = match card.tone {
                        Tone::Tabbed => MiniLook::Tabbed,
                        Tone::Plain => MiniLook::Plain,
                        Tone::Tinted => MiniLook::Tinted,
                        Tone::Constraint => MiniLook::Constraint,
                        Tone::Braces => MiniLook::Braces,
                        Tone::Diamond => MiniLook::Diamond,
                    };
                    (look, card.compartments.iter().map(Vec::len).collect())
                }
                None => (MiniLook::Plain, Vec::new()),
            },
        };
        boxes.push((r, look, rows));
    }
    // Every line's route in one pass over the index, not a lookup per line.
    let lines: Vec<Vec<Pos2>> = grid
        .lines()
        .map(|(_, points)| points.iter().map(|(x, y)| pos2(*x as f32, *y as f32)).collect::<Vec<_>>())
        .filter(|points| points.len() >= 2 && points.iter().all(|q| q.is_finite()))
        .collect();
    // A package's tab stands above its card, so it is in the bounds too.
    let mut bounds = boxes
        .iter()
        .map(|(r, look, _)| if *look == MiniLook::Tabbed { r.with_min_y(r.top() - PACKAGE_TAB_HEIGHT) } else { *r })
        .reduce(|a, b| a.union(b))?;
    for q in lines.iter().flatten() {
        bounds.extend_with(*q);
    }
    let scale = (area.width() / bounds.width().max(1.0)).min(area.height() / bounds.height().max(1.0)).min(PREVIEW_MOST);
    if !scale.is_finite() || scale <= 0.0 {
        return None;
    }
    let origin = area.center() - bounds.size() * scale / 2.0;
    let map = |q: Pos2| origin + (q - bounds.min) * scale;
    let cards = boxes.into_iter().map(|(r, look, rows)| MiniCard { rect: Rect::from_min_max(map(r.min), map(r.max)), look, rows }).collect();
    let lines = lines.into_iter().map(|l| l.into_iter().map(map).collect()).collect();
    Some(Preview { cards, lines, scale })
}

/// The thumbnail of the package `package`'s own diagram on its card at `rect` (AR-113), on the
/// page tint inside the card's margin; Empty diagram when the diagram shows nothing or there is
/// none. Gives the thumbnail's area.
fn draw_preview(paint: &egui::Painter, d: &Drawing<'_>, package: Obj, rect: Rect, z: f32) -> Rect {
    let p = d.p;
    let area = rect.shrink(PREVIEW_MARGIN * z);
    if area.width() <= 0.0 || area.height() <= 0.0 {
        return area;
    }
    paint.rect_filled(area, 4.0 * z, p.page);
    let led = d.led.get_or_init(|| leads(d.store));
    let shown = led.get(&package).and_then(|g| preview(d.store, d.notations, *g, area.shrink(PREVIEW_INSET * z)));
    let Some(shown) = shown else {
        let g = ux::galley(paint, &tr("Empty diagram"), types::CAPTION.scaled(z), p.text_muted, Some(area.width()));
        paint.galley(area.center() - g.size() / 2.0, g, p.text_muted.into());
        return area;
    };
    let clip = paint.with_clip_rect(area.intersect(paint.clip_rect()));
    for line in &shown.lines {
        clip.add(egui::Shape::line(line.clone(), Stroke::new(1.0, p.text_secondary)));
    }
    for c in &shown.cards {
        // The preview's scale is already the screen's: the area it was fitted to is zoomed.
        mini_card(&clip, p, c, shown.scale);
    }
    area
}

/// One card of a thumbnail drawn at `s` (its diagram's scale on screen): its outline, a bar for
/// its name, a line under its header, and a bar for each row that fits.
fn mini_card(paint: &egui::Painter, p: &Palette, c: &MiniCard, s: f32) {
    let r = c.rect;
    if c.look == MiniLook::Braces {
        paint.rect_filled(Rect::from_min_size(r.left_top(), vec2(r.width(), (5.0 * s).max(1.0))), 1.0, p.purple);
        return;
    }
    // An n-ary association's diamond, hollow in the lines' ink (AR-128).
    if c.look == MiniLook::Diamond {
        let points = vec![r.left_center(), r.center_top(), r.right_center(), r.center_bottom()];
        paint.add(egui::Shape::convex_polygon(points, p.surface, Stroke::new(1.0, p.text_secondary)));
        return;
    }
    let (fill, edge) = match c.look {
        MiniLook::Tinted => (p.card, p.card_edge),
        MiniLook::Note => (p.amber_tint, p.amber),
        MiniLook::Constraint => (p.surface, p.purple),
        MiniLook::Plain | MiniLook::Tabbed | MiniLook::Braces | MiniLook::Diamond => (p.surface, p.border),
    };
    if c.look == MiniLook::Tabbed {
        let tab =
            Rect::from_min_size(pos2(r.left(), r.top() - (PACKAGE_TAB_HEIGHT - 1.0) * s), vec2((r.width() * 0.4).max(2.0), PACKAGE_TAB_HEIGHT * s));
        paint.rect_filled(tab, 2.0, fill);
        paint.rect_stroke(tab, 2.0, Stroke::new(1.0, edge), StrokeKind::Inside);
    }
    paint.rect_filled(r, 2.0, fill);
    paint.rect_stroke(r, 2.0, Stroke::new(1.0, edge), StrokeKind::Inside);
    if matches!(c.look, MiniLook::Note | MiniLook::Tabbed | MiniLook::Constraint) {
        return;
    }
    let bar = |y: f32, w: f32, h: f32, colour: ux::Colour| {
        if y + h <= r.bottom() - 1.0 {
            paint.rect_filled(Rect::from_min_size(pos2(r.left() + CARD_INSET * s, y), vec2((r.width() * w).max(1.0), h.max(1.0))), 1.0, colour);
        }
    };
    bar(r.top() + 30.0 * s, 0.5, 8.0 * s, p.text_muted);
    let header = r.top() + CARD_HEADER * s;
    if header < r.bottom() {
        paint.rect_filled(Rect::from_min_size(pos2(r.left(), header), vec2(r.width(), 1.0)), 0.0, edge);
    }
    let mut y = r.top() + CARD_FIRST_ROW * s;
    for (i, rows) in c.rows.iter().enumerate() {
        if i > 0 {
            y += COMPARTMENT_GAP * CARD_ROW * s;
        }
        for _ in 0..*rows {
            bar(y + 4.0 * s, 0.45, 5.0 * s, p.border);
            y += CARD_ROW * s;
        }
    }
}
