//! The classifier tool of a class diagram: one tool in Class's place that places the kind its
//! chevron's menu chose (Class or Interface; only the kinds that work are listed), its face showing
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
}

impl Classifier {
    /// The kinds the menu lists, in its order.
    pub(crate) const ALL: [Classifier; 2] = [Classifier::Class, Classifier::Interface];

    /// Its name, in the menu, the tooltip and the Models' `+`.
    pub(crate) fn word(self) -> String {
        match self {
            Classifier::Class => tr("Class"),
            Classifier::Interface => tr("Interface"),
        }
    }

    /// Its automation key, and the stem of a fresh name (`Class 1`, `Interface 1`).
    pub(crate) fn key(self) -> &'static str {
        match self {
            Classifier::Class => "class",
            Classifier::Interface => "interface",
        }
    }

    pub(crate) fn stem(self) -> &'static str {
        match self {
            Classifier::Class => "Class",
            Classifier::Interface => "Interface",
        }
    }

    /// Its icon on the tool's face, and beside its name in the menu and in Models.
    pub(crate) fn tool_icon(self) -> &'static Icon {
        match self {
            Classifier::Class => &icons::TOOL_CLASS,
            Classifier::Interface => &icons::TOOL_INTERFACE,
        }
    }

    pub(crate) fn icon(self) -> &'static Icon {
        match self {
            Classifier::Class => &icons::CLASS,
            Classifier::Interface => &icons::INTERFACE,
        }
    }

    /// The metaclass it makes.
    pub(crate) fn metaclass(self) -> ClassId {
        match self {
            Classifier::Class => archforge_uml::CLASS,
            Classifier::Interface => archforge_uml::INTERFACE,
        }
    }

    /// The kind of `o`, when it is a classifier the tool places.
    pub(crate) fn of(store: &Store, o: Obj) -> Option<Classifier> {
        let c = store.class_of(o).ok()?;
        Classifier::ALL.into_iter().find(|k| k.metaclass() == c)
    }

    /// The commands that add, rename and move one, with the key that names it in the latter two.
    pub(crate) fn add_command(self) -> &'static str {
        match self {
            Classifier::Class => "uml.class.add",
            Classifier::Interface => "uml.interface.add",
        }
    }

    pub(crate) fn rename_command(self) -> (&'static str, &'static str) {
        match self {
            Classifier::Class => ("uml.class.rename", "class"),
            Classifier::Interface => ("uml.interface.rename", "interface"),
        }
    }

    pub(crate) fn move_command(self) -> (&'static str, &'static str) {
        match self {
            Classifier::Class => ("uml.class.move", "class"),
            Classifier::Interface => ("uml.interface.move", "interface"),
        }
    }

    /// The undo step that adds one, and the one that places one from Models.
    pub(crate) fn add_step(self) -> String {
        match self {
            Classifier::Class => tr("Add class"),
            Classifier::Interface => tr("Add interface"),
        }
    }

    pub(crate) fn place_step(self) -> String {
        match self {
            Classifier::Class => tr("Place class"),
            Classifier::Interface => tr("Place interface"),
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
        let resp = control(shell, ui, &format!("workspace.classifier.{}", kind.key()), "menu-item", row, true);
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
}

/// The classifier tool's automation key.
pub(crate) const TOOL: &str = "classifier";
