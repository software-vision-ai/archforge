//! Relating classes on a class diagram: the Relationship tool draws the kind
//! its chevron's menu chose (Association, Directed association, Aggregation, Composition,
//! Generalization or Dependency) from class to class, from the whole to the part, from the specific
//! class to the general one, from the client to the supplier; dropped on empty canvas it makes a new
//! class there, named in place. A selected line's ends drag onto another class to reconnect it.
//! Properties shows an association's name and both its ends (role, multiplicity, navigable,
//! aggregation), a dependency's name and its classes, a generalization's classes.

use archforge_diagram::figure::EndMark;
use archforge_mof::{Obj, Store, Uuid};
use egui::{Pos2, Rect, Stroke, StrokeKind, pos2, vec2};
use serde_json::json;

use crate::Shell;
use crate::home::control;
use crate::i18n::{fmt, tr};
use crate::panels::{Combo, ComboOut, FIELD, Menu, PAD, Subject, check, editable_select, menu, run, section, select_field, text_field};
use crate::ux::{self, Palette, types};

/// What the Relationship tool draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Relate {
    #[default]
    Association,
    Directed,
    Aggregation,
    Composition,
    Generalization,
    Dependency,
}

impl Relate {
    pub(crate) const ALL: [Relate; 6] =
        [Relate::Association, Relate::Directed, Relate::Aggregation, Relate::Composition, Relate::Generalization, Relate::Dependency];

    /// Its name in the menu, in the catalogue.
    fn word(self) -> &'static str {
        match self {
            Relate::Association => "Association",
            Relate::Directed => "Directed association",
            Relate::Aggregation => "Aggregation",
            Relate::Composition => "Composition",
            Relate::Generalization => "Generalization",
            Relate::Dependency => "Dependency",
        }
    }

    /// Its automation key.
    fn key(self) -> &'static str {
        match self {
            Relate::Association => "association",
            Relate::Directed => "directed",
            Relate::Aggregation => "aggregation",
            Relate::Composition => "composition",
            Relate::Generalization => "generalization",
            Relate::Dependency => "dependency",
        }
    }

    /// The marks at the line's start and end, and whether it is dashed, as the canvas draws it.
    fn look(self) -> ([EndMark; 2], bool) {
        match self {
            Relate::Association => ([EndMark::None, EndMark::None], false),
            Relate::Directed => ([EndMark::None, EndMark::OpenArrow], false),
            Relate::Aggregation => ([EndMark::HollowDiamond, EndMark::None], false),
            Relate::Composition => ([EndMark::FilledDiamond, EndMark::None], false),
            Relate::Generalization => ([EndMark::None, EndMark::Triangle], false),
            Relate::Dependency => ([EndMark::None, EndMark::OpenArrow], true),
        }
    }

    /// The command that draws it, its parameters for the two classes, and its undo step's name.
    fn command(self, from: Uuid, to: Uuid) -> (&'static str, serde_json::Value, &'static str) {
        let (from, to) = (from.to_string(), to.to_string());
        match self {
            Relate::Generalization => ("uml.generalization.add", json!({ "specific": from, "general": to }), "Add generalization"),
            Relate::Dependency => ("uml.dependency.add", json!({ "client": from, "supplier": to }), "Add dependency"),
            k => ("uml.association.add", json!({ "from": from, "to": to, "kind": k.key() }), "Add association"),
        }
    }
}

/// A short line with the kind's marks between `a` and `b`, over `fill`.
fn sample(paint: &egui::Painter, a: Pos2, b: Pos2, kind: Relate, ink: egui::Color32, fill: egui::Color32) {
    let ([start, end], dashed) = kind.look();
    let stroke = Stroke::new(1.2, ink);
    let z = 0.7;
    // The line stops short of a triangle or a diamond, as on the canvas.
    let short = |mark: EndMark, tip: Pos2, back: Pos2| {
        let len = match mark {
            EndMark::Triangle => 12.0 * 0.7,
            EndMark::HollowDiamond | EndMark::FilledDiamond => 20.0 * 0.7,
            _ => 0.0,
        };
        tip + (back - tip).normalized() * len
    };
    let line = [short(start, a, b), short(end, b, a)];
    if dashed {
        paint.extend(egui::Shape::dashed_line(&line, stroke, 4.0, 3.0));
    } else {
        paint.line_segment(line, stroke);
    }
    crate::canvas::line_mark(paint, start, a, b, stroke, z, fill);
    crate::canvas::line_mark(paint, end, b, a, stroke, z, fill);
}

/// The Relationship tool's face: the kind it draws, across its double-width button.
pub(crate) fn tool_face(shell: &Shell, paint: &egui::Painter, p: &Palette, b: Rect, ink: egui::Color32, active: bool) {
    let fill = if active { p.accent_tint } else { p.surface };
    sample(paint, b.center() - vec2(24.0, 0.0), b.center() + vec2(24.0, 0.0), shell.canvas.uml_relation, ink, fill);
}

/// The menu of kinds above the Relationship tool's chevron; a pick chooses and closes it.
pub(crate) fn popover(shell: &mut Shell, ui: &egui::Ui, p: &Palette, r: Rect, anchor: Rect) {
    let paint = ui.painter().clone();
    let size = vec2(248.0, 44.0 + Relate::ALL.len() as f32 * 36.0 + 8.0);
    let x = (anchor.center().x - size.x / 2.0).clamp(r.left() + 8.0, (r.right() - size.x - 8.0).max(r.left() + 8.0));
    let y = (anchor.top() - 8.0 - size.y).max(r.top() + 8.0);
    let m = Rect::from_min_size(pos2(x, y), size);
    ux::shadow(&paint, m, 10.0, p.shadow);
    paint.rect_filled(m, 10.0, p.surface);
    paint.rect_stroke(m, 10.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
    ux::text(&paint, m.min + vec2(16.0, 14.0), &tr("Relationship").to_uppercase(), types::EYEBROW, p.text_secondary, Some(200.0));
    let chosen = shell.canvas.uml_relation;
    for (i, kind) in Relate::ALL.iter().enumerate() {
        let row = Rect::from_min_size(m.min + vec2(8.0, 40.0 + i as f32 * 36.0), vec2(size.x - 16.0, 34.0));
        let resp = control(shell, ui, &format!("workspace.relate.{}", kind.key()), "menu-item", row, true);
        let on = *kind == chosen;
        if on {
            paint.rect_filled(row, 6.0, p.accent_tint);
        } else if resp.hovered() {
            paint.rect_filled(row, 6.0, p.page);
        }
        let (ink, fill) = if on { (p.accent, p.accent_tint) } else { (p.text_secondary, p.surface) };
        sample(&paint, row.left_center() + vec2(10.0, 0.0), row.left_center() + vec2(54.0, 0.0), *kind, ink, fill);
        ux::text_mid(
            &paint,
            row.left() + 70.0,
            row.center().y,
            &tr(kind.word()),
            types::UI,
            if on { p.accent } else { p.text },
            Some(row.width() - 78.0),
        );
        if resp.clicked() {
            shell.canvas.uml_relation = *kind;
            shell.canvas.popover = false;
        }
    }
    let outside = ui.input(|i| i.pointer.any_pressed() && !i.pointer.interact_pos().is_some_and(|pos| m.contains(pos) || anchor.contains(pos)));
    if outside || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        shell.canvas.popover = false;
    }
}

/// The class a node shows.
fn class_of_node(store: &Store, node: Uuid) -> Option<Obj> {
    let n = store.by_uuid(node)?;
    let c = store.get(n, archforge_diagram::node::ELEMENT).ok()?.as_ref()?;
    (store.class_of(c).ok() == Some(archforge_uml::CLASS)).then_some(c)
}

/// Draws the chosen kind between the classes of two nodes, its line on diagram `d` with its name
/// at the middle, as one undo step; selects it. A refusal is said at the canvas's top.
pub(crate) fn connect(shell: &mut Shell, d: Obj, from_node: Uuid, to_node: Uuid) {
    shell.canvas.notice = None;
    let kind = shell.canvas.uml_relation;
    let Some(session) = shell.session.as_mut() else { return };
    let store = session.doc();
    let (Some(from), Some(to)) = (class_of_node(store, from_node), class_of_node(store, to_node)) else {
        shell.canvas.notice = Some(tr("A relationship joins two classes"));
        return;
    };
    let (Ok(from), Ok(to), Ok(diagram)) = (store.uuid(from), store.uuid(to), store.uuid(d)) else { return };
    let (cmd, params, label) = kind.command(from, to);
    let out = session.batch(&tr(label), |b| {
        let r = b.execute(cmd, &params)?;
        let element = r.get("id").and_then(|i| i.as_str()).unwrap_or_default().to_string();
        let e = b.execute(
            "diagram.connect",
            &json!({ "diagram": diagram.to_string(), "element": element, "source": from_node.to_string(), "target": to_node.to_string() }),
        )?;
        if kind != Relate::Generalization {
            let edge = e.get("id").and_then(|i| i.as_str()).unwrap_or_default().to_string();
            b.execute("diagram.label", &json!({ "edge": edge, "kind": "name", "at": "middle" }))?;
        }
        Ok(())
    });
    match out {
        Ok(o) => {
            if let Some(id) = o.value.get(0).and_then(|v| v.get("id")).and_then(|v| v.as_str())
                && let Err(e) = run(shell, "view.select", json!({ "ids": [id] }))
            {
                shell.canvas.notice = Some(e);
            }
        }
        Err(e) => shell.canvas.notice = Some(e.to_string()),
    }
}

/// The chosen kind dropped on empty canvas at `(x, y)`: a new class there, related to the node's,
/// as one undo step; its name opens for typing.
pub(crate) fn connect_to_new(shell: &mut Shell, d: Obj, from_node: Uuid, x: f64, y: f64) {
    shell.canvas.notice = None;
    let kind = shell.canvas.uml_relation;
    let Some(session) = shell.session.as_mut() else { return };
    let store = session.doc();
    let Some(package) = store.get(d, archforge_diagram::diagram::ELEMENT).ok().and_then(|v| v.as_ref()) else {
        shell.canvas.notice = Some(tr("The diagram is of no package"));
        return;
    };
    let Some(from) = class_of_node(store, from_node) else {
        shell.canvas.notice = Some(tr("A relationship joins two classes"));
        return;
    };
    let (Ok(package_id), Ok(diagram), Ok(from_id)) = (store.uuid(package), store.uuid(d), store.uuid(from)) else { return };
    let names = archforge_uml::commands::classes(store, package).into_iter().map(|c| archforge_core::name_of(store, c));
    let Some(name) = crate::explorer::fresh_name("Class", names, false) else {
        shell.canvas.notice = Some(tr("No free name is left"));
        return;
    };
    let (w, h) = archforge_uml::CLASS_SIZE;
    let (px, py) = ((x - w / 2.0).round(), (y - h / 2.0).round());
    let name_for_batch = name.clone();
    let out = session.batch(&tr("Add class"), |b| {
        let c = b.execute("uml.class.add", &json!({ "package": package_id.to_string(), "name": name_for_batch }))?;
        let class = c.get("id").and_then(|i| i.as_str()).and_then(Uuid::parse).unwrap_or(from_id);
        let n = b.execute(
            "diagram.place",
            &json!({ "diagram": diagram.to_string(), "element": class.to_string(), "x": px, "y": py, "width": w, "height": h }),
        )?;
        let node = n.get("id").and_then(|i| i.as_str()).unwrap_or_default().to_string();
        let (cmd, params, _) = kind.command(from_id, class);
        let r = b.execute(cmd, &params)?;
        let element = r.get("id").and_then(|i| i.as_str()).unwrap_or_default().to_string();
        let e = b.execute(
            "diagram.connect",
            &json!({ "diagram": diagram.to_string(), "element": element, "source": from_node.to_string(), "target": node }),
        )?;
        if kind != Relate::Generalization {
            let edge = e.get("id").and_then(|i| i.as_str()).unwrap_or_default().to_string();
            b.execute("diagram.label", &json!({ "edge": edge, "kind": "name", "at": "middle" }))?;
        }
        Ok(())
    });
    match out {
        Ok(o) => {
            let id_at = |i: usize| o.value.get(i).and_then(|v| v.get("id")).and_then(|v| v.as_str()).and_then(Uuid::parse);
            if let (Some(class), Some(node)) = (id_at(0), id_at(1)) {
                let _ = run(shell, "view.select", json!({ "ids": [class.to_string()] }));
                shell.canvas.rename = Some(crate::editing::NodeRename { node, entity: class, text: name, opened: true, refused: None });
                shell.canvas.focused = true;
            }
        }
        Err(e) => shell.canvas.notice = Some(e.to_string()),
    }
}

/// Moves the source or target end of `element` to the class of `to_node`; dropped back on its own
/// class it changes nothing, and a refusal is said at the canvas's top.
pub(crate) fn reconnect(shell: &mut Shell, element: Uuid, source: bool, to_node: Uuid) {
    shell.canvas.notice = None;
    let Some(store) = shell.session.as_ref().map(|s| s.doc()) else { return };
    let Some(target) = class_of_node(store, to_node) else {
        shell.canvas.notice = Some(tr("A relationship joins two classes"));
        return;
    };
    let now = store.by_uuid(element).and_then(|e| archforge_uml::figures::ends(store, e)).map(|(a, b)| if source { a } else { b });
    if now == Some(target) {
        return;
    }
    let Ok(class) = store.uuid(target) else { return };
    let end = if source { "source" } else { "target" };
    if let Err(e) = run(shell, "uml.relationship.reconnect", json!({ "relationship": element.to_string(), "end": end, "class": class.to_string() })) {
        shell.canvas.notice = Some(e);
    }
}

/// An association's ends' role and multiplicity as typed in Properties, until they are written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EndsEdit {
    pub association: Uuid,
    pub role: [String; 2],
    pub multiplicity: [String; 2],
    /// Each end's role and multiplicity as the model last had them: what Escape brings back, and
    /// how a change made elsewhere (an undo, another view) reaches the fields.
    pub written_role: [String; 2],
    pub written: [String; 2],
    /// Why an end's typed multiplicity was refused, shown under its field until it changes.
    pub refused: [Option<String>; 2],
}

fn ends_edit(store: &Store, a: Obj, id: Uuid) -> Option<EndsEdit> {
    let ends = archforge_uml::figures::member_ends(store, a)?;
    let role = ends.map(|e| archforge_core::name_of(store, e));
    let multiplicity = ends.map(|e| archforge_uml::figures::multiplicity(store, e));
    Some(EndsEdit { association: id, written_role: role.clone(), written: multiplicity.clone(), role, multiplicity, refused: [None, None] })
}

/// `typed` in end `end`'s multiplicity of association `id`, committed and refused, as a sample
/// to look at.
#[cfg(feature = "snapshot")]
pub(crate) fn refuse_typed(shell: &mut Shell, id: Uuid, end: usize, typed: &str) -> Result<(), String> {
    let store = shell.session.as_ref().ok_or("no project is open")?.doc();
    let a = store.by_uuid(id).ok_or("no such association")?;
    let mut edit = ends_edit(store, a, id).ok_or("not an association")?;
    let why = archforge_uml::members::parse_multiplicity(typed).err().ok_or("the sample reads")?;
    *edit.multiplicity.get_mut(end).ok_or("no such end")? = typed.to_string();
    *edit.refused.get_mut(end).ok_or("no such end")? = Some(multiplicity_problem(&why));
    shell.panels.ends = Some(edit);
    Ok(())
}

/// The common multiplicities an end's menu offers, in the design's order.
pub(crate) const MULTIPLICITIES: [&str; 5] = ["0..1", "1", "0..*", "1..*", "*"];

/// Why a typed multiplicity does not read, in the user's language.
fn multiplicity_problem(e: &archforge_uml::members::MultiplicityError) -> String {
    use archforge_uml::members::MultiplicityError as E;
    match e {
        E::Malformed(_) => tr("Type a whole number, a range such as 2..5, or *."),
        E::TooLarge => fmt(&tr("A bound can be at most {max}."), &[("max", &archforge_uml::members::MAX_BOUND.to_string())]),
        E::LowerMany => tr("The lower bound cannot be *."),
        E::Reversed(lower) => fmt(&tr("The upper bound must be at least {lower}."), &[("lower", &lower.to_string())]),
    }
}

/// The aggregation of an end, as an index of None, Shared, Composite.
fn aggregation(store: &Store, end: Obj) -> usize {
    match store.get(end, archforge_uml::property::AGGREGATION) {
        Ok(archforge_mof::Value::Enum(i)) => (i as usize).min(2),
        _ => 0,
    }
}

const AGGREGATIONS: [&str; 3] = ["none", "shared", "composite"];

fn aggregation_words() -> [String; 3] {
    [tr("None"), tr("Shared"), tr("Composite")]
}

/// A line's Properties: an association's or a dependency's name, its classes, an association's
/// ends.
pub(crate) fn body(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect, mut y: f32, subject: &Subject) {
    let paint = ui.painter().clone();
    let width = (r.width() - 2.0 * PAD).max(0.0);
    let (Some(o), Some(id)) = (subject.obj, subject.id) else { return };
    let Some(store) = shell.session.as_ref().map(|s| s.doc()) else { return };
    let class = store.class_of(o).ok();
    let name = |c: Option<Obj>| c.map(|c| archforge_core::name_of(store, c)).unwrap_or_default();
    let ends = archforge_uml::figures::ends(store, o);
    let (from, to) = (name(ends.map(|e| e.0)), name(ends.map(|e| e.1)));
    if class != Some(archforge_uml::GENERALIZATION) {
        crate::panels::name_field(shell, ui, p, pos2(r.left() + PAD, y), width, subject, "uml.relationship.rename", "relationship");
        y += 17.0 + FIELD + 12.0;
    }
    // The classes it joins, read only: its ends drag onto another class on the canvas.
    let (first, second) = match class {
        Some(c) if c == archforge_uml::GENERALIZATION => (tr("Specific"), tr("General")),
        Some(c) if c == archforge_uml::DEPENDENCY => (tr("Client"), tr("Supplier")),
        _ => (tr("From"), tr("To")),
    };
    let half = ((width - 8.0) / 2.0).max(0.0);
    for (i, (label, value)) in [(first, from), (second, to)].into_iter().enumerate() {
        let at = pos2(r.left() + PAD + i as f32 * (half + 8.0), y);
        ux::text(&paint, at, &label, types::CAPTION, p.text_secondary, Some(half));
        let field = Rect::from_min_size(at + vec2(0.0, 17.0), vec2(half, FIELD));
        paint.rect_filled(field, 6.0, p.field);
        ux::text_mid(&paint, field.left() + 8.0, field.center().y, &value, types::SMALL, p.text_secondary, Some((half - 16.0).max(0.0)));
        control(shell, ui, &format!("workspace.properties.relationship.{}", ["from", "to"][i]), "field", field, false);
    }
    y += 17.0 + FIELD + 8.0;
    ux::text(&paint, pos2(r.left() + PAD, y), &tr("Drag a line end onto another class."), types::CAPTION, p.text_muted, Some(width));
    y += 28.0;
    if class == Some(archforge_uml::ASSOCIATION) {
        ends_sections(shell, ui, p, r, y, o, id);
    }
}

/// An association's two ends, each its own section: role, written when typed; the multiplicity,
/// an editable select of the common ones, written when picked or typed; Navigable and the
/// aggregation, written when changed. A multiplicity that does not read stays in its field, with
/// why under it, and the model keeps the one it had.
#[allow(clippy::too_many_arguments, reason = "the ends are drawn into the panel for their association")]
fn ends_sections(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect, mut y: f32, a: Obj, id: Uuid) {
    let width = (r.width() - 2.0 * PAD).max(0.0);
    let Some(store) = shell.session.as_ref().map(|s| s.doc()) else { return };
    let Some(ends) = archforge_uml::figures::member_ends(store, a) else { return };
    if shell.panels.ends.as_ref().is_none_or(|e| e.association != id) {
        shell.panels.ends = ends_edit(store, a, id);
    }
    let classes = ends.map(|e| {
        store.get(e, archforge_uml::typed_element::TYPE).ok().and_then(|v| v.as_ref()).map(|t| archforge_core::name_of(store, t)).unwrap_or_default()
    });
    let navigable = ends.map(|e| archforge_uml::figures::navigable(store, a, e));
    let aggregations = ends.map(|e| aggregation(store, e));
    let roles = ends.map(|e| archforge_core::name_of(store, e));
    let written = ends.map(|e| archforge_uml::figures::multiplicity(store, e));
    let Some(mut edit) = shell.panels.ends.clone() else { return };
    // A change the model took elsewhere (an undo, a redo, another view) reaches the fields.
    for i in 0..2 {
        if edit.written_role[i] != roles[i] {
            edit.role[i] = roles[i].clone();
            edit.written_role[i] = roles[i].clone();
        }
        if edit.written[i] != written[i] {
            edit.multiplicity[i] = written[i].clone();
            edit.written[i] = written[i].clone();
            edit.refused[i] = None;
        }
    }
    let words = aggregation_words();
    for i in 0..2 {
        let title = fmt(&tr("End at {class}"), &[("class", &classes[i])]);
        y = section(shell, ui, p, r, y, &title, None).unwrap_or(y + 50.0) - 6.0;
        let half = ((width - 8.0) / 2.0).max(0.0);
        let key = format!("workspace.properties.end.{i}");
        let at = pos2(r.left() + PAD, y);
        let role = text_field(
            shell,
            ui,
            p,
            at,
            half,
            &format!("{key}.role"),
            &tr("Role"),
            &mut edit.role[i],
            archforge_uml::commands::MAX_NAME,
            false,
            true,
        );
        let typed = edit.multiplicity[i].clone();
        let combo = Combo {
            id: &format!("{key}.multiplicity"),
            label: &tr("Multiplicity"),
            name: &format!("{title} · {}", tr("Multiplicity")),
            placeholder: "—",
            options: &MULTIPLICITIES,
            current: MULTIPLICITIES.iter().position(|m| *m == edit.written[i]),
            pick: &tr("Pick a multiplicity"),
            hint: &tr("Or type a number or range"),
            clear: &tr("Clear"),
            refused: edit.refused[i].is_some(),
            menu: Menu::EndMultiplicity(i),
            limit: 24,
        };
        let asked = editable_select(shell, ui, p, at + vec2(half + 8.0, 0.0), half, &mut edit.multiplicity[i], &combo);
        if edit.multiplicity[i] != typed {
            // Typing again takes the reason away until the next commit.
            edit.refused[i] = None;
        }
        let mut write_multiplicity = false;
        match asked {
            ComboOut::Idle => {}
            ComboOut::Commit => write_multiplicity = true,
            ComboOut::Pick(k) => {
                // A preset, or Clear past the presets, which takes the multiplicity away.
                edit.multiplicity[i] = MULTIPLICITIES.get(k).map(|m| (*m).to_string()).unwrap_or_default();
                write_multiplicity = true;
            }
            ComboOut::Cancel => {
                edit.multiplicity[i] = edit.written[i].clone();
                edit.refused[i] = None;
            }
        }
        // Presets and typing share one check; what reads as the model has it already writes nothing.
        if write_multiplicity {
            match archforge_uml::members::parse_multiplicity(&edit.multiplicity[i]) {
                Err(e) => {
                    edit.refused[i] = Some(multiplicity_problem(&e));
                    write_multiplicity = false;
                    // Refused on Enter, the field keeps the keyboard for the correction or Escape.
                    if ui.input(|inp| inp.key_pressed(egui::Key::Enter)) {
                        ui.memory_mut(|m| m.request_focus(crate::widgets::egui_id(combo.id)));
                    }
                }
                Ok(bounds) if Ok(bounds) == archforge_uml::members::parse_multiplicity(&edit.written[i]) => {
                    edit.multiplicity[i] = edit.written[i].clone();
                    write_multiplicity = false;
                }
                Ok(_) => {}
            }
        }
        y += 17.0 + FIELD + 12.0;
        if let Some(why) = &edit.refused[i] {
            let said = ux::text(&ui.painter().clone(), pos2(r.left() + PAD, y - 2.0), why, types::CAPTION, p.danger, Some(width));
            y += (said.height() + 26.0).max(42.0);
        }
        // The whole's end, where the diamond sits, is never navigable: it shows no Navigable.
        let whole = aggregations[1 - i] != 0;
        let mut nav = navigable[i] && !whole;
        if !whole {
            check(shell, ui, p, pos2(r.left() + PAD, y + 6.0), &format!("{key}.navigable"), &tr("Navigable"), &mut nav);
            y += 32.0;
        }
        let field = select_field(
            shell,
            ui,
            p,
            pos2(r.left() + PAD, y),
            width,
            &format!("{key}.aggregation"),
            &tr("Aggregation"),
            &words[aggregations[i]],
            Menu::EndAggregation(i),
        );
        y += 17.0 + FIELD + 16.0;
        let mut agg = aggregations[i];
        if shell.panels.menu == Some(Menu::EndAggregation(i))
            && let Some(k) = menu(shell, ui, p, field, &format!("{key}.aggregation"), &words)
        {
            agg = k;
        }
        shell.panels.ends = Some(edit.clone());
        if role || write_multiplicity || (!whole && nav != navigable[i]) || agg != aggregations[i] {
            // Only the field written changes: the others go as the model has them, so a
            // multiplicity still being corrected is never sent with a role or an aggregation.
            let params = json!({
                "association": id.to_string(),
                "end": i,
                "role": if role { edit.role[i].trim() } else { roles[i].as_str() },
                "multiplicity": if write_multiplicity { edit.multiplicity[i].trim() } else { written[i].as_str() },
                "navigable": nav,
                "aggregation": AGGREGATIONS[agg],
            });
            match run(shell, "uml.association.set-end", params) {
                Ok(_) => {
                    shell.panels.notice = None;
                    if let Some(e) = shell.panels.ends.as_mut() {
                        e.refused[i] = None;
                        e.role[i] = e.role[i].trim().to_string();
                    }
                }
                // A multiplicity the model refuses (a composition's whole allows one at most) is
                // said under its field; anything else at the panel's foot.
                Err(e) if write_multiplicity => {
                    if let Some(ends) = shell.panels.ends.as_mut() {
                        ends.refused[i] = Some(e);
                    }
                }
                Err(e) => shell.panels.notice = Some(e),
            }
            return;
        }
    }
}

/// Every word the kinds' menu and the panels say through `tr` at run time, for the catalogue.
#[cfg(test)]
pub(crate) fn texts() -> impl Iterator<Item = &'static str> {
    Relate::ALL.iter().map(|k| k.word()).chain(["Add association", "Add generalization", "Add dependency"])
}
