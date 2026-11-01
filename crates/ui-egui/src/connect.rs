//! Drawing relationships on the ER canvas (slice E2 of archforge-gm2.10, from the design's
//! `Popover / Cardinality`, the frame "ER · Properties · Relationship drawn" and the cards "Many
//! to many picked", "Roles on lines" and "Column removed with its keys"): the Relationship tool
//! drags a line from one entity to another, the cardinality popover beside the tool sets the ends
//! and whether the relationship is identifying, and the pick stays for the next line. On release
//! the relationship, its edge and its name label are one undo step; the foreign key comes with it
//! when the parent already has a primary key (owner, 2026-10-29: drawing never blocks). A
//! many-to-many line offers its mapping table; Delete removes a selected relationship and a toast
//! offers Undo.

use archforge_diagram::grid::bounds_of;
use archforge_er::Cardinality;
use archforge_mof::{Obj, Store, Uuid};
use egui::{Pos2, Rect, Stroke, StrokeKind, Vec2, pos2, vec2};
use serde_json::json;

use crate::canvas::{View, cardinality_text, end_glyph};
use crate::home::control;
use crate::i18n::{fmt, tr};
use crate::ux::{self, Palette, icons, types};
use crate::{Shell, widgets};

/// The popover's size and its rows.
const POPOVER: Vec2 = vec2(330.0, 281.0);
const CHOICE: Vec2 = vec2(64.0, 26.0);
const CHOICE_PITCH: f32 = 76.0;
/// The mapping table offer's size.
const OFFER: Vec2 = vec2(300.0, 200.0);
/// A toast's size and how many frames it stays.
const TOAST: Vec2 = vec2(440.0, 40.0);
const TOAST_FRAMES: u64 = 300;
/// How far a self relationship's loop reaches out of its entity.
/// The diagram's self-loop, on screen (the grid hit-tests the same shape).
const LOOP_REACH: f32 = archforge_diagram::grid::LOOP_REACH as f32;
const LOOP_HALF: f32 = archforge_diagram::grid::LOOP_HALF as f32;

/// The cardinalities a new relationship gets, and whether it is identifying.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RelKind {
    pub from: Cardinality,
    pub to: Cardinality,
    pub identifying: bool,
}

impl Default for RelKind {
    fn default() -> Self {
        Self { from: Cardinality::ExactlyOne, to: Cardinality::ZeroOrMore, identifying: false }
    }
}

impl RelKind {
    pub fn many_to_many(self) -> bool {
        archforge_er::rules::is_many(self.from) && archforge_er::rules::is_many(self.to)
    }
}

/// The presets of the popover: one to many, one to one-or-more, one to zero-or-one, many to many.
const PRESETS: [(Cardinality, Cardinality); 4] = [
    (Cardinality::ExactlyOne, Cardinality::ZeroOrMore),
    (Cardinality::ExactlyOne, Cardinality::OneOrMore),
    (Cardinality::ExactlyOne, Cardinality::ZeroOrOne),
    (Cardinality::ZeroOrMore, Cardinality::ZeroOrMore),
];
/// The ends a row offers, in the design's order.
const ENDS: [Cardinality; 4] = [Cardinality::ZeroOrOne, Cardinality::ExactlyOne, Cardinality::ZeroOrMore, Cardinality::OneOrMore];

/// The offer to add a mapping table for a many-to-many relationship just drawn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MappingOffer {
    pub relationship: Uuid,
    pub name: String,
    pub refused: Option<String>,
}

/// A line of text at the canvas's foot for a while, with Undo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Toast {
    pub text: String,
    pub until: u64,
    /// The undo step the toast speaks of: once another step tops the history, the toast goes.
    pub undo_label: Option<String>,
}

fn run(shell: &mut Shell, id: &str, params: serde_json::Value) -> Result<serde_json::Value, String> {
    let Some(session) = shell.session.as_mut() else { return Err(tr("The model could not be opened")) };
    session.execute(id, &params).map(|o| o.value).map_err(|e| e.to_string())
}

pub(crate) fn cardinality_word(c: Cardinality) -> &'static str {
    match c {
        Cardinality::ZeroOrOne => "zero-or-one",
        Cardinality::ExactlyOne => "exactly-one",
        Cardinality::ZeroOrMore => "zero-or-more",
        Cardinality::OneOrMore => "one-or-more",
    }
}

/// Whether a foreign key realises the relationship.
pub fn has_foreign_key(store: &Store, rel: Obj) -> bool {
    store.referrers(rel).into_iter().any(|(_, p)| p == archforge_er::foreign_key::RELATIONSHIP)
}

/// The points of a self relationship's loop on the entity's right side.
pub fn self_loop(rect: Rect, z: f32) -> Vec<Pos2> {
    let cy = rect.center().y;
    vec![
        pos2(rect.right(), cy - LOOP_HALF * z),
        pos2(rect.right() + LOOP_REACH * z, cy - LOOP_HALF * z),
        pos2(rect.right() + LOOP_REACH * z, cy + LOOP_HALF * z),
        pos2(rect.right(), cy + LOOP_HALF * z),
    ]
}

/// Draws the ends of a short sample line over `fill`, as the popover's choices show them.
#[allow(clippy::too_many_arguments, reason = "a sample is drawn from its ends, its ink and what it is drawn on")]
pub(crate) fn sample_line(
    paint: &egui::Painter,
    p: &Palette,
    centre: Pos2,
    from: Option<Cardinality>,
    to: Option<Cardinality>,
    ink: egui::Color32,
    fill: egui::Color32,
) {
    let z = 0.7;
    let (a, b) = (centre - vec2(18.0, 0.0), centre + vec2(18.0, 0.0));
    let stroke = Stroke::new(1.2, ink);
    paint.line_segment([a, b], stroke);
    crate::canvas::end_glyph_on(paint, p, a, b, from, stroke, z, false, fill);
    crate::canvas::end_glyph_on(paint, p, b, a, to, stroke, z, false, fill);
}

/// The relationship tool's icon: a line with the ends the tool draws, at the size the popover's
/// samples show them, across its double-width button, over `fill`.
pub(crate) fn tool_line(
    paint: &egui::Painter,
    p: &Palette,
    centre: Pos2,
    from: Cardinality,
    to: Cardinality,
    ink: egui::Color32,
    fill: egui::Color32,
) {
    let z = 0.7;
    let (a, b) = (centre - vec2(24.0, 0.0), centre + vec2(24.0, 0.0));
    let stroke = Stroke::new(1.2, ink);
    paint.line_segment([a, b], stroke);
    crate::canvas::end_glyph_on(paint, p, a, b, Some(from), stroke, z, false, fill);
    crate::canvas::end_glyph_on(paint, p, b, a, Some(to), stroke, z, false, fill);
}

/// The cardinality popover above the toolbar's Relationship chevron.
pub(crate) fn popover(shell: &mut Shell, ui: &egui::Ui, p: &Palette, r: Rect, anchor: Rect) {
    let paint = ui.painter().clone();
    let x = (anchor.center().x - POPOVER.x / 2.0).clamp(r.left() + 8.0, (r.right() - POPOVER.x - 8.0).max(r.left() + 8.0));
    let y = (anchor.top() - 8.0 - POPOVER.y).max(r.top() + 8.0);
    let m = Rect::from_min_size(pos2(x, y), POPOVER);
    ux::shadow(&paint, m, 10.0, p.shadow);
    paint.rect_filled(m, 10.0, p.surface);
    paint.rect_stroke(m, 10.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
    let at = |x: f32, y: f32| m.min + vec2(x, y);
    let kind = shell.canvas.relationship;
    ux::text(&paint, at(16.0, 14.0), &tr("Cardinality").to_uppercase(), types::EYEBROW, p.text_secondary, Some(160.0));
    let readout = format!("{} — {}", cardinality_text(kind.from), cardinality_text(kind.to));
    let g = ux::galley(&paint, &readout, types::SMALL, p.text, None);
    paint.galley(pos2(m.right() - 16.0 - g.size().x, m.top() + 14.0), g, p.text);
    let mut next = kind;
    // Three rows of four choices.
    let rows: [(&str, f32); 3] = [("Presets", 50.0), ("Start", 118.0), ("End", 186.0)];
    for (row, (label, ly)) in rows.iter().enumerate() {
        ux::text(&paint, at(16.0, *ly), &tr(label).to_uppercase(), types::EYEBROW, p.text_muted, Some(200.0));
        for i in 0..4 {
            let b = Rect::from_min_size(at(16.0 + i as f32 * CHOICE_PITCH, ly + 14.0), CHOICE);
            let (from, to, on) = match row {
                0 => (Some(PRESETS[i].0), Some(PRESETS[i].1), (kind.from, kind.to) == PRESETS[i]),
                1 => (Some(ENDS[i]), None, kind.from == ENDS[i]),
                _ => (None, Some(ENDS[i]), kind.to == ENDS[i]),
            };
            paint.rect_filled(b, 6.0, if on { p.accent_tint } else { p.field });
            if on {
                paint.rect_stroke(b, 6.0, Stroke::new(1.0, p.accent), StrokeKind::Inside);
            }
            sample_line(&paint, p, b.center(), from, to, if on { p.accent } else { p.text_secondary }, if on { p.accent_tint } else { p.field });
            let key = ["preset", "start", "end"][row];
            let hint = match (from, to) {
                (Some(f), Some(t)) => format!("{} — {}", cardinality_text(f), cardinality_text(t)),
                (Some(f), None) => cardinality_text(f),
                (None, Some(t)) => cardinality_text(t),
                _ => String::new(),
            };
            if control(shell, ui, &format!("workspace.cardinality.{key}.{i}"), "button", b, true).on_hover_text(hint).clicked() {
                match row {
                    0 => {
                        next.from = PRESETS[i].0;
                        next.to = PRESETS[i].1;
                    }
                    1 => next.from = ENDS[i],
                    _ => next.to = ENDS[i],
                }
            }
        }
    }
    // Identifying: a solid sample and the switch.
    let iy = m.top() + 250.0;
    paint.line_segment([pos2(m.left() + 16.0, iy + 9.0), pos2(m.left() + 28.0, iy + 9.0)], Stroke::new(1.5, p.text_secondary));
    ux::text(&paint, pos2(m.left() + 40.0, iy), &tr("Identifying"), types::UI, p.text, Some(200.0));
    let sw = Rect::from_min_size(pos2(m.right() - 16.0 - 36.0, iy), vec2(36.0, 20.0));
    paint.rect_filled(sw, 10.0, if kind.identifying { p.accent_fill } else { p.border });
    let knob = if kind.identifying { sw.right_center() - vec2(10.0, 0.0) } else { sw.left_center() + vec2(10.0, 0.0) };
    paint.circle_filled(knob, 8.0, p.surface);
    if control(shell, ui, "workspace.cardinality.identifying", "toggle", sw, true).on_hover_text(tr("Identifying")).clicked() {
        next.identifying = !kind.identifying;
    }
    shell.canvas.relationship = next;
    let outside = ui.input(|i| i.pointer.any_pressed() && !i.pointer.interact_pos().is_some_and(|pos| m.contains(pos) || anchor.contains(pos)));
    if outside || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        shell.canvas.popover = false;
    }
}

/// The line being drawn: from the source entity's border to the pointer, with the chosen ends,
/// and a ring on the entity under the pointer.
pub(crate) fn live_line(shell: &Shell, paint: &egui::Painter, p: &Palette, view: View, from_node: Uuid, pointer: Pos2, target: Option<Rect>) {
    let Some(store) = shell.session.as_ref().map(|s| s.doc()) else { return };
    let Some(n) = store.by_uuid(from_node) else { return };
    let rect = view.rect(&bounds_of(store, n));
    if !rect.is_finite() || !pointer.is_finite() {
        return;
    }
    let kind = shell.canvas.relationship;
    let z = view.zoom;
    let stroke = Stroke::new((1.5 * z).clamp(1.0, 3.0), p.accent);
    let (start, end) = match target {
        Some(t) if t == rect => {
            let points = self_loop(rect, z);
            paint.add(egui::Shape::line(points.clone(), stroke));
            (points[0], points[3])
        }
        Some(t) => {
            let end = crate::canvas::exit_point(t, rect.center());
            let start = crate::canvas::exit_point(rect, end);
            line(paint, &[start, end], stroke, kind.identifying, z);
            (start, end)
        }
        None => {
            let start = crate::canvas::exit_point(rect, pointer);
            line(paint, &[start, pointer], stroke, kind.identifying, z);
            (start, pointer)
        }
    };
    end_glyph(paint, p, start, end, Some(kind.from), stroke, z, true);
    end_glyph(paint, p, end, start, Some(kind.to), stroke, z, true);
    if let Some(t) = target {
        paint.rect_stroke(t.expand(4.0), 4.0, Stroke::new(1.5, p.accent), StrokeKind::Inside);
    }
}

/// A line, solid for an identifying relationship and dashed for the rest.
pub fn line(paint: &egui::Painter, points: &[Pos2], stroke: Stroke, identifying: bool, z: f32) {
    if identifying {
        paint.add(egui::Shape::line(points.to_vec(), stroke));
    } else {
        paint.add(egui::Shape::dashed_line(points, stroke, 6.0 * z.max(0.5), 4.0 * z.max(0.5)));
    }
}

/// Makes the relationship a finished drag asks for, with its edge and its name label as one undo
/// step; the foreign key comes along when the parent has a primary key and neither table is
/// without columns (one still to be shaped is not given key columns unasked). Selects the
/// relationship; a many-to-many one opens the mapping table offer.
pub fn connect(shell: &mut Shell, d: Obj, from_node: Uuid, to_node: Uuid) {
    shell.canvas.notice = None;
    let kind = shell.canvas.relationship;
    let Some(session) = shell.session.as_mut() else { return };
    let store = session.doc();
    let node_entity = |id: Uuid| {
        let n = store.by_uuid(id)?;
        let e = store.get(n, archforge_diagram::node::ELEMENT).ok().and_then(|v| v.as_ref())?;
        (store.class_of(e).ok() == Some(archforge_er::ENTITY)).then_some(e)
    };
    let (Some(from), Some(to)) = (node_entity(from_node), node_entity(to_node)) else {
        shell.canvas.notice = Some(tr("A relationship joins two tables"));
        return;
    };
    let (Some(from_id), Some(to_id), Some(diagram_id)) = (store.uuid(from).ok(), store.uuid(to).ok(), store.uuid(d).ok()) else { return };
    // The parent is the one side; its primary key decides whether the foreign key comes now.
    let parent = if archforge_er::rules::is_many(kind.from) && !archforge_er::rules::is_many(kind.to) { to } else { from };
    let has_columns = |e: Obj| {
        store.list(e, archforge_er::entity::ATTRIBUTE).unwrap_or_default().iter().filter_map(archforge_mof::Value::as_ref).any(|a| store.is_alive(a))
    };
    let with_key = !kind.many_to_many() && archforge_er::rules::primary_key_of(store, parent).is_some() && has_columns(from) && has_columns(to);
    let add = json!({
        "from": from_id.to_string(),
        "to": to_id.to_string(),
        "from_end": cardinality_word(kind.from),
        "to_end": cardinality_word(kind.to),
        "identifying": kind.identifying && with_key,
        "create_foreign_key": with_key,
    });
    let out = session.batch(&tr("Add relationship"), |b| {
        let r = b.execute("er.relationship.add", &add)?;
        let rel = r.get("id").and_then(|i| i.as_str()).unwrap_or_default().to_string();
        let e = b.execute(
            "diagram.connect",
            &json!({ "diagram": diagram_id.to_string(), "element": &rel, "source": from_node.to_string(), "target": to_node.to_string() }),
        )?;
        let edge = e.get("id").and_then(|i| i.as_str()).unwrap_or_default().to_string();
        b.execute("diagram.label", &json!({ "edge": edge, "kind": "name", "at": "middle" }))?;
        // The key columns the foreign key added stay in sight.
        crate::editing::grow_to_fit(b, &[from_id, to_id])
    });
    match out {
        Ok(o) => {
            let Some(rel) = o.value.get(0).and_then(|v| v.get("id")).and_then(|v| v.as_str()).and_then(Uuid::parse) else { return };
            if let Err(e) = run(shell, "view.select", json!({ "ids": [rel.to_string()] })) {
                shell.canvas.notice = Some(e);
            }
            if kind.many_to_many() {
                offer_mapping(shell, rel);
            } else if kind.identifying && !with_key {
                shell.canvas.notice = Some(tr("Identifying needs the parent's primary key; drawn as not identifying"));
            }
        }
        Err(e) => shell.canvas.notice = Some(e.to_string()),
    }
}

/// Opens the mapping table offer for a many-to-many relationship, named after its two tables.
pub fn offer_mapping(shell: &mut Shell, relationship: Uuid) {
    let Some(store) = shell.session.as_ref().map(|s| s.doc()) else { return };
    let Some(rel) = store.by_uuid(relationship) else { return };
    let end = |p| store.get(rel, p).ok().and_then(|v| v.as_ref()).map(|e| archforge_core::name_of(store, e)).unwrap_or_default();
    let name = format!("{}{}", end(archforge_er::relationship::FROM), end(archforge_er::relationship::TO));
    shell.canvas.mapping = Some(MappingOffer { relationship, name, refused: None });
}

/// The mapping table offer, over the canvas: the design's card "Many to many picked".
pub(crate) fn mapping_offer(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect) {
    let Some(offer) = shell.canvas.mapping.clone() else { return };
    let (from, to, chips) = match shell.session.as_ref().and_then(|s| s.doc().by_uuid(offer.relationship).map(|rel| (s.doc(), rel))) {
        Some((store, rel)) => {
            let end = |p| store.get(rel, p).ok().and_then(|v| v.as_ref());
            let name = |e: Option<Obj>| e.map(|e| archforge_core::name_of(store, e)).unwrap_or_default();
            let (f, t) = (end(archforge_er::relationship::FROM), end(archforge_er::relationship::TO));
            let mut chips = Vec::new();
            for (i, e) in [f, t].into_iter().enumerate() {
                let Some(e) = e else { continue };
                for c in archforge_er::rules::key_columns(store, e) {
                    chips.push(format!("{} · PK FK{}", archforge_core::name_of(store, c), i + 1));
                }
            }
            (name(f), name(t), chips)
        }
        None => {
            shell.canvas.mapping = None;
            return;
        }
    };
    let paint = ui.painter().clone();
    let m = Rect::from_center_size(r.center(), OFFER);
    ux::shadow(&paint, m, 10.0, p.shadow);
    paint.rect_filled(m, 10.0, p.surface);
    paint.rect_stroke(m, 10.0, Stroke::new(1.0, p.dialog_edge), StrokeKind::Inside);
    let at = |x: f32, y: f32| m.min + vec2(x, y);
    let width = OFFER.x - 32.0;
    ux::text(&paint, at(16.0, 16.0), &fmt(&tr("Many to many · {from} and {to}"), &[("from", &from), ("to", &to)]), types::UI, p.text, Some(width));
    ux::text(&paint, at(16.0, 44.0), &tr("Mapping table"), types::CAPTION, p.text_secondary, Some(width));
    let field = Rect::from_min_size(at(16.0, 61.0), vec2(width, 28.0));
    paint.rect_filled(field, 6.0, p.field);
    widgets::record_visible(ui, &mut shell.registry, "workspace.mapping.name", "field", field);
    let inner = crate::panels::field_inner(ui, Rect::from_min_max(pos2(field.left(), field.top()), pos2(field.right(), field.bottom())));
    if let Some(o) = shell.canvas.mapping.as_mut() {
        let edit = egui::TextEdit::singleline(&mut o.name)
            .id(widgets::egui_id("workspace.mapping.name"))
            .frame(egui::Frame::NONE)
            .margin(egui::Margin::ZERO)
            .font(types::SMALL.font())
            .text_color(p.text)
            .char_limit(archforge_er::rules::MAX_NAME)
            .desired_width(inner.width());
        let out = edit.show(&mut ui.new_child(egui::UiBuilder::new().max_rect(inner)));
        let _ = (*out.response).clone().accessible_name(tr("Mapping table"));
    }
    ux::text(&paint, at(16.0, 101.0), &tr("Columns"), types::CAPTION, p.text_secondary, Some(width));
    let mut cx = at(16.0, 118.0).x;
    for chip in chips.iter().take(4) {
        let g = ux::galley(&paint, chip, types::CAPTION, p.text, Some(width));
        let b = Rect::from_min_size(pos2(cx, at(0.0, 118.0).y), vec2(g.size().x + 16.0, 24.0));
        if b.right() > m.right() - 16.0 {
            break;
        }
        paint.rect_filled(b, 6.0, p.field);
        paint.galley(b.min + vec2(8.0, 4.0), g, p.text);
        cx = b.right() + 8.0;
    }
    if let Some(refused) = &offer.refused {
        ux::text(&paint, at(16.0, 150.0), refused, types::CAPTION, p.amber, Some(width));
    }
    let mut x = m.right() - 16.0;
    let mut action: Option<bool> = None;
    for (id, label, primary) in [("workspace.mapping.add", tr("Add mapping table"), true), ("workspace.mapping.keep", tr("Keep the line"), false)] {
        let g = ux::galley(&paint, &label, types::SMALL, p.text, None);
        let b = Rect::from_min_max(pos2(x - g.size().x - 24.0, m.bottom() - 44.0), pos2(x, m.bottom() - 16.0));
        paint.rect_filled(b, 6.0, if primary { p.accent_fill } else { p.field });
        paint.galley(b.min + vec2(12.0, (b.height() - g.size().y) / 2.0), g, if primary { p.on_accent } else { p.text });
        if control(shell, ui, id, "button", b, true).clicked() {
            action = Some(primary);
        }
        x = b.left() - 8.0;
    }
    if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
        action = Some(false);
    }
    match action {
        Some(true) => {
            let name = shell.canvas.mapping.as_ref().map(|o| o.name.trim().to_string()).unwrap_or_default();
            match run(shell, "er.relationship.resolve", json!({ "relationship": offer.relationship.to_string(), "name": name })) {
                Ok(_) => shell.canvas.mapping = None,
                Err(e) => {
                    if let Some(o) = shell.canvas.mapping.as_mut() {
                        o.refused = Some(e);
                    }
                }
            }
        }
        Some(false) => shell.canvas.mapping = None,
        None => {}
    }
}

/// Removes the selected relationship from the model and says so with Undo. Its foreign key and the
/// key columns that key brought go with it, as the foreign key switch takes them off, all one undo
/// step: a line deleted leaves no key behind that nothing draws.
pub fn remove_selected_relationship(shell: &mut Shell) -> bool {
    let Some(session) = shell.session.as_ref() else { return false };
    let store = session.doc();
    let rel = session.view().selection.iter().copied().find_map(|o| match store.class_of(o).ok() {
        Some(c) if c == archforge_er::RELATIONSHIP => Some(o),
        Some(c) if c == archforge_diagram::EDGE => store.get(o, archforge_diagram::edge::ELEMENT).ok().and_then(|v| v.as_ref()),
        _ => None,
    });
    let Some(rel) = rel.filter(|r| store.class_of(*r).ok() == Some(archforge_er::RELATIONSHIP)) else { return false };
    let Some(id) = store.uuid(rel).ok() else { return false };
    let name = archforge_core::name_of(store, rel);
    let mut ids: Vec<String> = Vec::new();
    for (fk, _) in store.referrers(rel).into_iter().filter(|(_, p)| *p == archforge_er::foreign_key::RELATIONSHIP) {
        ids.extend(store.uuid(fk).ok().map(|u| u.to_string()));
        for c in store.list(fk, archforge_er::foreign_key::COLUMN).unwrap_or_default().iter().filter_map(archforge_mof::Value::as_ref) {
            if store.is_alive(c) {
                ids.extend(store.uuid(c).ok().map(|u| u.to_string()));
            }
        }
    }
    ids.push(id.to_string());
    let Some(session) = shell.session.as_mut() else { return false };
    let removed =
        session.batch(&tr("Remove relationship"), |b| b.execute("er.remove", &json!({ "ids": ids })).map(|_| ())).map_err(|e| e.to_string());
    match removed {
        Ok(_) => {
            let text = if name.is_empty() { tr("Relationship removed") } else { fmt(&tr("{name} removed"), &[("name", &name)]) };
            let undo_label = shell.session.as_ref().and_then(|s| s.undo_label().map(str::to_string));
            shell.canvas.toast = Some(Toast { text, until: shell.frames.saturating_add(TOAST_FRAMES), undo_label });
        }
        Err(e) => shell.canvas.notice = Some(e),
    }
    true
}

/// The toast at the canvas's foot: the text and Undo, gone after its frames or on Undo.
pub(crate) fn toast(shell: &mut Shell, ui: &egui::Ui, p: &Palette, r: Rect) {
    let Some(t) = shell.canvas.toast.clone() else { return };
    let top = shell.session.as_ref().and_then(|s| s.undo_label().map(str::to_string));
    if shell.frames > t.until || top != t.undo_label {
        shell.canvas.toast = None;
        return;
    }
    let paint = ui.painter().clone();
    let m = Rect::from_min_size(pos2(r.center().x - TOAST.x / 2.0, r.bottom() - 78.0 - TOAST.y), TOAST);
    ux::shadow(&paint, m, 8.0, p.shadow);
    paint.rect_filled(m, 8.0, p.text);
    let undo = ux::galley(&paint, &tr("Undo"), types::SMALL, p.accent_tint, None);
    let b = Rect::from_min_max(pos2(m.right() - 16.0 - undo.size().x - 8.0, m.top() + 6.0), pos2(m.right() - 8.0, m.bottom() - 6.0));
    ux::text(&paint, m.min + vec2(16.0, 11.0), &t.text, types::SMALL, p.surface, Some((b.left() - 12.0 - m.left() - 16.0).max(0.0)));
    paint.galley(b.min + vec2(8.0, (b.height() - undo.size().y) / 2.0), undo, p.accent_tint);
    if control(shell, ui, "workspace.toast.undo", "button", b, true).clicked() {
        shell.canvas.toast = None;
        if let Err(e) = run(shell, "edit.undo", serde_json::Value::Null) {
            shell.canvas.notice = Some(e);
        }
    }
}

/// A problem badge on the canvas: the problems on hover; for a many-to-many relationship a click
/// opens the mapping table offer.
#[allow(clippy::too_many_arguments, reason = "a badge is drawn from its place, its control id, its element and its texts")]
pub(crate) fn badge(shell: &mut Shell, ui: &egui::Ui, p: &Palette, at: Pos2, control_id: Uuid, element: Uuid, texts: &[String], many_to_many: bool) {
    let paint = ui.painter().clone();
    let b = Rect::from_center_size(at, Vec2::splat(18.0));
    paint.circle_filled(b.center(), 9.0, p.surface);
    paint.circle_stroke(b.center(), 9.0, Stroke::new(1.2, p.key_foreign));
    if many_to_many {
        ux::icon_in(&paint, Rect::from_center_size(b.center(), Vec2::splat(10.0)), &icons::PLUS, p.key_foreign);
    } else {
        let g = ux::galley(&paint, "!", types::CAPTION, p.key_foreign, None);
        paint.galley(b.center() - g.size() / 2.0, g, p.key_foreign);
    }
    let hint = if many_to_many { tr("Many to many · add a mapping table") } else { texts.join("\n") };
    // Live for its hover; only a many-to-many badge acts on a click.
    if control(shell, ui, &format!("workspace.canvas.problem.{control_id}"), "badge", b, true).on_hover_text(hint).clicked() && many_to_many {
        offer_mapping(shell, element);
    }
}

/// Every hover text to translate, for the catalogue tests.
#[cfg(test)]
pub(crate) fn texts() -> impl Iterator<Item = &'static str> {
    ["Presets", "Start", "End"].into_iter()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_self_loop_leaves_and_returns_on_the_right_side_around_the_centre() {
        let rect = Rect::from_min_size(pos2(100.0, 100.0), vec2(240.0, 166.0));
        let points = self_loop(rect, 1.0);
        assert_eq!(points.len(), 4);
        assert!(points.iter().all(|p| p.is_finite()));
        assert_eq!(points[0].x, rect.right());
        assert_eq!(points[3].x, rect.right());
        assert_eq!(points[1].x, rect.right() + LOOP_REACH);
        assert!((points[0].y + points[3].y) / 2.0 - rect.center().y < 0.001);
    }
}
