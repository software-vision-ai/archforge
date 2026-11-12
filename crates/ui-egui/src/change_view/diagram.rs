//! A changed diagram's Before and After (frame V3.4 of page 09): what differs in the model and in
//! the layout, said apart, then each side drawn from its own state in one shared framing, so a box
//! that stayed is in the same place on both, with what differs marked.

use std::collections::{BTreeMap, HashMap};

use archforge_diagram::grid::{bounds_of, middle_of};
use archforge_diagram::{LabelPlace, diagram, edge, label, node, node_tree};
use archforge_mof::{Obj, Uuid, Value};
use archforge_notations::differences::{Apart, ChangeKind, DiagramSide, DiagramSides, Difference, Placing, shown_name};
use egui::{Align2, CornerRadius, Key, Pos2, Rect, Response, Sense, Shape, Stroke, StrokeKind, Vec2, pos2, vec2};

use super::cards::{self, BAND, EYEBROW, GUTTER};
use super::related;
use crate::Shell;
use crate::canvas::{Live, Scene, State, View, ZOOM_RANGE, draw_scene, fit, real, refs};
use crate::i18n::{fmt, tr, trn};
use crate::settings::{control, focus_ring};
use crate::ux::{self, Palette, types};
use crate::version_details::MARGIN;
use crate::versions::kind_look;
use crate::widgets::{Registry, egui_id, record_visible};

/// A mark's width around a box or along a line.
const MARK: f32 = 2.0;
/// A mark's label's room around its word.
const LABEL_PAD: Vec2 = vec2(6.0, 2.0);
/// The Model and Layout cards' height, the gap between them and the room under them.
const APART: f32 = 92.0;
const APART_GAP: f32 = 16.0;
const UNDER_APART: f32 = 18.0;
/// How far an arrow key moves both sides, and how much + and − zoom them.
const STEP: f32 = 40.0;
const ZOOM_STEP: f32 = 1.25;

/// Draws the two sides side by side from `top` to the end of `r`. Everything comes from the sides
/// already held: nothing here asks the host or touches the session.
pub(super) fn draw(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect, top: f32, sides: &DiagramSides) {
    let area = Rect::from_min_max(pos2(r.left(), top), pos2(r.right(), r.bottom().max(top)));
    let mut ui = ui.new_child(egui::UiBuilder::new().max_rect(area));
    ui.set_clip_rect(area.intersect(ui.clip_rect()));
    let card = vec2(((r.width() - 2.0 * MARGIN - APART_GAP) / 2.0).max(0.0), APART);
    for (i, (area, words)) in ["model", "layout"].into_iter().zip(apart_words(&sides.apart)).enumerate() {
        let at = Rect::from_min_size(pos2(r.left() + MARGIN + i as f32 * (card.x + APART_GAP), top), card);
        said_apart(shell, &ui, p, (area, at), &words);
    }
    let top = top + APART + UNDER_APART;
    let each = ((r.width() - 2.0 * MARGIN - GUTTER) / 2.0).max(0.0);
    let size = vec2(each, (r.bottom() - MARGIN - top - EYEBROW).max(0.0));
    let before = Rect::from_min_size(pos2(r.left() + MARGIN, top + EYEBROW), size);
    let after = before.translate(vec2(each + GUTTER, 0.0));
    let fitted = framing(sides, vec2(each, (size.y - BAND).max(0.0)));
    let (framing, [on_before, on_after]) =
        navigate(shell, &ui, [("before", before, sides.before.is_some()), ("after", after, sides.after.is_some())], fitted);
    let (head, mut go) = (cards::head_before(shell), None);
    pane(shell, &mut ui, p, (before, true, on_before), (sides, framing), &head, &mut go);
    pane(shell, &mut ui, p, (after, false, on_after), (sides, framing), &tr("Current work"), &mut go);
    if let Some(row) = go {
        super::open(shell, row);
    }
}

/// The zoom and pan both sides are drawn with, and the pane of each side in `panes` the diagram is
/// on as a control. Both start at `fitted` and keep what the person makes of them until the diagram
/// closes: a drag or the wheel over either pane moves both, a pinch or the wheel with the command key
/// zooms both about the pointer, and a pane with the keys moves both by the arrows, zooms by + and −
/// and fits both again by 0. Each pane is read before either is drawn, so the two never differ.
fn navigate(shell: &mut Shell, ui: &egui::Ui, panes: [(&str, Rect, bool); 2], fitted: (f32, Vec2)) -> ((f32, Vec2), [Option<Response>; 2]) {
    let shown = shell.versions.element.as_ref().and_then(|o| o.nav);
    let (mut zoom, mut pan) = shown.unwrap_or(fitted);
    let mut fit_again = false;
    let said = tr("Drag, scroll or use the arrow keys to move both sides; + and − zoom them, and 0 fits them again.");
    let panes = panes.map(|(side, card, on)| {
        let id = format!("workspace.change-view.{side}");
        on.then(|| {
            record_visible(ui, &mut shell.registry, &id, "canvas", card);
            let resp = ui.interact(card, egui_id(&id), Sense::click_and_drag()).on_hover_text(&said);
            if resp.clicked() || resp.drag_started() {
                resp.request_focus();
            }
            let body = body_of(card);
            pan += resp.drag_delta();
            if ui.rect_contains_pointer(body) {
                let (scroll, pinch, at) = ui.input(|i| (i.smooth_scroll_delta, i.zoom_delta(), i.pointer.hover_pos()));
                (zoom, pan) = zoomed((zoom, pan), pinch, at.map_or(body.center(), |a| a) - body.min, fitted.0);
                pan += if pinch == 1.0 { scroll } else { Vec2::ZERO };
            }
            if resp.has_focus() {
                ui.memory_mut(|m| {
                    m.set_focus_lock_filter(resp.id, egui::EventFilter { horizontal_arrows: true, vertical_arrows: true, ..Default::default() })
                });
                let (by, factor, fit) = keys(ui);
                (zoom, pan) = zoomed((zoom, pan + by), factor, body.size() / 2.0, fitted.0);
                fit_again |= fit;
            }
            resp
        })
    });
    let now =
        Some((zoom, pan)).filter(|(z, p)| !fit_again && z.is_finite() && p.is_finite() && (shown.is_some() || (z, p) != (&fitted.0, &fitted.1)));
    if let Some(o) = shell.versions.element.as_mut() {
        o.nav = now;
    }
    (now.unwrap_or(fitted), panes)
}

/// `(zoom, pan)` zoomed by `factor` about `at`, a point of a pane, so what is under it stays there:
/// out no further than `least` or the canvas's least zoom, whichever is less, and in no further than the canvas's most.
fn zoomed((zoom, pan): (f32, Vec2), factor: f32, at: Vec2, least: f32) -> (f32, Vec2) {
    if !factor.is_finite() || factor <= 0.0 || factor == 1.0 {
        return (zoom, pan);
    }
    let new = (zoom * factor).clamp(least.min(ZOOM_RANGE.0), ZOOM_RANGE.1);
    (new, at - (at - pan) * (new / zoom))
}

/// What the keys pressed this frame ask of a pane that has them: how far to move both sides, by how
/// much to zoom them, and whether to fit them again. Only without the command, control or option
/// key, so the window's chords on the same keys stay the window's.
fn keys(ui: &egui::Ui) -> (Vec2, f32, bool) {
    ui.input(|i| {
        let (mut by, mut factor, mut fit) = (Vec2::ZERO, 1.0, false);
        for e in &i.events {
            let egui::Event::Key { key, pressed: true, modifiers, .. } = e else { continue };
            if modifiers.command || modifiers.ctrl || modifiers.mac_cmd || modifiers.alt {
                continue;
            }
            match key {
                Key::ArrowLeft => by.x += STEP,
                Key::ArrowRight => by.x -= STEP,
                Key::ArrowUp => by.y += STEP,
                Key::ArrowDown => by.y -= STEP,
                Key::Plus | Key::Equals => factor *= ZOOM_STEP,
                Key::Minus => factor /= ZOOM_STEP,
                Key::Num0 => fit = true,
                _ => {}
            }
        }
        (by, factor, fit)
    })
}

/// The part of a side's pane `card` the diagram is drawn in, under its band.
fn body_of(card: Rect) -> Rect {
    Rect::from_min_max(pos2(card.left(), (card.top() + BAND).min(card.bottom())), card.max)
}

/// One of the cards above the sides in `at`, registered under `area`: what it is about, its headline
/// and the names or sentence under it, read whole on hover. Names are in the accent colour, a sentence is not.
fn said_apart(shell: &mut Shell, ui: &egui::Ui, p: &Palette, (area, at): (&str, Rect), (what, head, detail, names): &(String, String, String, bool)) {
    let paint = ui.painter();
    paint.rect_filled(at, 10.0, p.surface);
    paint.rect_stroke(at, 10.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
    let room = Some((at.width() - 40.0).max(0.0));
    ux::text(paint, at.min + vec2(20.0, 14.0), what, types::SMALL, p.text_secondary, room);
    ux::text(paint, at.min + vec2(20.0, 34.0), head, types::DIALOG_TITLE, p.text, room);
    ux::text(paint, at.min + vec2(20.0, 60.0), detail, types::SMALL, if *names { p.accent } else { p.text_secondary }, room);
    let whole = listed(&tr("{first}, {then}"), [what, head, detail].into_iter().map(String::as_str));
    let _ = control(shell, ui, &format!("workspace.change-view.{area}"), "status", at, false).on_hover_text(&whole).accessible_name(whole);
}

/// `items` one after another in one buffer, the empty ones left out, with what `template` puts
/// between its `{first}` and `{then}` between each two; that is worked out once for the whole list.
fn listed<'a>(template: &str, items: impl Iterator<Item = &'a str>) -> String {
    let between = fmt(template, &[("first", ""), ("then", "")]);
    let mut all = String::new();
    for item in items.filter(|i| !i.is_empty()) {
        if !all.is_empty() {
            all.push_str(&between);
        }
        all.push_str(item);
    }
    all
}

/// The Model card's words, then the Layout card's: what it is about, its headline, the names or
/// sentence under it, and whether those are names. A side with nothing says so, and says the other alone changed.
pub(super) fn apart_words(apart: &Apart) -> [(String, String, String, bool); 2] {
    let model = |k: ChangeKind| apart.model.iter().filter(|e| e.kind == k).count() as u64;
    let (added, changed, removed) = (model(ChangeKind::Added), model(ChangeKind::Changed), model(ChangeKind::Removed));
    let model = [
        (added, trn("{n} element added", "{n} elements added", added)),
        (changed, trn("{n} element changed", "{n} elements changed", changed)),
        (removed, trn("{n} element removed", "{n} elements removed", removed)),
    ];
    let layout = |line: bool, k: Placing| apart.layout.iter().filter(|e| e.line == line && e.kind == k).count() as u64;
    let boxes = [Placing::Placed, Placing::Moved, Placing::Changed, Placing::TakenOff].map(|k| layout(false, k));
    let lines = [Placing::Placed, Placing::Changed, Placing::TakenOff].map(|k| layout(true, k));
    let layout = [
        (boxes[0], trn("{n} box placed", "{n} boxes placed", boxes[0])),
        (boxes[1], trn("{n} box moved", "{n} boxes moved", boxes[1])),
        (boxes[2], trn("{n} box changed", "{n} boxes changed", boxes[2])),
        (boxes[3], trn("{n} box taken off", "{n} boxes taken off", boxes[3])),
        (lines[0], trn("{n} line drawn", "{n} lines drawn", lines[0])),
        (lines[1], trn("{n} line changed", "{n} lines changed", lines[1])),
        (lines[2], trn("{n} line taken off", "{n} lines taken off", lines[2])),
    ];
    let counted = |pieces: &[(u64, String)]| {
        Some(listed(&tr("{first} · {then}"), pieces.iter().filter(|(n, _)| *n > 0).map(|(_, words)| words.as_str()))).filter(|s| !s.is_empty())
    };
    let unnamed = tr("Unnamed");
    let names = |of: &mut dyn Iterator<Item = &Option<String>>| listed(&tr("{first}, {then}"), of.map(|n| n.as_deref().unwrap_or(&unnamed)));
    let (model_said, layout_said) = (counted(&model), counted(&layout));
    let model = match model_said {
        Some(head) => (head, names(&mut apart.model.iter().map(|e| &e.name)), true),
        None if layout_said.is_some() || apart.itself || apart.diagram.is_some() => {
            (tr("The model didn't change"), tr("Only the layout changed."), false)
        }
        None => (tr("The model didn't change"), String::new(), false),
    };
    let layout = match layout_said {
        Some(head) => (head, names(&mut apart.layout.iter().map(|e| &e.name)), true),
        None if apart.diagram == Some(ChangeKind::Added) => (tr("This diagram was added"), tr("It has no boxes or lines yet."), false),
        None if apart.diagram == Some(ChangeKind::Removed) => (tr("This diagram was removed"), tr("It had no boxes or lines."), false),
        None if apart.itself => (tr("The diagram's own details changed"), tr("Such as its name, or the order its boxes are drawn in."), false),
        None if model.2 => (tr("The layout didn't change"), tr("Only the model changed."), false),
        None => (tr("The layout didn't change"), String::new(), false),
    };
    [(tr("Model"), model.0, model.1, model.2), (tr("Layout"), layout.0, layout.1, layout.2)]
}

/// The one zoom and pan both sides are drawn with: what fits all either side draws into `size`.
/// Neither comes from the project or the canvas, so how someone last looked at it is never a difference.
fn framing(sides: &DiagramSides, size: Vec2) -> (f32, Vec2) {
    let mut fitted = State::default();
    // Below the canvas's least zoom too, so both sides start with all of either in view.
    fit(&mut fitted, size, [&sides.before, &sides.after].into_iter().flatten().filter_map(extent).reduce(Rect::union), f32::MIN_POSITIVE);
    (fitted.zoom, fitted.pan)
}

/// The rectangle around what `side` draws: every box, nested ones too, as nothing keeps them inside
/// their parent, every line, whose bends can reach past the boxes, and where each line's labels were
/// put, however far from it. A label's own size is left to the margin.
fn extent(side: &DiagramSide) -> Option<Rect> {
    let (store, point) = (&side.store, |(x, y): (f64, f64)| Rect::from_min_size(pos2(x as f32, y as f32), Vec2::ZERO));
    let tree = refs(store, side.diagram, diagram::NODE).into_iter().flat_map(|top| node_tree(store, top));
    let boxes = tree.map(|n| bounds_of(store, n)).map(|b| Rect::from_min_size(pos2(b.x as f32, b.y as f32), vec2(b.width as f32, b.height as f32)));
    let bends = side.grid.lines().flat_map(|(_, points)| points.iter().copied());
    let labels = side.grid.lines().flat_map(|(e, points)| {
        refs(store, e, edge::LABEL).into_iter().filter_map(move |l| {
            let from = match store.get(l, label::AT) {
                Ok(Value::Enum(i)) if LabelPlace::from_index(i) == Some(LabelPlace::Source) => points.first().copied(),
                Ok(Value::Enum(i)) if LabelPlace::from_index(i) == Some(LabelPlace::Target) => points.last().copied(),
                _ => Some(middle_of(points)),
            };
            from.map(|(x, y)| (x + real(store, l, label::DX), y + real(store, l, label::DY)))
        })
    });
    boxes.chain(bends.chain(labels).map(point)).filter(Rect::is_finite).reduce(Rect::union)
}

/// One side's pane in `card` under its eyebrow; `before` says which, and `moving` is the pane as a
/// control when the diagram is on this side. A side the diagram is not on is a tinted pane saying so:
/// nothing yet on an added diagram's Before, Removed on a removed one's After. A mark that links to
/// its element's change and is followed puts that change in `go`.
fn pane(
    shell: &mut Shell,
    ui: &mut egui::Ui,
    p: &Palette,
    (card, before, moving): (Rect, bool, Option<Response>),
    (sides, framing): (&DiagramSides, (f32, Vec2)),
    head: &str,
    go: &mut Option<Difference>,
) {
    let (side, words, eyebrow, drawn) =
        if before { ("before", tr("Nothing here yet"), tr("Before"), &sides.before) } else { ("after", tr("Removed"), tr("After"), &sides.after) };
    let paint = ui.painter().clone();
    ux::text(&paint, card.min - vec2(0.0, EYEBROW), &eyebrow.to_uppercase(), types::EYEBROW, p.text_secondary, None);
    let said = format!("{eyebrow}, {}", if drawn.is_some() { head } else { &words });
    let resp = moving.unwrap_or_else(|| control(shell, ui, &format!("workspace.change-view.{side}"), "status", card, false));
    let resp = resp.accessible_name(said);
    paint.rect_filled(card, 8.0, if drawn.is_some() { p.surface } else { p.field });
    let Some(on_it) = drawn.as_ref() else {
        paint.rect_stroke(card, 8.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
        let g = ux::galley(&paint, &words, types::UI, p.text_muted, Some((card.width() - 48.0).max(0.0)));
        paint.galley(card.center() - g.size() / 2.0, g, p.text_muted);
        return;
    };
    let body = body_of(card);
    paint.rect_filled(Rect::from_min_max(card.min, body.right_top()), CornerRadius { nw: 8, ne: 8, sw: 0, se: 0 }, p.page);
    paint.rect_stroke(card, 8.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
    focus_ring(&paint, p, card, 8.0, &resp);
    paint.hline(card.x_range(), body.top(), Stroke::new(1.0, p.border));
    ux::text_mid(&paint, card.left() + 20.0, card.top() + BAND / 2.0, head, types::UI, p.text, Some((card.width() - 40.0).max(0.0)));
    let mut clip = ui.new_child(egui::UiBuilder::new().max_rect(body));
    clip.set_clip_rect(body.intersect(ui.clip_rect()));
    let view = View { origin: body.min, pan: framing.1, zoom: framing.0 };
    scene(shell, &clip, p, (side, body, &view), on_it);
    let rows = related::index(&shell.versions.changes);
    marks(shell, &clip, p, (side, &view), (on_it, sides), &rows, go);
}

/// Draws the side's diagram read-only: no selection, gesture or problem. It records under the side's
/// own ids, as both sides hold the same boxes and lines.
fn scene(shell: &mut Shell, clip: &egui::Ui, p: &Palette, (side, area, view): (&str, Rect, &View), on_it: &DiagramSide) {
    let (notations, none, clean) = (archforge_notations::view(), BTreeMap::new(), BTreeMap::new());
    let scene = Scene { p, store: &on_it.store, notations: &notations, diagram: on_it.diagram, view, grid: &on_it.grid, area };
    let live = Live {
        selection: &[],
        overrides: &none,
        key_status: false,
        renaming: None,
        problems: &clean,
        hovered: None,
        end_drag: None,
        segment_drag: None,
        draft: None,
        rename_text: None,
        set_drag: None,
        looking_only: true,
    };
    let mut drawn = Registry::default();
    let _ = draw_scene(clip, clip.painter(), &scene, live, &mut drawn);
    let prefix = format!("workspace.change-view.{side}");
    for e in drawn.entries() {
        let at = Rect::from_min_max(pos2(e.rect[0], e.rect[1]), pos2(e.rect[2], e.rect[3]));
        shell.registry.record(&e.id.replacen("workspace.canvas", &prefix, 1), e.kind, at);
    }
}

/// Marks what differs on this side: an outline round a box, the line drawn over in the change's
/// colour, and the change in words on each, linked through `rows`, the comparison's changes by element.
fn marks(
    shell: &mut Shell,
    clip: &egui::Ui,
    p: &Palette,
    (side, view): (&str, &View),
    (on_it, sides): (&DiagramSide, &DiagramSides),
    rows: &HashMap<Uuid, related::Link>,
    go: &mut Option<Difference>,
) {
    let store = &on_it.store;
    let kind_of = |o: Obj| store.uuid(o).ok().and_then(|id| Some((id, *sides.marks.get(&id)?)));
    for (e, points) in on_it.grid.lines() {
        let Some((id, kind)) = kind_of(e) else { continue };
        let ink = kind_look(kind, p).1;
        let line: Vec<Pos2> = points.iter().map(|(x, y)| view.to_screen(*x, *y)).collect();
        clip.painter().add(Shape::line(line, Stroke::new(MARK, ink)));
        let (x, y) = middle_of(points);
        label(shell, clip, p, (side, on_it, e, id), (kind, view), (Align2::CENTER_CENTER, view.to_screen(x, y)), rows, go);
    }
    for n in refs(store, on_it.diagram, diagram::NODE).into_iter().flat_map(|top| node_tree(store, top)) {
        let Some((id, kind)) = kind_of(n) else { continue };
        let area = view.rect(&bounds_of(store, n));
        // As the canvas draws no such box.
        if !area.is_finite() || area.width() <= 0.0 || area.height() <= 0.0 {
            continue;
        }
        clip.painter().rect_stroke(area, 6.0, Stroke::new(MARK, kind_look(kind, p).1), StrokeKind::Outside);
        label(shell, clip, p, (side, on_it, n, id), (kind, view), (Align2::LEFT_BOTTOM, area.left_top() - vec2(0.0, MARK + 2.0)), rows, go);
    }
}

/// The change in words, placed by its `corner` at `at` in `view`, naming `o`, whose id is `id`, to
/// whoever reads it out or hovers. When `rows` holds a change of the element `o` draws, the words
/// link to its row: a click or Enter puts it in `go`, and the keys reaching it out of sight move both sides to it.
fn label(
    shell: &mut Shell,
    clip: &egui::Ui,
    p: &Palette,
    (side, on_it, o, id): (&str, &DiagramSide, Obj, Uuid),
    (kind, view): (ChangeKind, &View),
    (corner, at): (Align2, Pos2),
    rows: &HashMap<Uuid, related::Link>,
    go: &mut Option<Difference>,
) {
    let (word, ink, tint) = kind_look(kind, p);
    let g = ux::galley(clip.painter(), &word, types::CAPTION, ink, None);
    let area = corner.anchor_size(at, g.size() + 2.0 * LABEL_PAD);
    // The framing's margin holds a label above the topmost box or half one past a line's middle,
    // but not one running right from a box narrowed at the right edge; one whose box was moved past it goes with it.
    let room = if at.x <= clip.max_rect().right() { (clip.max_rect().right() - area.right()).min(0.0) } else { 0.0 };
    let area = area.translate(vec2(room, 0.0));
    let said = format!("{}, {word}", shown_name(&on_it.store, o).unwrap_or_else(|| tr("Unnamed")));
    let store = &on_it.store;
    let element = [node::ELEMENT, edge::ELEMENT].into_iter().find_map(|prop| store.get(o, prop).ok()?.as_ref()).and_then(|e| store.uuid(e).ok());
    let row = element.and_then(|e| rows.get(&e)).and_then(|(at, _)| shell.versions.changes.get(*at).cloned());
    let to = row.as_ref().map(|r| fmt(&tr("Go to {name}"), &[("name", r.name.as_deref().filter(|n| !n.is_empty()).unwrap_or(&tr("Unnamed")))]));
    let said = to.as_ref().map_or(said.clone(), |to| listed(&tr("{first}, {then}"), [said.as_str(), to].into_iter()));
    let id = format!("workspace.change-view.{side}.mark.{id}");
    let resp = control(shell, clip, &id, if row.is_some() { "link" } else { "status" }, area, row.is_some()).on_hover_text(&said);
    let resp = resp.accessible_name(said);
    if resp.gained_focus() && !clip.max_rect().contains_rect(area) {
        let by = (clip.max_rect().center() - area.center()).round();
        if let Some(o) = shell.versions.element.as_mut() {
            o.nav = Some((view.zoom, view.pan + by));
        }
        clip.ctx().request_repaint();
    }
    clip.painter().rect_filled(area, 4.0, tint);
    clip.painter().rect_stroke(area, 4.0, Stroke::new(1.0, ink), StrokeKind::Inside);
    focus_ring(clip.painter(), p, area, 4.0, &resp);
    clip.painter().galley(area.center() - g.size() / 2.0, g, ink);
    if resp.clicked() {
        *go = row;
    }
}
