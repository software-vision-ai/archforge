//! The Appearance panel (the design's frames "ER · Appearance · Entity selected" and "ER ·
//! Appearance · Relationship selected"): how the selection looks on the diagram open in the centre,
//! and on that diagram only. A table, a note or another card takes a fill, a border and a text
//! colour and a border width; a line takes a colour and a width. Each colour is one of the design's
//! swatches or one of the user's own; Use diagram defaults takes the selection's own look away.
//! Every change goes through `diagram.style`, one undo step for the whole selection. A line's own
//! colour shows while the key status is off, which the panel offers to turn off. Lines run
//! straight, at right angles or curved, as the diagram says or as a line's own look says; right
//! angles can go around the tables in the way. With nothing selected the panel sets the diagram's
//! routing (owner decisions).

use archforge_diagram::{DIAGRAM, NODE, ProjectView, diagram, edge, node, style};
use archforge_mof::{Obj, Store, Value};
use egui::{Color32, Pos2, Rect, Stroke, StrokeKind, Vec2, pos2, vec2};
use serde_json::json;

use crate::canvas::{Look, colour, hex, look_of};
use crate::home::control;
use crate::i18n::{fmt, tr, trn};
use crate::panels::{FIELD, PAD, section};
use crate::ux::{self, Palette, icons, types};
use crate::{Shell, widgets};

/// The design's swatches after each row's first, which is the diagram's own colour in the theme
/// shown and takes the row's own colour away.
const FILLS: [&str; 4] = ["#FFFFFF", "#F5F7FA", "#EBF0FE", "#EAF5F0"];
const BORDERS: [&str; 4] = ["#DFE5ED", "#9EAFE7", "#A9D3C2", "#1B2B3C"];
const TEXTS: [&str; 4] = ["#657386", "#C25E0A", "#345EDB", "#27816D"];
const LINES: [&str; 5] = ["#1B2B3C", "#C25E0A", "#345EDB", "#27816D", "#7A56B5"];
/// The widths offered for a border or a line.
const WIDTHS: [f32; 4] = [1.0, 1.5, 2.0, 3.0];
/// The routings by `Routing` literal, as `diagram.style` names them.
const ROUTINGS: [&str; 4] = ["as-diagram", "straight", "right-angled", "curved"];

/// What a colour row sets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group {
    Fill,
    Border,
    Text,
    Line,
}

impl Group {
    fn key(self) -> &'static str {
        match self {
            Group::Fill => "fill",
            Group::Border => "border",
            Group::Text => "text",
            Group::Line => "line",
        }
    }
}

/// The colour picker open on a row: the colour being chosen as hue, saturation and brightness
/// (so the hue stays while the colour is grey), and its code as typed.
#[derive(Debug, Clone, PartialEq)]
pub struct Picker {
    pub group: Group,
    pub hsv: [f32; 3],
    pub code: String,
}

impl Picker {
    pub fn new(group: Group, colour: Color32) -> Self {
        let h = egui::ecolor::HsvaGamma::from(colour);
        Picker { group, hsv: [h.h, h.s, h.v], code: hex(colour) }
    }

    fn colour(&self) -> Color32 {
        hsv(self.hsv[0], self.hsv[1], self.hsv[2])
    }
}

fn hsv(h: f32, s: f32, v: f32) -> Color32 {
    egui::ecolor::HsvaGamma { h, s, v, a: 1.0 }.into()
}

/// The nodes and edges of the open diagram the selection names: the selected nodes and edges
/// themselves, and the ones that show a selected element. A generalization's line and a note's
/// link keep the notation's look and are left out.
fn targets(shell: &Shell) -> (Vec<Obj>, Vec<Obj>) {
    let (Some(session), Some(active)) = (shell.session.as_ref(), shell.canvas.active) else { return (Vec::new(), Vec::new()) };
    let store = session.doc();
    let Some(d) = store.by_uuid(active).filter(|d| store.is_alive(*d) && store.class_of(*d).ok() == Some(DIAGRAM)) else {
        return (Vec::new(), Vec::new());
    };
    let refs = |o: Obj, p| store.list(o, p).unwrap_or_default().iter().filter_map(Value::as_ref).filter(|x| store.is_alive(*x)).collect::<Vec<_>>();
    let element = |o: Obj, p| store.get(o, p).ok().and_then(|v| v.as_ref()).filter(|e| store.is_alive(*e));
    let all_nodes = refs(d, diagram::NODE);
    let all_edges = refs(d, diagram::EDGE);
    let (mut nodes, mut edges) = (Vec::new(), Vec::new());
    for o in &session.view().selection {
        for n in all_nodes.iter().filter(|n| **n == *o || element(**n, node::ELEMENT) == Some(*o)) {
            if !nodes.contains(n) {
                nodes.push(*n);
            }
        }
        for e in all_edges.iter().filter(|e| **e == *o || element(**e, edge::ELEMENT) == Some(*o)) {
            // A line its notation draws in its own fixed look (ER's generalization) keeps it.
            let styled = element(*e, edge::ELEMENT)
                .is_some_and(|el| session.view().of_element(store, el).and_then(|n| n.line(store, el)).is_none_or(|l| l.styled));
            if styled && !edges.contains(e) {
                edges.push(*e);
            }
        }
    }
    (nodes, edges)
}

/// The colour a node shows for `group` without a look of its own.
fn default_colour(view: &ProjectView, store: &Store, n: Obj, group: Group, p: &Palette) -> Color32 {
    let el = store.get(n, node::ELEMENT).ok().and_then(|v| v.as_ref());
    // A tinted card its notation draws (ER's table) takes the card's colours; a plain one (a
    // class) the surface's.
    let entity = el.is_some_and(|e| {
        let d = archforge_diagram::diagram_of(store, n);
        let card = d.zip(view.of_element(store, e)).and_then(|(d, notation)| notation.card(store, e, d));
        card.is_some_and(|c| c.tone == archforge_diagram::figure::Tone::Tinted)
    });
    match (group, el, entity) {
        (Group::Fill, _, true) => p.entity,
        (Group::Border, _, true) => p.entity_edge,
        (Group::Fill, None, _) => p.amber_tint,
        (Group::Border, None, _) => p.amber,
        (Group::Fill, ..) => p.surface,
        (Group::Border, ..) => p.border,
        (Group::Text, ..) => p.text,
        (Group::Line, ..) => p.text_secondary,
    }
}

/// Every target's own colour for `group` (none for the diagram's), or none when they differ.
fn own(store: &Store, targets: &[Obj], group: Group) -> Option<Option<Color32>> {
    same(targets.iter().map(|o| {
        let prop = if store.class_of(*o).ok() == Some(NODE) { node::STYLE } else { edge::STYLE };
        let look = look_of(store, *o, prop);
        match group {
            Group::Fill => look.fill,
            Group::Border | Group::Line => look.stroke,
            Group::Text => look.text,
        }
    }))
}

/// The diagram's own colour for `group` on the first target, as the row's first swatch shows it.
fn diagram_colour(view: &ProjectView, store: &Store, targets: &[Obj], group: Group, p: &Palette) -> Color32 {
    match targets.first() {
        Some(n) if group != Group::Line => default_colour(view, store, *n, group, p),
        _ => p.text_secondary,
    }
}

/// The width every target shows, or none when they differ.
fn shown_width(store: &Store, targets: &[Obj]) -> Option<f32> {
    same(targets.iter().map(|o| {
        let is_node = store.class_of(*o).ok() == Some(NODE);
        let look: Look = look_of(store, *o, if is_node { node::STYLE } else { edge::STYLE });
        look.width.unwrap_or(if is_node { 1.0 } else { 1.5 })
    }))
}

fn same<T: PartialEq>(mut each: impl Iterator<Item = T>) -> Option<T> {
    let first = each.next()?;
    each.all(|x| x == first).then_some(first)
}

/// Changes one part of every target's look and keeps the rest, one undo step for them all.
fn change(shell: &mut Shell, targets: &[Obj], group: Option<Group>, colour: Option<&str>, width: Option<f32>) {
    restyle(shell, targets, |params| {
        if let (Some(g), Some(c)) = (group, colour) {
            let key = match g {
                Group::Fill => "fill",
                Group::Border | Group::Line => "stroke",
                Group::Text => "text",
            };
            params[key] = json!(c);
        }
        if let Some(w) = width {
            params["line"] = json!(w);
        }
    });
}

/// The style property of `o`: a node's, an edge's or a diagram's.
fn style_prop(store: &Store, o: Obj) -> archforge_mof::PropId {
    match store.class_of(o).ok() {
        Some(c) if c == NODE => node::STYLE,
        Some(c) if c == DIAGRAM => diagram::STYLE,
        _ => edge::STYLE,
    }
}

/// Sets every target's look as `edit` changes it from what it is, one undo step for them all.
fn restyle(shell: &mut Shell, targets: &[Obj], edit: impl Fn(&mut serde_json::Value)) {
    let Some(session) = shell.session.as_mut() else { return };
    let store = session.doc();
    let mut calls = Vec::new();
    for o in targets {
        let Ok(id) = store.uuid(*o) else { continue };
        let s = store.get(*o, style_prop(store, *o)).ok().and_then(|v| v.as_ref());
        let text = |p| s.and_then(|s| store.get(s, p).ok()).and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default();
        let line = s.and_then(|s| store.get(s, style::LINE).ok()).and_then(|v| if let Value::Real(r) = v { Some(r) } else { None }).unwrap_or(0.0);
        let routing = match s.and_then(|s| store.get(s, style::ROUTING).ok()) {
            Some(Value::Enum(i)) => ROUTINGS.get(i as usize).copied().unwrap_or(ROUTINGS[0]),
            _ => ROUTINGS[0],
        };
        let avoid = s.is_some_and(|s| matches!(store.get(s, style::AVOID), Ok(Value::Bool(true))));
        let mut params = json!({
            "ids": [id.to_string()], "name": text(style::NAME), "fill": text(style::FILL),
            "stroke": text(style::STROKE), "text": text(style::TEXT), "line": line,
            "routing": routing, "avoid": avoid,
        });
        edit(&mut params);
        calls.push(params);
    }
    let outcome = session.batch(&tr("Change appearance"), |b| {
        for params in &calls {
            b.execute("diagram.style", params)?;
        }
        Ok(())
    });
    shell.panels.notice = outcome.err().map(|e| e.to_string());
}

/// The routing `o`'s own style gives (a `Routing` literal past as-diagram) and whether it avoids
/// tables; none when it leaves that to its diagram.
fn own_routing(store: &Store, o: Obj) -> Option<(usize, bool)> {
    let s = store.get(o, style_prop(store, o)).ok()?.as_ref()?;
    match store.get(s, style::ROUTING).ok()? {
        Value::Enum(i) if i > 0 && (i as usize) < ROUTINGS.len() => Some((i as usize, matches!(store.get(s, style::AVOID), Ok(Value::Bool(true))))),
        _ => None,
    }
}

/// How the lines of diagram `d` run when they have no routing of their own: straight unless it
/// says.
fn diagram_routing(store: &Store, d: Obj) -> (usize, bool) {
    own_routing(store, d).unwrap_or((1, false))
}

/// The open diagram.
fn open_diagram(shell: &Shell) -> Option<Obj> {
    let store = shell.session.as_ref()?.doc();
    shell.canvas.active.and_then(|a| store.by_uuid(a)).filter(|d| store.is_alive(*d) && store.class_of(*d).ok() == Some(DIAGRAM))
}

/// The routing row (Straight, Right-angled, Curved) under its label, and for right angles the
/// switch that takes the lines around the tables in their way. `current` is the routing every
/// target shows and whether it avoids, or none when they differ. Gives the y under them.
#[allow(clippy::too_many_arguments)]
fn routing_row(shell: &mut Shell, ui: &egui::Ui, p: &Palette, at: Pos2, width: f32, current: Option<(usize, bool)>, targets: &[Obj]) -> f32 {
    let paint = ui.painter().clone();
    ux::text(&paint, at, &tr("Line routing"), types::CAPTION, p.text_secondary, Some(width));
    let w = ((width - 2.0 * 6.0) / 3.0).max(0.0);
    let options = [
        (1, "straight", &icons::ROUTE_STRAIGHT, tr("Straight")),
        (2, "right-angled", &icons::ROUTE_RIGHT_ANGLED, tr("Right-angled")),
        (3, "curved", &icons::ROUTE_CURVED, tr("Curved")),
    ];
    let mut picked = None;
    for (i, (literal, key, icon, hint)) in options.into_iter().enumerate() {
        let b = Rect::from_min_size(at + vec2(i as f32 * (w + 6.0), 19.0), vec2(w, FIELD));
        let resp = control(shell, ui, &format!("workspace.appearance.routing.{key}"), "toggle", b, true).on_hover_text(hint);
        let on = current.is_some_and(|(r, _)| r == literal);
        paint.rect_filled(b, 6.0, if on { p.accent_tint } else { p.field });
        if on {
            paint.rect_stroke(b, 6.0, Stroke::new(1.0, p.accent), StrokeKind::Inside);
        }
        let ink = if on {
            p.accent
        } else if resp.hovered() {
            p.text
        } else {
            p.text_secondary
        };
        ux::icon_mid(&paint, b.center(), icon, ink);
        if resp.clicked() {
            picked = Some(literal);
        }
    }
    let mut y = at.y + 19.0 + FIELD + 14.0;
    // Around the tables: only for right angles.
    if let Some((2, avoid)) = current {
        let label = ux::text(&paint, pos2(at.x, y + 2.0), &tr("Route around tables"), types::UI, p.text, Some((width - 48.0).max(0.0)));
        let sw = Rect::from_min_size(pos2(at.x + width - 36.0, label.center().y - 10.0), vec2(36.0, 20.0));
        paint.rect_filled(sw, 10.0, if avoid { p.accent_fill } else { p.border });
        let knob = if avoid { sw.right_center() - vec2(10.0, 0.0) } else { sw.left_center() + vec2(10.0, 0.0) };
        paint.circle_filled(knob, 8.0, p.surface);
        if control(shell, ui, "workspace.appearance.routing.avoid", "toggle", sw, true).on_hover_text(tr("Route around tables")).clicked() {
            restyle(shell, targets, |params| {
                params["routing"] = json!(ROUTINGS[2]);
                params["avoid"] = json!(!avoid);
            });
        }
        y += 36.0;
    }
    if let Some(literal) = picked {
        // Right angles go around the tables unless the targets said otherwise already.
        let avoid = current.is_none_or(|(r, a)| r != 2 || a);
        restyle(shell, targets, |params| {
            params["routing"] = json!(ROUTINGS[literal]);
            params["avoid"] = json!(literal == 2 && avoid);
        });
    }
    y
}

/// Takes every target's own look away.
fn defaults(shell: &mut Shell, targets: &[Obj]) {
    let Some(session) = shell.session.as_mut() else { return };
    let ids: Vec<String> = targets.iter().filter_map(|o| session.doc().uuid(*o).ok()).map(|u| u.to_string()).collect();
    let outcome = session.execute("diagram.style", &json!({ "ids": ids }));
    shell.panels.notice = outcome.err().map(|e| e.to_string());
}

/// The heading's two lines for the targets: a name and what it is, on this diagram.
fn heading(uis: &[&'static dyn crate::notation_ui::NotationUi], store: &Store, nodes: &[Obj], edges: &[Obj]) -> (String, String) {
    let all: Vec<Obj> = nodes.iter().chain(edges).copied().collect();
    if all.len() != 1 {
        let n = all.len() as u64;
        return (trn("{n} item", "{n} items", n), tr("Selection · this diagram"));
    }
    let o = all[0];
    let prop = if nodes.is_empty() { edge::ELEMENT } else { node::ELEMENT };
    match store.get(o, prop).ok().and_then(|v| v.as_ref()) {
        Some(e) => {
            let word = crate::notation_ui::of(uis, store, e).and_then(|ui| ui.words(store, e)).map_or_else(|| tr("Element"), |w| w.word);
            (archforge_core::name_of(store, e), fmt(&tr("{kind} · this diagram"), &[("kind", &word)]))
        }
        None => {
            let text = store.get(o, node::TEXT).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default();
            let first = text.lines().next().unwrap_or_default().trim().to_string();
            let name = if first.is_empty() { tr("Note") } else { first };
            (name, fmt(&tr("{kind} · this diagram"), &[("kind", &tr("Note"))]))
        }
    }
}

/// Draws the panel body for the selection under the header, from `y` below the panel's top.
pub fn draw(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect, y: f32) {
    let paint = ui.painter().clone();
    let width = (r.width() - 2.0 * PAD).max(0.0);
    let (nodes, edges) = targets(shell);
    if nodes.is_empty() && edges.is_empty() {
        shell.panels.picker = None;
        // The diagram's own look: how its lines run.
        let Some((store, d)) = shell.session.as_ref().map(|s| s.doc()).zip(open_diagram(shell)) else {
            ux::text(&paint, r.min + vec2(PAD, y), &tr("Select a table or a line on the diagram"), types::UI, p.text_muted, Some(width));
            return;
        };
        let name = store.get(d, diagram::NAME).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default();
        let current = diagram_routing(store, d);
        ux::text(&paint, r.min + vec2(PAD, y), &name, types::CARD_TITLE, p.text, Some(width));
        ux::text(&paint, r.min + vec2(PAD, y + 26.0), &tr("Diagram · its lines"), types::CAPTION, p.text_secondary, Some(width));
        let at = section(shell, ui, p, r, r.top() + y + 51.0, &tr("Style"), None).unwrap_or(r.top() + y + 101.0) - 8.0;
        let at = routing_row(shell, ui, p, pos2(r.left() + PAD, at), width, Some(current), &[d]);
        ux::text(&paint, pos2(r.left() + PAD, at + 4.0), &tr("Select a table or a line on the diagram"), types::UI, p.text_muted, Some(width));
        return;
    }
    // What the targets show now, read before anything changes.
    let Some(session) = shell.session.as_ref() else { return };
    let (store, view) = (session.doc(), session.view());
    let (name, kind) = heading(shell.notation_uis, store, &nodes, &edges);
    let node_colours = [Group::Fill, Group::Border, Group::Text].map(|g| (own(store, &nodes, g), diagram_colour(view, store, &nodes, g, p)));
    let node_width = shown_width(store, &nodes);
    let line_colour = (own(store, &edges, Group::Line), diagram_colour(view, store, &edges, Group::Line, p));
    let line_width = shown_width(store, &edges);
    // What each line shows: its own routing, else its diagram's.
    let line_routing = same(edges.iter().map(|e| {
        own_routing(store, *e).unwrap_or_else(|| store.container(*e).ok().flatten().map_or((1, false), |(d, _)| diagram_routing(store, d)))
    }));
    ux::text(&paint, r.min + vec2(PAD, y), &name, types::CARD_TITLE, p.text, Some(width));
    ux::text(&paint, r.min + vec2(PAD, y + 26.0), &kind, types::CAPTION, p.text_secondary, Some(width));
    let mut at = section(shell, ui, p, r, r.top() + y + 51.0, &tr("Style"), None).unwrap_or(r.top() + y + 101.0) - 8.0;
    let mut anchors: Vec<(Group, Rect)> = Vec::new();
    if !nodes.is_empty() {
        let rows = [(Group::Fill, "Fill", &FILLS[..]), (Group::Border, "Border", &BORDERS[..]), (Group::Text, "Text colour", &TEXTS[..])];
        for ((group, label, colours), current) in rows.into_iter().zip(node_colours) {
            let (next, custom) = swatches(shell, ui, p, pos2(r.left() + PAD, at), width, group, &tr(label), colours, current, &nodes);
            anchors.push((group, custom));
            at = next;
        }
        at = widths(shell, ui, p, pos2(r.left() + PAD, at), width, "border", &tr("Border width"), node_width, &nodes);
    }
    if !edges.is_empty() {
        let (next, custom) = swatches(shell, ui, p, pos2(r.left() + PAD, at), width, Group::Line, &tr("Line colour"), &LINES, line_colour, &edges);
        anchors.push((Group::Line, custom));
        at = widths(shell, ui, p, pos2(r.left() + PAD, next), width, "line", &tr("Line width"), line_width, &edges);
        at = routing_row(shell, ui, p, pos2(r.left() + PAD, at), width, line_routing, &edges);
        // Lines show their key status while it is on; their own colour shows with it off.
        if shell.canvas.key_status {
            if crate::panels::ghost_button(
                shell,
                ui,
                p,
                pos2(r.left() + PAD - 6.0, at),
                "workspace.appearance.key-status",
                &icons::KEY_STATUS,
                &tr("Show line colours"),
            ) {
                shell.canvas.key_status = false;
            }
            at += 32.0;
        }
    }
    // Use diagram defaults: the selection's own look goes.
    paint.rect_filled(Rect::from_min_size(pos2(r.left(), at + 4.0), vec2(r.width(), 1.0)), 0.0, p.border);
    let button = Rect::from_min_size(pos2(r.left() + PAD, at + 18.0), vec2(width, 36.0));
    let resp = control(shell, ui, "workspace.appearance.defaults", "button", button, true);
    paint.rect_filled(button, 6.0, if resp.hovered() { p.accent_tint } else { p.page });
    ux::text_mid(&paint, button.left() + 10.0, button.center().y, &tr("Use diagram defaults"), types::UI, p.accent, Some(width - 20.0));
    let targets: Vec<Obj> = nodes.iter().chain(&edges).copied().collect();
    if resp.clicked() {
        shell.panels.picker = None;
        defaults(shell, &targets);
    }
    if let Some(why) = shell.panels.notice.clone() {
        ux::text(&paint, pos2(r.left() + PAD, button.bottom() + 10.0), &why, types::CAPTION, p.amber, Some(width));
    }
    // The picker over the panel, under its row's own-colour swatch.
    if let Some(picker) = shell.panels.picker.clone()
        && let Some((_, anchor)) = anchors.iter().find(|(g, _)| *g == picker.group)
    {
        let group_targets = if picker.group == Group::Line { edges.clone() } else { nodes.clone() };
        picker_card(shell, ui, p, *anchor, r, picker, &group_targets);
    }
}

/// A row of swatches under its label: the diagram's own colour first, the design's colours, the
/// user's own colour last. `current` is the targets' own colour (none for the diagram's), or none
/// when they differ. Gives the y under the row and the own-colour swatch, where the picker opens.
#[allow(clippy::too_many_arguments)]
fn swatches(
    shell: &mut Shell,
    ui: &egui::Ui,
    p: &Palette,
    at: Pos2,
    width: f32,
    group: Group,
    label: &str,
    colours: &[&str],
    (current, diagram): (Option<Option<Color32>>, Color32),
    targets: &[Obj],
) -> (f32, Rect) {
    enum Swatch<'a> {
        Diagram,
        Preset(&'a str),
        Own,
    }
    let paint = ui.painter().clone();
    ux::text(&paint, at, label, types::CAPTION, p.text_secondary, Some(width));
    let row: Vec<Swatch<'_>> = std::iter::once(Swatch::Diagram).chain(colours.iter().map(|c| Swatch::Preset(c))).chain([Swatch::Own]).collect();
    let step = ((width + 6.0) / row.len() as f32).min(38.0);
    let mut picked: Option<String> = None;
    let mut own_rect = Rect::NOTHING;
    let preset = matches!(current, Some(Some(c)) if colours.iter().any(|h| colour(h) == Some(c)));
    for (i, swatch) in row.iter().enumerate() {
        let b = Rect::from_min_size(at + vec2(i as f32 * step, 19.0), Vec2::splat(32.0));
        let (id, hint) = match swatch {
            Swatch::Diagram => (format!("workspace.appearance.{}.diagram", group.key()), tr("Diagram default")),
            Swatch::Preset(c) => (format!("workspace.appearance.{}.{}", group.key(), i - 1), (*c).to_string()),
            Swatch::Own => (format!("workspace.appearance.{}.own", group.key()), tr("Your own colour")),
        };
        let resp = control(shell, ui, &id, "swatch", b, true).on_hover_text(hint);
        let on = match swatch {
            Swatch::Diagram => current == Some(None),
            Swatch::Preset(c) => current == Some(colour(c)),
            Swatch::Own => matches!(current, Some(Some(_))) && !preset,
        };
        if on {
            paint.circle_stroke(b.center(), 15.0, Stroke::new(2.0, p.accent));
        } else if resp.hovered() {
            paint.circle_stroke(b.center(), 15.0, Stroke::new(1.0, p.border));
        }
        match swatch {
            Swatch::Diagram => {
                paint.circle_filled(b.center(), 11.0, diagram);
                paint.circle_stroke(b.center(), 11.0, Stroke::new(1.0, p.border));
            }
            Swatch::Preset(c) => {
                paint.circle_filled(b.center(), 11.0, colour(c).unwrap_or(diagram));
                paint.circle_stroke(b.center(), 11.0, Stroke::new(1.0, p.border));
            }
            Swatch::Own => {
                spectrum(&paint, b.center(), 11.0);
                own_rect = b;
            }
        }
        if resp.clicked() {
            match swatch {
                Swatch::Diagram => picked = Some(String::new()),
                Swatch::Preset(c) => picked = Some((*c).to_string()),
                Swatch::Own => {
                    let start = current.flatten().unwrap_or(diagram);
                    shell.panels.picker = Some(Picker::new(group, start));
                }
            }
        }
    }
    if let Some(c) = picked {
        shell.panels.picker = None;
        change(shell, targets, Some(group), Some(&c), None);
    }
    (at.y + 19.0 + 32.0 + 14.0, own_rect)
}

/// The own-colour swatch: a disc of every hue.
fn spectrum(paint: &egui::Painter, centre: Pos2, radius: f32) {
    const SLICES: usize = 24;
    let mut mesh = egui::epaint::Mesh::default();
    for i in 0..SLICES {
        let (a, b) = (i as f32 / SLICES as f32 * std::f32::consts::TAU, (i + 1) as f32 / SLICES as f32 * std::f32::consts::TAU);
        let c: Color32 = egui::ecolor::Hsva::new(i as f32 / SLICES as f32, 0.75, 0.95, 1.0).into();
        let base = mesh.vertices.len() as u32;
        mesh.colored_vertex(centre, c);
        mesh.colored_vertex(centre + vec2(a.cos(), a.sin()) * radius, c);
        mesh.colored_vertex(centre + vec2(b.cos(), b.sin()) * radius, c);
        mesh.add_triangle(base, base + 1, base + 2);
    }
    paint.add(mesh);
}

/// The width choices under their label; gives the y under them.
#[allow(clippy::too_many_arguments)]
fn widths(shell: &mut Shell, ui: &egui::Ui, p: &Palette, at: Pos2, width: f32, key: &str, label: &str, current: Option<f32>, targets: &[Obj]) -> f32 {
    let paint = ui.painter().clone();
    ux::text(&paint, at, label, types::CAPTION, p.text_secondary, Some(width));
    let w = ((width - 3.0 * 6.0) / 4.0).max(0.0);
    let mut picked = None;
    for (i, line) in WIDTHS.iter().enumerate() {
        let b = Rect::from_min_size(at + vec2(i as f32 * (w + 6.0), 19.0), vec2(w, FIELD));
        let resp = control(shell, ui, &format!("workspace.appearance.{key}-width.{i}"), "toggle", b, true).on_hover_text(format!("{line}"));
        let on = current == Some(*line);
        paint.rect_filled(b, 6.0, if on { p.accent_tint } else { p.field });
        if on {
            paint.rect_stroke(b, 6.0, Stroke::new(1.0, p.accent), StrokeKind::Inside);
        }
        let ink = if on {
            p.accent
        } else if resp.hovered() {
            p.text
        } else {
            p.text_secondary
        };
        paint.hline((b.center().x - 12.0)..=(b.center().x + 12.0), b.center().y, Stroke::new(*line, ink));
        if resp.clicked() {
            picked = Some(*line);
        }
    }
    if let Some(line) = picked {
        change(shell, targets, None, None, Some(line));
    }
    at.y + 19.0 + FIELD + 14.0
}

/// The picker card under `anchor`: a field of saturation across and brightness down, a hue bar,
/// the colour's code, Cancel and Apply. Nothing changes until Apply; Esc or a click outside cancels.
fn picker_card(shell: &mut Shell, ui: &egui::Ui, p: &Palette, anchor: Rect, panel: Rect, mut picker: Picker, targets: &[Obj]) {
    let size = vec2(240.0, 12.0 + 140.0 + 10.0 + 14.0 + 12.0 + FIELD + 10.0 + FIELD + 12.0);
    let left = (anchor.right() - size.x).clamp(panel.left() + 8.0, (panel.right() - size.x - 8.0).max(panel.left() + 8.0));
    let card = ux::drop_rect(ui, anchor, left, size);
    let (apply, cancel) = ux::overlay(ui, widgets::egui_id("workspace.appearance.picker.layer"), card, |ui| {
        let paint = ui.painter().clone();
        ux::shadow(&paint, card, 8.0, p.shadow);
        paint.rect_filled(card, 8.0, p.surface);
        paint.rect_stroke(card, 8.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
        let inner = card.shrink(12.0);
        let [h, s, v] = picker.hsv;
        // Saturation across, brightness down, for the hue chosen.
        let field = Rect::from_min_size(inner.min, vec2(inner.width(), 140.0));
        let mut mesh = egui::epaint::Mesh::default();
        const CELLS: usize = 16;
        for row in 0..=CELLS {
            for col in 0..=CELLS {
                let (x, y) = (col as f32 / CELLS as f32, row as f32 / CELLS as f32);
                mesh.colored_vertex(field.min + vec2(x * field.width(), y * field.height()), hsv(h, x, 1.0 - y));
            }
        }
        let w = (CELLS + 1) as u32;
        for row in 0..CELLS as u32 {
            for col in 0..CELLS as u32 {
                let a = row * w + col;
                mesh.add_triangle(a, a + 1, a + w);
                mesh.add_triangle(a + 1, a + w + 1, a + w);
            }
        }
        paint.add(mesh);
        let mark = pos2(field.left() + s * field.width(), field.top() + (1.0 - v) * field.height());
        paint.circle_stroke(mark, 6.0, Stroke::new(2.0, Color32::WHITE));
        paint.circle_stroke(mark, 7.0, Stroke::new(1.0, Color32::from_black_alpha(90)));
        widgets::record_visible(ui, &mut shell.registry, "workspace.appearance.picker.field", "slider", field);
        let resp = ui.interact(field, widgets::egui_id("workspace.appearance.picker.field"), egui::Sense::click_and_drag());
        if let Some(at) = resp.interact_pointer_pos().filter(|_| resp.is_pointer_button_down_on() || resp.clicked()) {
            picker.hsv[1] = ((at.x - field.left()) / field.width().max(1.0)).clamp(0.0, 1.0);
            picker.hsv[2] = 1.0 - ((at.y - field.top()) / field.height().max(1.0)).clamp(0.0, 1.0);
            picker.code = hex(picker.colour());
        }
        // Every hue, left to right.
        let bar = Rect::from_min_size(pos2(inner.left(), field.bottom() + 10.0), vec2(inner.width(), 14.0));
        let mut mesh = egui::epaint::Mesh::default();
        const STEPS: usize = 36;
        for i in 0..=STEPS {
            let t = i as f32 / STEPS as f32;
            let c = hsv(t, 1.0, 1.0);
            mesh.colored_vertex(pos2(bar.left() + t * bar.width(), bar.top()), c);
            mesh.colored_vertex(pos2(bar.left() + t * bar.width(), bar.bottom()), c);
        }
        for i in 0..STEPS as u32 {
            let a = i * 2;
            mesh.add_triangle(a, a + 1, a + 2);
            mesh.add_triangle(a + 1, a + 3, a + 2);
        }
        paint.add(mesh);
        let knob = Rect::from_center_size(pos2(bar.left() + h * bar.width(), bar.center().y), vec2(6.0, bar.height() + 4.0));
        paint.rect_filled(knob, 3.0, Color32::WHITE);
        paint.rect_stroke(knob, 3.0, Stroke::new(1.0, Color32::from_black_alpha(90)), StrokeKind::Outside);
        widgets::record_visible(ui, &mut shell.registry, "workspace.appearance.picker.hue", "slider", bar);
        let resp = ui.interact(bar.expand2(vec2(0.0, 4.0)), widgets::egui_id("workspace.appearance.picker.hue"), egui::Sense::click_and_drag());
        if let Some(at) = resp.interact_pointer_pos().filter(|_| resp.is_pointer_button_down_on() || resp.clicked()) {
            picker.hsv[0] = ((at.x - bar.left()) / bar.width().max(1.0)).clamp(0.0, 1.0);
            picker.code = hex(picker.colour());
        }
        // The colour's code, typed or as picked.
        let code = Rect::from_min_size(pos2(inner.left(), bar.bottom() + 12.0), vec2(inner.width(), FIELD));
        paint.rect_filled(code, 6.0, p.field);
        paint.circle_filled(pos2(code.left() + 14.0, code.center().y), 7.0, picker.colour());
        widgets::record_visible(ui, &mut shell.registry, "workspace.appearance.picker.code", "field", code);
        let text_rect = Rect::from_min_max(pos2(code.left() + 28.0, code.top() + 6.0), pos2(code.right() - 8.0, code.bottom() - 6.0));
        let out = egui::TextEdit::singleline(&mut picker.code)
            .id(widgets::egui_id("workspace.appearance.picker.code"))
            .frame(egui::Frame::NONE)
            .margin(egui::Margin::ZERO)
            .font(types::SMALL.font())
            .text_color(p.text)
            .char_limit(7)
            .desired_width(text_rect.width())
            .show(&mut ui.new_child(egui::UiBuilder::new().max_rect(text_rect)));
        let _ = (*out.response).clone().accessible_name(tr("Colour code"));
        if out.response.changed()
            && let Some(c) = colour(picker.code.trim())
        {
            let typed = egui::ecolor::HsvaGamma::from(c);
            picker.hsv = [if typed.s > 0.0 { typed.h } else { picker.hsv[0] }, typed.s, typed.v];
        }
        let y = card.bottom() - 12.0 - FIELD;
        let cancel = crate::panels::icon_button(
            shell,
            ui,
            p,
            pos2(card.right() - 12.0 - 2.0 * FIELD - 8.0, y),
            "workspace.appearance.picker.cancel",
            &icons::CLOSE,
            p.text_secondary,
            None,
            "Cancel",
        );
        let apply = crate::panels::icon_button(
            shell,
            ui,
            p,
            pos2(card.right() - 12.0 - FIELD, y),
            "workspace.appearance.picker.apply",
            &icons::TICK,
            p.on_accent,
            Some(p.accent_fill),
            "Apply",
        );
        let enter = out.response.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        (apply || enter, cancel)
    });
    let outside = ui.input(|i| i.pointer.any_pressed() && !i.pointer.interact_pos().is_some_and(|pos| card.contains(pos) || anchor.contains(pos)));
    let escape = ui.input(|i| i.key_pressed(egui::Key::Escape));
    if apply {
        shell.panels.picker = None;
        let chosen = hex(picker.colour());
        change(shell, targets, Some(picker.group), Some(&chosen), None);
    } else if cancel || outside || escape {
        shell.panels.picker = None;
    } else {
        shell.panels.picker = Some(picker);
    }
}

/// Every text that reaches `tr` through a variable, for the catalogue tests.
#[cfg(test)]
pub(crate) fn texts() -> impl Iterator<Item = &'static str> {
    ["Fill", "Border", "Text colour", "Cancel", "Apply"].into_iter()
}
