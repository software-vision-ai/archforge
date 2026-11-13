//! A classifier's template parameters in Properties (AR-130, design page 08, frames C1, C4 and
//! C5): the section `Template parameters · n` of a class's, an interface's or a data type's panel,
//! its `+` opening the row editor for a new parameter, a row per parameter reading `Key : DataType
//! = Integer` with its pencil (a double click opens it too), and the editor: Name and Kind side by
//! side, then Constrained to and Default, a line saying what the parameter asks of its arguments,
//! the bin and the moves at its foot on the left, Cancel and Apply on the right. Apply is one undo
//! step (`uml.template-parameter.add` or `.set`); the arrows move the parameter
//! (`uml.template-parameter.move`) and the bin removes it (`element.remove`).
//!
//! The Kind menu, headed PARAMETER KIND, offers Class, Interface, Data type, Enumeration and
//! Primitive type, then, after a line, Value and Operation, disabled and saying "not yet", never
//! taken for another kind (C4). Constrained to offers Any and the model's classifiers of the kind;
//! Default offers None and those of the kind that fit the constraint. A name another parameter
//! has, whatever the case, and a default that does not fit are refused in place, in a tinted box
//! under the fields, the editor kept open and nothing written. Removing a parameter a binding
//! substitutes is refused under the rows on the danger tint, naming the binding, with Show the
//! binding, which selects the binding itself, its line and its Properties (C5, AR-131).

use archforge_mof::{Obj, Store, TypeRef, Uuid};
use archforge_uml::templates::{self, Kind};
use egui::{Rect, Sense, Stroke, StrokeKind, Vec2, pos2, vec2};
use serde_json::json;

use crate::Shell;
use crate::home::control;
use crate::i18n::{fmt, tr, trn};
use crate::panels::{FIELD, Menu, PAD, ROW, icon_button, run, section, select_field, text_field};
use crate::ux::{self, Palette, icons, types};

/// The ids' stem: `workspace.properties.class.templateParameters.add`,
/// `workspace.properties.class.templateParameter.<k>` and its fields.
const ID: &str = "workspace.properties.class.templateParameter";

/// A template parameter being edited: the parameter, or none for one about to be added to
/// `classifier`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateEdit {
    pub classifier: Uuid,
    pub target: Option<Uuid>,
    pub name: String,
    pub kind: Kind,
    /// The constraint and the default as the commands take them: empty, a built-in type's name or
    /// a classifier's id.
    pub constraint: String,
    pub default: String,
    pub refused: Option<String>,
    /// The field the refusal sits under (`name`, `default`, `constraint`, `kind`), or none for the
    /// card's foot.
    pub refused_at: Option<&'static str>,
    /// True until the editor has been shown once: the name field takes the keyboard then.
    pub opened: bool,
}

/// A removal refused while a binding uses the parameter (C5): the parameter, the reason, and the
/// binding that stands in the way.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InUse {
    pub parameter: Uuid,
    pub why: String,
    pub binding: Option<Uuid>,
}

fn name_of(store: &Store, o: Obj) -> String {
    archforge_core::name_of(store, o)
}

/// A classifier as the commands take it for a constraint or a default: a built-in type by its name,
/// any other by its id.
pub(crate) fn value_of(store: &Store, c: Option<Obj>) -> String {
    match c {
        Some(t) if archforge_uml::members::is_built_in(store, t) => name_of(store, t),
        Some(t) => store.uuid(t).map(|u| u.to_string()).unwrap_or_default(),
        None => String::new(),
    }
}

/// The classifier a constraint's or a default's value names, a built-in type made yet or not.
pub(crate) fn classifier_named(store: &Store, value: &str) -> Option<Obj> {
    if value.is_empty() {
        return None;
    }
    match Uuid::parse(value) {
        Some(u) => store.by_uuid(u),
        None => archforge_uml::members::types_package(store)
            .and_then(|pkg| archforge_uml::commands::primitive_types(store, pkg).into_iter().find(|t| name_of(store, *t) == value)),
    }
}

/// A kind as the menu and the editor name it.
pub(crate) fn kind_word(kind: Kind) -> String {
    match kind {
        Kind::Class => tr("Class"),
        Kind::Interface => tr("Interface"),
        Kind::DataType => tr("Data type"),
        Kind::Enumeration => tr("Enumeration"),
        Kind::PrimitiveType => tr("Primitive type"),
        Kind::Value => tr("Value"),
        Kind::Operation => tr("Operation"),
    }
}

/// Opens the editor on parameter `target`, or on a new one of `classifier`.
pub(crate) fn open(shell: &mut Shell, classifier: Uuid, target: Option<Uuid>) {
    let mut edit = TemplateEdit {
        classifier,
        target,
        name: String::new(),
        kind: Kind::Class,
        constraint: String::new(),
        default: String::new(),
        refused: None,
        refused_at: None,
        opened: true,
    };
    if let Some(store) = shell.session.as_ref().map(|s| s.doc())
        && let Some(p) = target.and_then(|t| store.by_uuid(t))
    {
        edit.name = templates::parameter_name(store, p);
        edit.kind = templates::kind_of(store, p).unwrap_or(Kind::Class);
        edit.constraint = value_of(store, templates::constraint_of(store, p));
        edit.default = value_of(store, templates::default_of(store, p));
    }
    shell.panels.template = Some(edit);
    shell.panels.template_in_use = None;
    shell.panels.menu = None;
}

/// The ids of parameter row `k`'s controls (and its editor's fields).
fn row_key(k: usize) -> String {
    format!("{ID}.{k}")
}

/// The classifiers a constraint of a `kind` parameter may be, as a menu offers them: the model's
/// own whose metaclass is of the kind, and, for a data type or a primitive type, the built-in
/// types, each as it reads and as the commands take it, by name.
pub(crate) fn of_kind(store: &Store, kind: Kind) -> Vec<(String, String)> {
    let Some(meta) = kind.metaclass() else { return Vec::new() };
    let u = store.universe();
    let mut own: Vec<(String, String)> = store
        .alive()
        .filter(|o| store.class_of(*o).is_ok_and(|c| archforge_uml::is_classifier(c) && u.conforms(c, TypeRef::Class(meta))))
        .filter(|o| !archforge_uml::members::is_built_in(store, *o) && !templates::is_parameter_element(store, *o))
        .filter(|o| archforge_uml::constraints::in_model(store, *o))
        .filter_map(|o| Some((name_of(store, o), store.uuid(o).ok()?.to_string())))
        .filter(|(n, _)| !n.is_empty())
        .collect();
    own.sort_by_key(|(n, _)| n.to_lowercase());
    let built_in = u.conforms(archforge_uml::PRIMITIVE_TYPE, TypeRef::Class(meta));
    let mut out: Vec<(String, String)> =
        if built_in { archforge_uml::members::BUILT_IN.iter().map(|b| (b.to_string(), b.to_string())).collect() } else { Vec::new() };
    out.extend(own);
    out
}

/// What the Constrained to menu offers: Any, then the classifiers of the kind.
fn constraint_choices(store: &Store, kind: Kind) -> Vec<(String, String)> {
    std::iter::once((tr("Any"), String::new())).chain(of_kind(store, kind)).collect()
}

/// What the Default menu offers: None, then the classifiers of the kind that fit the constraint
/// (a built-in type not made yet fits only none).
fn default_choices(store: &Store, kind: Kind, constraint: &str) -> Vec<(String, String)> {
    let c = classifier_named(store, constraint);
    let fits = |value: &str| match (classifier_named(store, value), c) {
        (_, None) if constraint.is_empty() => true,
        (Some(d), c) => templates::fits(store, d, kind, c),
        (None, _) => false,
    };
    std::iter::once((tr("None"), String::new())).chain(of_kind(store, kind).into_iter().filter(|(_, v)| fits(v))).collect()
}

/// What parameter `name` of `kind` constrained to `constraint` asks of its arguments, in the user's
/// words: `Arguments for Key must be data types.`
pub(crate) fn must(name: &str, kind: Kind, constraint: Option<&str>) -> String {
    let line = match (kind, constraint) {
        (_, Some(c)) => return fmt(&tr("Arguments for {name} must be {constraint} or specialise it."), &[("name", name), ("constraint", c)]),
        (Kind::Class, None) => tr("Arguments for {name} must be classes."),
        (Kind::Interface, None) => tr("Arguments for {name} must be interfaces."),
        (Kind::DataType, None) => tr("Arguments for {name} must be data types."),
        (Kind::Enumeration, None) => tr("Arguments for {name} must be enumerations."),
        (Kind::PrimitiveType | Kind::Value | Kind::Operation, None) => tr("Arguments for {name} must be primitive types."),
    };
    fmt(&line, &[("name", name)])
}

/// What the parameter asks of its arguments, in the user's words: `Arguments for Key must be data
/// types. Bindings that leave it out use Integer. Renaming or reordering keeps every binding.`
fn help(store: &Store, edit: &TemplateEdit) -> String {
    let name = edit.name.trim();
    let name = if name.is_empty() { tr("this parameter") } else { name.to_string() };
    let must = must(&name, edit.kind, classifier_named(store, &edit.constraint).map(|c| name_of(store, c)).as_deref());
    let default = if edit.default.is_empty() {
        String::new()
    } else {
        let d = classifier_named(store, &edit.default).map_or_else(|| edit.default.clone(), |d| name_of(store, d));
        format!(" {}", fmt(&tr("Bindings that leave it out use {default}."), &[("default", &d)]))
    };
    format!("{must}{default} {}", tr("Renaming or reordering keeps every binding."))
}

/// The section `Template parameters · n` of classifier `c` (`c_id`) at `y`, with its rows, the
/// editor and a refused removal; gives the y under it.
pub(crate) fn section_of(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect, mut y: f32, c: Obj, c_id: Uuid) -> f32 {
    let paint = ui.painter().clone();
    let width = (r.width() - 2.0 * PAD).max(0.0);
    let Some(store) = shell.session.as_ref().map(|s| s.doc()) else { return y };
    let rows: Vec<(Uuid, String)> =
        templates::parameters(store, c).into_iter().filter_map(|q| Some((store.uuid(q).ok()?, templates::parameter_text(store, q)))).collect();
    let title = fmt(&tr("Template parameters · {n}"), &[("n", &rows.len().to_string())]);
    let Some(next) = section(shell, ui, p, r, y, &title, Some(("workspace.properties.class.templateParameters.add", "Add template parameter")))
    else {
        open(shell, c_id, None);
        return y + 50.0;
    };
    y = next - 6.0;
    let editing = |shell: &Shell| shell.panels.template.as_ref().filter(|e| e.classifier == c_id).map(|e| e.target);
    let in_use = shell.panels.template_in_use.clone().filter(|u| rows.iter().any(|(id, _)| *id == u.parameter));
    for (k, (id, text)) in rows.iter().enumerate() {
        if editing(shell) == Some(Some(*id)) {
            y = editor(shell, ui, p, r, y, k);
            continue;
        }
        let row = Rect::from_min_size(pos2(r.left() + PAD, y), vec2(width, ROW));
        let key = row_key(k);
        let resp = control(shell, ui, &key, "row", row, true).accessible_name(text.as_str()).on_hover_text(text.as_str());
        if in_use.as_ref().is_some_and(|u| u.parameter == *id) {
            paint.rect_filled(row, 6.0, p.accent_tint);
        } else if resp.contains_pointer() {
            paint.rect_filled(row, 6.0, p.page);
        }
        ux::text(&paint, pos2(row.left() + 10.0, row.top() + 9.0), text, types::UI, p.text, Some((width - 50.0).max(0.0)));
        if crate::panels::edit_button(shell, ui, p, row, &key, resp.contains_pointer()) || resp.double_clicked() {
            open(shell, c_id, Some(*id));
        }
        y += ROW;
    }
    if editing(shell) == Some(None) {
        y = editor(shell, ui, p, r, y, rows.len());
    }
    if let Some(u) = in_use {
        y = in_use_box(shell, ui, p, r, y + 4.0, &u);
    }
    y + 10.0
}

/// The refusal of a removal a binding stands in the way of (C5): the reason on the danger tint,
/// Show the binding under it, and what may still change; gives the y under it.
fn in_use_box(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect, mut y: f32, u: &InUse) -> f32 {
    let paint = ui.painter().clone();
    let width = (r.width() - 2.0 * PAD).max(0.0);
    let left = r.left() + PAD;
    let text = ux::galley_rows(&paint, &u.why, types::SMALL, p.text, Some((width - 42.0).max(0.0)), 8);
    let card = Rect::from_min_size(pos2(left, y), vec2(width, text.size().y + 20.0));
    paint.rect_filled(card, 6.0, p.danger_tint);
    ux::icon_in(&paint, Rect::from_min_size(pos2(left + 10.0, y + 11.5), vec2(14.0, 14.0)), &icons::WARNING, p.danger);
    paint.galley(pos2(left + 32.0, y + 10.0), text, p.text);
    crate::widgets::record_visible(ui, &mut shell.registry, &format!("{ID}.refusal"), "text", card);
    y = card.bottom() + 12.0;
    let label = tr("Show the binding");
    let g = ux::galley(&paint, &label, types::UI, p.text, None);
    let b = Rect::from_min_size(pos2(left, y), vec2((g.size().x + 24.0).min(width), FIELD));
    paint.rect_stroke(b, 6.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
    paint.galley(pos2(b.left() + 12.0, b.center().y - g.size().y / 2.0), g, p.text);
    if control(shell, ui, &format!("{ID}.showBinding"), "button", b, true).accessible_name(label).clicked()
        && let Some(binding) = u.binding
    {
        shell.panels.template_in_use = None;
        crate::panels::select_element(shell, binding);
    }
    y = b.bottom() + 12.0;
    let name = shell.session.as_ref().and_then(|s| s.doc().by_uuid(u.parameter).map(|o| templates::parameter_name(s.doc(), o))).unwrap_or_default();
    let renaming = fmt(&tr("Renaming {name} or changing its default is allowed: bindings follow the parameter, not its name."), &[("name", &name)]);
    let said = format!("{} {renaming}", tr("Show the binding selects it, on the canvas and in Properties."));
    let g = ux::galley_rows(&paint, &said, types::CAPTION, p.text_muted, Some(width), 5);
    let h = g.size().y;
    paint.galley(pos2(left, y), g, p.text_muted);
    y + h
}

/// The refusal box under the fields: the danger's tint, its icon and the reason; gives its height.
fn refusal_box(paint: &egui::Painter, p: &Palette, at: egui::Pos2, width: f32, why: &str) -> f32 {
    let g = ux::galley_rows(paint, why, types::SMALL, p.text, Some((width - 40.0).max(0.0)), 6);
    let r = Rect::from_min_size(at, vec2(width.max(0.0), g.size().y + 20.0));
    paint.rect_filled(r, 6.0, p.danger_tint);
    ux::icon_in(paint, Rect::from_min_size(r.min + vec2(10.0, 11.0), Vec2::splat(14.0)), &icons::ISSUE, p.danger);
    paint.galley(r.min + vec2(32.0, 10.0), g, p.text);
    r.height()
}

/// A classifier choice as its select shows it.
fn label_of(store: &Store, value: &str, empty: &str) -> String {
    if value.is_empty() {
        return empty.to_string();
    }
    classifier_named(store, value).map_or_else(|| value.to_string(), |c| name_of(store, c))
}

/// The parameter editor card for the parameter at place `k` (their count for a new one); gives the
/// y under it.
fn editor(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect, y: f32, k: usize) -> f32 {
    let paint = ui.painter().clone();
    let width = (r.width() - 2.0 * PAD).max(0.0);
    let Some(mut edit) = shell.panels.template.clone() else { return y };
    let background = paint.add(egui::Shape::Noop);
    let left = r.left() + PAD;
    let inner = (width - 24.0).max(0.0);
    let at = |x: f32, dy: f32| pos2(left + 12.0 + x, y + dy);
    let row = 17.0 + FIELD + 10.0;
    let take = std::mem::take(&mut edit.opened);
    let half = ((inner - 8.0) / 2.0).max(0.0);
    let key = row_key(k);
    let mut cy = 12.0;
    let mut enter = false;
    let typed = (edit.name.clone(), edit.kind, edit.constraint.clone(), edit.default.clone());
    let name_id = format!("{key}.name");
    enter |= text_field(shell, ui, p, at(0.0, cy), half, &name_id, &tr("Name"), &mut edit.name, archforge_uml::commands::MAX_NAME, take, false);
    let kind_id = format!("{key}.kind");
    let kind_field = select_field(shell, ui, p, at(half + 8.0, cy), half, &kind_id, &tr("Kind"), &kind_word(edit.kind), Menu::TemplateKind);
    let ring = |field: &str, x: f32, top: f32| {
        if edit.refused_at == Some(field) {
            paint.rect_stroke(Rect::from_min_size(at(x, top + 17.0), vec2(half, FIELD)), 6.0, Stroke::new(1.0, p.danger), StrokeKind::Inside);
        }
    };
    ring("name", 0.0, cy);
    ring("kind", half + 8.0, cy);
    cy += row;
    let store = shell.session.as_ref().map(|s| s.doc());
    let (c_label, d_label) =
        store.map(|st| (label_of(st, &edit.constraint, &tr("Any")), label_of(st, &edit.default, &tr("None")))).unwrap_or_default();
    let c_id = format!("{key}.constraint");
    let c_field = select_field(shell, ui, p, at(0.0, cy), half, &c_id, &tr("Constrained to"), &c_label, Menu::TemplateConstraint);
    let d_id = format!("{key}.default");
    let d_field = select_field(shell, ui, p, at(half + 8.0, cy), half, &d_id, &tr("Default"), &d_label, Menu::TemplateDefault);
    ring("constraint", 0.0, cy);
    ring("default", half + 8.0, cy);
    cy += row;
    if let (Some(why), Some(_)) = (edit.refused.clone(), edit.refused_at) {
        cy += refusal_box(&paint, p, at(0.0, cy - 4.0), inner, &why) + 8.0;
    } else if let Some(said) = shell.session.as_ref().map(|s| help(s.doc(), &edit)) {
        let g = ux::galley_rows(&paint, &said, types::CAPTION, p.text_secondary, Some(inner), 5);
        let h = g.size().y;
        paint.galley(at(0.0, cy - 4.0), g, p.text_secondary);
        cy += h + 8.0;
    }
    let (mut remove, mut up, mut down) = (false, false, false);
    if edit.target.is_some() {
        remove = icon_button(shell, ui, p, at(0.0, cy), &format!("{key}.remove"), &icons::TRASH, p.danger, None, "Remove");
        up = icon_button(shell, ui, p, at(FIELD + 8.0, cy), &format!("{key}.up"), &icons::MOVE_UP, p.text_secondary, None, "Move up");
        down = icon_button(shell, ui, p, at(2.0 * (FIELD + 8.0), cy), &format!("{key}.down"), &icons::MOVE_DOWN, p.text_secondary, None, "Move down");
    }
    let cancel =
        icon_button(shell, ui, p, at(inner - 2.0 * FIELD - 8.0, cy), &format!("{key}.cancel"), &icons::CLOSE, p.text_secondary, None, "Cancel");
    let apply = icon_button(shell, ui, p, at(inner - FIELD, cy), &format!("{key}.apply"), &icons::TICK, p.on_accent, Some(p.accent_fill), "Apply");
    cy += FIELD + 12.0;
    if let Some(why) = edit.refused.as_ref().filter(|_| edit.refused_at.is_none()) {
        let g = ux::galley_rows(&paint, why, types::CAPTION, p.amber, Some(inner), 4);
        let h = g.size().y;
        paint.galley(at(0.0, cy - 4.0), g, p.amber);
        cy += h + 8.0;
    }
    let card = Rect::from_min_size(pos2(left, y), vec2(width, cy));
    paint.set(background, egui::Shape::rect_filled(card, 8.0, p.surface));
    paint.rect_stroke(card, 8.0, Stroke::new(1.0, p.accent), StrokeKind::Inside);
    crate::panels::reveal(shell, ui, card);
    // The menus last, over the card; Escape with one open closes the menu alone.
    let menu_open = shell.panels.menu.is_some();
    if shell.panels.menu == Some(Menu::TemplateKind)
        && let Some(kind) = kind_menu(shell, ui, p, kind_field, &kind_id, edit.kind)
    {
        edit.kind = kind;
        // A constraint or a default of another kind no longer fits it.
        if let Some(st) = shell.session.as_ref().map(|s| s.doc()) {
            if !constraint_choices(st, kind).iter().any(|(_, v)| *v == edit.constraint) {
                edit.constraint.clear();
            }
            if !default_choices(st, kind, &edit.constraint).iter().any(|(_, v)| *v == edit.default) {
                edit.default.clear();
            }
        }
    }
    if shell.panels.menu == Some(Menu::TemplateConstraint)
        && let Some(choices) = shell.session.as_ref().map(|s| constraint_choices(s.doc(), edit.kind))
    {
        let items: Vec<String> = choices.iter().map(|(l, _)| l.clone()).collect();
        if let Some(v) = crate::panels::menu(shell, ui, p, c_field, &c_id, &items).and_then(|i| choices.get(i)) {
            edit.constraint = v.1.clone();
            if let Some(st) = shell.session.as_ref().map(|s| s.doc())
                && !default_choices(st, edit.kind, &edit.constraint).iter().any(|(_, d)| *d == edit.default)
            {
                edit.default.clear();
            }
        }
    }
    if shell.panels.menu == Some(Menu::TemplateDefault)
        && let Some(choices) = shell.session.as_ref().map(|s| default_choices(s.doc(), edit.kind, &edit.constraint))
    {
        let items: Vec<String> = choices.iter().map(|(l, _)| l.clone()).collect();
        if let Some(v) = crate::panels::menu(shell, ui, p, d_field, &d_id, &items).and_then(|i| choices.get(i)) {
            edit.default = v.1.clone();
        }
    }
    // Changing what was typed takes the reason away until the next Apply.
    if (edit.name.clone(), edit.kind, edit.constraint.clone(), edit.default.clone()) != typed {
        edit.refused = None;
        edit.refused_at = None;
    }
    shell.panels.template = Some(edit.clone());
    if cancel || (!menu_open && ui.input(|i| i.key_pressed(egui::Key::Escape))) {
        shell.panels.template = None;
    } else if remove {
        remove_parameter(shell, &edit);
    } else if up || down {
        move_parameter(shell, &edit, up);
    } else if apply || enter {
        commit(shell, &edit);
    }
    card.bottom() + 8.0
}

/// The Kind menu under `field` (C4): PARAMETER KIND, the supported kinds with the current one
/// ticked, then, after a line, Value and Operation, disabled and saying "not yet"; gives the kind
/// picked. The arrows walk the kinds that may be picked, Enter picks, Escape or a press elsewhere
/// closes it.
fn kind_menu(shell: &mut Shell, ui: &egui::Ui, p: &Palette, field: Rect, id: &str, current: Kind) -> Option<Kind> {
    let supported: Vec<Kind> = Kind::ALL.into_iter().filter(|k| k.supported()).collect();
    let heading = 28.0;
    let height = 8.0 + heading + Kind::ALL.len() as f32 * 30.0 + 9.0;
    // Inside the window: under the field, or over it when there is no room below.
    let window = ui.ctx().content_rect();
    let width = field.width().max(190.0).min((window.width() - 16.0).max(field.width()));
    let left = field.left().min(window.right() - 8.0 - width).max(window.left() + 8.0);
    let top = if field.bottom() + 4.0 + height > window.bottom() - 8.0 {
        (field.top() - 4.0 - height).max(window.top() + 8.0)
    } else {
        field.bottom() + 4.0
    };
    let m = Rect::from_min_size(pos2(left, top), vec2(width, height));
    let picked = ux::overlay(ui, crate::widgets::egui_id(&format!("{id}.layer")), m, |ui| {
        let paint = ui.painter().clone();
        ux::shadow(&paint, m, 8.0, p.shadow);
        paint.rect_filled(m, 8.0, p.surface);
        paint.rect_stroke(m, 8.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
        let eyebrow = crate::canvas::eyebrow_case(&tr("Parameter kind"));
        ux::text(&paint, pos2(m.left() + 14.0, m.top() + 12.0), &eyebrow, types::EYEBROW, p.text_muted, Some(m.width() - 28.0));
        let mut reached = shell.panels.menu_reached.as_ref().filter(|(of, _)| of == id).map(|(_, i)| *i);
        let (down, up, enter) = ui.input_mut(|i| {
            (
                i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown),
                i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp),
                reached.is_some() && i.consume_key(egui::Modifiers::NONE, egui::Key::Enter),
            )
        });
        if down || up {
            let last = supported.len().saturating_sub(1);
            reached = Some(match reached {
                None => supported.iter().position(|k| *k == current).unwrap_or(0),
                Some(i) if down => (i + 1).min(last),
                Some(i) => i.saturating_sub(1),
            });
        }
        shell.panels.menu_reached = reached.map(|i| (id.to_string(), i));
        let mut picked = None;
        let mut y = m.top() + 4.0 + heading;
        for (i, kind) in Kind::ALL.into_iter().enumerate() {
            if i == supported.len() {
                paint.rect_filled(Rect::from_min_size(pos2(m.left() + 12.0, y + 4.0), vec2(m.width() - 24.0, 1.0)), 0.0, p.border);
                y += 9.0;
            }
            let r = Rect::from_min_size(pos2(m.left() + 4.0, y), vec2(m.width() - 8.0, 30.0));
            y += 30.0;
            let item = format!("{id}.{}", kind.key());
            let word = kind_word(kind);
            let live = kind.supported();
            let resp = if live {
                control(shell, ui, &item, "menu-item", r, true).accessible_name(word.as_str())
            } else {
                // Named, disabled and saying why, never taken for another kind.
                let why = fmt(&tr("{kind} template parameters are not supported yet."), &[("kind", &word)]);
                crate::widgets::record_visible(ui, &mut shell.registry, &item, "menu-item", r);
                let resp = ui.interact(r, crate::widgets::egui_id(&item), Sense::hover());
                ui.ctx().accesskit_node_builder(crate::widgets::egui_id(&item), |node| {
                    node.set_role(egui::accesskit::Role::MenuItem);
                    node.set_disabled();
                    node.set_description(why.clone());
                });
                resp.on_hover_text(why).accessible_name(word.as_str())
            };
            let is_current = kind == current;
            if live && (resp.hovered() || reached == Some(i)) || is_current {
                paint.rect_filled(r, 5.0, if is_current { p.accent_tint } else { p.page });
            }
            let ink = if !live {
                p.text_muted
            } else if is_current {
                p.accent
            } else {
                p.text
            };
            ux::text_mid(&paint, r.left() + 10.0, r.center().y, &word, types::SMALL, ink, Some((r.width() - 80.0).max(0.0)));
            if is_current {
                ux::icon_mid(&paint, pos2(r.right() - 18.0, r.center().y), &icons::TICK, p.accent);
            }
            if !live {
                let g = ux::galley(&paint, &tr("not yet"), types::CAPTION, p.text_muted, None);
                paint.galley(pos2(r.right() - 10.0 - g.size().x, r.center().y - g.size().y / 2.0), g, p.text_muted);
            }
            if live && (resp.clicked() || (enter && reached == Some(i))) {
                picked = Some(kind);
            }
        }
        picked
    });
    let outside = ui.input(|i| i.pointer.any_pressed() && !i.pointer.interact_pos().is_some_and(|pos| m.contains(pos) || field.contains(pos)));
    if picked.is_some() || outside || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        shell.panels.menu = None;
        shell.panels.menu_reached = None;
    }
    picked
}

fn refuse(shell: &mut Shell, why: String, at: Option<&'static str>) {
    if let Some(e) = shell.panels.template.as_mut() {
        e.refused = Some(why);
        e.refused_at = at;
    }
}

/// Why parameter `p` may not be removed, in the user's words, with the binding in the way: a
/// binding substitutes it (C5), or it is its template's last and a binding names the template,
/// substitutions or none. The model keeps the same rule (`templates::removals_problem`).
pub(crate) fn in_use(store: &Store, p: Obj) -> Option<(String, Option<Uuid>)> {
    let users = templates::users(store, p);
    let Some(&(binding, bound, actual)) = users.first() else { return last_in_use(store, p) };
    let name = templates::parameter_name(store, p);
    let bound_name = bound.map(|b| name_of(store, b)).unwrap_or_default();
    let actual = actual.map(|a| name_of(store, a)).unwrap_or_default();
    let mut why = fmt(
        &tr("{bound} binds {name} -> {actual}. Change or remove that binding first, then remove {name}."),
        &[("bound", &bound_name), ("name", &name), ("actual", &actual)],
    );
    if users.len() > 1 {
        let more = u64::try_from(users.len() - 1).unwrap_or(u64::MAX);
        why.push(' ');
        why.push_str(&trn("{n} more binding uses it too.", "{n} more bindings use it too.", more));
    }
    Some((why, store.uuid(binding).ok()))
}

/// Why `p`, its template's last parameter, may not be removed while a binding names the template:
/// the template's signature would go with it.
fn last_in_use(store: &Store, p: Obj) -> Option<(String, Option<Uuid>)> {
    templates::last_parameter_problem(store, p)?;
    let t = templates::template_of(store, p)?;
    let binding = templates::bindings_to(store, t).first().copied();
    let bound = binding.and_then(|b| templates::bound_element(store, b));
    let name = templates::parameter_name(store, p);
    let why = fmt(
        &tr("{bound} binds {template}, and {name} is its last parameter. Remove that binding first, then remove {name}."),
        &[("bound", &bound.map(|b| name_of(store, b)).unwrap_or_default()), ("template", &name_of(store, t)), ("name", &name)],
    );
    Some((why, binding.and_then(|b| store.uuid(b).ok())))
}

/// The bin: removes the parameter, or, while a binding substitutes it, closes the editor and says
/// why under the rows, with the way to the binding (C5); nothing is written.
fn remove_parameter(shell: &mut Shell, edit: &TemplateEdit) {
    let Some(target) = edit.target else { return };
    let refused = shell.session.as_ref().and_then(|s| s.doc().by_uuid(target).and_then(|o| in_use(s.doc(), o)));
    if let Some((why, binding)) = refused {
        shell.panels.template = None;
        shell.panels.template_in_use = Some(InUse { parameter: target, why, binding });
        return;
    }
    match run(shell, "element.remove", json!({ "ids": [target.to_string()] })) {
        Ok(_) => shell.panels.template = None,
        Err(e) => refuse(shell, e, None),
    }
}

fn move_parameter(shell: &mut Shell, edit: &TemplateEdit, up: bool) {
    let Some(target) = edit.target else { return };
    let Some(store) = shell.session.as_ref().map(|s| s.doc()) else { return };
    let Some(q) = store.by_uuid(target) else { return };
    let Some(c) = templates::template_of(store, q) else { return };
    let all = templates::parameters(store, c);
    let Some(i) = all.iter().position(|x| *x == q) else { return };
    let to = if up { i.checked_sub(1) } else { Some(i + 1).filter(|j| *j < all.len()) };
    let Some(to) = to else { return };
    match run(shell, "uml.template-parameter.move", json!({ "parameter": target.to_string(), "at": to })) {
        Ok(_) => {
            if let Some(e) = shell.panels.template.as_mut() {
                e.refused = None;
                e.refused_at = None;
            }
        }
        Err(e) => refuse(shell, e, None),
    }
}

/// Why the parameter as edited cannot be written, in the user's words, with the field it sits
/// under: an empty name, a name another parameter of the template has whatever the case, a
/// default that does not fit, or a kind or constraint a binding's argument would no longer fit.
/// The commands keep the same rules.
pub(crate) fn refusal(store: &Store, edit: &TemplateEdit) -> Option<(String, &'static str)> {
    let name = edit.name.trim();
    if name.is_empty() {
        return Some((tr("Type a name: a template parameter without one stands for nothing."), "name"));
    }
    let template = store.by_uuid(edit.classifier)?;
    let target = edit.target.and_then(|t| store.by_uuid(t));
    if let Some(other) = templates::name_clash(store, template, name, target) {
        let said = fmt(
            &tr("{template} already has a template parameter named {name}. Pick another name."),
            &[("template", &name_of(store, template)), ("name", &templates::parameter_name(store, other))],
        );
        return Some((said, "name"));
    }
    let constraint = classifier_named(store, &edit.constraint);
    if let Some(d) = classifier_named(store, &edit.default)
        && !templates::fits(store, d, edit.kind, constraint)
    {
        let said =
            fmt(&tr("{default} does not fit {name}: pick a default its arguments could be."), &[("default", &name_of(store, d)), ("name", name)]);
        return Some((said, "default"));
    }
    if let Some(p) = target {
        let changed = templates::kind_of(store, p) != Some(edit.kind) || templates::constraint_of(store, p) != constraint;
        let bad = templates::users(store, p).into_iter().find(|(_, _, a)| a.is_some_and(|a| !templates::fits(store, a, edit.kind, constraint)));
        if changed && let Some((_, bound, actual)) = bad {
            let said = fmt(
                &tr("{bound} binds {name} -> {actual}, which would no longer fit. Change or remove that binding first."),
                &[
                    ("bound", &bound.map(|b| name_of(store, b)).unwrap_or_default()),
                    ("name", &templates::parameter_name(store, p)),
                    ("actual", &actual.map(|a| name_of(store, a)).unwrap_or_default()),
                ],
            );
            let at = if templates::kind_of(store, p) != Some(edit.kind) { "kind" } else { "constraint" };
            return Some((said, at));
        }
    }
    None
}

/// Writes the editor as one undo step, or says why not, leaving the model and its history as
/// they were.
fn commit(shell: &mut Shell, edit: &TemplateEdit) {
    if let Some((why, at)) = shell.session.as_ref().and_then(|s| refusal(s.doc(), edit)) {
        refuse(shell, why, Some(at));
        return;
    }
    let (cmd, params) = match edit.target {
        Some(t) => (
            "uml.template-parameter.set",
            json!({ "parameter": t.to_string(), "name": edit.name.trim(), "kind": edit.kind.key(), "constraint": edit.constraint, "default": edit.default }),
        ),
        None => (
            "uml.template-parameter.add",
            json!({
                "classifier": edit.classifier.to_string(),
                "name": edit.name.trim(),
                "kind": edit.kind.key(),
                "constraint": edit.constraint,
                "default": edit.default,
            }),
        ),
    };
    match run(shell, cmd, params) {
        Ok(_) => {
            shell.panels.template = None;
            shell.panels.notice = None;
        }
        Err(e) => refuse(shell, e, None),
    }
}

/// The open editor's bin pressed, as a sample to look at: the removal refused while a binding
/// uses the parameter.
#[cfg(any(test, feature = "snapshot"))]
pub(crate) fn remove_sample(shell: &mut Shell) {
    if let Some(edit) = shell.panels.template.clone() {
        remove_parameter(shell, &edit);
    }
}

/// Drops an editor, or a refused removal, whose classifier or parameter is gone.
pub(crate) fn retain_valid(edit: &mut Option<TemplateEdit>, in_use: &mut Option<InUse>, store: &Store) {
    if edit.as_ref().is_some_and(|e| store.by_uuid(e.classifier).is_none() || e.target.is_some_and(|t| store.by_uuid(t).is_none())) {
        *edit = None;
    }
    if in_use.as_ref().is_some_and(|u| store.by_uuid(u.parameter).is_none_or(|p| !templates::is_parameter(store, p))) {
        *in_use = None;
    }
}

/// The words of the panel's kind line for a template: `Class · template, 2 parameters · model, in
/// every diagram`.
pub(crate) fn kind_line(kind: &str, n: usize) -> String {
    let n = u64::try_from(n).unwrap_or(u64::MAX);
    let line = trn("{kind} · template, {n} parameter · model, in every diagram", "{kind} · template, {n} parameters · model, in every diagram", n);
    fmt(&line, &[("kind", kind)])
}

/// Every word this module says through `tr` at run time, for the catalogue.
#[cfg(test)]
pub(crate) fn texts() -> impl Iterator<Item = &'static str> {
    ["Add template parameter"].into_iter()
}
