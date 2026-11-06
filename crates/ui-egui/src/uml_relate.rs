//! Relating classes on a class diagram: the Relationship tool draws the kind
//! its chevron's menu chose (Association, Directed association, Aggregation, Composition,
//! Generalization or Dependency, and after a separator Interface realization and Association
//! class) from class to class,
//! from the whole to the part, from the specific class to the general one, from the client to the
//! supplier, from the implementing class to its interface; dropped on empty canvas it makes a new
//! class there, or for a realization a new interface, named in place. A realization starts only at
//! a class and ends only at an interface it does not realize yet; an association of any kind
//! neither starts nor ends at an enumeration, a data type or a primitive type; anything else is
//! said at the canvas's top and draws nothing. A generalization dragged out of an enumeration onto
//! empty canvas makes an enumeration, out of a data type a data type, and out of a primitive type a
//! primitive type, the only kind each specializes. A selected
//! line's ends drag onto another class to reconnect it.
//! Properties shows an association's name and both its ends (role, multiplicity, navigable,
//! aggregation), a dependency's name and its classes, a generalization's classes, a realization's
//! implementing class and interface.
//!
//! An association class (AR-125) is drawn between two classes as an association is: its line and
//! its card above the line's middle come as one element and one undo step, the card's name open
//! for typing; dropped on empty canvas it makes a new class there, named in place. Its ends are
//! classes only, and a drop on anything else is said at the canvas's top. Its Properties are an
//! association's, its name, classes and ends, then its attributes and operations as a class's.

use archforge_diagram::figure::EndMark;
use archforge_mof::{Obj, Store, Uuid};
use egui::{Pos2, Rect, Stroke, StrokeKind, pos2, vec2};
use serde_json::json;

use crate::Shell;
use crate::home::control;
use crate::i18n::{fmt, tr};
use crate::panels::{Combo, ComboOut, FIELD, Menu, PAD, Subject, check, editable_select, menu, run, section, select_field, text_field};
use crate::ux::{self, Palette, icons, types};

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
    /// An interface realization, from the implementing class to its interface.
    Realization,
    /// An association class: a line between two classes with a card of its own (AR-125).
    AssociationClass,
    /// A template binding, from the bound class to its template (AR-131): released on the
    /// template, it opens its draft in Properties instead of drawing at once.
    TemplateBinding,
    /// An n-ary association (AR-128): drawn by clicking each class in turn, not by dragging; the
    /// gesture is `crate::uml_nary`'s.
    NAry,
}

impl Relate {
    pub(crate) const ALL: [Relate; 10] = [
        Relate::Association,
        Relate::Directed,
        Relate::Aggregation,
        Relate::Composition,
        Relate::Generalization,
        Relate::Dependency,
        Relate::Realization,
        Relate::AssociationClass,
        Relate::NAry,
        Relate::TemplateBinding,
    ];

    /// Its name in the menu, in the catalogue.
    fn word(self) -> &'static str {
        match self {
            Relate::Association => "Association",
            Relate::Directed => "Directed association",
            Relate::Aggregation => "Aggregation",
            Relate::Composition => "Composition",
            Relate::Generalization => "Generalization",
            Relate::Dependency => "Dependency",
            Relate::Realization => "Interface realization",
            Relate::AssociationClass => "Association class",
            Relate::TemplateBinding => "Template binding",
            Relate::NAry => "N-ary association",
        }
    }

    /// What it draws and which way, where its word alone does not say: the tool's tooltip and the
    /// accessible name of the tool and of its menu row.
    fn hint(self) -> Option<&'static str> {
        match self {
            Relate::Realization => Some("Interface realization: from the implementing class to its interface"),
            Relate::AssociationClass => Some("Association class: between two classes, with a card of its own"),
            Relate::TemplateBinding => Some("Template binding: from the bound class to its template"),
            Relate::NAry => Some("N-ary association: click each class in turn, then press Enter"),
            _ => None,
        }
    }

    /// Whether a separator sets it apart above it in the menu: realizing is neither relating nor
    /// specializing; the association class follows the realization in that group; an n-ary
    /// association, drawn class by class, and a template binding share the group after it (design
    /// A2), the n-ary association first.
    fn after_separator(self) -> bool {
        matches!(self, Relate::Realization | Relate::NAry)
    }

    /// Whether its line carries the relationship's name at its middle: a generalization and a
    /// realization have none, and an association class's name is on its card.
    fn named(self) -> bool {
        !matches!(self, Relate::Generalization | Relate::Realization | Relate::AssociationClass | Relate::TemplateBinding | Relate::NAry)
    }

    /// Whether it draws an association, of any kind.
    fn associates(self) -> bool {
        matches!(self, Relate::Association | Relate::Directed | Relate::Aggregation | Relate::Composition)
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
            Relate::Realization => "realization",
            Relate::AssociationClass => "association-class",
            Relate::TemplateBinding => "template-binding",
            Relate::NAry => "n-ary",
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
            Relate::Realization => ([EndMark::None, EndMark::Triangle], true),
            Relate::AssociationClass | Relate::NAry => ([EndMark::None, EndMark::None], false),
            Relate::TemplateBinding => ([EndMark::None, EndMark::OpenArrow], true),
        }
    }

    /// The command that draws it, its parameters for the two classes, and its undo step's name. An
    /// association class's is drawn with its name by [`connect_class`] instead, and an n-ary
    /// association never between two classes (`crate::uml_nary` draws it).
    fn command(self, from: Uuid, to: Uuid) -> (&'static str, serde_json::Value, &'static str) {
        let (from, to) = (from.to_string(), to.to_string());
        match self {
            Relate::AssociationClass => ("uml.association-class.add", json!({ "from": from, "to": to }), "Add association class"),
            Relate::Generalization => ("uml.generalization.add", json!({ "specific": from, "general": to }), "Add generalization"),
            Relate::Dependency => ("uml.dependency.add", json!({ "client": from, "supplier": to }), "Add dependency"),
            Relate::Realization => ("uml.realization.add", json!({ "class": from, "interface": to }), "Add interface realization"),
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
    // An association class's card, small, above the line's middle, tied to it by a short dash.
    if kind == Relate::AssociationClass {
        let middle = a + (b - a) / 2.0;
        let card = Rect::from_center_size(middle - vec2(0.0, 9.0), vec2(11.0, 7.0));
        paint.rect_filled(card, 1.0, fill);
        paint.rect_stroke(card, 1.0, stroke, StrokeKind::Inside);
        paint.extend(egui::Shape::dashed_line(&[pos2(middle.x, card.bottom()), middle], Stroke::new(1.0, ink), 1.5, 1.5));
    }
    // A template binding's keyword over its line, small, as its label sits over the canvas's line.
    if kind == Relate::TemplateBinding {
        let g = ux::galley(paint, &tr("«bind»"), types::TINY, ink, None);
        let middle = a + (b - a) / 2.0;
        paint.galley(pos2(middle.x - g.size().x / 2.0, middle.y - g.size().y - 1.0), g, ink);
    }
    // An n-ary association (AR-128): a small hollow diamond at the line's middle with a short leg
    // down from it, the third end.
    if kind == Relate::NAry {
        let middle = a + (b - a) / 2.0;
        let (w, h) = (6.0, 4.0);
        let diamond = vec![middle - vec2(w, 0.0), middle - vec2(0.0, h), middle + vec2(w, 0.0), middle + vec2(0.0, h)];
        paint.add(egui::Shape::convex_polygon(diamond, fill, stroke));
        paint.line_segment([middle + vec2(0.0, h), middle + vec2(0.0, h + 6.0)], stroke);
    }
}

/// The Relationship tool's face: the kind it draws, across its double-width button.
pub(crate) fn tool_face(shell: &Shell, paint: &egui::Painter, p: &Palette, b: Rect, ink: egui::Color32, active: bool) {
    let fill = if active { p.accent_tint } else { p.surface };
    sample(paint, b.center() - vec2(24.0, 0.0), b.center() + vec2(24.0, 0.0), shell.canvas.uml_relation, ink, fill);
}

/// The height of the separator between the menu's groups of kinds: a hairline with room above and
/// below it.
const SEPARATOR: f32 = 13.0;

/// The top of the menu's row `i`, from the menu's top: under the heading, each row 36 high, and a
/// separator above every kind that starts a group.
fn row_top(i: usize) -> f32 {
    let separators = Relate::ALL.iter().take(i.saturating_add(1)).filter(|k| k.after_separator()).count();
    40.0 + i as f32 * 36.0 + separators as f32 * SEPARATOR
}

/// The menu of kinds above the Relationship tool's chevron, the current one tinted and ticked; a
/// pick chooses and closes it.
pub(crate) fn popover(shell: &mut Shell, ui: &egui::Ui, p: &Palette, r: Rect, anchor: Rect) {
    let paint = ui.painter().clone();
    let size = vec2(248.0, row_top(Relate::ALL.len()) + 4.0 + 8.0);
    let x = (anchor.center().x - size.x / 2.0).clamp(r.left() + 8.0, (r.right() - size.x - 8.0).max(r.left() + 8.0));
    let y = (anchor.top() - 8.0 - size.y).max(r.top() + 8.0);
    let m = Rect::from_min_size(pos2(x, y), size);
    ux::shadow(&paint, m, 10.0, p.shadow);
    paint.rect_filled(m, 10.0, p.surface);
    paint.rect_stroke(m, 10.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
    ux::text(&paint, m.min + vec2(16.0, 14.0), &tr("Relationship").to_uppercase(), types::EYEBROW, p.text_secondary, Some(200.0));
    let chosen = shell.canvas.uml_relation;
    for (i, kind) in Relate::ALL.iter().enumerate() {
        let row = Rect::from_min_size(m.min + vec2(8.0, row_top(i)), vec2(size.x - 16.0, 34.0));
        if kind.after_separator() {
            let y = row.top() - SEPARATOR / 2.0 - 1.0;
            paint.line_segment([pos2(m.left() + 16.0, y), pos2(m.right() - 16.0, y)], Stroke::new(1.0, p.border));
        }
        let name = tr(kind.hint().unwrap_or(kind.word()));
        let resp = control(shell, ui, &format!("workspace.relate.{}", kind.key()), "menu-item", row, true).accessible_name(name);
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
            Some(row.width() - 70.0 - 32.0),
        );
        // The current kind ticked at the row's right, as the classifier tool's menu ticks its own.
        if on {
            ux::icon_mid(&paint, row.right_center() - vec2(16.0, 0.0), &icons::TICK, p.accent);
        }
        if resp.clicked() {
            shell.canvas.uml_relation = *kind;
            shell.canvas.popover = None;
        }
    }
    let outside = ui.input(|i| i.pointer.any_pressed() && !i.pointer.interact_pos().is_some_and(|pos| m.contains(pos) || anchor.contains(pos)));
    if outside || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        shell.canvas.popover = None;
    }
}

/// The classifier a node shows: a class, an interface, an enumeration, a data type or a primitive
/// type.
pub(crate) fn class_of_node(store: &Store, node: Uuid) -> Option<Obj> {
    let n = store.by_uuid(node)?;
    let c = store.get(n, archforge_diagram::node::ELEMENT).ok()?.as_ref()?;
    store.class_of(c).is_ok_and(archforge_uml::is_classifier).then_some(c)
}

/// Why a realization from `implementing` to `contract` is refused, in the user's language, when it
/// is: neither may be missing (a node of something else than a classifier), only a class
/// implements, only an interface is realized, and a class realizes an interface once. `except` is
/// the realization being reconnected.
fn realization_refusal(store: &Store, implementing: Option<Obj>, contract: Option<Obj>, except: Option<Obj>) -> Option<String> {
    use archforge_uml::relations::RealizationProblem as P;
    let problem = match (implementing, contract) {
        (Some(i), Some(c)) => archforge_uml::relations::realization_problem(store, i, c, except)?,
        (None, _) => P::NotAClass,
        (Some(_), None) => P::NotAnInterface,
    };
    Some(match problem {
        P::NotAClass => tr("Only a class implements an interface"),
        P::NotAnInterface => tr("A class realizes only an interface"),
        P::Again => tr("This class realizes that interface already"),
    })
}

/// Why a generalization from `specific` to `general` is refused, in the user's words, when it is:
/// the model's rules (`generalization_problem`) said as the canvas says them, a final general
/// classifier by its name (AR-137). `except` is the generalization being reconnected.
fn generalization_refusal(store: &Store, specific: Obj, general: Obj, except: Option<Obj>) -> Option<String> {
    use archforge_uml::relations::GeneralizationProblem as P;
    Some(match archforge_uml::relations::generalization_problem(store, specific, general, except)? {
        P::NotAnInterface => tr("An interface specializes only an interface"),
        P::ClassOfInterface => tr("A class cannot specialize an interface"),
        P::NotAnEnumeration => tr("An enumeration specializes only an enumeration"),
        P::NoEnumeration => tr("Only an enumeration specializes an enumeration"),
        P::NotADataType => tr("A data type specializes only a data type"),
        P::NoDataType => tr("Only a data type specializes a data type"),
        P::NotAPrimitiveType => tr("A primitive type specializes only a primitive type"),
        P::NoPrimitiveType => tr("Only a primitive type specializes a primitive type"),
        P::BuiltIn => tr("A built-in type specializes nothing"),
        P::AssociationClass => tr("Nothing specializes an association class"),
        P::Itself => tr("A classifier cannot specialize itself"),
        P::Final => fmt(
            &tr("{name} is final: nothing may specialise it. Clear \"No subclasses\" in its Properties first."),
            &[("name", &archforge_core::name_of(store, general))],
        ),
        P::Again => tr("This one specializes that one already"),
        P::Cycle => tr("That one specializes this one already, which would make a cycle"),
    })
}

/// Whether `element` is a class: a plain class or an association class, which is one too.
fn is_class(store: &Store, element: Obj) -> bool {
    store.class_of(element).is_ok_and(|c| c == archforge_uml::CLASS || c == archforge_uml::ASSOCIATION_CLASS)
}

/// Why `kind` starts no line at `element`, when it does not: a realization starts only at a class
/// (an association class is one), no association at an enumeration, a data type or a primitive
/// type, and an association class only at a class.
fn kind_start_problem(kind: Relate, store: &Store, element: Obj) -> Option<String> {
    // An n-ary association is drawn by clicks, never dragged from a class (AR-128).
    if kind == Relate::NAry {
        return Some(tr("An n-ary association is drawn by clicking each class in turn"));
    }
    if kind == Relate::Realization && !is_class(store, element) {
        return Some(tr("Only a class implements an interface"));
    }
    if kind == Relate::TemplateBinding {
        return crate::uml_bindings::start_problem(store, element);
    }
    association_refusal(kind, store, element).or_else(|| class_end_refusal(kind, store, element))
}

/// Why an association class of `kind` has no end at `element`, when it has none, in the user's
/// words: its ends are classes only (`association_class_end_problem`).
fn class_end_refusal(kind: Relate, store: &Store, element: Obj) -> Option<String> {
    (kind == Relate::AssociationClass && archforge_uml::relations::association_class_end_problem(store, element, None).is_some())
        .then(|| tr("An association class joins only classes"))
}

/// Why an association of `kind` has no end at `element`, when it has none, in the user's words for
/// what it is: a data type joins no association, an enumeration is one, and a primitive type joins
/// none either.
pub(crate) fn association_refusal(kind: Relate, store: &Store, element: Obj) -> Option<String> {
    if !kind.associates() || archforge_uml::relations::association_end_problem(store, element).is_none() {
        return None;
    }
    Some(match store.class_of(element).ok() {
        Some(c) if c == archforge_uml::ENUMERATION => tr("An enumeration is a data type and joins no association"),
        Some(c) if c == archforge_uml::PRIMITIVE_TYPE => tr("A primitive type joins no association"),
        _ => tr("A data type joins no association"),
    })
}

/// Why the chosen kind starts no line at `element`, when it does not.
pub(crate) fn start_problem(shell: &Shell, store: &Store, element: Obj) -> Option<String> {
    kind_start_problem(shell.canvas.uml_relation, store, element)
}

/// The Relationship tool's tooltip and accessible name when the kind it draws needs its direction
/// said: none takes the tool's own.
pub(crate) fn tool_hint(shell: &Shell) -> Option<String> {
    shell.canvas.uml_relation.hint().map(tr)
}

/// Draws the chosen kind between the classes of two nodes, its line on diagram `d` with its name
/// at the middle, as one undo step; selects it. A refusal is said at the canvas's top, in the
/// user's words where the shell knows the rule (a realization's, a generalization's, an
/// association's at an enumeration, a data type or a primitive type).
pub(crate) fn connect(shell: &mut Shell, d: Obj, from_node: Uuid, to_node: Uuid) {
    shell.canvas.notice = None;
    let kind = shell.canvas.uml_relation;
    // An n-ary association is drawn by clicks, never dragged (AR-128).
    if kind == Relate::NAry {
        shell.canvas.notice = Some(tr("An n-ary association is drawn by clicking each class in turn"));
        return;
    }
    let Some(session) = shell.session.as_mut() else { return };
    let store = session.doc();
    // A realization refused draws nothing, and says why in the user's words.
    if kind == Relate::Realization
        && let Some(why) = realization_refusal(store, class_of_node(store, from_node), class_of_node(store, to_node), None)
    {
        shell.canvas.notice = Some(why);
        return;
    }
    let (Some(from), Some(to)) = (class_of_node(store, from_node), class_of_node(store, to_node)) else {
        shell.canvas.notice = Some(tr("A relationship joins two classes or interfaces"));
        return;
    };
    // An association to or from an enumeration, a data type or a primitive type draws nothing either, nor a generalization the
    // classifier rules refuse.
    let refusal = association_refusal(kind, store, from)
        .or_else(|| association_refusal(kind, store, to))
        .or_else(|| class_end_refusal(kind, store, from))
        .or_else(|| class_end_refusal(kind, store, to))
        .or_else(|| (kind == Relate::Generalization).then(|| generalization_refusal(store, from, to, None)).flatten());
    if let Some(why) = refusal {
        shell.canvas.notice = Some(why);
        return;
    }
    if kind == Relate::AssociationClass {
        connect_class(shell, d, (from, from_node), (to, to_node), None);
        return;
    }
    // A template binding writes nothing yet: its draft opens in Properties for the arguments (C3).
    if kind == Relate::TemplateBinding {
        match crate::uml_bindings::refusal(store, from, to, None) {
            Some(why) => shell.canvas.notice = Some(why),
            None => crate::uml_bindings::open_new(shell, d, (from_node, to_node), from, to),
        }
        return;
    }
    let (Ok(from), Ok(to), Ok(diagram)) = (store.uuid(from), store.uuid(to), store.uuid(d)) else { return };
    let (cmd, params, label) = kind.command(from, to);
    let out = session.batch(&tr(label), |b| {
        let r = b.execute(cmd, &params)?;
        let element = r.get("id").and_then(|i| i.as_str()).unwrap_or_default().to_string();
        let e = b.execute(
            "diagram.connect",
            &json!({ "diagram": diagram.to_string(), "element": element, "source": from_node.to_string(), "target": to_node.to_string() }),
        )?;
        if kind.named() {
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

/// How far above its line's middle an association class's card sits, to the card's foot.
const CARD_RISE: f64 = 56.0;

/// Where an association class's card goes for a line between the rectangles `a` and `b`: centred
/// over the middle between them, its foot [`CARD_RISE`] above it, at the card's size.
fn card_place(a: archforge_diagram::Rect, b: archforge_diagram::Rect) -> (f64, f64, f64, f64) {
    let (w, h) = crate::uml_classifier::Classifier::AssociationClass.size();
    let middle = ((a.x + a.width / 2.0 + b.x + b.width / 2.0) / 2.0, (a.y + a.height / 2.0 + b.y + b.height / 2.0) / 2.0);
    ((middle.0 - w / 2.0).round(), (middle.1 - CARD_RISE - h).round(), w, h)
}

/// A new class to make for an association class dropped on empty canvas: the class's name and its
/// card's place, x, y, width and height.
type NewEnd = (String, (f64, f64, f64, f64));

/// Draws an association class from the class `from` shows to the one `to` shows (each its element
/// and its node), or, with `new`, to a new class made in the diagram's package at its place (`to`
/// is then not read): the
/// element, its line and its card above the line's middle, as one undo step. The association class
/// takes a fresh name in the first class's package, its card's name opens for typing; with a new
/// class, that class's name does, as an association's new class's.
pub(crate) fn connect_class(shell: &mut Shell, d: Obj, from: (Obj, Uuid), to: (Obj, Uuid), new: Option<NewEnd>) {
    let Some(session) = shell.session.as_mut() else { return };
    let store = session.doc();
    let Some(package) = archforge_uml::nesting::home_package(store, from.0) else {
        shell.canvas.notice = Some(tr("A relationship joins two classes or interfaces"));
        return;
    };
    let mut taken: Vec<String> =
        archforge_uml::commands::classifiers(store, package).into_iter().map(|c| archforge_core::name_of(store, c)).collect();
    taken.extend(new.as_ref().map(|(n, _)| n.clone()));
    let Some(name) = crate::explorer::fresh_name(crate::uml_classifier::Classifier::AssociationClass.stem(), taken.into_iter(), false) else {
        shell.canvas.notice = Some(tr("No free name is left"));
        return;
    };
    let rect_of = |node: Uuid| store.by_uuid(node).map(|n| archforge_diagram::grid::bounds_of(store, n));
    let to_rect = match &new {
        Some((_, (x, y, w, h))) => Some(archforge_diagram::Rect::new(*x, *y, *w, *h)),
        None => rect_of(to.1),
    };
    let (Some(from_rect), Some(to_rect)) = (rect_of(from.1), to_rect) else { return };
    let (cx, cy, cw, ch) = card_place(from_rect, to_rect);
    let (Ok(diagram), Ok(from_id), Ok(home)) = (store.uuid(d), store.uuid(from.0), store.uuid(package)) else { return };
    let to_id = store.uuid(to.0).ok();
    let diagram_package = store.get(d, archforge_diagram::diagram::ELEMENT).ok().and_then(|v| v.as_ref()).and_then(|p| store.uuid(p).ok());
    let named = name.clone();
    let made = new.clone();
    let out = session.batch(&tr("Add association class"), |b| {
        // A new class at the far end first, placed where the line was let go.
        let (to_class, to_node) = match &made {
            Some((class_name, (x, y, w, h))) => {
                let package = diagram_package.unwrap_or(home).to_string();
                let c = b.execute("uml.class.add", &json!({ "package": package, "name": class_name }))?;
                let class = c.get("id").and_then(|i| i.as_str()).unwrap_or_default().to_string();
                let n = b.execute(
                    "diagram.place",
                    &json!({ "diagram": diagram.to_string(), "element": class, "x": x, "y": y, "width": w, "height": h }),
                )?;
                (class, n.get("id").and_then(|i| i.as_str()).unwrap_or_default().to_string())
            }
            None => (to_id.map(|u| u.to_string()).unwrap_or_default(), to.1.to_string()),
        };
        let a = b.execute("uml.association-class.add", &json!({ "from": from_id.to_string(), "to": to_class, "name": named }))?;
        let element = a.get("id").and_then(|i| i.as_str()).unwrap_or_default().to_string();
        b.execute(
            "diagram.connect",
            &json!({ "diagram": diagram.to_string(), "element": element, "source": from.1.to_string(), "target": to_node }),
        )?;
        b.execute("diagram.place", &json!({ "diagram": diagram.to_string(), "element": element, "x": cx, "y": cy, "width": cw, "height": ch }))?;
        Ok(())
    });
    match out {
        Ok(o) => {
            let id_at = |i: usize| o.value.get(i).and_then(|v| v.get("id")).and_then(|v| v.as_str()).and_then(Uuid::parse);
            // With a new class the steps are its class, its node, the association class, its line
            // and its card; else the association class, its line and its card.
            let (element, card) = if new.is_some() { (id_at(2), id_at(4)) } else { (id_at(0), id_at(2)) };
            let Some(element) = element else { return };
            let _ = run(shell, "view.select", json!({ "ids": [element.to_string()] }));
            let rename = match new {
                Some((class_name, _)) => id_at(0).zip(id_at(1)).map(|(class, node)| (node, class, class_name)),
                None => card.map(|card| (card, element, name)),
            };
            if let Some((node, entity, text)) = rename {
                shell.canvas.rename = Some(crate::editing::NodeRename { node, entity, text, opened: true, refused: None });
                shell.canvas.focused = true;
            }
        }
        Err(e) => shell.canvas.notice = Some(e.to_string()),
    }
}

/// The chosen kind dropped on empty canvas at `(x, y)`: a new class there, related to the node's,
/// as one undo step; its name opens for typing. A generalization dragged out of an interface makes
/// an interface, the only kind an interface specializes, one out of an enumeration an enumeration,
/// one out of a data type a data type, one out of a primitive type a primitive type, and a
/// realization an interface, the only kind a class realizes.
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
        shell.canvas.notice = Some(tr("A relationship joins two classes or interfaces"));
        return;
    };
    if let Some(why) = kind_start_problem(kind, store, from) {
        shell.canvas.notice = Some(why);
        return;
    }
    // A binding ends at a template that exists: empty canvas makes none.
    if kind == Relate::TemplateBinding {
        shell.canvas.notice = Some(tr("A binding ends at a template: a class, an interface or a data type with template parameters"));
        return;
    }
    if kind == Relate::AssociationClass {
        let names = archforge_uml::commands::classifiers(store, package).into_iter().map(|c| archforge_core::name_of(store, c));
        let Some(name) = crate::explorer::fresh_name(crate::uml_classifier::Classifier::Class.stem(), names, false) else {
            shell.canvas.notice = Some(tr("No free name is left"));
            return;
        };
        let (w, h) = crate::uml_classifier::Classifier::Class.size();
        let place = ((x - w / 2.0).round(), (y - h / 2.0).round(), w, h);
        connect_class(shell, d, (from, from_node), (from, from_node), Some((name, place)));
        return;
    }
    let (Ok(package_id), Ok(diagram), Ok(from_id)) = (store.uuid(package), store.uuid(d), store.uuid(from)) else { return };
    let made = match crate::uml_classifier::Classifier::of(store, from) {
        _ if kind == Relate::Realization => crate::uml_classifier::Classifier::Interface,
        Some(crate::uml_classifier::Classifier::Interface) if kind == Relate::Generalization => crate::uml_classifier::Classifier::Interface,
        Some(crate::uml_classifier::Classifier::Enumeration) if kind == Relate::Generalization => crate::uml_classifier::Classifier::Enumeration,
        Some(crate::uml_classifier::Classifier::DataType) if kind == Relate::Generalization => crate::uml_classifier::Classifier::DataType,
        Some(crate::uml_classifier::Classifier::PrimitiveType) if kind == Relate::Generalization => crate::uml_classifier::Classifier::PrimitiveType,
        _ => crate::uml_classifier::Classifier::Class,
    };
    let names = archforge_uml::commands::classifiers(store, package).into_iter().map(|c| archforge_core::name_of(store, c));
    let Some(name) = crate::explorer::fresh_name(made.stem(), names, false) else {
        shell.canvas.notice = Some(tr("No free name is left"));
        return;
    };
    let (w, h) = made.size();
    let (px, py) = ((x - w / 2.0).round(), (y - h / 2.0).round());
    let name_for_batch = name.clone();
    let out = session.batch(&made.add_step(), |b| {
        let c = b.execute(made.add_command(), &json!({ "package": package_id.to_string(), "name": name_for_batch }))?;
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
        if kind.named() {
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
/// class it changes nothing, and a refusal is said at the canvas's top: a realization's, a
/// generalization's, and an association's moved onto an enumeration, a data type or a primitive type, in the user's
/// words.
pub(crate) fn reconnect(shell: &mut Shell, element: Uuid, source: bool, to_node: Uuid) {
    shell.canvas.notice = None;
    let Some(store) = shell.session.as_ref().map(|s| s.doc()) else { return };
    let line = store.by_uuid(element);
    // An n-ary association's leg moves its end onto another class (AR-128).
    if line.is_some_and(|l| archforge_uml::nary::association_of_end(store, l).is_some()) {
        crate::uml_nary::reconnect_end(shell, element, to_node);
        return;
    }
    if let Some(r) = line.filter(|r| store.class_of(*r).ok() == Some(archforge_uml::INTERFACE_REALIZATION))
        && let Some((implementing, contract)) = archforge_uml::figures::ends(store, r)
    {
        let moved = class_of_node(store, to_node);
        let (i, c) = if source { (moved, Some(contract)) } else { (Some(implementing), moved) };
        // When the end that stays is wrong already (a project edited by hand), the model says so.
        let staying_fine = if source { store.class_of(contract).ok() == Some(archforge_uml::INTERFACE) } else { is_class(store, implementing) };
        if staying_fine
            && moved != Some(if source { implementing } else { contract })
            && let Some(why) = realization_refusal(store, i, c, Some(r))
        {
            shell.canvas.notice = Some(why);
            return;
        }
    }
    let Some(target) = class_of_node(store, to_node) else {
        shell.canvas.notice = Some(tr("A relationship joins two classes or interfaces"));
        return;
    };
    if line.is_some_and(|l| store.class_of(l).ok() == Some(archforge_uml::ASSOCIATION))
        && let Some(why) = association_refusal(Relate::Association, store, target)
    {
        shell.canvas.notice = Some(why);
        return;
    }
    // An association class's end moves onto another class only, never onto its own card.
    if let Some(l) = line.filter(|l| store.class_of(*l).ok() == Some(archforge_uml::ASSOCIATION_CLASS)) {
        let why = if l == target {
            Some(tr("An association class does not join itself"))
        } else {
            class_end_refusal(Relate::AssociationClass, store, target)
        };
        if let Some(why) = why {
            shell.canvas.notice = Some(why);
            return;
        }
    }
    let ends = line.and_then(|e| archforge_uml::figures::ends(store, e));
    let now = ends.map(|(a, b)| if source { a } else { b });
    if now == Some(target) {
        return;
    }
    // A binding's ends keep the binding rules; another template needs its arguments picked (AR-131).
    if let (Some(r), Some((bound, template))) = (line.filter(|l| store.class_of(*l).ok() == Some(archforge_uml::TEMPLATE_BINDING)), ends) {
        let (b, t) = if source { (target, template) } else { (bound, target) };
        let why = crate::uml_bindings::refusal(store, b, t, Some(r))
            .or_else(|| (!source).then(|| crate::uml_bindings::needs_arguments(store, t)).flatten());
        if let Some(why) = why {
            shell.canvas.notice = Some(why);
            return;
        }
    }
    // A generalization in a set keeps its ends (AR-129): said here and in its Properties.
    if let Some(r) = line.filter(|l| store.class_of(*l).ok() == Some(archforge_uml::GENERALIZATION))
        && let Some(why) = crate::uml_sets::reconnect_refusal(store, r, target)
    {
        shell.canvas.notice = Some(why.clone());
        shell.panels.set_refused = Some((element, why));
        return;
    }
    // A generalization's moved end keeps the classifier rules, said in the user's words.
    if let (Some(r), Some((specific, general))) = (line.filter(|l| store.class_of(*l).ok() == Some(archforge_uml::GENERALIZATION)), ends) {
        let (s, g) = if source { (target, general) } else { (specific, target) };
        if let Some(why) = generalization_refusal(store, s, g, Some(r)) {
            shell.canvas.notice = Some(why);
            return;
        }
    }
    let Ok(class) = store.uuid(target) else { return };
    let end = if source { "source" } else { "target" };
    match run(shell, "uml.relationship.reconnect", json!({ "relationship": element.to_string(), "end": end, "class": class.to_string() })) {
        // What the moved ends no longer subset or redefine went in the same step: said.
        Ok(out) => {
            let names: Vec<&str> =
                out.get("dropped").and_then(|d| d.as_array()).into_iter().flatten().filter_map(|d| d.get("name")?.as_str()).collect();
            if !names.is_empty() {
                shell.canvas.notice =
                    Some(fmt(&tr("The moved ends no longer subset or redefine {names}, so those were dropped."), &[("names", &names.join(", "))]));
            }
        }
        Err(e) => shell.canvas.notice = Some(e),
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
    /// Each end's static and modifiers as edited, and as the model last had them (AR-126): Apply
    /// writes the edited ones, Cancel and Escape bring the model's back.
    pub modifiers: [EndModifiers; 2],
    pub written_modifiers: [EndModifiers; 2],
    /// Why an end's modifiers were refused, with the field it sits under.
    pub modifier_refused: [Option<(&'static str, String)>; 2],
    /// Whether each end's Modifiers are open: closed, a line sums them up.
    pub modifiers_open: [bool; 2],
}

/// An association end's static and modifiers as its editor holds them (AR-126).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EndModifiers {
    pub is_static: bool,
    pub default: String,
    pub derived: bool,
    pub read_only: bool,
    pub ordered: bool,
    pub unique: bool,
    pub subsets: Vec<Uuid>,
    pub redefines: Vec<Uuid>,
}

impl EndModifiers {
    /// The modifiers end `e` has in the model.
    pub(crate) fn of(store: &Store, e: Obj) -> EndModifiers {
        let (derived, read_only, ordered, unique) = archforge_uml::modifiers::flags(store, e);
        let ids = |ts: Vec<Obj>| ts.into_iter().filter_map(|t| store.uuid(t).ok()).collect();
        EndModifiers {
            is_static: store.get(e, archforge_uml::feature::IS_STATIC).ok().and_then(|v| v.as_bool()).unwrap_or(false),
            default: archforge_uml::modifiers::default_text(store, e),
            derived,
            read_only,
            ordered,
            unique,
            subsets: ids(archforge_uml::modifiers::subsets(store, e)),
            redefines: ids(archforge_uml::modifiers::redefines(store, e)),
        }
    }

    /// Puts these modifiers into `uml.association.set-end`'s parameters `params`.
    pub(crate) fn put_into(&self, params: &mut serde_json::Value) {
        if let Some(fields) = params.as_object_mut() {
            fields.insert("static".into(), json!(self.is_static));
            fields.insert("default".into(), json!(self.default));
            fields.insert("derived".into(), json!(self.derived));
            fields.insert("read_only".into(), json!(self.read_only));
            fields.insert("ordered".into(), json!(self.ordered));
            fields.insert("unique".into(), json!(self.unique));
            fields.insert("subsets".into(), json!(self.subsets.iter().map(Uuid::to_string).collect::<Vec<_>>()));
            fields.insert("redefines".into(), json!(self.redefines.iter().map(Uuid::to_string).collect::<Vec<_>>()));
        }
    }
}

/// The ends' editor of association `a`, whose id is `id`, as the model has it.
fn ends_edit(store: &Store, a: Obj, id: Uuid) -> Option<EndsEdit> {
    let ends = archforge_uml::figures::member_ends(store, a)?;
    let role = ends.map(|e| archforge_core::name_of(store, e));
    let multiplicity = ends.map(|e| archforge_uml::figures::multiplicity(store, e));
    let modifiers = ends.map(|e| EndModifiers::of(store, e));
    Some(EndsEdit {
        association: id,
        written_role: role.clone(),
        written: multiplicity.clone(),
        role,
        multiplicity,
        refused: [None, None],
        written_modifiers: modifiers.clone(),
        modifiers,
        modifier_refused: [None, None],
        modifiers_open: [false, false],
    })
}

/// Why end `end`'s modifiers as edited cannot be written, in the user's words, with the field it
/// sits under: a default its type does not read, or a property it adds to what it subsets or
/// redefines that the rules refuse; the commands keep the same rules.
pub(crate) fn end_refusal(store: &Store, end: Obj, m: &EndModifiers) -> Option<(&'static str, String)> {
    let ty = store.get(end, archforge_uml::typed_element::TYPE).ok().and_then(|v| v.as_ref());
    let form = archforge_uml::modifiers::DefaultForm::of(store, ty);
    if let Err(e) = archforge_uml::modifiers::parse_default(store, form, &m.default) {
        return Some(("default", crate::uml_panel::default_refusal(store, &e)));
    }
    let shape = archforge_uml::modifiers::Shape::of(store, end);
    let had = EndModifiers::of(store, end);
    crate::uml_panel::target_refusals(store, &shape, Some(&shape), (&m.subsets, &had.subsets), (&m.redefines, &had.redefines))
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

/// The common multiplicities an end's menu offers, in the design's order: none to many once, as
/// `*`, the way the model writes it; `0..*` typed still reads as it.
pub(crate) const MULTIPLICITIES: [&str; 4] = ["0..1", "1", "*", "1..*"];

/// Why a typed multiplicity does not read, in the user's language.
pub(crate) fn multiplicity_problem(e: &archforge_uml::members::MultiplicityError) -> String {
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

/// A line's Properties: an association's or a dependency's name, its classes (a realization's
/// implementing class and the interface it realizes), an association's ends; an association
/// class's name, classes and ends, then its attributes and operations as a class's.
pub(crate) fn body(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect, mut y: f32, subject: &Subject) {
    let paint = ui.painter().clone();
    let width = (r.width() - 2.0 * PAD).max(0.0);
    let (Some(o), Some(id)) = (subject.obj, subject.id) else { return };
    let Some(store) = shell.session.as_ref().map(|s| s.doc()) else { return };
    // An n-ary association's panel is its own: its name and package, then its ends (AR-128).
    if archforge_uml::nary::is_nary(store, o) {
        crate::uml_nary::body(shell, ui, p, r, y, subject, o, id);
        return;
    }
    let class = store.class_of(o).ok();
    let name = |c: Option<Obj>| c.map(|c| archforge_core::name_of(store, c)).unwrap_or_default();
    let ends = archforge_uml::figures::ends(store, o);
    let (from, to) = (name(ends.map(|e| e.0)), name(ends.map(|e| e.1)));
    // A generalization and a realization have no name to show; an association class is named as
    // the classifier it is.
    let association_class = class == Some(archforge_uml::ASSOCIATION_CLASS);
    let (rename, key) =
        if association_class { ("uml.association-class.rename", "association_class") } else { ("uml.relationship.rename", "relationship") };
    if class != Some(archforge_uml::GENERALIZATION) && class != Some(archforge_uml::INTERFACE_REALIZATION) {
        crate::panels::name_field(shell, ui, p, pos2(r.left() + PAD, y), width, subject, rename, key);
        y += 17.0 + FIELD + 12.0;
    }
    // The classes it joins, read only: its ends drag onto another class on the canvas.
    let (first, second) = match class {
        Some(c) if c == archforge_uml::GENERALIZATION => (tr("Specific"), tr("General")),
        Some(c) if c == archforge_uml::DEPENDENCY => (tr("Client"), tr("Supplier")),
        Some(c) if c == archforge_uml::INTERFACE_REALIZATION => (tr("Implementing class"), tr("Realized interface")),
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
    // A realization's ends each move onto their own kind.
    let hint = if class == Some(archforge_uml::INTERFACE_REALIZATION) {
        tr("Drag the class end onto another class, the interface end onto another interface.")
    } else {
        tr("Drag a line end onto another class.")
    };
    // Wrapped, so the realization's longer hint reads whole in a narrow panel.
    let said = ux::galley_rows(&paint, &hint, types::CAPTION, p.text_muted, Some(width), 3);
    let height = said.size().y;
    paint.galley(pos2(r.left() + PAD, y), said, p.text_muted);
    y += (height + 12.0).max(28.0);
    if class.is_some_and(archforge_uml::is_association) {
        y = ends_sections(shell, ui, p, r, y, o, id);
    }
    // The sets a generalization is in (AR-129).
    if class == Some(archforge_uml::GENERALIZATION) {
        crate::uml_sets::generalization_section(shell, ui, p, r, y, o, id);
    }
    if association_class {
        use crate::uml_panel::{MemberKind, members_section};
        let y = members_section(shell, ui, p, r, y, o, id, MemberKind::Attribute);
        let y = members_section(shell, ui, p, r, y + 4.0, o, id, MemberKind::Operation);
        // Its constraints, as the class it is (AR-133).
        if shell.session.as_ref().is_some_and(|s| archforge_uml::constraints::is_context(s.doc(), o)) {
            crate::uml_constraints::section_of(shell, ui, p, r, y + 4.0, o, id);
        }
    }
}

/// An association's or an association class's two ends, each its own section: role, written when
/// typed; the multiplicity, an editable select of the common ones, written when picked or typed;
/// Navigable and the aggregation, written when changed, the end's modifiers sent as the model has
/// them; then the end's Modifiers (AR-126), written by their own Apply. A multiplicity that does
/// not read stays in its field, with why under it, and the model keeps the one it had. Gives the y
/// under the ends.
#[allow(clippy::too_many_arguments, reason = "the ends are drawn into the panel for their association")]
fn ends_sections(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect, mut y: f32, a: Obj, id: Uuid) -> f32 {
    let width = (r.width() - 2.0 * PAD).max(0.0);
    let Some(store) = shell.session.as_ref().map(|s| s.doc()) else { return y };
    let Some(ends) = archforge_uml::figures::member_ends(store, a) else { return y };
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
    let modifiers = ends.map(|e| EndModifiers::of(store, e));
    let Some(mut edit) = shell.panels.ends.clone() else { return y };
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
        if edit.written_modifiers[i] != modifiers[i] {
            edit.modifiers[i] = modifiers[i].clone();
            edit.written_modifiers[i] = modifiers[i].clone();
            edit.modifier_refused[i] = None;
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
        let refusal = edit.refused[i].clone();
        // Where the other end is qualified, this end's multiplicity counts per qualifier value (AR-127).
        let per_value = shell.session.as_ref().and_then(|s| crate::uml_qualifiers::per_value_note(s.doc(), ends[1 - i], ends[i]));
        let m_label = if per_value.is_some() { tr("Per qualifier value") } else { tr("Multiplicity") };
        let combo = Combo {
            id: &format!("{key}.multiplicity"),
            label: &m_label,
            name: &format!("{title} · {m_label}"),
            placeholder: "—",
            options: &MULTIPLICITIES,
            current: MULTIPLICITIES.iter().position(|m| *m == edit.written[i]),
            pick: &tr("Pick a multiplicity"),
            pick_name: &format!("{title} · {}", tr("Pick a multiplicity")),
            hint: &tr("Or type a number or range"),
            clear: &tr("Clear"),
            refused: refusal.as_deref(),
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
                    // Corrected back to what the model has: nothing to write, and nothing wrong.
                    edit.multiplicity[i] = edit.written[i].clone();
                    edit.refused[i] = None;
                    write_multiplicity = false;
                }
                Ok(_) => {}
            }
        }
        y += 17.0 + FIELD + 12.0;
        if let Some(why) = &edit.refused[i] {
            // Wrapped, so a long refusal from the model shows its reason, not only its start.
            let paint = ui.painter().clone();
            let said = ux::galley_rows(&paint, why, types::CAPTION, p.danger, Some(width), 4);
            let height = said.size().y;
            paint.galley(pos2(r.left() + PAD, y - 2.0), said, p.danger);
            y += (height + 26.0).max(42.0);
        } else if let Some(note) = &per_value {
            let paint = ui.painter().clone();
            let said = ux::galley_rows(&paint, note, types::CAPTION, p.text_muted, Some(width), 3);
            let height = said.size().y;
            paint.galley(pos2(r.left() + PAD, y - 4.0), said, p.text_muted);
            y += height + 8.0;
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
        // Its static and modifiers, under a disclosure that sums them up while closed, written
        // together by their own Apply (AR-126).
        let open = edit.modifiers_open[i];
        let more = Rect::from_min_size(pos2(r.left() + PAD, y - 6.0), vec2(width, 24.0));
        ux::icon_in(
            &ui.painter().clone(),
            Rect::from_min_size(more.min + vec2(0.0, 4.0), egui::Vec2::splat(16.0)),
            if open { &crate::ux::icons::CHEVRON_DOWN } else { &crate::ux::icons::CHEVRON_RIGHT },
            p.text_secondary,
        );
        let title = tr("Modifiers");
        let g = ux::text(&ui.painter().clone(), more.min + vec2(22.0, 2.0), &title, types::UI, p.text, Some((width - 22.0).max(0.0)));
        let summary = shell.session.as_ref().map(|s| modifiers_summary(s.doc(), ends[i])).unwrap_or_default();
        if !open {
            let x = g.right() + 10.0;
            ux::text(&ui.painter().clone(), pos2(x, more.top() + 3.0), &summary, types::SMALL, p.text_muted, Some((more.right() - x).max(0.0)));
        }
        let name = if open { title.clone() } else { format!("{title}: {summary}") };
        if control(shell, ui, &format!("{key}.modifiers"), "disclosure", more, true).accessible_name(name).clicked() {
            edit.modifiers_open[i] = !open;
        }
        y += 28.0;
        let (next, asked) = if open { end_modifiers(shell, ui, p, r, y, &mut edit, ends[i], i) } else { (y, Asked::Nothing) };
        y = next;
        // Its qualifiers, with their editor (AR-127).
        y = crate::uml_qualifiers::section_of(shell, ui, p, r, y, id, i, ends[i]);
        shell.panels.ends = Some(edit.clone());
        match asked {
            Asked::Nothing => {}
            Asked::Cancel => {
                if let Some(e) = shell.panels.ends.as_mut() {
                    e.modifiers[i] = e.written_modifiers[i].clone();
                    e.modifier_refused[i] = None;
                }
                return y;
            }
            Asked::Apply => {
                apply_end_modifiers(shell, id, ends[i], i, &roles[i], &written[i], navigable[i] && !whole, aggregations[i]);
                return y;
            }
        }
        // An association class's end takes no role one of its attributes is named, as they share
        // its names: said in the user's words, and nothing is written.
        let clash = shell.session.as_ref().and_then(|s| archforge_uml::members::attribute_clash(s.doc(), a, edit.role[i].trim()));
        if role && let Some(attribute) = clash {
            let named = shell.session.as_ref().map(|s| archforge_core::name_of(s.doc(), attribute)).unwrap_or_default();
            shell.panels.notice =
                Some(fmt(&tr("This association class has an attribute named {name}; an end's role takes another name."), &[("name", &named)]));
            return y;
        }
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
            // The modifiers go as the model has them: only Apply writes the edited ones.
            let mut params = params;
            modifiers[i].put_into(&mut params);
            match run(shell, "uml.association.set-end", params) {
                Ok(_) => {
                    shell.panels.notice = None;
                    if let Some(e) = shell.panels.ends.as_mut() {
                        // A refused multiplicity still in its field keeps its reason while
                        // another field of the end is written.
                        if write_multiplicity {
                            e.refused[i] = None;
                        }
                        e.role[i] = e.role[i].trim().to_string();
                    }
                }
                // A multiplicity the model refuses (a composition's whole allows one at most) is
                // said under its field; anything else at the panel's foot.
                Err(e) if write_multiplicity => {
                    if let Some(ends) = shell.panels.ends.as_mut() {
                        ends.refused[i] = Some(e);
                    }
                    if ui.input(|inp| inp.key_pressed(egui::Key::Enter)) {
                        ui.memory_mut(|m| m.request_focus(crate::widgets::egui_id(&format!("{key}.multiplicity"))));
                    }
                }
                Err(e) => shell.panels.notice = Some(e),
            }
            return y;
        }
    }
    y
}

/// End `end`'s static and modifiers summed up for its closed Modifiers: Static and Derived, then
/// what its line says after the multiplicity (`= none {ordered}`), or None when it has none.
fn modifiers_summary(store: &Store, end: Obj) -> String {
    let mut parts = Vec::new();
    if store.get(end, archforge_uml::feature::IS_STATIC).ok().and_then(|v| v.as_bool()).unwrap_or(false) {
        parts.push(tr("Static"));
    }
    if archforge_uml::modifiers::is_derived(store, end) {
        parts.push(tr("Derived"));
    }
    let said = archforge_uml::modifiers::suffix(store, end);
    if !said.trim().is_empty() {
        parts.push(said.trim().to_string());
    }
    if parts.is_empty() { tr("None") } else { parts.join(" · ") }
}

/// What an end's modifiers block asks of the frame: nothing, to bring the model's back, or to
/// write the edited ones.
enum Asked {
    Nothing,
    Cancel,
    Apply,
}

/// End `i`'s (`end`'s) static and modifiers in its section at `y` (AR-126): Default value; Static,
/// Derived, Read only, Ordered and Unique; Subsets and Redefines, which list only the properties
/// the rules allow for the end as the model has it, built once per state of the project, a muted
/// line under each that has none; then Cancel and Apply. A refusal sits under its field. Gives the
/// y under it and what was asked; Escape, with no menu open and something edited, brings the
/// model's back as Cancel does.
#[allow(clippy::too_many_arguments, reason = "the block is drawn into the end's section of the panel")]
fn end_modifiers(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect, mut y: f32, edit: &mut EndsEdit, end: Obj, i: usize) -> (f32, Asked) {
    let paint = ui.painter().clone();
    let width = (r.width() - 2.0 * PAD).max(0.0);
    let half = ((width - 8.0) / 2.0).max(0.0);
    let left = r.left() + PAD;
    let right = left + half + 8.0;
    let key = format!("workspace.properties.end.{i}");
    let refused = edit.modifier_refused[i].clone();
    let under = |paint: &egui::Painter, y: f32, fields: &[&str]| -> f32 {
        match &refused {
            Some((field, why)) if fields.contains(field) => {
                let said = ux::galley_rows(paint, why, types::CAPTION, p.danger, Some(width), 4);
                let height = said.size().y;
                paint.galley(pos2(left, y - 4.0), said, p.danger);
                y + height + 6.0
            }
            _ => y,
        }
    };
    let Some((end_id, role)) = shell.session.as_ref().and_then(|s| Some((s.doc().uuid(end).ok()?, archforge_core::name_of(s.doc(), end)))) else {
        return (y, Asked::Nothing);
    };
    // The choices, built once per state of the project: Subsets', Redefines' and the literals of
    // the end's type when it is an enumeration.
    let targets = crate::uml_panel::take_targets(
        shell,
        &format!("end|{end_id}"),
        |store| Some(archforge_uml::modifiers::Shape::of(store, end)),
        |store| store.get(end, archforge_uml::typed_element::TYPE).ok().and_then(|v| v.as_ref()),
    );
    let m = &mut edit.modifiers[i];
    let limit = archforge_uml::modifiers::MAX_DEFAULT;
    let default_id = format!("{key}.default");
    // Enter in the Default field applies, as in the attribute editor.
    let mut enter = false;
    let default = match &targets.literals {
        Some(_) => {
            let shown = if m.default.trim().is_empty() { tr("None") } else { m.default.clone() };
            Some(select_field(shell, ui, p, pos2(left, y), width, &default_id, &tr("Default value"), &shown, Menu::EndDefault(i)))
        }
        None => {
            enter = text_field(shell, ui, p, pos2(left, y), width, &default_id, &tr("Default value"), &mut m.default, limit, false, false);
            None
        }
    };
    y += 17.0 + FIELD + 10.0;
    y = under(&paint, y, &["default"]);
    check(shell, ui, p, pos2(left, y), &format!("{key}.static"), &tr("Static"), &mut m.is_static);
    check(shell, ui, p, pos2(right, y), &format!("{key}.derived"), &tr("Derived"), &mut m.derived);
    y += 26.0;
    check(shell, ui, p, pos2(left, y), &format!("{key}.read-only"), &tr("Read only"), &mut m.read_only);
    check(shell, ui, p, pos2(right, y), &format!("{key}.ordered"), &tr("Ordered"), &mut m.ordered);
    y += 26.0;
    check(shell, ui, p, pos2(left, y), &format!("{key}.unique"), &tr("Unique"), &mut m.unique);
    y += 16.0 + 16.0;
    let shown = |ids: &[Uuid]| match shell.session.as_ref() {
        Some(s) if !ids.is_empty() => {
            ids.iter().filter_map(|u| s.doc().by_uuid(*u)).map(|o| archforge_core::name_of(s.doc(), o)).collect::<Vec<_>>().join(", ")
        }
        _ => tr("None"),
    };
    let (subsets_shown, redefines_shown) = (shown(&m.subsets), shown(&m.redefines));
    let subsets = select_field(shell, ui, p, pos2(left, y), half, &format!("{key}.subsets"), &tr("Subsets"), &subsets_shown, Menu::EndSubsets(i));
    let redefines =
        select_field(shell, ui, p, pos2(right, y), half, &format!("{key}.redefines"), &tr("Redefines"), &redefines_shown, Menu::EndRedefines(i));
    y += 17.0 + FIELD + 6.0;
    let mut said = 0.0_f32;
    let offered = crate::uml_panel::offered_subsets(&targets, &role);
    for (empty, x, text) in [
        (offered.is_empty(), left, tr("No property to subset")),
        (targets.redefines.is_empty() && targets.leaf.is_empty(), right, tr("No property to redefine")),
    ] {
        if empty {
            let g = ux::galley_rows(&paint, &text, types::CAPTION, p.text_muted, Some(half), 2);
            said = said.max(g.size().y);
            paint.galley(pos2(x, y), g, p.text_muted);
        }
    }
    if said > 0.0 {
        y += said + 6.0;
    }
    y = under(&paint, y + 4.0, &["subsets", "redefines"]);
    // Cancel and Apply at the right, as an editor's are.
    let cancel = crate::panels::icon_button(
        shell,
        ui,
        p,
        pos2(left + width - 2.0 * FIELD - 8.0, y),
        &format!("{key}.cancel"),
        &crate::ux::icons::CLOSE,
        p.text_secondary,
        None,
        "Cancel",
    );
    let apply = crate::panels::icon_button(
        shell,
        ui,
        p,
        pos2(left + width - FIELD, y),
        &format!("{key}.apply"),
        &crate::ux::icons::TICK,
        p.on_accent,
        Some(p.accent_fill),
        "Apply",
    );
    y += FIELD + 16.0;
    // The menus over what is under them.
    match shell.panels.menu {
        Some(Menu::EndDefault(k)) if k == i => {
            if let (Some(field), Some(names)) = (default, targets.literals.as_ref())
                && let Some(pick) = crate::uml_panel::literal_menu(shell, ui, p, field, &default_id, names)
            {
                m.default = pick;
            }
        }
        Some(Menu::EndSubsets(k)) if k == i => {
            if let Some(pick) = crate::uml_panel::target_menu(shell, ui, p, subsets, &format!("{key}.subsets"), &offered) {
                m.subsets = pick;
            }
        }
        Some(Menu::EndRedefines(k)) if k == i => {
            match crate::uml_panel::redefines_menu(shell, ui, p, redefines, &format!("{key}.redefines"), &targets) {
                Some(Ok(pick)) => {
                    m.redefines = pick;
                    if edit.modifier_refused[i].as_ref().is_some_and(|(field, _)| *field == "redefines") {
                        edit.modifier_refused[i] = None;
                    }
                }
                // A leaf property: nothing changes, and why sits under Redefines (AR-138).
                Some(Err(why)) => edit.modifier_refused[i] = Some(("redefines", why)),
                None => {}
            }
        }
        _ => {}
    }
    drop(offered);
    crate::uml_panel::keep_targets(shell, targets);
    let edited = edit.modifiers[i] != edit.written_modifiers[i];
    let escape = edited && shell.panels.menu.is_none() && ui.input(|inp| inp.key_pressed(egui::Key::Escape));
    let asked = if cancel || escape {
        Asked::Cancel
    } else if apply || enter {
        Asked::Apply
    } else {
        Asked::Nothing
    };
    (y, asked)
}

/// Writes end `i`'s (`end`'s) modifiers as edited, with its role, multiplicity, navigability and
/// aggregation as the model has them, as one undo step, then closes the end's Modifiers; a refusal
/// sits under its field in the user's words, the edit kept and the Modifiers open, and nothing is
/// written.
#[allow(clippy::too_many_arguments, reason = "the end's other fields go as the model has them")]
fn apply_end_modifiers(shell: &mut Shell, id: Uuid, end: Obj, i: usize, role: &str, multiplicity: &str, navigable: bool, aggregation: usize) {
    let Some(m) = shell.panels.ends.as_ref().filter(|e| e.association == id).map(|e| e.modifiers[i].clone()) else { return };
    if let Some(why) = shell.session.as_ref().and_then(|s| end_refusal(s.doc(), end, &m)) {
        if let Some(e) = shell.panels.ends.as_mut() {
            e.modifier_refused[i] = Some(why);
        }
        return;
    }
    let mut params = json!({
        "association": id.to_string(),
        "end": i,
        "role": role,
        "multiplicity": multiplicity,
        "navigable": navigable,
        "aggregation": AGGREGATIONS[aggregation.min(AGGREGATIONS.len() - 1)],
    });
    m.put_into(&mut params);
    match run(shell, "uml.association.set-end", params) {
        // Written, the end's Modifiers close, their summary saying what they are now.
        Ok(_) => {
            shell.panels.notice = None;
            if let Some(e) = shell.panels.ends.as_mut() {
                e.modifier_refused[i] = None;
                e.modifiers_open[i] = false;
            }
        }
        // Anything the shell did not foresee is said at the panel's foot, as for the end's other
        // fields.
        Err(e) => shell.panels.notice = Some(e),
    }
}

/// Every word the kinds' menu and the panels say through `tr` at run time, for the catalogue.
#[cfg(test)]
pub(crate) fn texts() -> impl Iterator<Item = &'static str> {
    Relate::ALL.iter().flat_map(|k| std::iter::once(k.word()).chain(k.hint())).chain([
        "Add association",
        "Add generalization",
        "Add dependency",
        "Add interface realization",
        "Add association class",
        "An association class joins only classes",
        "An association class does not join itself",
        "Nothing specializes an association class",
        "This association class has an attribute named {name}; an end's role takes another name.",
    ])
}
