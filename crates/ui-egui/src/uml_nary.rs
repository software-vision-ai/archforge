//! N-ary associations in the shell (AR-128, design page 08, frames A2 to A5).
//!
//! Drawing (A3): the Relationship tool's N-ary association is drawn by clicking each class in
//! turn. The ends picked so far are a draft the shell keeps and draws alone (a preview diamond at
//! their cards' centroid from the second end, its legs, and a dashed leg to the pointer), with the
//! gesture hint at the canvas's top counting them and Properties showing the draft; nothing enters
//! the model until Enter, or a double click on the last class, finishes at three ends or more, as
//! one undo step (`uml.association.add-nary`, the diamond placed, a leg per end), the association
//! then selected. Backspace drops the last end; Escape, another tool or kind, a click on empty
//! canvas, another diagram or another project drops the draft. A class clicked twice gives two
//! ends at it, told apart by their roles.
//!
//! Selected (A4): the diamond and every leg select the one association. Properties shows its name
//! and package, then `Ends · n` with `+` (Add end: the next class clicked on the canvas gets a new
//! end) and a row per end with its editor (Role, Type, Multiplicity, Navigable, Ordered, Unique;
//! never Aggregation), applied as one undo step. A leg's class end drags onto another class to
//! move that end there. The context menu on the diamond or a leg holds Add end, Reconnect end,
//! Remove from this diagram and Delete from the model.
//!
//! Removing an end (A5): of four or more it goes at once, one undo step with a toast and Undo; of
//! three, two would remain, so the panel asks to keep the three ends (focused, and what Escape
//! does) or to make the association binary, one undo step.

use archforge_mof::{Obj, Store, Uuid};
use archforge_uml::nary;
use egui::{Pos2, Rect, Stroke, StrokeKind, Vec2, pos2, vec2};
use serde_json::json;

use crate::Shell;
use crate::canvas::{Tool, View};
use crate::home::control;
use crate::i18n::{fmt, tr, trn};
use crate::panels::{FIELD, Menu, PAD, ROW, Subject, check, icon_button, run, section, select_field, text_field};
use crate::uml_relate::Relate;
use crate::ux::{self, Palette, icons, types};
use crate::widgets;

/// The panel's longer sentences, kept whole for the catalogue.
const FINISHES: &str = "Enter, or a double-click on the last class, finishes once there are 3 ends or more. One undo step adds the association, its diamond and every end.";
const SELECTING: &str = "Selecting the diamond or any leg selects the one association. Remove from this diagram hides it here; Delete from the model deletes it and its ends everywhere.";
const BINARY_UNNAMED: &str = "Make it binary turns it into a plain association between {first} and {second}, keeping both ends, as one undo step. Keep 3 ends, or Esc, changes nothing.";
const BINARY_NAMED: &str = "Make it binary turns {name} into a plain association between {first} and {second}, keeping its name and both ends, as one undo step. Keep 3 ends, or Esc, changes nothing.";

/// The class diagram's tool that draws an n-ary association, as its kind names it.
const TOOL: &str = "relationship";

/// An n-ary association under way on the canvas: a draft being drawn, or a class click awaited
/// for a new end of an existing one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Nary {
    /// The ends picked so far on diagram `diagram`, in order: each its class and the card clicked.
    Draft { diagram: Uuid, ends: Vec<DraftEnd> },
    /// Add end armed for `association`: the next class clicked on `diagram` gets a new end.
    AddEnd { diagram: Uuid, association: Uuid },
}

/// An end of a draft: its class and the card it was picked on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DraftEnd {
    pub class: Uuid,
    pub node: Uuid,
}

/// An n-ary association's end being edited in Properties: its place among the ends and its
/// fields as typed, until Apply writes them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EndEdit {
    pub association: Uuid,
    pub end: usize,
    pub role: String,
    /// The class or interface at the end.
    pub ty: Option<Uuid>,
    pub multiplicity: String,
    pub navigable: bool,
    pub ordered: bool,
    pub unique: bool,
    /// Why Apply was refused, with the field it sits under (`role`, `multiplicity`), or none for
    /// the editor's foot.
    pub refused: Option<(Option<&'static str>, String)>,
    /// True until the editor has been shown once: the role field takes the keyboard then.
    pub opened: bool,
}

/// The question removing one of three ends asks (A5): keep the three, or make it binary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RemoveAsk {
    pub association: Uuid,
    pub end: usize,
    /// True until shown once: Keep 3 ends takes the keyboard then.
    pub opened: bool,
}

fn name_of(store: &Store, o: Obj) -> String {
    archforge_core::name_of(store, o)
}

fn uuid(store: &Store, o: Obj) -> Option<Uuid> {
    store.uuid(o).ok()
}

/// Whether the Relationship tool is armed with N-ary association on a class diagram.
pub(crate) fn armed(shell: &Shell) -> bool {
    shell.canvas.tool == Tool::Connect(TOOL)
        && shell.canvas.uml_relation == Relate::NAry
        && crate::kinds::active_kind(shell).is_some_and(|k| k.kind() == archforge_uml::CLASS_DIAGRAM)
}

/// Whether the connect tool `tool` of a class diagram draws n-ary associations now, by clicks.
pub(crate) fn clicks(shell: &Shell, tool: &str) -> bool {
    tool == TOOL && armed(shell)
}

/// The draft's ends on the open diagram, none when there is no draft there.
fn draft(shell: &Shell) -> Vec<DraftEnd> {
    match &shell.canvas.nary {
        Some(Nary::Draft { diagram, ends }) if shell.canvas.active == Some(*diagram) => ends.clone(),
        _ => Vec::new(),
    }
}

/// The association Add end waits on, on the open diagram.
pub(crate) fn adding_end(shell: &Shell) -> Option<Uuid> {
    match &shell.canvas.nary {
        Some(Nary::AddEnd { diagram, association }) if shell.canvas.active == Some(*diagram) => Some(*association),
        _ => None,
    }
}

/// Drops what is under way when it no longer holds: a draft once the tool is not N-ary
/// association any more, its diagram is not the open one, or a class or card it picked is gone;
/// Add end once another tool is chosen, another diagram opened, or the association is gone or no
/// longer n-ary. A draft never outlives its gesture.
pub fn retain(state: &mut crate::canvas::State, store: &Store) {
    let alive = |id: &Uuid| store.by_uuid(*id).is_some_and(|o| store.is_alive(o));
    let keep = match &state.nary {
        None => true,
        Some(Nary::Draft { diagram, ends }) => {
            state.active == Some(*diagram)
                && state.tool == Tool::Connect(TOOL)
                && state.uml_relation == Relate::NAry
                && ends.iter().all(|e| alive(&e.class) && alive(&e.node))
        }
        Some(Nary::AddEnd { diagram, association }) => {
            state.active == Some(*diagram)
                && state.tool == Tool::Select
                && store.by_uuid(*association).is_some_and(|a| store.is_alive(a) && nary::is_nary(store, a))
        }
    };
    if !keep {
        state.nary = None;
    }
    if state.nary_menu.as_ref().is_some_and(|(_, a)| store.by_uuid(*a).is_none_or(|a| !store.is_alive(a) || !nary::is_nary(store, a))) {
        state.nary_menu = None;
    }
}

/// The roles new ends at `classes` take, in order, as `uml.association.add-nary` names them.
fn roles(store: &Store, classes: &[Obj]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for c in classes {
        let role = nary::default_role(store, *c, &out);
        out.push(role);
    }
    out
}

/// The association an element stands for when it is selected on the canvas: an n-ary
/// association's end, whose leg was clicked, stands for its association; anything else for itself.
pub(crate) fn selectable(store: &Store, element: Obj) -> Obj {
    nary::association_of_end(store, element).unwrap_or(element)
}

/// The n-ary association a node (its diamond) or a line (a leg) of the canvas shows, if any.
pub(crate) fn shown_by(store: &Store, element: Obj) -> Option<Obj> {
    if nary::is_nary(store, element) {
        return Some(element);
    }
    nary::association_of_end(store, element)
}

// Drawing.

/// A click of the armed tool on the card `node` of diagram `d`: the card's class becomes the
/// draft's next end; a double click with three ends or more finishes instead (its first click
/// picked the last class). Anything but a class or an interface is refused at the canvas's top,
/// in the user's words, and adds no end.
pub(crate) fn click(shell: &mut Shell, d: Obj, node: Uuid, double: bool) {
    shell.canvas.notice = None;
    let mut ends = draft(shell);
    if double && ends.len() >= nary::MIN_ENDS {
        finish(shell, d);
        return;
    }
    let Some(store) = shell.session.as_ref().map(|s| s.doc()) else { return };
    let Some(diagram) = uuid(store, d) else { return };
    let Some(c) = crate::uml_relate::class_of_node(store, node) else {
        shell.canvas.notice = Some(tr("An n-ary association joins classes and interfaces"));
        return;
    };
    if let Some(why) = crate::uml_relate::association_refusal(Relate::Association, store, c) {
        shell.canvas.notice = Some(why);
        return;
    }
    if ends.len() >= nary::MAX_ENDS {
        shell.canvas.notice = Some(fmt(&tr("An association takes at most {n} ends"), &[("n", &nary::MAX_ENDS.to_string())]));
        return;
    }
    let Some(class) = uuid(store, c) else { return };
    ends.push(DraftEnd { class, node });
    shell.canvas.nary = Some(Nary::Draft { diagram, ends });
}

/// Drops the draft's last end.
pub(crate) fn back(shell: &mut Shell) {
    if let Some(Nary::Draft { ends, .. }) = shell.canvas.nary.as_mut() {
        ends.pop();
    }
}

/// Drops what is under way, a draft or Add end, and hands back to Select: nothing is written.
pub(crate) fn cancel(shell: &mut Shell) {
    shell.canvas.nary = None;
    shell.canvas.tool = Tool::Select;
}

/// The draft's cards on screen, in its order, where the store still has them.
fn cards(shell: &Shell, view: View, ends: &[DraftEnd]) -> Vec<Rect> {
    let Some(store) = shell.session.as_ref().map(|s| s.doc()) else { return Vec::new() };
    ends.iter().filter_map(|e| store.by_uuid(e.node)).map(|n| view.rect(&archforge_diagram::grid::bounds_of(store, n))).collect()
}

/// Where the diamond of an association with ends at `cards` goes, in diagram units: centred on the
/// cards' centroid, at the design's size.
fn diamond_place(cards: &[archforge_diagram::Rect]) -> Option<archforge_diagram::Rect> {
    if cards.is_empty() {
        return None;
    }
    let n = cards.len() as f64;
    let cx = cards.iter().map(|r| r.x + r.width / 2.0).sum::<f64>() / n;
    let cy = cards.iter().map(|r| r.y + r.height / 2.0).sum::<f64>() / n;
    let (w, h) = nary::DIAMOND_SIZE;
    let place = archforge_diagram::Rect::new((cx - w / 2.0).round(), (cy - h / 2.0).round(), w, h);
    (place.x.is_finite() && place.y.is_finite()).then_some(place)
}

/// Finishes the draft on diagram `d` at three ends or more, as one undo step: the association with
/// an end at each class picked, its diamond where the preview showed it, and a leg per end; then
/// selects it and hands back to Select. With fewer ends it says what is missing and keeps the
/// draft.
pub(crate) fn finish(shell: &mut Shell, d: Obj) {
    let ends = draft(shell);
    if ends.len() < nary::MIN_ENDS {
        shell.canvas.notice = Some(tr("An n-ary association needs three ends or more: click another class"));
        return;
    }
    let Some(session) = shell.session.as_mut() else { return };
    let store = session.doc();
    let Some(diagram) = uuid(store, d) else { return };
    let rects: Vec<archforge_diagram::Rect> =
        ends.iter().filter_map(|e| store.by_uuid(e.node)).map(|n| archforge_diagram::grid::bounds_of(store, n)).collect();
    let Some(place) = diamond_place(&rects) else { return };
    let classes: Vec<String> = ends.iter().map(|e| e.class.to_string()).collect();
    let nodes: Vec<String> = ends.iter().map(|e| e.node.to_string()).collect();
    let out = session.batch(&tr("Add n-ary association"), |b| {
        let a = b.execute("uml.association.add-nary", &json!({ "classes": classes }))?;
        let id = a.get("id").and_then(|i| i.as_str()).unwrap_or_default().to_string();
        let made: Vec<String> =
            a.get("ends").and_then(|e| e.as_array()).into_iter().flatten().filter_map(|e| e.as_str().map(str::to_string)).collect();
        let n = b.execute(
            "diagram.place",
            &json!({ "diagram": diagram.to_string(), "element": id, "x": place.x, "y": place.y, "width": place.width, "height": place.height }),
        )?;
        let diamond = n.get("id").and_then(|i| i.as_str()).unwrap_or_default().to_string();
        for (end, card) in made.iter().zip(&nodes) {
            b.execute("diagram.connect", &json!({ "diagram": diagram.to_string(), "element": end, "source": diamond, "target": card }))?;
        }
        Ok(())
    });
    match out {
        Ok(o) => {
            shell.canvas.nary = None;
            shell.canvas.tool = Tool::Select;
            if let Some(id) = o.value.get(0).and_then(|v| v.get("id")).and_then(|v| v.as_str())
                && let Err(e) = run(shell, "view.select", json!({ "ids": [id] }))
            {
                shell.canvas.notice = Some(e);
            }
        }
        // Refused, the draft stays for another try and the model is as it was.
        Err(e) => shell.canvas.notice = Some(e.to_string()),
    }
}

/// The keys while something n-ary is under way on diagram `d`: with a draft, Enter finishes,
/// Backspace (or Delete) drops the last end while there is one and Escape cancels; with Add end, Escape cancels. True
/// when a key was taken.
pub(crate) fn keys(shell: &mut Shell, ui: &egui::Ui, d: Obj) -> bool {
    let drafting = armed(shell);
    let adding = adding_end(shell).is_some();
    if !drafting && !adding {
        return false;
    }
    let (enter, back_key, escape) = ui.input(|i| {
        (i.key_pressed(egui::Key::Enter), i.key_pressed(egui::Key::Backspace) || i.key_pressed(egui::Key::Delete), i.key_pressed(egui::Key::Escape))
    });
    if escape {
        if adding {
            shell.canvas.nary = None;
        } else {
            cancel(shell);
        }
        return true;
    }
    if drafting && enter {
        finish(shell, d);
        return true;
    }
    // Only a draft with an end to drop takes Backspace; before the first end it reaches the
    // selection's removal as everywhere else.
    if drafting && back_key && !draft(shell).is_empty() {
        back(shell);
        return true;
    }
    false
}

/// The points of a preview leg from the diamond `diamond` to the card `card`, on screen: straight
/// up or down from the diamond's tip to a card above or below it, straight across to one beside
/// it, else across from its side and then up or down to the card, as the design draws it.
fn leg_points(diamond: Rect, card: Rect) -> Vec<Pos2> {
    let (m, c) = (diamond.center(), card.center());
    if m.x >= card.left() && m.x <= card.right() {
        let from = if c.y < m.y { diamond.center_top() } else { diamond.center_bottom() };
        let to = pos2(m.x, if c.y < m.y { card.bottom() } else { card.top() });
        return vec![from, to];
    }
    let from = if c.x < m.x { diamond.left_center() } else { diamond.right_center() };
    if m.y >= card.top() && m.y <= card.bottom() {
        return vec![from, pos2(if c.x < m.x { card.right() } else { card.left() }, m.y)];
    }
    vec![from, pos2(c.x, m.y), pos2(c.x, if c.y < m.y { card.bottom() } else { card.top() })]
}

/// A hollow diamond filling `rect`, over `fill`.
fn diamond_shape(paint: &egui::Painter, rect: Rect, fill: egui::Color32, stroke: Stroke) {
    let points = vec![rect.left_center(), rect.center_top(), rect.right_center(), rect.center_bottom()];
    paint.add(egui::Shape::convex_polygon(points, fill, stroke));
}

/// Draws an n-ary association's diamond node at `rect` (AR-128): a hollow diamond in the line's
/// ink, the accent while it is selected, its name above it.
#[allow(clippy::too_many_arguments, reason = "a diamond is drawn with its name, its stereotypes and its look")]
pub(crate) fn draw_diamond(
    paint: &egui::Painter,
    p: &Palette,
    rect: Rect,
    name: &str,
    keyword: Option<&str>,
    z: f32,
    selected: bool,
    look: &crate::canvas::Look,
) {
    let ink = if selected { p.accent } else { look.stroke.unwrap_or(p.text_secondary) };
    let width = look.width.map_or((1.5 * z).clamp(1.0, 3.0), |w| (w * z).clamp(1.0, 6.0));
    diamond_shape(paint, rect, look.fill.unwrap_or(p.canvas), Stroke::new(width, ink));
    let text_ink = look.text.unwrap_or(p.text_secondary);
    let mut top = rect.top() - 8.0 * z;
    if !name.is_empty() {
        let g = ux::galley(paint, name, types::SMALL.scaled(z), text_ink, None);
        top -= g.size().y;
        paint.galley(pos2(rect.center().x - g.size().x / 2.0, top), g, text_ink);
    }
    // Its stereotypes above its name, `«Reference»`, as a line's are (AR-135).
    if let Some(k) = keyword {
        let g = ux::galley(paint, k, types::CAPTION.scaled(z), text_ink, None);
        paint.galley(pos2(rect.center().x - g.size().x / 2.0, top - g.size().y - 1.0 * z), g, text_ink);
    }
}

/// Where an n-ary leg's end at its diamond `rect` meets the diamond's outline: `tip`, on the
/// node's border, moved on along the leg into the node (`towards` is the leg's next point) until it
/// reaches the diamond, so the leg touches the diamond wherever it leaves the node. None when the
/// leg does not cross the diamond.
pub(crate) fn onto_diamond(rect: Rect, tip: Pos2, towards: Pos2) -> Option<Pos2> {
    let dir = tip - towards;
    if dir.length() <= f32::EPSILON || !dir.is_finite() {
        return None;
    }
    let corners = [rect.left_center(), rect.center_top(), rect.right_center(), rect.center_bottom()];
    let mut best: Option<f32> = None;
    for k in 0..4 {
        let (a, b) = (corners[k], corners[(k + 1) % 4]);
        let e = b - a;
        let den = dir.x * e.y - dir.y * e.x;
        if den.abs() <= f32::EPSILON {
            continue;
        }
        let w = a - tip;
        let t = (w.x * e.y - w.y * e.x) / den;
        let s = (w.x * dir.y - w.y * dir.x) / den;
        if t >= 0.0 && (0.0..=1.0).contains(&s) {
            best = Some(best.map_or(t, |b| b.min(t)));
        }
    }
    best.map(|t| tip + dir * t)
}

/// The draft as the canvas previews it on diagram `d` (A3): from the second end a diamond at the
/// cards' centroid and a solid leg to each card, its role and `*` beside the card; a dashed leg to
/// the pointer, and the class under it ringed. Drawn by the shell alone: nothing is in the model.
fn draw_draft(shell: &Shell, paint: &egui::Painter, p: &Palette, view: View, d: Obj, pointer: Option<Pos2>) {
    let ends = draft(shell);
    let Some(store) = shell.session.as_ref().map(|s| s.doc()) else { return };
    let z = view.zoom;
    let stroke = Stroke::new((1.5 * z).clamp(1.0, 3.0), p.accent);
    let dashed = |paint: &egui::Painter, a: Pos2, b: Pos2| {
        paint.extend(egui::Shape::dashed_line(&[a, b], Stroke::new((1.2 * z).clamp(1.0, 2.5), p.accent), 5.0 * z.max(0.5), 4.0 * z.max(0.5)));
    };
    // The class under the pointer, ringed as the next end.
    let target = pointer.and_then(|at| {
        let grid = archforge_diagram::Grid::build(store, d);
        let (x, y) = view.to_canvas(at);
        match crate::editing::hit(store, &grid, x, y) {
            Some(archforge_diagram::Hit::Node(n)) if crate::uml_relate::class_of_node(store, uuid(store, n)?).is_some() => {
                Some(view.rect(&archforge_diagram::grid::bounds_of(store, n)))
            }
            _ => None,
        }
    });
    if let Some(t) = target {
        paint.rect_stroke(t.expand(3.0), 6.0, Stroke::new(2.0, p.accent), StrokeKind::Outside);
    }
    let rects = cards(shell, view, &ends);
    if rects.len() < 2 {
        // One end: a dashed line from its card to the pointer.
        if let (Some(card), Some(at)) = (rects.first(), pointer) {
            let end = target.map_or(at, |t| crate::canvas::exit_point(t, card.center()));
            dashed(paint, crate::canvas::exit_point(*card, end), end);
        }
        return;
    }
    let units: Vec<archforge_diagram::Rect> =
        ends.iter().filter_map(|e| store.by_uuid(e.node)).map(|n| archforge_diagram::grid::bounds_of(store, n)).collect();
    let Some(place) = diamond_place(&units) else { return };
    let diamond = view.rect(&place);
    let classes: Vec<Obj> = ends.iter().filter_map(|e| store.by_uuid(e.class)).collect();
    let names = roles(store, &classes);
    for (card, role) in rects.iter().zip(&names) {
        let points = leg_points(diamond, *card);
        paint.add(egui::Shape::line(points.clone(), stroke));
        // Beside the leg's end at the card, as a line's end texts sit: right and left of an
        // upright leg, the role above and the multiplicity under a level one.
        let (Some(end), Some(before)) = (points.last().copied(), points.get(points.len().saturating_sub(2)).copied()) else { continue };
        let role_g = ux::galley(paint, role, types::SMALL.scaled(z), p.text_secondary, None);
        let many_g = ux::galley(paint, nary::NEW_MULTIPLICITY, types::SMALL.scaled(z), p.text_secondary, None);
        let (role_size, many_size) = (role_g.size(), many_g.size());
        let gap = 6.0 * z;
        let (role_at, many_at) = if (before.y - end.y).abs() > (before.x - end.x).abs() {
            let y = if before.y > end.y { end.y + gap } else { end.y - gap - role_size.y };
            (pos2(end.x + 2.0 * gap, y), pos2(end.x - 2.0 * gap - many_size.x, y))
        } else {
            let x = |w: f32| if before.x > end.x { end.x + 2.0 * gap } else { end.x - 2.0 * gap - w };
            (pos2(x(role_size.x), end.y - gap - role_size.y), pos2(x(many_size.x), end.y + gap))
        };
        paint.galley(role_at, role_g, p.text_secondary);
        paint.galley(many_at, many_g, p.text_secondary);
    }
    diamond_shape(paint, diamond, p.canvas, stroke);
    if let Some(at) = pointer.filter(|at| !diamond.contains(*at)) {
        let end = target.map_or(at, |t| crate::canvas::exit_point(t, diamond.center()));
        let from = onto_diamond(diamond, crate::canvas::exit_point(diamond, end), end).unwrap_or(diamond.center());
        dashed(paint, from, end);
    }
}

/// What the canvas draws over the diagram for n-ary associations on diagram `d`: the draft's
/// preview, the class end handles of a selected association's legs, and its context menu.
#[allow(clippy::too_many_arguments, reason = "the overlay sits over the canvas, in its view, for its diagram")]
pub(crate) fn overlay(shell: &mut Shell, ui: &egui::Ui, p: &Palette, r: Rect, view: View, d: Obj) {
    let paint = ui.painter().with_clip_rect(r);
    if armed(shell) {
        let pointer = ui.input(|i| i.pointer.hover_pos()).filter(|at| r.contains(*at));
        draw_draft(shell, &paint, p, view, d, pointer);
    }
    if shell.canvas.gesture.is_none() {
        leg_handles(shell, ui, p, r, view, d);
    }
    if let Some((at, a)) = shell.canvas.nary_menu {
        menu(shell, ui, p, r, at, a, d);
    }
}

/// The n-ary association selected alone, when one is: itself, or the end of one an agent selected.
fn selected(shell: &Shell) -> Option<Obj> {
    let s = shell.session.as_ref()?;
    match s.view().selection.as_slice() {
        [o] => shown_by(s.doc(), *o),
        _ => None,
    }
}

/// The handles of a selected n-ary association's legs on diagram `d` (A4): one at each leg's class
/// end, which drags onto another class to move that end there; the diamond's end does not move.
fn leg_handles(shell: &mut Shell, ui: &egui::Ui, p: &Palette, r: Rect, view: View, d: Obj) {
    let Some(a) = selected(shell) else { return };
    let Some(store) = shell.session.as_ref().map(|s| s.doc()) else { return };
    let grid = archforge_diagram::Grid::build(store, d);
    let mut handles: Vec<(usize, Uuid, Pos2, Pos2)> = Vec::new();
    for (i, end) in nary::ends(store, a).into_iter().enumerate() {
        let Some(end_id) = uuid(store, end) else { continue };
        let mark = archforge_uml::figures::line(store, end).map(|l| l.ends[1].mark).unwrap_or(archforge_diagram::figure::EndMark::None);
        for leg in archforge_diagram::views_of(store, end).1.into_iter().filter(|e| archforge_diagram::diagram_of(store, *e) == Some(d)) {
            let Some(route) = grid.line(leg) else { continue };
            let screen: Vec<Pos2> = route.iter().map(|(x, y)| view.to_screen(*x, *y)).collect();
            let (Some(first), Some(last)) = (screen.first().copied(), screen.last().copied()) else { continue };
            let at = crate::canvas::end_handles_at(&screen, [archforge_diagram::figure::EndMark::None, mark], view.zoom)[1].unwrap_or(last);
            handles.push((i, end_id, at, first));
        }
    }
    let paint = ui.painter().with_clip_rect(r);
    for (i, end, at, fixed) in handles {
        if !r.contains(at) {
            continue;
        }
        let radius = crate::canvas::END_HANDLE;
        paint.circle_filled(at, radius, p.surface);
        paint.circle_stroke(at, radius, Stroke::new(1.5, p.accent));
        let id = format!("workspace.canvas.leg-end.{i}");
        let handle = Rect::from_center_size(at, Vec2::splat(12.0));
        widgets::record_visible(ui, &mut shell.registry, &id, "button", handle);
        let resp = ui
            .interact(handle.expand(2.0), widgets::egui_id(&id), egui::Sense::click_and_drag())
            .on_hover_cursor(egui::CursorIcon::Crosshair)
            .on_hover_text(tr("Drag onto another class to move this end"));
        if resp.drag_started_by(egui::PointerButton::Primary) {
            shell.canvas.gesture = Some(crate::editing::Gesture::Reconnect { element: end, source: false, fixed });
        }
    }
}

/// The gesture hint at the canvas's top (A3): what the drawing has so far and what the keys do;
/// with Add end armed, what the next click does.
pub(crate) fn hint_bar(shell: &mut Shell, ui: &egui::Ui, p: &Palette, r: Rect, below: f32) {
    let (main, keys) = if armed(shell) {
        let n = u64::try_from(draft(shell).len()).unwrap_or(u64::MAX);
        (
            trn("N-ary association · {n} end — click a class for the next end", "N-ary association · {n} ends — click a class for the next end", n),
            tr("Enter finishes at 3+ · Backspace drops the last · Esc cancels"),
        )
    } else if adding_end(shell).is_some() {
        (tr("Click a class for the new end"), tr("Esc cancels"))
    } else {
        return;
    };
    let paint = ui.painter().with_clip_rect(r);
    let room = (r.width() - 32.0 - 24.0).max(0.0);
    let main_g = ux::galley(&paint, &main, types::SMALL, p.surface, Some(room));
    let keys_room = (room - main_g.size().x - 10.0).max(0.0);
    let keys_g = (keys_room > 40.0).then(|| ux::galley(&paint, &keys, types::SMALL, p.text_muted, Some(keys_room)));
    let width = 12.0 + main_g.size().x + keys_g.as_ref().map_or(0.0, |g| 10.0 + g.size().x) + 12.0;
    let bar = Rect::from_min_size(pos2(r.center().x - width / 2.0, r.top() + 12.0 + below), vec2(width, 33.0));
    ux::shadow(&paint, bar, 8.0, p.shadow);
    paint.rect_filled(bar, 8.0, p.text);
    let main_w = main_g.size().x;
    paint.galley(pos2(bar.left() + 12.0, bar.center().y - main_g.size().y / 2.0), main_g, p.surface);
    if let Some(g) = keys_g {
        paint.galley(pos2(bar.left() + 12.0 + main_w + 10.0, bar.center().y - g.size().y / 2.0), g, p.text_muted);
    }
    widgets::record_visible(ui, &mut shell.registry, "workspace.canvas.gestureHint", "status", bar);
}

/// A class click while Add end is armed: a new end of the association at the card `node`'s class,
/// its leg drawn wherever the diamond and the class are shown, one undo step; a refusal is said at
/// the canvas's top. Add end then ends, whatever the answer.
pub(crate) fn add_end(shell: &mut Shell, node: Uuid) {
    shell.canvas.notice = None;
    let Some(a) = adding_end(shell) else { return };
    shell.canvas.nary = None;
    let Some(store) = shell.session.as_ref().map(|s| s.doc()) else { return };
    let Some(c) = crate::uml_relate::class_of_node(store, node) else {
        shell.canvas.notice = Some(tr("An n-ary association joins classes and interfaces"));
        return;
    };
    if let Some(why) = crate::uml_relate::association_refusal(Relate::Association, store, c) {
        shell.canvas.notice = Some(why);
        return;
    }
    let Some(class) = uuid(store, c) else { return };
    match run(shell, "uml.association.add-end", json!({ "association": a.to_string(), "class": class.to_string() })) {
        Ok(_) => {
            let _ = run(shell, "view.select", json!({ "ids": [a.to_string()] }));
        }
        Err(e) => shell.canvas.notice = Some(e),
    }
}

/// Arms Add end for association `a` on the open diagram: the next class clicked gets a new end.
pub(crate) fn arm_add_end(shell: &mut Shell, a: Uuid) {
    let Some(diagram) = shell.canvas.active else {
        shell.panels.notice = Some(tr("Open a class diagram to pick the new end's class"));
        return;
    };
    shell.canvas.tool = Tool::Select;
    shell.canvas.nary = Some(Nary::AddEnd { diagram, association: a });
    // The canvas takes the keys, so Escape cancels at once.
    shell.canvas.focused = true;
}

/// Moves the end `end` (an n-ary association's end, its leg dragged) onto the class of the card
/// `to_node`, its legs following on every diagram, one undo step; let go on its own class it
/// changes nothing; a refusal, and what the ends no longer subset or redefine, is said at the
/// canvas's top.
pub(crate) fn reconnect_end(shell: &mut Shell, end: Uuid, to_node: Uuid) {
    let Some(store) = shell.session.as_ref().map(|s| s.doc()) else { return };
    let Some(e) = store.by_uuid(end) else { return };
    let Some(a) = nary::association_of_end(store, e) else { return };
    let Some(i) = nary::ends(store, a).iter().position(|x| *x == e) else { return };
    let Some(c) = crate::uml_relate::class_of_node(store, to_node) else {
        shell.canvas.notice = Some(tr("An n-ary association joins classes and interfaces"));
        return;
    };
    if nary::end_type(store, e) == Some(c) {
        return;
    }
    if let Some(why) = crate::uml_relate::association_refusal(Relate::Association, store, c) {
        shell.canvas.notice = Some(why);
        return;
    }
    let (Some(a_id), Some(class)) = (uuid(store, a), uuid(store, c)) else { return };
    match run(shell, "uml.association.reconnect-end", json!({ "association": a_id.to_string(), "end": i, "class": class.to_string() })) {
        Ok(out) => say_dropped(shell, &out),
        Err(e) => shell.canvas.notice = Some(e),
    }
}

/// Says at the canvas's top what moved ends no longer subset or redefine, which went in the step.
fn say_dropped(shell: &mut Shell, out: &serde_json::Value) {
    let names: Vec<&str> = out.get("dropped").and_then(|d| d.as_array()).into_iter().flatten().filter_map(|d| d.get("name")?.as_str()).collect();
    if !names.is_empty() {
        shell.canvas.notice =
            Some(fmt(&tr("The moved ends no longer subset or redefine {names}, so those were dropped."), &[("names", &names.join(", "))]));
    }
}

// The context menu.

/// The menu's width and its rows' height, as the canvas's other menus.
const MENU_WIDTH: f32 = 260.0;
const MENU_ROW: f32 = 30.0;

/// The context menu of n-ary association `a` on diagram `d`, open at `at` (A4): Add end, Reconnect
/// end, then Remove from this diagram and Delete from the model.
#[allow(clippy::too_many_arguments, reason = "the menu sits over the canvas for its association and diagram")]
fn menu(shell: &mut Shell, ui: &egui::Ui, p: &Palette, r: Rect, at: Pos2, a: Uuid, d: Obj) {
    let paint = ui.painter().with_clip_rect(r);
    let mac = ui.ctx().os() == egui::os::OperatingSystem::Mac;
    let (delete_chord, model_chord) = if mac { ("⌫", "⇧⌫") } else { ("Del", "Shift Del") };
    let rows: [(&str, String, String, bool); 4] = [
        ("add-end", tr("Add end"), tr("Draws a new leg"), false),
        ("reconnect-end", tr("Reconnect end"), tr("Drag a leg end"), false),
        ("remove", tr("Remove from this diagram"), delete_chord.to_string(), false),
        ("delete", tr("Delete from the model…"), model_chord.to_string(), true),
    ];
    let height = 8.0 + rows.len() as f32 * MENU_ROW + 1.0;
    let x = at.x.min(r.right() - MENU_WIDTH - 8.0).max(r.left() + 8.0);
    let y = at.y.min(r.bottom() - height - 8.0).max(r.top() + 8.0);
    let m = Rect::from_min_size(pos2(x, y), vec2(MENU_WIDTH, height));
    ux::shadow(&paint, m, 8.0, p.shadow);
    paint.rect_filled(m, 8.0, p.surface);
    paint.rect_stroke(m, 8.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
    let mut picked = None;
    let mut ry = m.top() + 4.0;
    for (i, (key, label, detail, danger)) in rows.iter().enumerate() {
        if i == 2 {
            paint.rect_filled(Rect::from_min_size(pos2(m.left() + 8.0, ry), vec2(MENU_WIDTH - 16.0, 1.0)), 0.0, p.border);
            ry += 1.0;
        }
        let row = Rect::from_min_size(pos2(m.left() + 4.0, ry), vec2(MENU_WIDTH - 8.0, MENU_ROW));
        let name = format!("{label}, {detail}");
        let resp = control(shell, ui, &format!("workspace.canvas.menu.{key}"), "menu-item", row, true).accessible_name(name);
        if resp.hovered() {
            paint.rect_filled(row, 5.0, p.page);
        }
        let g = ux::galley(&paint, detail, types::CAPTION, p.text_muted, Some(MENU_WIDTH / 2.0));
        let ink = if *danger { p.danger } else { p.text };
        ux::text(&paint, row.min + vec2(10.0, 7.5), label, types::SMALL, ink, Some((row.width() - 24.0 - g.size().x).max(0.0)));
        paint.galley(pos2(row.right() - 10.0 - g.size().x, row.top() + 8.0), g, p.text_muted);
        if resp.clicked() {
            picked = Some(*key);
        }
        ry += MENU_ROW;
    }
    let outside = ui.input(|i| i.pointer.any_pressed() && !i.pointer.interact_pos().is_some_and(|pos| m.contains(pos)));
    if outside || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        shell.canvas.nary_menu = None;
    }
    let Some(key) = picked else { return };
    shell.canvas.nary_menu = None;
    let diamond = shell.session.as_ref().and_then(|s| {
        let store = s.doc();
        uuid(store, nary::node_in(store, d, store.by_uuid(a)?)?)
    });
    let _ = run(shell, "view.select", json!({ "ids": [a.to_string()] }));
    match key {
        "add-end" => arm_add_end(shell, a),
        "remove" => {
            if let Some(n) = diamond {
                crate::editing::unplace(shell, vec![n]);
            }
        }
        "delete" => {
            if let Some(n) = diamond {
                crate::editing::ask_delete(shell, n, a);
            }
        }
        // Reconnect end: the association selected, its legs' ends take the drag.
        _ => {}
    }
}

// Properties.

/// The draft's panel (A3) in place of the selection's while one is being drawn: what it is, its
/// ends so far and what to do next. True when it was drawn.
pub(crate) fn draft_body(shell: &mut Shell, ui: &egui::Ui, p: &Palette, r: Rect, y: f32) -> bool {
    let ends = draft(shell);
    if !armed(shell) || ends.is_empty() {
        return false;
    }
    let Some(store) = shell.session.as_ref().map(|s| s.doc()) else { return false };
    let paint = ui.painter().clone();
    let width = (r.width() - 2.0 * PAD).max(0.0);
    let left = r.left() + PAD;
    ux::text(&paint, pos2(left, r.top() + y), &tr("New n-ary association"), types::CARD_TITLE, p.text, Some(width));
    ux::text(
        &paint,
        pos2(left, r.top() + y + 26.0),
        &tr("Drawing · not in the model until you finish"),
        types::CAPTION,
        p.text_secondary,
        Some(width),
    );
    let mut at = r.top() + y + 51.0;
    let classes: Vec<Obj> = ends.iter().filter_map(|e| store.by_uuid(e.class)).collect();
    let names = roles(store, &classes);
    let rows: Vec<String> =
        classes.iter().zip(&names).map(|(c, role)| format!("{role} : {} [{}]", name_of(store, *c), nary::NEW_MULTIPLICITY)).collect();
    let title = fmt(&tr("Ends so far · {n}"), &[("n", &rows.len().to_string())]);
    at = section(shell, ui, p, r, at, &title, None).unwrap_or(at + 50.0) - 6.0;
    for (i, text) in rows.iter().enumerate() {
        let row = Rect::from_min_size(pos2(left, at), vec2(width, ROW));
        ux::text_mid(&paint, row.left() + 10.0, row.center().y, text, types::UI, p.text, Some((width - 20.0).max(0.0)));
        widgets::record_visible(ui, &mut shell.registry, &format!("workspace.properties.nary.draft.{i}"), "text", row);
        at += ROW;
    }
    let next = if rows.len() >= nary::MIN_ENDS {
        tr("Next: click a class on the canvas, or press Enter to finish")
    } else {
        tr("Next: click a class on the canvas")
    };
    let g = ux::galley_rows(&paint, &next, types::UI, p.text_muted, Some((width - 10.0).max(0.0)), 2);
    let h = g.size().y;
    paint.galley(pos2(left + 10.0, at + 9.0), g, p.text_muted);
    at += h + 26.0;
    paint.rect_filled(Rect::from_min_size(pos2(r.left(), at), vec2(r.width(), 1.0)), 0.0, p.border);
    at += 24.0;
    for said in [
        tr(FINISHES),
        tr("Esc, switching tools or clicking empty canvas cancels and leaves nothing in the model. Backspace takes the last end off."),
        tr("New ends are named after their class and start at *; change them after finishing."),
    ] {
        let g = ux::galley_rows(&paint, &said, types::CAPTION, p.text_secondary, Some(width), 6);
        let h = g.size().y;
        paint.galley(pos2(left, at), g, p.text_secondary);
        at += h + 10.0;
    }
    true
}

/// The classes and interfaces an end may be at, by name, each with its id: every classifier of
/// the model an association may join, the built-in types aside.
pub(crate) fn end_types(store: &Store) -> Vec<(Uuid, String)> {
    let mut out: Vec<(Uuid, String)> = store
        .alive()
        .filter(|o| store.class_of(*o).is_ok_and(archforge_uml::is_classifier))
        .filter(|o| !archforge_uml::members::is_built_in(store, *o))
        .filter(|o| archforge_uml::relations::association_end_problem(store, *o).is_none())
        .filter_map(|o| Some((uuid(store, o)?, name_of(store, o))))
        .collect();
    out.sort_by(|a, b| a.1.to_lowercase().cmp(&b.1.to_lowercase()).then(a.0.cmp(&b.0)));
    out
}

/// A row's text for end `e`: `role : Class [multiplicity]`, the parts it has.
fn row_text(store: &Store, e: Obj) -> String {
    let role = name_of(store, e);
    let class = nary::end_type(store, e).map(|t| name_of(store, t)).unwrap_or_default();
    let m = archforge_uml::figures::multiplicity(store, e);
    let mut text = if role.is_empty() { class } else { format!("{role} : {class}") };
    if !m.is_empty() {
        text.push_str(&format!(" [{m}]"));
    }
    text
}

/// Opens the editor on end `i` of association `a` (whose id is `id`), as the model has it.
pub(crate) fn open(shell: &mut Shell, id: Uuid, i: usize) {
    let Some(store) = shell.session.as_ref().map(|s| s.doc()) else { return };
    let Some(e) = store.by_uuid(id).and_then(|a| nary::ends(store, a).get(i).copied()) else { return };
    let (_, _, ordered, unique) = archforge_uml::modifiers::flags(store, e);
    let navigable = store.by_uuid(id).is_some_and(|a| archforge_uml::figures::navigable(store, a, e));
    shell.panels.nary_end = Some(EndEdit {
        association: id,
        end: i,
        role: name_of(store, e),
        ty: nary::end_type(store, e).and_then(|t| uuid(store, t)),
        multiplicity: archforge_uml::figures::multiplicity(store, e),
        navigable,
        ordered,
        unique,
        refused: None,
        opened: true,
    });
    shell.panels.nary_remove = None;
    shell.panels.menu = None;
}

/// An n-ary association's Properties (A4) under its heading: Name and Package, then `Ends · n`
/// with its `+`, its rows and the editor of the end being edited, the question removing one of
/// three ends asks (A5), and what selecting and removing it mean.
#[allow(clippy::too_many_arguments, reason = "the body is drawn into the panel for its subject")]
pub(crate) fn body(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect, mut y: f32, subject: &Subject, a: Obj, id: Uuid) {
    let width = (r.width() - 2.0 * PAD).max(0.0);
    let left = r.left() + PAD;
    let half = ((width - 8.0) / 2.0).max(0.0);
    crate::panels::name_field(shell, ui, p, pos2(left, y), half, subject, "uml.relationship.rename", "relationship");
    // Its package, read only: an association does not move on its own.
    let Some(store) = shell.session.as_ref().map(|s| s.doc()) else { return };
    let parts = archforge_uml::commands::package_of(store, a).map(|pkg| crate::uml_panel::package_parts(store, pkg)).unwrap_or_default();
    let path = parts.join(crate::panels::PATH_SEP);
    let at = pos2(left + half + 8.0, y);
    let paint = ui.painter().clone();
    let label = tr("Package");
    ux::text(&paint, at, &label, types::CAPTION, p.text_secondary, Some(half));
    let field = Rect::from_min_size(at + vec2(0.0, 17.0), vec2(half, FIELD));
    paint.rect_filled(field, 6.0, p.field);
    ux::text_mid_front(
        &paint,
        field.left() + 8.0,
        field.center().y,
        &parts,
        crate::panels::PATH_SEP,
        types::SMALL,
        p.text_secondary,
        (half - 16.0).max(0.0),
    );
    widgets::record_visible(ui, &mut shell.registry, "workspace.properties.association.package", "field", field);
    let resp = ui.interact(field, widgets::egui_id("workspace.properties.association.package"), egui::Sense::hover());
    let resp = resp.accessible_name(format!("{label}: {path}"));
    if !path.is_empty() {
        let _ = resp.on_hover_text(&path);
    }
    y += 17.0 + FIELD + 16.0;
    // Its stereotypes (AR-135, F3).
    y = crate::uml_stereotypes::section_for(shell, ui, p, r, y, a);
    let Some(store) = shell.session.as_ref().map(|s| s.doc()) else { return };
    let ends = nary::ends(store, a);
    let rows: Vec<String> = ends.iter().map(|e| row_text(store, *e)).collect();
    let title = fmt(&tr("Ends · {n}"), &[("n", &rows.len().to_string())]);
    let Some(next) = section(shell, ui, p, r, y, &title, Some(("workspace.properties.association.ends.add", "Add end"))) else {
        arm_add_end(shell, id);
        return;
    };
    y = next - 6.0;
    let asking = shell.panels.nary_remove.filter(|q| q.association == id);
    for (i, text) in rows.iter().enumerate() {
        if shell.panels.nary_end.as_ref().is_some_and(|e| e.association == id && e.end == i) {
            y = editor(shell, ui, p, r, y);
            continue;
        }
        let row = Rect::from_min_size(pos2(left, y), vec2(width, ROW));
        let key = format!("workspace.properties.end.{i}");
        let resp = control(shell, ui, &key, "row", row, true).accessible_name(text.as_str()).on_hover_text(text.as_str());
        let paint = ui.painter().clone();
        if asking.is_some_and(|q| q.end == i) {
            paint.rect_filled(row, 6.0, p.accent_tint);
        } else if resp.contains_pointer() {
            paint.rect_filled(row, 6.0, p.page);
        }
        ux::text_mid(&paint, row.left() + 10.0, row.center().y, text, types::UI, p.text, Some((width - 50.0).max(0.0)));
        if crate::panels::edit_button(shell, ui, p, row, &key, resp.contains_pointer()) || resp.double_clicked() {
            open(shell, id, i);
        }
        y += ROW;
    }
    if let Some(q) = asking {
        y = ask_remove(shell, ui, p, r, y + 8.0, a, q);
    }
    let paint = ui.painter().clone();
    paint.rect_filled(Rect::from_min_size(pos2(r.left(), y + 10.0), vec2(r.width(), 1.0)), 0.0, p.border);
    let said = tr(SELECTING);
    let g = ux::galley_rows(&paint, &said, types::CAPTION, p.text_secondary, Some(width), 6);
    let h = g.size().y;
    let note = Rect::from_min_size(pos2(left, y + 26.0), vec2(width, h));
    paint.galley(note.min, g, p.text_secondary);
    widgets::record_visible(ui, &mut shell.registry, "workspace.properties.association.note", "text", note);
}

/// The refusal box under a field: the danger's tint, its icon and the reason; gives its height.
fn refusal_box(paint: &egui::Painter, p: &Palette, at: Pos2, width: f32, why: &str) -> f32 {
    let g = ux::galley_rows(paint, why, types::SMALL, p.text, Some((width - 40.0).max(0.0)), 6);
    let r = Rect::from_min_size(at, vec2(width.max(0.0), g.size().y + 20.0));
    paint.rect_filled(r, 6.0, p.danger_tint);
    ux::icon_in(paint, Rect::from_min_size(r.min + vec2(10.0, 11.0), Vec2::splat(14.0)), &icons::ISSUE, p.danger);
    paint.galley(r.min + vec2(32.0, 10.0), g, p.text);
    r.height()
}

/// The longest multiplicity typed.
const MAX_MULTIPLICITY: usize = 24;

/// The end editor card (A4) for the end being edited, at `y`: Role and Type, Multiplicity and
/// Navigable, Ordered and Unique, the note on aggregation, then the bin at its foot on the left and
/// Cancel and Apply on the right. Gives the y under it.
fn editor(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect, y: f32) -> f32 {
    let paint = ui.painter().clone();
    let width = (r.width() - 2.0 * PAD).max(0.0);
    let Some(mut edit) = shell.panels.nary_end.clone() else { return y };
    let background = paint.add(egui::Shape::Noop);
    let left = r.left() + PAD;
    let inner = (width - 24.0).max(0.0);
    let half = ((inner - 8.0) / 2.0).max(0.0);
    let at = |x: f32, dy: f32| pos2(left + 12.0 + x, y + dy);
    let row = 17.0 + FIELD + 10.0;
    let take = std::mem::take(&mut edit.opened);
    let key = format!("workspace.properties.end.{}", edit.end);
    let refused_at = edit.refused.as_ref().and_then(|(at, _)| *at);
    let mut cy = 12.0;
    let mut enter = false;
    let typed = edit.role.clone();
    let role_id = format!("{key}.role");
    enter |= text_field(shell, ui, p, at(0.0, cy), half, &role_id, &tr("Role"), &mut edit.role, archforge_uml::commands::MAX_NAME, take, false);
    if refused_at == Some("role") {
        if edit.role != typed {
            // Typing again takes the reason away until the next Apply.
            edit.refused = None;
        } else {
            let field = Rect::from_min_size(at(0.0, cy + 17.0), vec2(half, FIELD));
            paint.rect_stroke(field, 6.0, Stroke::new(1.0, p.danger), StrokeKind::Inside);
        }
    }
    let types_of = shell.session.as_ref().map(|s| end_types(s.doc())).unwrap_or_default();
    let type_id = format!("{key}.type");
    let shown = edit.ty.and_then(|t| types_of.iter().find(|(id, _)| *id == t)).map(|(_, n)| n.clone()).unwrap_or_default();
    let type_field = select_field(shell, ui, p, at(half + 8.0, cy), half, &type_id, &tr("Type"), &shown, Menu::NaryEndType);
    cy += row;
    if let Some((Some("role"), why)) = edit.refused.clone() {
        cy += refusal_box(&paint, p, at(0.0, cy - 4.0), inner, &why) + 8.0;
    }
    let m_id = format!("{key}.multiplicity");
    let typed_m = edit.multiplicity.clone();
    let m_label = tr("Multiplicity");
    let combo = crate::panels::Combo {
        id: &m_id,
        label: &m_label,
        name: &m_label,
        placeholder: "—",
        options: &crate::uml_relate::MULTIPLICITIES,
        current: crate::uml_relate::MULTIPLICITIES.iter().position(|m| *m == edit.multiplicity),
        pick: &tr("Pick a multiplicity"),
        pick_name: &tr("Pick a multiplicity"),
        hint: &tr("Or type a number or range"),
        clear: &tr("Clear"),
        refused: edit.refused.as_ref().filter(|(at, _)| *at == Some("multiplicity")).map(|(_, why)| why.as_str()),
        menu: Menu::NaryEndMultiplicity,
        limit: MAX_MULTIPLICITY,
    };
    match crate::panels::editable_select(shell, ui, p, at(0.0, cy), half, &mut edit.multiplicity, &combo) {
        crate::panels::ComboOut::Pick(k) => {
            edit.multiplicity = crate::uml_relate::MULTIPLICITIES.get(k).map(|m| (*m).to_string()).unwrap_or_default();
        }
        crate::panels::ComboOut::Commit => enter |= ui.input(|i| i.key_pressed(egui::Key::Enter)),
        crate::panels::ComboOut::Cancel | crate::panels::ComboOut::Idle => {}
    }
    if refused_at == Some("multiplicity") && edit.multiplicity != typed_m {
        edit.refused = None;
    }
    check(shell, ui, p, at(half + 8.0, cy + 23.0), &format!("{key}.navigable"), &tr("Navigable"), &mut edit.navigable);
    cy += row;
    if let Some((Some("multiplicity"), why)) = edit.refused.clone() {
        cy += refusal_box(&paint, p, at(0.0, cy - 4.0), inner, &why) + 8.0;
    }
    check(shell, ui, p, at(0.0, cy), &format!("{key}.ordered"), &tr("Ordered"), &mut edit.ordered);
    check(shell, ui, p, at(half + 8.0, cy), &format!("{key}.unique"), &tr("Unique"), &mut edit.unique);
    cy += 28.0;
    let note =
        tr("Ends of an n-ary association are not aggregations, so Aggregation is not offered. Repeat a type for two ends when the roles differ.");
    let g = ux::galley_rows(&paint, &note, types::CAPTION, p.text_secondary, Some(inner), 5);
    let h = g.size().y;
    paint.galley(at(0.0, cy), g, p.text_secondary);
    cy += h + 12.0;
    let remove = icon_button(shell, ui, p, at(0.0, cy), &format!("{key}.remove"), &icons::TRASH, p.danger, None, "Remove");
    let cancel =
        icon_button(shell, ui, p, at(inner - 2.0 * FIELD - 8.0, cy), &format!("{key}.cancel"), &icons::CLOSE, p.text_secondary, None, "Cancel");
    let apply = icon_button(shell, ui, p, at(inner - FIELD, cy), &format!("{key}.apply"), &icons::TICK, p.on_accent, Some(p.accent_fill), "Apply");
    cy += FIELD + 12.0;
    if let Some((None, why)) = &edit.refused {
        let g = ux::galley_rows(&paint, why, types::CAPTION, p.amber, Some(inner), 4);
        let h = g.size().y;
        paint.galley(at(0.0, cy - 4.0), g, p.amber);
        cy += h + 8.0;
    }
    let card = Rect::from_min_size(pos2(left, y), vec2(width, cy));
    paint.set(background, egui::Shape::rect_filled(card, 8.0, p.surface));
    paint.rect_stroke(card, 8.0, Stroke::new(1.0, p.accent), StrokeKind::Inside);
    crate::panels::reveal(shell, ui, card);
    // The type menu last, over the card: a pick changes the field; Apply moves the end.
    if shell.panels.menu == Some(Menu::NaryEndType) {
        let names: Vec<String> = types_of.iter().map(|(_, n)| n.clone()).collect();
        if let Some(k) = crate::panels::menu(shell, ui, p, type_field, &type_id, &names)
            && let Some((t, _)) = types_of.get(k)
        {
            edit.ty = Some(*t);
            shell.panels.menu = None;
        }
    }
    shell.panels.nary_end = Some(edit.clone());
    let escape = shell.panels.menu.is_none() && ui.input(|i| i.key_pressed(egui::Key::Escape));
    if cancel || escape {
        shell.panels.nary_end = None;
    } else if remove {
        shell.panels.nary_end = None;
        remove_end(shell, edit.association, edit.end);
    } else if apply || enter {
        commit(shell, &edit);
    }
    card.bottom() + 8.0
}

/// Why the end as edited cannot be written, in the user's words, with the field it sits under: a
/// role another end has, whatever the case, or a multiplicity that does not read. The commands keep
/// the same rules.
fn refusal(store: &Store, edit: &EndEdit) -> Option<(Option<&'static str>, String)> {
    let a = store.by_uuid(edit.association)?;
    let ends = nary::ends(store, a);
    let role = edit.role.trim();
    if !role.is_empty()
        && let Some(other) = ends.iter().enumerate().find(|(k, e)| *k != edit.end && name_of(store, **e).to_lowercase() == role.to_lowercase())
    {
        return Some((Some("role"), fmt(&tr("Another end has the role {role} already. Pick another role."), &[("role", &name_of(store, *other.1))])));
    }
    if let Err(e) = archforge_uml::members::parse_multiplicity(edit.multiplicity.trim()) {
        return Some((Some("multiplicity"), crate::uml_relate::multiplicity_problem(&e)));
    }
    None
}

/// Writes the end as edited as one undo step, its class moved first when Type changed, then closes
/// the editor; a refusal keeps it open with the reason under its field, the model untouched.
fn commit(shell: &mut Shell, edit: &EndEdit) {
    if let Some(why) = shell.session.as_ref().and_then(|s| refusal(s.doc(), edit)) {
        if let Some(e) = shell.panels.nary_end.as_mut() {
            e.refused = Some(why);
        }
        return;
    }
    let Some(store) = shell.session.as_ref().map(|s| s.doc()) else { return };
    let moved = store
        .by_uuid(edit.association)
        .and_then(|a| nary::ends(store, a).get(edit.end).copied())
        .and_then(|e| nary::end_type(store, e))
        .and_then(|t| uuid(store, t))
        != edit.ty;
    let (a, i) = (edit.association.to_string(), edit.end);
    let class = edit.ty.map(|t| t.to_string());
    let set = json!({
        "association": a,
        "end": i,
        "role": edit.role.trim(),
        "multiplicity": edit.multiplicity.trim(),
        "navigable": edit.navigable,
        "ordered": edit.ordered,
        "unique": edit.unique,
    });
    let Some(session) = shell.session.as_mut() else { return };
    let out = session.batch(&tr("Set association end"), |b| {
        if moved && let Some(class) = &class {
            b.execute("uml.association.reconnect-end", &json!({ "association": a, "end": i, "class": class }))?;
        }
        b.execute("uml.association.set-end", &set)?;
        Ok(())
    });
    match out {
        Ok(o) => {
            shell.panels.nary_end = None;
            shell.panels.notice = None;
            if let Some(first) = o.value.get(0).filter(|_| moved) {
                say_dropped(shell, first);
            }
        }
        Err(e) => {
            if let Some(ed) = shell.panels.nary_end.as_mut() {
                ed.refused = Some((None, e.to_string()));
            }
        }
    }
}

/// The bin of end `i` of association `id`: of four ends or more the end goes at once, one undo
/// step said by a toast with Undo; of three, the question of A5 opens instead.
pub(crate) fn remove_end(shell: &mut Shell, id: Uuid, i: usize) {
    let Some(store) = shell.session.as_ref().map(|s| s.doc()) else { return };
    let Some(a) = store.by_uuid(id) else { return };
    let ends = nary::ends(store, a);
    let Some(end) = ends.get(i).copied() else { return };
    if ends.len() <= nary::MIN_ENDS {
        shell.panels.nary_remove = Some(RemoveAsk { association: id, end: i, opened: true });
        return;
    }
    let called = {
        let role = name_of(store, end);
        if role.is_empty() { nary::end_type(store, end).map(|t| name_of(store, t)).unwrap_or_default() } else { role }
    };
    match run(shell, "uml.association.remove-end", json!({ "association": id.to_string(), "end": i })) {
        Ok(_) => {
            let text = fmt(&tr("End {role} removed"), &[("role", &called)]);
            let undo_label = shell.session.as_ref().and_then(|s| s.undo_label().map(str::to_string));
            shell.canvas.toast = Some(crate::connect::Toast { text, until: shell.frames.saturating_add(crate::connect::TOAST_FRAMES), undo_label });
        }
        Err(e) => shell.panels.notice = Some(e),
    }
}

/// What removing one of three ends asks (A5), at `y` under the rows: two ends would remain, so keep
/// the three (focused, and what Escape does) or make it binary, one undo step. Gives the y under
/// it.
#[allow(clippy::too_many_arguments, reason = "the question is drawn into the panel for its association")]
fn ask_remove(shell: &mut Shell, ui: &egui::Ui, p: &Palette, r: Rect, y: f32, a: Obj, q: RemoveAsk) -> f32 {
    let Some(store) = shell.session.as_ref().map(|s| s.doc()) else { return y };
    let ends = nary::ends(store, a);
    let Some(end) = ends.get(q.end).copied() else {
        shell.panels.nary_remove = None;
        return y;
    };
    let class_of = |e: Obj| nary::end_type(store, e).map(|t| name_of(store, t)).unwrap_or_default();
    let left_over: Vec<String> = ends.iter().filter(|e| **e != end).map(|e| class_of(*e)).collect();
    let (first, second) = (left_over.first().cloned().unwrap_or_default(), left_over.get(1).cloned().unwrap_or_default());
    let role = {
        let role = name_of(store, end);
        if role.is_empty() { class_of(end) } else { role }
    };
    let name = name_of(store, a);
    let a_id = uuid(store, a).map(|u| u.to_string()).unwrap_or_default();
    let paint = ui.painter().clone();
    let width = (r.width() - 2.0 * PAD).max(0.0);
    let left = r.left() + PAD;
    let inner = (width - 28.0).max(0.0);
    let background = paint.add(egui::Shape::Noop);
    let mut cy = y + 14.0;
    ux::icon_in(&paint, Rect::from_min_size(pos2(left + 14.0, cy + 1.0), Vec2::splat(15.0)), &icons::WARNING, p.amber);
    ux::text(&paint, pos2(left + 37.0, cy), &tr("Two ends would remain"), types::UI_STRONG, p.text, Some((inner - 23.0).max(0.0)));
    cy += 26.0;
    let why = fmt(
        &tr("An n-ary association needs three ends or more. Without {role}, only {first} and {second} remain."),
        &[("role", &role), ("first", &first), ("second", &second)],
    );
    let g = ux::galley_rows(&paint, &why, types::SMALL, p.text_secondary, Some(inner), 6);
    let h = g.size().y;
    paint.galley(pos2(left + 14.0, cy), g, p.text_secondary);
    cy += h + 12.0;
    // The two buttons, Keep 3 ends quiet and focused, Make it binary primary, side by side.
    let keep_label = tr("Keep 3 ends");
    let binary_label = tr("Make it binary");
    let keep_g = ux::galley(&paint, &keep_label, types::SMALL, p.text, None);
    let binary_g = ux::galley(&paint, &binary_label, types::SMALL, p.on_accent, None);
    let keep = Rect::from_min_size(pos2(left + 14.0, cy), vec2(keep_g.size().x + 24.0, FIELD));
    let binary = Rect::from_min_size(pos2(keep.right() + 8.0, cy), vec2(binary_g.size().x + 24.0, FIELD));
    paint.rect_filled(keep, 6.0, p.surface);
    paint.rect_stroke(keep, 6.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
    paint.galley(keep.center() - keep_g.size() / 2.0, keep_g, p.text);
    paint.rect_filled(binary, 6.0, p.accent_fill);
    paint.galley(binary.center() - binary_g.size() / 2.0, binary_g, p.on_accent);
    let keep_id = "workspace.properties.association.ends.removeConfirm.keep";
    let binary_id = "workspace.properties.association.ends.removeConfirm.binary";
    let keep_resp = control(shell, ui, keep_id, "button", keep, true).accessible_name(keep_label.as_str());
    let binary_resp = control(shell, ui, binary_id, "button", binary, true).accessible_name(binary_label.as_str());
    if q.opened {
        keep_resp.request_focus();
        if let Some(ask) = shell.panels.nary_remove.as_mut() {
            ask.opened = false;
        }
    }
    for (resp, b) in [(&keep_resp, keep), (&binary_resp, binary)] {
        if resp.has_focus() {
            paint.rect_stroke(b.expand(2.0), 8.0, Stroke::new(2.0, p.focus), StrokeKind::Outside);
        }
    }
    cy += FIELD + 14.0;
    let card = Rect::from_min_max(pos2(left, y), pos2(left + width, cy));
    paint.set(background, egui::Shape::rect_filled(card, 8.0, p.amber_tint));
    let after = if name.is_empty() {
        fmt(&tr(BINARY_UNNAMED), &[("first", &first), ("second", &second)])
    } else {
        fmt(&tr(BINARY_NAMED), &[("name", &name), ("first", &first), ("second", &second)])
    };
    let g = ux::galley_rows(&paint, &after, types::CAPTION, p.text_secondary, Some(width), 6);
    let h = g.size().y;
    paint.galley(pos2(left, card.bottom() + 12.0), g, p.text_secondary);
    crate::panels::reveal(shell, ui, card);
    let escape = ui.input(|i| i.key_pressed(egui::Key::Escape));
    if keep_resp.clicked() || escape {
        shell.panels.nary_remove = None;
    } else if binary_resp.clicked() {
        shell.panels.nary_remove = None;
        if let Err(e) = run(shell, "uml.association.remove-end", json!({ "association": a_id, "end": q.end, "binary": true })) {
            shell.panels.notice = Some(e);
        }
    }
    card.bottom() + 12.0 + h + 8.0
}

/// Drops an editor or a question whose association, or end, is gone.
pub(crate) fn retain_valid(state: &mut crate::panels::State, store: &Store) {
    let ends = |id: Uuid| store.by_uuid(id).filter(|a| store.is_alive(*a) && nary::is_nary(store, *a)).map(|a| nary::ends(store, a).len());
    if state.nary_end.as_ref().is_some_and(|e| ends(e.association).is_none_or(|n| e.end >= n)) {
        state.nary_end = None;
    }
    if state.nary_remove.as_ref().is_some_and(|q| ends(q.association).is_none_or(|n| q.end >= n)) {
        state.nary_remove = None;
    }
}

/// In the sample class model, a class diagram Recommendations of Commerce with Customer and
/// Product side by side and Catalog under them, opened; `drawn`, the n-ary association recommends
/// between the three as one step, its Catalog end at `1`. The project is then marked saved as it
/// stands. Gives the ids by name: the classes, their cards (lower case), the diagram, and, drawn,
/// the association and its diamond.
#[cfg(any(test, feature = "snapshot"))]
pub(crate) fn put_recommends(shell: &mut Shell, drawn: bool) -> Result<std::collections::BTreeMap<&'static str, String>, String> {
    let session = shell.session.as_mut().ok_or("the sample needs a session")?;
    let store = session.doc();
    let find = |n: &str| {
        store
            .alive()
            .find(|o| store.class_of(*o).is_ok_and(archforge_uml::is_classifier) && name_of(store, *o) == n)
            .ok_or_else(|| format!("the sample has no {n}"))
    };
    let (customer, product, catalog) = (find("Customer")?, find("Product")?, find("Catalog")?);
    let pkg = archforge_uml::nesting::home_package(store, customer).ok_or("Customer's package")?;
    let id = |o: Obj| store.uuid(o).map(|u| u.to_string()).map_err(|e| e.to_string());
    let height = |c: Obj| {
        crate::uml_ui::members_height(archforge_uml::members::attributes(store, c).len(), archforge_uml::members::operations(store, c).len())
    };
    let cards = [(id(customer)?, 80, 80, height(customer)), (id(product)?, 640, 80, height(product)), (id(catalog)?, 360, 560, height(catalog))];
    let mut ids = std::collections::BTreeMap::new();
    ids.insert("Customer", cards[0].0.clone());
    ids.insert("Product", cards[1].0.clone());
    ids.insert("Catalog", cards[2].0.clone());
    let pkg = id(pkg)?;
    let pick = |v: &serde_json::Value| v.get("id").and_then(|i| i.as_str()).unwrap_or_default().to_string();
    let mut made: Vec<String> = Vec::new();
    session
        .batch("Add the recommendations diagram", |b| {
            let d = pick(&b.execute("diagram.add", &json!({ "kind": archforge_uml::CLASS_DIAGRAM, "element": pkg, "name": "Recommendations" }))?);
            made.push(d.clone());
            for (c, x, y, h) in &cards {
                made.push(pick(&b.execute("diagram.place", &json!({ "diagram": d, "element": c, "x": x, "y": y, "width": 200, "height": h }))?));
            }
            Ok(())
        })
        .map_err(|e| e.to_string())?;
    let [d, customer_node, product_node, catalog_node] = <[String; 4]>::try_from(made).map_err(|_| "the diagram and three cards".to_string())?;
    if drawn {
        let place = {
            let store = session.doc();
            let rects: Vec<archforge_diagram::Rect> = [&customer_node, &product_node, &catalog_node]
                .iter()
                .filter_map(|n| Uuid::parse(n).and_then(|u| store.by_uuid(u)))
                .map(|n| archforge_diagram::grid::bounds_of(store, n))
                .collect();
            diamond_place(&rects).ok_or("the diamond's place")?
        };
        let classes = [cards[0].0.clone(), cards[1].0.clone(), cards[2].0.clone()];
        let nodes = [customer_node.clone(), product_node.clone(), catalog_node.clone()];
        let mut out: Vec<String> = Vec::new();
        session
            .batch("Add n-ary association", |b| {
                let a = b.execute("uml.association.add-nary", &json!({ "classes": classes, "name": "recommends" }))?;
                let ends: Vec<String> =
                    a.get("ends").and_then(|e| e.as_array()).into_iter().flatten().filter_map(|e| e.as_str().map(str::to_string)).collect();
                let n = b.execute(
                    "diagram.place",
                    &json!({ "diagram": d, "element": pick(&a), "x": place.x, "y": place.y, "width": place.width, "height": place.height }),
                )?;
                for (end, card) in ends.iter().zip(&nodes) {
                    b.execute("diagram.connect", &json!({ "diagram": d, "element": end, "source": pick(&n), "target": card }))?;
                }
                b.execute("uml.association.set-end", &json!({ "association": pick(&a), "end": 2, "multiplicity": "1" }))?;
                out.push(pick(&a));
                out.push(pick(&n));
                Ok(())
            })
            .map_err(|e| e.to_string())?;
        ids.insert("recommends", out.first().cloned().unwrap_or_default());
        ids.insert("diamond", out.get(1).cloned().unwrap_or_default());
    }
    session.mark_saved().map_err(|e| e.to_string())?;
    ids.insert("customer", customer_node);
    ids.insert("product", product_node);
    ids.insert("catalog", catalog_node);
    ids.insert("diagram", d.clone());
    crate::canvas::open(shell, Uuid::parse(&d).ok_or("the diagram's id")?);
    Ok(ids)
}

/// Every word this module says through `tr` other than as a literal call, for the catalogue.
#[cfg(test)]
pub(crate) fn texts() -> impl Iterator<Item = &'static str> {
    ["Add end", "Remove", "Cancel", "Apply", FINISHES, SELECTING, BINARY_UNNAMED, BINARY_NAMED].into_iter()
}
