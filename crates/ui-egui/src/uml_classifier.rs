//! The classifier tool of a class diagram: one tool in Class's place that places the kind its
//! chevron's menu chose (Class, Interface, Enumeration, Data type or Primitive type; only the kinds that work are listed), its face showing
//! that kind's icon and its tooltip naming it. The menu marks the current kind; the arrow keys
//! walk it, Enter picks, Escape closes, and a pick arms the tool. The kind stays for the window's
//! next placements; a new window starts at Class.

use archforge_mof::{ClassId, Obj, Store};
use egui::{Rect, Stroke, StrokeKind, pos2, vec2};

use crate::Shell;
use crate::home::control;
use crate::i18n::tr;
use crate::ux::{self, Icon, Palette, icons, types};

/// What the classifier tool places.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Classifier {
    #[default]
    Class,
    Interface,
    /// An enumeration, whose members are its literals.
    Enumeration,
    /// A data type: a value without identity, with attributes and operations as a class has.
    DataType,
    /// A primitive type of the model's own: a value with no members, its card its name alone.
    PrimitiveType,
    /// An association class (AR-125): a class and an association at once, made only by drawing
    /// between two classes, so it is in neither the tool's menu nor Models' `+`.
    AssociationClass,
}

impl Classifier {
    /// The kinds the menu lists, in its order: those the tool places and Models' `+` adds.
    pub(crate) const ALL: [Classifier; 5] =
        [Classifier::Class, Classifier::Interface, Classifier::Enumeration, Classifier::DataType, Classifier::PrimitiveType];
    /// Every kind a class diagram shows as a card: the menu's, and the association class, which is
    /// only drawn.
    pub(crate) const EVERY: [Classifier; 6] = [
        Classifier::Class,
        Classifier::Interface,
        Classifier::Enumeration,
        Classifier::DataType,
        Classifier::PrimitiveType,
        Classifier::AssociationClass,
    ];

    /// Its name, in the menu, the tooltip and the Models' `+`.
    pub(crate) fn word(self) -> String {
        match self {
            Classifier::Class => tr("Class"),
            Classifier::Interface => tr("Interface"),
            Classifier::Enumeration => tr("Enumeration"),
            Classifier::DataType => tr("Data type"),
            Classifier::PrimitiveType => tr("Primitive type"),
            Classifier::AssociationClass => tr("Association class"),
        }
    }

    /// Its automation key.
    pub(crate) fn key(self) -> &'static str {
        match self {
            Classifier::Class => "class",
            Classifier::Interface => "interface",
            Classifier::Enumeration => "enumeration",
            Classifier::DataType => "data-type",
            Classifier::PrimitiveType => "primitive-type",
            Classifier::AssociationClass => "association-class",
        }
    }

    /// The stem of a fresh name for one: `Class 1`, `Interface 1`, `Enumeration 1`, `DataType 1`,
    /// `PrimitiveType 1`, `AssociationClass 1`.
    pub(crate) fn stem(self) -> &'static str {
        match self {
            Classifier::Class => "Class",
            Classifier::Interface => "Interface",
            Classifier::Enumeration => "Enumeration",
            Classifier::DataType => "DataType",
            Classifier::PrimitiveType => "PrimitiveType",
            Classifier::AssociationClass => "AssociationClass",
        }
    }

    /// Its icon on the tool's face, and beside its name in the menu and in Models; an association
    /// class, which no tool places, shows its tree icon.
    pub(crate) fn tool_icon(self) -> &'static Icon {
        match self {
            Classifier::Class => &icons::TOOL_CLASS,
            Classifier::Interface => &icons::TOOL_INTERFACE,
            Classifier::Enumeration => &icons::TOOL_ENUMERATION,
            Classifier::DataType => &icons::TOOL_DATA_TYPE,
            Classifier::PrimitiveType => &icons::TOOL_PRIMITIVE_TYPE,
            Classifier::AssociationClass => &icons::ASSOCIATION_CLASS,
        }
    }

    /// Its icon in the menu of kinds, in Models and in the search.
    pub(crate) fn icon(self) -> &'static Icon {
        match self {
            Classifier::Class => &icons::CLASS,
            Classifier::Interface => &icons::INTERFACE,
            Classifier::Enumeration => &icons::ENUMERATION,
            Classifier::DataType => &icons::DATA_TYPE,
            Classifier::PrimitiveType => &icons::PRIMITIVE_TYPE,
            Classifier::AssociationClass => &icons::ASSOCIATION_CLASS,
        }
    }

    /// The metaclass it makes.
    pub(crate) fn metaclass(self) -> ClassId {
        match self {
            Classifier::Class => archforge_uml::CLASS,
            Classifier::Interface => archforge_uml::INTERFACE,
            Classifier::Enumeration => archforge_uml::ENUMERATION,
            Classifier::DataType => archforge_uml::DATA_TYPE,
            Classifier::PrimitiveType => archforge_uml::PRIMITIVE_TYPE,
            Classifier::AssociationClass => archforge_uml::ASSOCIATION_CLASS,
        }
    }

    /// The kind of `o`, when it is a classifier a class diagram shows as a card: by its own
    /// metaclass, so an enumeration is never taken for a data type, nor an association class for a
    /// class.
    pub(crate) fn of(store: &Store, o: Obj) -> Option<Classifier> {
        let c = store.class_of(o).ok()?;
        Classifier::EVERY.into_iter().find(|k| k.metaclass() == c)
    }

    /// The command that adds one in a package; an association class's takes the two classes it
    /// joins instead, so only the Relationship tool runs it.
    pub(crate) fn add_command(self) -> &'static str {
        match self {
            Classifier::Class => "uml.class.add",
            Classifier::Interface => "uml.interface.add",
            Classifier::Enumeration => "uml.enumeration.add",
            Classifier::DataType => "uml.data-type.add",
            Classifier::PrimitiveType => "uml.primitive-type.add",
            Classifier::AssociationClass => "uml.association-class.add",
        }
    }

    /// The command that renames one, with the key that names it.
    pub(crate) fn rename_command(self) -> (&'static str, &'static str) {
        match self {
            Classifier::Class => ("uml.class.rename", "class"),
            Classifier::Interface => ("uml.interface.rename", "interface"),
            Classifier::Enumeration => ("uml.enumeration.rename", "enumeration"),
            Classifier::DataType => ("uml.data-type.rename", "data_type"),
            Classifier::PrimitiveType => ("uml.primitive-type.rename", "primitive_type"),
            Classifier::AssociationClass => ("uml.association-class.rename", "association_class"),
        }
    }

    /// The command that moves one into another package, with the key that names it.
    pub(crate) fn move_command(self) -> (&'static str, &'static str) {
        match self {
            Classifier::Class => ("uml.class.move", "class"),
            Classifier::Interface => ("uml.interface.move", "interface"),
            Classifier::Enumeration => ("uml.enumeration.move", "enumeration"),
            Classifier::DataType => ("uml.data-type.move", "data_type"),
            Classifier::PrimitiveType => ("uml.primitive-type.move", "primitive_type"),
            Classifier::AssociationClass => ("uml.association-class.move", "association_class"),
        }
    }

    /// The undo step that adds one, and the one that places one from Models.
    pub(crate) fn add_step(self) -> String {
        match self {
            Classifier::Class => tr("Add class"),
            Classifier::Interface => tr("Add interface"),
            Classifier::Enumeration => tr("Add enumeration"),
            Classifier::DataType => tr("Add data type"),
            Classifier::PrimitiveType => tr("Add primitive type"),
            Classifier::AssociationClass => tr("Add association class"),
        }
    }

    /// Whether its card takes attributes and operations: a class's, an interface's, a data type's
    /// and an association class's do; an enumeration's members are its literals, and a primitive
    /// type has none.
    pub(crate) fn takes_members(self) -> bool {
        !matches!(self, Classifier::Enumeration | Classifier::PrimitiveType)
    }

    /// Whether it has members of any kind to add: every kind but a primitive type, whose card is
    /// its name alone (no adorner, compartment `+`, menu row or ⌘↩).
    pub(crate) fn has_members(self) -> bool {
        self != Classifier::PrimitiveType
    }

    /// A placed card's width and height: a primitive type's its header alone, every other kind's
    /// a class's.
    pub(crate) fn size(self) -> (f64, f64) {
        if self == Classifier::PrimitiveType { archforge_uml::PRIMITIVE_TYPE_SIZE } else { archforge_uml::CLASS_SIZE }
    }

    /// The undo step that places one dragged from Models.
    pub(crate) fn place_step(self) -> String {
        match self {
            Classifier::Class => tr("Place class"),
            Classifier::Interface => tr("Place interface"),
            Classifier::Enumeration => tr("Place enumeration"),
            Classifier::DataType => tr("Place data type"),
            Classifier::PrimitiveType => tr("Place primitive type"),
            Classifier::AssociationClass => tr("Place association class"),
        }
    }
}

/// The tool's face: the icon of the kind it places.
pub(crate) fn tool_face(shell: &Shell, paint: &egui::Painter, b: Rect, ink: egui::Color32) {
    ux::icon_in(paint, b, shell.canvas.uml_classifier.tool_icon(), ink);
}

/// The menu of kinds above the classifier tool's chevron: the current kind marked; the arrow keys
/// move, Enter or a click picks and arms the tool, Escape or a press elsewhere closes it.
pub(crate) fn popover(shell: &mut Shell, ui: &egui::Ui, p: &Palette, r: Rect, anchor: Rect) {
    let paint = ui.painter().clone();
    let kinds = Classifier::ALL;
    let size = vec2(224.0, 44.0 + kinds.len() as f32 * 36.0 + 8.0);
    let x = (anchor.center().x - size.x / 2.0).clamp(r.left() + 8.0, (r.right() - size.x - 8.0).max(r.left() + 8.0));
    let y = (anchor.top() - 8.0 - size.y).max(r.top() + 8.0);
    let m = Rect::from_min_size(pos2(x, y), size);
    ux::shadow(&paint, m, 10.0, p.shadow);
    paint.rect_filled(m, 10.0, p.surface);
    paint.rect_stroke(m, 10.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
    ux::text(&paint, m.min + vec2(16.0, 14.0), &tr("Classifier").to_uppercase(), types::EYEBROW, p.text_secondary, Some(size.x - 32.0));
    let chosen = shell.canvas.uml_classifier;
    let current = kinds.iter().position(|k| *k == chosen).unwrap_or(0);
    // The keys: the arrows walk the rows from the current kind, Enter picks the row they are on.
    let (down, up, enter) = ui.input_mut(|i| {
        (
            i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown),
            i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp),
            i.consume_key(egui::Modifiers::NONE, egui::Key::Enter),
        )
    });
    let last = kinds.len().saturating_sub(1);
    let mut focus = shell.canvas.menu_focus.unwrap_or(current).min(last);
    if down {
        focus = (focus + 1).min(last);
    }
    if up {
        focus = focus.saturating_sub(1);
    }
    if down || up {
        shell.canvas.menu_focus = Some(focus);
    }
    let mut picked = enter.then(|| kinds.get(focus).copied()).flatten();
    for (i, kind) in kinds.iter().enumerate() {
        let row = Rect::from_min_size(m.min + vec2(8.0, 40.0 + i as f32 * 36.0), vec2(size.x - 16.0, 34.0));
        // Its name for assistive technology is the kind's.
        let resp = control(shell, ui, &format!("workspace.classifier.{}", kind.key()), "menu-item", row, true).accessible_name(kind.word());
        let on = *kind == chosen;
        let keyed = shell.canvas.menu_focus == Some(i);
        if on {
            paint.rect_filled(row, 6.0, p.accent_tint);
        } else if resp.hovered() || keyed {
            paint.rect_filled(row, 6.0, p.page);
        }
        if keyed && on {
            paint.rect_stroke(row, 6.0, Stroke::new(1.0, p.accent), StrokeKind::Inside);
        }
        let ink = if on { p.accent } else { p.text_secondary };
        ux::icon_mid(&paint, row.left_center() + vec2(18.0, 0.0), kind.icon(), ink);
        ux::text_mid(
            &paint,
            row.left() + 36.0,
            row.center().y,
            &kind.word(),
            types::UI,
            if on { p.accent } else { p.text },
            Some(row.width() - 64.0),
        );
        if on {
            ux::icon_mid(&paint, row.right_center() - vec2(16.0, 0.0), &icons::TICK, p.accent);
        }
        if resp.clicked() {
            picked = Some(*kind);
        }
    }
    if let Some(kind) = picked {
        shell.canvas.uml_classifier = kind;
        shell.canvas.tool = crate::canvas::Tool::Place(TOOL);
        shell.canvas.popover = None;
        shell.canvas.menu_focus = None;
        return;
    }
    let outside = ui.input(|i| i.pointer.any_pressed() && !i.pointer.interact_pos().is_some_and(|pos| m.contains(pos) || anchor.contains(pos)));
    if outside || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        shell.canvas.popover = None;
        shell.canvas.menu_focus = None;
    }
    // The press that closed the menu only closes it: the canvas takes nothing from it, so the
    // armed tool places nothing where the press lets go.
    if outside {
        shell.canvas.dismissing = true;
    }
}

/// Whether the popover of the chevron `key` is a menu the arrow keys and Enter walk.
pub(crate) fn takes_keys(key: &str) -> bool {
    key == KINDS
}

/// The classifier tool's chevron key.
pub(crate) const KINDS: &str = "classifier-kinds";

/// The classifier tool's automation key.
pub(crate) const TOOL: &str = "classifier";
