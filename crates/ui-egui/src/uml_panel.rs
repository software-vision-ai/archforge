//! A class in Properties: its name and whether it is abstract, then the
//! Attributes and Operations sections, each row read as the class's card shows it. `+` opens the
//! editor for a new member, the row's edit button or a double click opens the editor in its place,
//! as a table's columns do. The attribute editor takes the visibility, the name, the type (none, a
//! built-in type or one of the model's), the multiplicity as typed, and Static; the operation
//! editor the visibility, the name, the return type, Static, Abstract and its parameters, each a
//! name, a type and a multiplicity. Apply is one command (`uml.attribute.add`/`set`,
//! `uml.operation.add`/`set`); the arrows move the member up or down and the bin removes it. A
//! selected member shows its class's panel, its row marked.

use archforge_mof::{Obj, PropId, Store, TypeRef, Uuid, Value};
use archforge_uml::members::{BUILT_IN, TYPES, attributes, operations, parameters};
use egui::{Rect, Stroke, StrokeKind, pos2, vec2};
use serde_json::json;

use crate::Shell;
use crate::home::control;
use crate::i18n::{fmt, tr};
use crate::panels::{FIELD, Menu, PAD, ROW, Subject, check, field_inner, icon_button, menu, run, section, select_field, text_field};
use crate::uml_classifier::Classifier;
use crate::ux::{self, Palette, icons, types};
use crate::widgets;

/// The most of the model's own types the type menu lists, so it fits the window.
const MAX_TYPES: usize = 16;
/// The visibilities, in the metamodel's order, as the commands name them.
const VISIBILITIES: [&str; 4] = ["public", "private", "protected", "package"];
/// The longest multiplicity typed.
const MAX_MULTIPLICITY: usize = 24;

/// Which kind of member an editor edits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemberKind {
    Attribute,
    Operation,
}

/// A parameter as the operation editor holds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParamEdit {
    pub name: String,
    /// A type as the commands take it: empty, a built-in type's name or an element's id.
    pub ty: String,
    pub multiplicity: String,
}

/// An attribute or operation being edited: the member, or none for one about to be added to
/// `class`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberEdit {
    pub kind: MemberKind,
    pub target: Option<Uuid>,
    pub class: Uuid,
    pub visibility: usize,
    pub name: String,
    /// The attribute's type or the operation's return type, as the commands take it.
    pub ty: String,
    pub multiplicity: String,
    pub is_static: bool,
    pub is_abstract: bool,
    pub params: Vec<ParamEdit>,
    pub refused: Option<String>,
    /// True until the editor has been shown once: the name field takes the keyboard then.
    pub opened: bool,
}

fn one(store: &Store, o: Obj, prop: PropId) -> Option<Obj> {
    store.get(o, prop).ok().and_then(|v| v.as_ref()).filter(|x| store.is_alive(*x))
}

fn flag(store: &Store, o: Obj, prop: PropId) -> bool {
    store.get(o, prop).ok().and_then(|v| v.as_bool()).unwrap_or(false)
}

fn name(store: &Store, o: Obj) -> String {
    archforge_core::name_of(store, o)
}

/// Whether `t` is a built-in type, kept in the Types package.
fn built_in(store: &Store, t: Obj) -> bool {
    let in_types =
        store.container(t).ok().flatten().is_some_and(|(p, _)| store.class_of(p).ok() == Some(archforge_uml::PACKAGE) && name(store, p) == TYPES);
    in_types && store.class_of(t).ok() == Some(archforge_uml::PRIMITIVE_TYPE) && BUILT_IN.contains(&name(store, t).as_str())
}

/// The type of `o` as the commands take it: a built-in type by its name, any other by its id.
fn type_value(store: &Store, o: Obj) -> String {
    match one(store, o, archforge_uml::typed_element::TYPE) {
        Some(t) if built_in(store, t) => name(store, t),
        Some(t) => store.uuid(t).map(|u| u.to_string()).unwrap_or_default(),
        None => String::new(),
    }
}

/// A type as the editor shows it.
pub(crate) fn type_label(store: &Store, ty: &str) -> String {
    if ty.is_empty() {
        return tr("None");
    }
    match Uuid::parse(ty).and_then(|u| store.by_uuid(u)) {
        Some(t) => name(store, t),
        None => ty.to_string(),
    }
}

/// The model's own named types, by name, at most [`MAX_TYPES`], as the type menu offers them.
/// An association is a classifier but types nothing.
fn model_types(store: &Store) -> Vec<(String, String)> {
    let universe = store.universe();
    let mut out: Vec<(String, String)> = store
        .alive()
        .filter(|o| {
            store.class_of(*o).is_ok_and(|c| {
                universe.conforms(c, TypeRef::Class(archforge_uml::TYPE)) && !universe.conforms(c, TypeRef::Class(archforge_uml::ASSOCIATION))
            })
        })
        .filter(|t| !built_in(store, *t))
        .filter_map(|t| Some((store.uuid(t).ok()?.to_string(), name(store, t))))
        .filter(|(_, n)| !n.is_empty())
        .collect();
    out.sort_by_key(|a| a.1.to_lowercase());
    out.truncate(MAX_TYPES);
    out
}

/// Opens the editor of `kind` on `target`, or on a new member of `class`.
pub(crate) fn open(shell: &mut Shell, class: Uuid, kind: MemberKind, target: Option<Uuid>) {
    let edit = edit_of(shell.session.as_ref().map(|s| s.doc()), class, kind, target);
    shell.panels.member = Some(edit);
    shell.panels.menu = None;
}

/// The editor's values for `target` as the model holds them, or empty ones for a new member of
/// `class`: what the Properties editor and the canvas's inline row start from.
pub(crate) fn edit_of(store: Option<&Store>, class: Uuid, kind: MemberKind, target: Option<Uuid>) -> MemberEdit {
    let mut edit = MemberEdit {
        kind,
        target,
        class,
        visibility: 0,
        name: String::new(),
        ty: String::new(),
        multiplicity: String::new(),
        is_static: false,
        is_abstract: false,
        params: Vec::new(),
        refused: None,
        opened: true,
    };
    if let (Some(t), Some(store)) = (target, store)
        && let Some(o) = store.by_uuid(t)
    {
        edit.name = name(store, o);
        edit.visibility = match store.get(o, archforge_uml::named_element::VISIBILITY) {
            Ok(Value::Enum(i)) => (i as usize).min(VISIBILITIES.len() - 1),
            _ => 0,
        };
        edit.is_static = flag(store, o, archforge_uml::feature::IS_STATIC);
        match kind {
            MemberKind::Attribute => {
                edit.ty = type_value(store, o);
                edit.multiplicity = archforge_uml::figures::multiplicity(store, o);
            }
            MemberKind::Operation => {
                edit.is_abstract = flag(store, o, archforge_uml::behavioral_feature::IS_ABSTRACT);
                let (params, ret) = parameters(store, o);
                edit.params = params
                    .into_iter()
                    .map(|p| ParamEdit {
                        name: name(store, p),
                        ty: type_value(store, p),
                        multiplicity: archforge_uml::figures::multiplicity(store, p),
                    })
                    .collect();
                edit.ty = ret.map(|r| type_value(store, r)).unwrap_or_default();
            }
        }
    }
    edit
}

/// The class or interface a member belongs to, with the member's kind: an attribute that is no
/// association's end, or an operation.
pub(crate) fn member_of(store: &Store, o: Obj) -> Option<(Obj, MemberKind)> {
    let (c, _) = store.container(o).ok()??;
    if !archforge_uml::is_classifier(store.class_of(c).ok()?) {
        return None;
    }
    match store.class_of(o).ok()? {
        k if k == archforge_uml::PROPERTY && one(store, o, archforge_uml::property::ASSOCIATION).is_none() => Some((c, MemberKind::Attribute)),
        k if k == archforge_uml::OPERATION => Some((c, MemberKind::Operation)),
        _ => None,
    }
}

/// A class's or an interface's panel: its name and package, a class's Abstract, its attributes and
/// its operations.
pub(crate) fn class_body(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect, y: f32, subject: &Subject) {
    let (Some(class), Some(class_id)) = (subject.obj, subject.id) else { return };
    let kind = shell.session.as_ref().and_then(|s| Classifier::of(s.doc(), class)).unwrap_or_default();
    let width = r.width() - 2.0 * PAD;
    let half = ((width - 8.0) / 2.0).max(0.0);
    let (rename, key) = kind.rename_command();
    crate::panels::name_field(shell, ui, p, pos2(r.left() + PAD, y), half, subject, rename, key);
    // Its package: a pick moves it there.
    let (here, choices) = match shell.session.as_ref() {
        Some(s) => {
            let store = s.doc();
            let here = archforge_uml::commands::package_of(store, class).map(|p| package_path(store, p)).unwrap_or_default();
            let choices: Vec<(Uuid, String)> = archforge_uml::commands::all_packages(store)
                .into_iter()
                .filter_map(|p| Some((store.uuid(p).ok()?, package_path(store, p))))
                .collect();
            (here, choices)
        }
        None => (String::new(), Vec::new()),
    };
    let package_id = "workspace.properties.class.package";
    let field = select_field(shell, ui, p, pos2(r.left() + PAD + half + 8.0, y), half, package_id, &tr("Package"), &here, Menu::ClassPackage);
    let mut y = y + 17.0 + FIELD + 12.0;
    // An interface is abstract by what it is: it has no Abstract of its own.
    if kind == Classifier::Class {
        let was = shell.session.as_ref().is_some_and(|s| flag(s.doc(), class, archforge_uml::class::IS_ABSTRACT));
        let mut is_abstract = was;
        check(shell, ui, p, pos2(r.left() + PAD, y), "workspace.properties.class.abstract", &tr("Abstract"), &mut is_abstract);
        y += 16.0 + 16.0;
        if is_abstract != was {
            match run(shell, "uml.class.set", json!({ "class": class_id.to_string(), "abstract": is_abstract })) {
                Ok(_) => shell.panels.notice = None,
                Err(e) => shell.panels.notice = Some(e),
            }
        }
    }
    let y = members_section(shell, ui, p, r, y, class, class_id, MemberKind::Attribute);
    members_section(shell, ui, p, r, y + 4.0, class, class_id, MemberKind::Operation);
    // The package menu last, over the sections.
    if shell.panels.menu == Some(Menu::ClassPackage) {
        let items: Vec<String> = choices.iter().map(|(_, n)| n.clone()).collect();
        if let Some(i) = menu(shell, ui, p, field, package_id, &items)
            && let Some((to, _)) = choices.get(i)
        {
            let (cmd, key) = kind.move_command();
            match run(shell, cmd, json!({ key: class_id.to_string(), "package": to.to_string() })) {
                Ok(_) => shell.panels.notice = None,
                Err(e) => shell.panels.notice = Some(e),
            }
        }
    }
}

/// A package's panel: its name, and the package it sits in, a pick moving it there with what it
/// holds and its diagram (AR-102), the keyboard's way to move it. The root package stays, so it
/// shows its name alone.
pub(crate) fn package_body(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect, y: f32, subject: &Subject) {
    let (Some(package), Some(_)) = (subject.obj, subject.id) else { return };
    let width = r.width() - 2.0 * PAD;
    crate::panels::name_field(shell, ui, p, pos2(r.left() + PAD, y), width, subject, "uml.package.rename", "package");
    // Where it may go: every package but itself and those it holds, on a row of its own, so the
    // menu has room for their paths.
    let (here, choices) = match shell.session.as_ref() {
        Some(s) => {
            let store = s.doc();
            let here = archforge_uml::commands::package_of(store, package).map(|p| package_path(store, p)).unwrap_or_default();
            let choices: Vec<(Uuid, String)> = archforge_uml::commands::all_packages(store)
                .into_iter()
                .filter(|o| !archforge_uml::commands::is_within(store, *o, package))
                .filter_map(|o| Some((store.uuid(o).ok()?, package_path(store, o))))
                .collect();
            (here, choices)
        }
        None => (String::new(), Vec::new()),
    };
    let parent_id = "workspace.properties.package.parent";
    let field = select_field(
        shell,
        ui,
        p,
        pos2(r.left() + PAD, y + 17.0 + FIELD + 12.0),
        width,
        parent_id,
        &tr("Parent package"),
        &here,
        Menu::PackageParent,
    );
    if shell.panels.menu == Some(Menu::PackageParent) {
        let items: Vec<String> = choices.iter().map(|(_, n)| n.clone()).collect();
        if let Some(i) = menu(shell, ui, p, field, parent_id, &items)
            && let Some((to, _)) = choices.get(i)
        {
            // Shown on a diagram, it goes onto its new home's diagram too, as a canvas drop takes it.
            let to = shell.session.as_ref().and_then(|s| s.doc().by_uuid(*to));
            if let Some(to) = to {
                shell.panels.notice = crate::package_nav::move_package(shell, package, to).err();
            }
        }
    }
}

/// A package as the menus name it: its path from the top, `sales › orders`.
fn package_path(store: &Store, package: Obj) -> String {
    let mut names = Vec::new();
    let mut at = Some(package);
    while let Some(p) = at.filter(|_| names.len() < 32) {
        names.push(name(store, p));
        at = archforge_uml::commands::package_of(store, p);
    }
    names.reverse();
    names.join(" › ")
}

/// The Attributes or Operations section with its rows and the editor; gives the y under it.
#[allow(clippy::too_many_arguments, reason = "a section is drawn into the panel for its class and kind")]
fn members_section(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect, mut y: f32, class: Obj, class_id: Uuid, kind: MemberKind) -> f32 {
    let paint = ui.painter().clone();
    let width = (r.width() - 2.0 * PAD).max(0.0);
    let Some(store) = shell.session.as_ref().map(|s| s.doc()) else { return y };
    let rows: Vec<(Uuid, String, bool)> = match kind {
        MemberKind::Attribute => attributes(store, class)
            .into_iter()
            .filter_map(|a| Some((store.uuid(a).ok()?, archforge_uml::figures::attribute_text(store, a), false)))
            .collect(),
        MemberKind::Operation => operations(store, class)
            .into_iter()
            .filter_map(|o| {
                Some((
                    store.uuid(o).ok()?,
                    archforge_uml::figures::operation_text(store, o),
                    flag(store, o, archforge_uml::behavioral_feature::IS_ABSTRACT),
                ))
            })
            .collect(),
    };
    let (title, add, hint, row_id) = match kind {
        MemberKind::Attribute => (tr("Attributes · {n}"), "workspace.properties.attributes.add", "Add attribute", "workspace.properties.attribute"),
        MemberKind::Operation => (tr("Operations · {n}"), "workspace.properties.operations.add", "Add operation", "workspace.properties.operation"),
    };
    let title = fmt(&title, &[("n", &rows.len().to_string())]);
    let Some(next) = section(shell, ui, p, r, y, &title, Some((add, hint))) else {
        open(shell, class_id, kind, None);
        return y + 50.0;
    };
    y = next - 6.0;
    let editing = |shell: &Shell| shell.panels.member.as_ref().filter(|e| e.kind == kind && e.class == class_id).map(|e| e.target);
    let chosen = crate::panels::selected_id(shell);
    for (id, text, slanted) in &rows {
        if editing(shell) == Some(Some(*id)) {
            y = editor(shell, ui, p, r, y);
            continue;
        }
        let row = Rect::from_min_size(pos2(r.left() + PAD, y), vec2(width, ROW));
        let key = format!("{row_id}.{id}");
        let resp = control(shell, ui, &key, "row", row, true);
        if Some(*id) == chosen {
            paint.rect_filled(row, 6.0, p.accent_tint);
        } else if resp.contains_pointer() {
            paint.rect_filled(row, 6.0, p.page);
        }
        // The row as the card lists it, an abstract operation slanted.
        let at = pos2(row.left() + 10.0, row.top() + 9.0);
        let room = Some((width - 50.0).max(0.0));
        if *slanted {
            ux::text_slanted(&paint, at, text, types::UI, p.text, room);
        } else {
            ux::text(&paint, at, text, types::UI, p.text, room);
        }
        if crate::panels::edit_button(shell, ui, p, row, &key, resp.contains_pointer()) || resp.double_clicked() {
            crate::panels::select_element(shell, *id);
            open(shell, class_id, kind, Some(*id));
        } else if resp.clicked() {
            crate::panels::select_element(shell, *id);
        }
        y += ROW;
    }
    if editing(shell) == Some(None) {
        y = editor(shell, ui, p, r, y);
    }
    y + 10.0
}

/// What a type menu offers, each as it reads and as the commands take it: None, the built-in
/// types the model has no type of the same name for, then the model's own.
pub(crate) fn type_choices(store: &Store) -> Vec<(String, String)> {
    let own = model_types(store);
    let shadowed = |b: &str| own.iter().any(|(_, n)| n.eq_ignore_ascii_case(b));
    std::iter::once((tr("None"), String::new()))
        .chain(BUILT_IN.iter().filter(|b| !shadowed(b)).map(|b| (b.to_string(), b.to_string())))
        .chain(own.iter().map(|(id, n)| (n.clone(), id.clone())))
        .collect()
}

/// Every type named `name`, ignoring case, as the commands take it: a built-in type the model has
/// none of the same name for, and each of the model's own types, whichever package holds it.
pub(crate) fn types_named(store: &Store, name: &str) -> Vec<String> {
    let universe = store.universe();
    let own: Vec<String> = store
        .alive()
        .filter(|o| {
            store.class_of(*o).is_ok_and(|c| {
                universe.conforms(c, TypeRef::Class(archforge_uml::TYPE)) && !universe.conforms(c, TypeRef::Class(archforge_uml::ASSOCIATION))
            })
        })
        .filter(|t| !built_in(store, *t) && self::name(store, *t).eq_ignore_ascii_case(name))
        .filter_map(|t| store.uuid(t).ok().map(|u| u.to_string()))
        .collect();
    let builtin = BUILT_IN.iter().find(|b| b.eq_ignore_ascii_case(name)).filter(|_| own.is_empty()).map(|b| b.to_string());
    builtin.into_iter().chain(own).collect()
}

/// The menu of a type select, when it is open; sets `ty` to the type picked.
fn type_menu(shell: &mut Shell, ui: &egui::Ui, p: &Palette, field: Rect, id: &str, ty: &mut String, which: Menu) {
    if shell.panels.menu != Some(which) {
        return;
    }
    let choices = shell.session.as_ref().map(|s| type_choices(s.doc())).unwrap_or_default();
    let items: Vec<String> = choices.iter().map(|(label, _)| label.clone()).collect();
    if let Some(i) = menu(shell, ui, p, field, id, &items) {
        *ty = choices.get(i).map(|(_, v)| v.clone()).unwrap_or_default();
    }
}

/// A filled field without a label, as a parameter's row has, its label the hint while it is empty;
/// gives whether Enter was pressed.
#[allow(clippy::too_many_arguments, reason = "a field is drawn with its text and limit")]
fn bare_field(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, field: Rect, id: &str, label: &str, text: &mut String, limit: usize) -> bool {
    let paint = ui.painter().clone();
    paint.rect_filled(field, 6.0, p.field);
    widgets::record_visible(ui, &mut shell.registry, id, "field", field);
    let inner = field_inner(ui, field);
    let edit = egui::TextEdit::singleline(text)
        .id(widgets::egui_id(id))
        .frame(egui::Frame::NONE)
        .margin(egui::Margin::ZERO)
        .font(types::SMALL.font())
        .text_color(p.text)
        .char_limit(limit)
        .hint_text(egui::RichText::new(label).color(p.text_muted).font(types::SMALL.font()))
        .desired_width(inner.width());
    let out = edit.show(&mut ui.new_child(egui::UiBuilder::new().max_rect(inner)));
    let _ = (*out.response).clone().accessible_name(label);
    if out.response.has_focus() {
        paint.rect_stroke(field, 6.0, Stroke::new(1.0, p.focus), StrokeKind::Inside);
    }
    // A one-line field gives up the keyboard on Enter in the same frame.
    (out.response.has_focus() || out.response.lost_focus()) && ui.input(|i| i.key_pressed(egui::Key::Enter))
}

/// A filled select without a label, showing `value`; a click opens or closes `which`.
fn bare_select(shell: &mut Shell, ui: &egui::Ui, p: &Palette, field: Rect, id: &str, value: &str, which: Menu) {
    let paint = ui.painter();
    paint.rect_filled(field, 6.0, p.field);
    ux::text_mid(paint, field.left() + 8.0, field.center().y, value, types::SMALL, p.text, Some((field.width() - 30.0).max(0.0)));
    ux::icon_mid(paint, pos2(field.right() - 12.0, field.center().y), &icons::CHEVRON_DOWN, p.text_secondary);
    if control(shell, ui, id, "select", field, true).clicked() {
        shell.panels.menu = if shell.panels.menu == Some(which) { None } else { Some(which) };
    }
}

fn visibility_word(v: &str) -> String {
    match v {
        "private" => tr("Private"),
        "protected" => tr("Protected"),
        "package" => tr("Package"),
        _ => tr("Public"),
    }
}

/// The attribute or operation editor card; gives the y under it.
fn editor(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect, y: f32) -> f32 {
    let paint = ui.painter().clone();
    let width = (r.width() - 2.0 * PAD).max(0.0);
    let Some(mut edit) = shell.panels.member.clone() else { return y };
    // The card is drawn under the fields once their height is known.
    let background = paint.add(egui::Shape::Noop);
    let left = r.left() + PAD;
    let inner = (width - 24.0).max(0.0);
    let at = |x: f32, dy: f32| pos2(left + 12.0 + x, y + dy);
    let row = 17.0 + FIELD + 10.0;
    let take = std::mem::take(&mut edit.opened);
    let mut cy = 12.0;
    let mut enter = false;
    // Visibility narrow, the name wide.
    let narrow = 112.0_f32.min(inner / 2.0);
    let wide = (inner - narrow - 8.0).max(0.0);
    let words: Vec<String> = VISIBILITIES.iter().map(|v| visibility_word(v)).collect();
    let vis_id = "workspace.properties.member.visibility";
    let vis = select_field(shell, ui, p, at(0.0, cy), narrow, vis_id, &tr("Visibility"), &words[edit.visibility.min(3)], Menu::MemberVisibility);
    let name_id = "workspace.properties.member.name";
    enter |=
        text_field(shell, ui, p, at(narrow + 8.0, cy), wide, name_id, &tr("Name"), &mut edit.name, archforge_uml::commands::MAX_NAME, take, false);
    cy += row;
    // Type selects, whose menus open over what is under them, drawn last.
    let mut menus: Vec<(Rect, String, Menu)> = Vec::new();
    let type_id = "workspace.properties.member.type";
    match edit.kind {
        MemberKind::Attribute => {
            let half = ((inner - 8.0) / 2.0).max(0.0);
            let value = shell.session.as_ref().map(|s| type_label(s.doc(), &edit.ty)).unwrap_or_default();
            let field = select_field(shell, ui, p, at(0.0, cy), half, type_id, &tr("Type"), &value, Menu::MemberType);
            menus.push((field, type_id.into(), Menu::MemberType));
            let m_id = "workspace.properties.member.multiplicity";
            enter |=
                text_field(shell, ui, p, at(half + 8.0, cy), half, m_id, &tr("Multiplicity"), &mut edit.multiplicity, MAX_MULTIPLICITY, false, false);
            cy += row;
            check(shell, ui, p, at(0.0, cy), "workspace.properties.member.static", &tr("Static"), &mut edit.is_static);
            cy += 16.0 + 16.0;
        }
        MemberKind::Operation => {
            let value = shell.session.as_ref().map(|s| type_label(s.doc(), &edit.ty)).unwrap_or_default();
            let field = select_field(shell, ui, p, at(0.0, cy), inner, type_id, &tr("Return type"), &value, Menu::MemberType);
            menus.push((field, type_id.into(), Menu::MemberType));
            cy += row;
            let right = check(shell, ui, p, at(0.0, cy), "workspace.properties.member.static", &tr("Static"), &mut edit.is_static);
            check(shell, ui, p, pos2(right + 4.0, y + cy), "workspace.properties.member.abstract", &tr("Abstract"), &mut edit.is_abstract);
            cy += 16.0 + 16.0;
            // Parameters: a name, a type and a multiplicity on a row.
            ux::text(&paint, at(0.0, cy), &tr("Parameters"), types::CAPTION, p.text_secondary, Some(inner));
            cy += 20.0;
            // Each parameter on two lines: its name and ✕, then its type and its multiplicity.
            let narrow = 56.0_f32.min(inner / 4.0);
            let wide = (inner - FIELD - 8.0).max(0.0);
            let ty_width = (inner - narrow - 8.0).max(0.0);
            let mut gone = None;
            for i in 0..edit.params.len() {
                let id = format!("workspace.properties.member.parameter.{i}");
                let mut param = edit.params[i].clone();
                let n = Rect::from_min_size(at(0.0, cy), vec2(wide, FIELD));
                enter |= bare_field(shell, ui, p, n, &format!("{id}.name"), &tr("Name"), &mut param.name, archforge_uml::commands::MAX_NAME);
                if icon_button(
                    shell,
                    ui,
                    p,
                    at(inner - FIELD, cy),
                    &format!("{id}.remove"),
                    &icons::CLOSE,
                    p.text_secondary,
                    None,
                    "Remove parameter",
                ) {
                    gone = Some(i);
                }
                cy += FIELD + 6.0;
                let field = Rect::from_min_size(at(0.0, cy), vec2(ty_width, FIELD));
                let value = shell.session.as_ref().map(|s| type_label(s.doc(), &param.ty)).unwrap_or_default();
                bare_select(shell, ui, p, field, &format!("{id}.type"), &value, Menu::ParamType(i));
                menus.push((field, format!("{id}.type"), Menu::ParamType(i)));
                let m = Rect::from_min_size(at(ty_width + 8.0, cy), vec2(narrow, FIELD));
                enter |= bare_field(shell, ui, p, m, &format!("{id}.multiplicity"), "1", &mut param.multiplicity, MAX_MULTIPLICITY);
                edit.params[i] = param;
                cy += FIELD + 12.0;
            }
            if let Some(i) = gone {
                edit.params.remove(i);
                shell.panels.menu = None;
            }
            let add_id = "workspace.properties.member.parameter.add";
            if edit.params.len() < archforge_uml::members::MAX_PARAMETERS
                && crate::panels::ghost_button(shell, ui, p, at(-6.0, cy), add_id, &icons::PLUS, &tr("Add parameter"))
            {
                edit.params.push(ParamEdit { name: String::new(), ty: String::new(), multiplicity: String::new() });
            }
            cy += 24.0 + 12.0;
        }
    }
    // Remove and the moves on the left, Cancel and Apply on the right.
    let (mut remove, mut up, mut down) = (false, false, false);
    if edit.target.is_some() {
        remove = icon_button(shell, ui, p, at(0.0, cy), "workspace.properties.member.remove", &icons::TRASH, p.danger, None, "Remove");
        up = icon_button(shell, ui, p, at(FIELD + 8.0, cy), "workspace.properties.member.up", &icons::MOVE_UP, p.text_secondary, None, "Move up");
        down = icon_button(
            shell,
            ui,
            p,
            at(2.0 * (FIELD + 8.0), cy),
            "workspace.properties.member.down",
            &icons::MOVE_DOWN,
            p.text_secondary,
            None,
            "Move down",
        );
    }
    let cancel = icon_button(
        shell,
        ui,
        p,
        at(inner - 2.0 * FIELD - 8.0, cy),
        "workspace.properties.member.cancel",
        &icons::CLOSE,
        p.text_secondary,
        None,
        "Cancel",
    );
    let apply = icon_button(
        shell,
        ui,
        p,
        at(inner - FIELD, cy),
        "workspace.properties.member.apply",
        &icons::TICK,
        p.on_accent,
        Some(p.accent_fill),
        "Apply",
    );
    cy += FIELD + 12.0;
    if let Some(why) = &edit.refused {
        let g = ux::text(&paint, at(0.0, cy - 4.0), why, types::CAPTION, p.amber, Some(inner));
        cy += g.height() + 8.0;
    }
    let card = Rect::from_min_size(pos2(left, y), vec2(width, cy));
    paint.set(background, egui::Shape::rect_filled(card, 8.0, p.surface));
    paint.rect_stroke(card, 8.0, Stroke::new(1.0, p.accent), StrokeKind::Inside);
    crate::panels::reveal(shell, ui, card);
    // The menus last, over the card.
    if shell.panels.menu == Some(Menu::MemberVisibility)
        && let Some(i) = menu(shell, ui, p, vis, vis_id, &words)
    {
        edit.visibility = i;
    }
    for (field, id, which) in menus {
        let slot = match which {
            Menu::ParamType(i) => edit.params.get_mut(i).map(|p| &mut p.ty),
            _ => Some(&mut edit.ty),
        };
        if let Some(ty) = slot {
            type_menu(shell, ui, p, field, &id, ty, which);
        }
    }
    shell.panels.member = Some(edit.clone());
    if cancel || (shell.panels.menu.is_none() && ui.input(|i| i.key_pressed(egui::Key::Escape))) {
        shell.panels.member = None;
    } else if remove {
        let target = edit.target.map(|t| t.to_string()).unwrap_or_default();
        match run(shell, "element.remove", json!({ "ids": [target] })) {
            Ok(_) => {
                shell.panels.member = None;
                reselect(shell, edit.class);
            }
            Err(e) => refuse(shell, Some(e)),
        }
    } else if up || down {
        move_member(shell, &edit, up);
    } else if apply || enter {
        commit(shell, &edit);
    }
    card.bottom() + 8.0
}

/// Shows why the editor's command was refused, or clears it.
fn refuse(shell: &mut Shell, why: Option<String>) {
    if let Some(e) = shell.panels.member.as_mut() {
        e.refused = why;
    }
}

/// Selects the class: what the panel shows after its member went or was written.
fn reselect(shell: &mut Shell, class: Uuid) {
    if let Err(e) = run(shell, "view.select", json!({ "ids": [class.to_string()] })) {
        shell.panels.notice = Some(e);
    }
}

/// Moves the member above its neighbour before it, or below the one after it, among the class's
/// attributes or operations.
fn move_member(shell: &mut Shell, edit: &MemberEdit, up: bool) {
    let Some(target) = edit.target else { return };
    let at = shell.session.as_ref().and_then(|s| {
        let store = s.doc();
        let (c, o) = (store.by_uuid(edit.class)?, store.by_uuid(target)?);
        let (owned_attributes, owned_operations) = archforge_uml::members::member_lists(store, c)?;
        let (list, prop) = match edit.kind {
            MemberKind::Attribute => (attributes(store, c), owned_attributes),
            MemberKind::Operation => (operations(store, c), owned_operations),
        };
        let i = list.iter().position(|x| *x == o)?;
        let neighbour = if up { list.get(i.checked_sub(1)?) } else { list.get(i + 1) }?;
        // The neighbour's place in the class's whole list, which holds association ends too.
        store.list(c, prop).ok()?.iter().position(|v| v.as_ref() == Some(*neighbour))
    });
    let Some(at) = at else { return };
    let (cmd, key) = match edit.kind {
        MemberKind::Attribute => ("uml.attribute.move", "attribute"),
        MemberKind::Operation => ("uml.operation.move", "operation"),
    };
    match run(shell, cmd, json!({ key: target.to_string(), "at": at })) {
        Ok(_) => refuse(shell, None),
        Err(e) => refuse(shell, Some(e)),
    }
}

/// Writes the member through its command.
fn commit(shell: &mut Shell, edit: &MemberEdit) {
    let (cmd, params) = command_of(edit);
    match write_in_sight(shell, edit, cmd, params) {
        Ok(_) => {
            shell.panels.member = None;
            shell.panels.notice = None;
            reselect(shell, edit.class);
        }
        Err(e) => refuse(shell, Some(e)),
    }
}

/// The command that writes the member as edited, with its parameters.
pub(crate) fn command_of(edit: &MemberEdit) -> (&'static str, serde_json::Value) {
    let visibility = VISIBILITIES[edit.visibility.min(VISIBILITIES.len() - 1)];
    let (cmd, mut params) = match edit.kind {
        MemberKind::Attribute => (
            if edit.target.is_some() { "uml.attribute.set" } else { "uml.attribute.add" },
            json!({
                "name": edit.name.trim(), "visibility": visibility, "type": edit.ty,
                "multiplicity": edit.multiplicity.trim(), "static": edit.is_static,
            }),
        ),
        MemberKind::Operation => (
            if edit.target.is_some() { "uml.operation.set" } else { "uml.operation.add" },
            json!({
                "name": edit.name.trim(), "visibility": visibility, "returns": edit.ty, "static": edit.is_static, "abstract": edit.is_abstract,
                "parameters": edit.params.iter().map(|p| json!({ "name": p.name.trim(), "type": p.ty, "multiplicity": p.multiplicity.trim() })).collect::<Vec<_>>(),
            }),
        ),
    };
    match (edit.target, edit.kind) {
        (Some(t), MemberKind::Attribute) => params["attribute"] = json!(t.to_string()),
        (Some(t), MemberKind::Operation) => params["operation"] = json!(t.to_string()),
        (None, _) => params["class"] = json!(edit.class.to_string()),
    }
    (cmd, params)
}

/// Runs the member's command and, in the same undo step, grows every card of its class that would
/// then hide a member, on every diagram, keeping its width (owner decision: a card the user made
/// shorter stays so until a member of its class is added or edited).
pub(crate) fn write_in_sight(shell: &mut Shell, edit: &MemberEdit, cmd: &str, params: serde_json::Value) -> Result<serde_json::Value, String> {
    let Some(session) = shell.session.as_mut() else { return Err(tr("The model could not be opened")) };
    let store = session.doc();
    let more = match (edit.target, edit.kind) {
        (None, MemberKind::Attribute) => (1, 0),
        (None, MemberKind::Operation) => (0, 1),
        (Some(_), _) => (0, 0),
    };
    let short = store.by_uuid(edit.class).map(|c| crate::uml_ui::short_cards(store, c, more)).unwrap_or_default();
    if short.is_empty() {
        return session.execute(cmd, &params).map(|o| o.value).map_err(|e| e.to_string());
    }
    // The command's own label, as the step would carry without the growing.
    let label = match cmd {
        "uml.attribute.add" => "Add attribute",
        "uml.attribute.set" => "Set attribute",
        "uml.operation.add" => "Add operation",
        _ => "Set operation",
    };
    session
        .batch(label, |b| {
            b.execute(cmd, &params)?;
            for (node, width, height) in &short {
                b.execute("diagram.resize", &json!({ "node": node.to_string(), "width": width, "height": height }))?;
            }
            Ok(())
        })
        .map(|o| o.value.get(0).cloned().unwrap_or(serde_json::Value::Null))
        .map_err(|e| e.to_string())
}

/// Drops the editor whose class or member is gone.
pub(crate) fn retain_valid(edit: &mut Option<MemberEdit>, store: &Store) {
    if edit.as_ref().is_some_and(|e| store.by_uuid(e.class).is_none() || e.target.is_some_and(|t| store.by_uuid(t).is_none())) {
        *edit = None;
    }
}

/// The hover texts given by key and the visibilities' words, for the catalogue tests.
#[cfg(test)]
pub(crate) fn texts() -> impl Iterator<Item = &'static str> {
    ["Add attribute", "Add operation", "Remove", "Remove parameter", "Move up", "Move down", "Cancel", "Apply"].into_iter()
}
