//! A class in Properties: its name and whether it is abstract, then the
//! Attributes and Operations sections, each row read as the class's card shows it. `+` opens the
//! editor for a new member, the row's edit button or a double click opens the editor in its place,
//! as a table's columns do. The attribute editor takes the visibility, the name, the type (none, a
//! built-in type or one of the model's), the multiplicity as typed, Static, and the modifiers
//! (AR-126): the default value (a select of the literals for an enumeration's type), Derived, Read
//! only, Ordered and Unique, and Subsets and Redefines, which list only the properties the rules
//! allow, built once per state of the project; a refusal sits under its field; the operation
//! editor the visibility, the name, the return type, Static, Abstract and its parameters, each a
//! name, a type and a multiplicity. Apply is one command (`uml.attribute.add`/`set`,
//! `uml.operation.add`/`set`); the arrows move the member up or down and the bin removes it. A
//! selected member shows its class's panel, its row marked. An enumeration's panel lists its
//! literals under `Literals · n` in the same pattern (AR-122): `+`, the pencil or a double click
//! open the literal editor, which holds its Name (`uml.literal.add`/`rename`, `uml.literal.move`);
//! a name another literal of the enumeration has, whatever the case, is refused in the user's
//! words before anything is written.

use archforge_mof::{Obj, PropId, Store, TypeRef, Uuid, Value};
use archforge_uml::members::{BUILT_IN, attributes, operations, parameters};
use egui::{Rect, Stroke, StrokeKind, pos2, vec2};
use serde_json::json;

use crate::Shell;
use crate::home::control;
use crate::i18n::{fmt, tr, trn};
use crate::panels::{FIELD, Menu, PAD, ROW, Subject, check, field_inner, icon_button, menu, run, section, select_field, text_field};
use crate::uml_classifier::Classifier;
use crate::ux::{self, Palette, icons, types};
use crate::widgets;

/// The visibilities, in the metamodel's order, as the commands name them.
const VISIBILITIES: [&str; 4] = ["public", "private", "protected", "package"];
/// The longest multiplicity typed.
const MAX_MULTIPLICITY: usize = 24;

/// Which kind of member an editor edits: a class's, an interface's or a data type's attribute or operation, or an
/// enumeration's literal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemberKind {
    Attribute,
    Operation,
    Literal,
}

/// A parameter as the operation editor holds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParamEdit {
    pub name: String,
    /// A type as the commands take it: empty, a built-in type's name or an element's id.
    pub ty: String,
    pub multiplicity: String,
}

/// An attribute, an operation or a literal being edited: the member, or none for one about to be
/// added to `class` (an enumeration, for a literal).
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
    /// No redefinition (leaf), an attribute's or an operation's (AR-138).
    pub is_leaf: bool,
    pub params: Vec<ParamEdit>,
    /// An attribute's modifiers (AR-126): its default as typed, its flags, and the ids of the
    /// properties it subsets and redefines.
    pub default: String,
    pub derived: bool,
    pub read_only: bool,
    pub ordered: bool,
    pub unique: bool,
    pub subsets: Vec<Uuid>,
    pub redefines: Vec<Uuid>,
    pub refused: Option<String>,
    /// The field the refusal sits under (`type`, `multiplicity`, `default`, `subsets`,
    /// `redefines`, `leaf`), or none for the card's foot.
    pub refused_at: Option<&'static str>,
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
    archforge_uml::members::is_built_in(store, t)
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

/// The model's own named types, by name, as the type menu offers them: every one, so each can be
/// chosen; the menu scrolls and lays out only the rows in sight, and [`choices_for_menu`] builds
/// this once per change. An association is a classifier but types nothing.
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
        // A template parameter's classifier is offered only inside its template (AR-130).
        .filter(|t| !archforge_uml::templates::is_parameter_element(store, *t))
        // UML's own metaclasses and the stereotypes type nothing of the model (AR-134).
        .filter(|t| !archforge_uml::profiles::is_stereotype(store, *t) && !archforge_uml::profiles::in_library(store, *t))
        .filter_map(|t| Some((store.uuid(t).ok()?.to_string(), name(store, t))))
        .filter(|(_, n)| !n.is_empty())
        .collect();
    out.sort_by_key(|a| a.1.to_lowercase());
    out
}

/// The template parameters of `within`, when it is a template, as a type menu offers them: each by
/// its name and its classifier's id, in their order (AR-130).
fn parameter_types(store: &Store, within: Option<Obj>) -> Vec<(String, String)> {
    within
        .map(|c| archforge_uml::templates::parameters(store, c))
        .unwrap_or_default()
        .into_iter()
        .filter_map(|p| {
            let e = archforge_uml::templates::element_of(store, p)?;
            Some((name(store, e), store.uuid(e).ok()?.to_string()))
        })
        .collect()
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
        is_leaf: false,
        params: Vec::new(),
        default: String::new(),
        derived: false,
        read_only: false,
        ordered: false,
        unique: true,
        subsets: Vec::new(),
        redefines: Vec::new(),
        refused: None,
        refused_at: None,
        opened: true,
    };
    if let (Some(t), Some(store)) = (target, store)
        && let Some(o) = store.by_uuid(t)
    {
        edit.name = name(store, o);
        // A literal is its name alone.
        if kind == MemberKind::Literal {
            return edit;
        }
        edit.visibility = match store.get(o, archforge_uml::named_element::VISIBILITY) {
            Ok(Value::Enum(i)) => (i as usize).min(VISIBILITIES.len() - 1),
            _ => 0,
        };
        edit.is_static = flag(store, o, archforge_uml::feature::IS_STATIC);
        edit.is_leaf = archforge_uml::relations::is_leaf(store, o);
        match kind {
            MemberKind::Attribute => {
                edit.ty = type_value(store, o);
                edit.multiplicity = archforge_uml::figures::multiplicity(store, o);
                edit.default = archforge_uml::modifiers::default_text(store, o);
                (edit.derived, edit.read_only, edit.ordered, edit.unique) = archforge_uml::modifiers::flags(store, o);
                let ids = |ts: Vec<Obj>| ts.into_iter().filter_map(|t| store.uuid(t).ok()).collect();
                edit.subsets = ids(archforge_uml::modifiers::subsets(store, o));
                edit.redefines = ids(archforge_uml::modifiers::redefines(store, o));
            }
            MemberKind::Literal => {}
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

/// The classifier a member belongs to, with the member's kind: an attribute that is no
/// association's end, an operation, or an enumeration's literal.
pub(crate) fn member_of(store: &Store, o: Obj) -> Option<(Obj, MemberKind)> {
    let (c, _) = store.container(o).ok()??;
    // A stereotype's properties are members as a class's attributes are (AR-134); its base_
    // attributes, an extension's ends, are not, as an association's end is not.
    let owner = store.class_of(c).ok()?;
    if !archforge_uml::is_classifier(owner) && owner != archforge_uml::STEREOTYPE {
        return None;
    }
    match store.class_of(o).ok()? {
        k if k == archforge_uml::PROPERTY && one(store, o, archforge_uml::property::ASSOCIATION).is_none() => Some((c, MemberKind::Attribute)),
        k if k == archforge_uml::OPERATION => Some((c, MemberKind::Operation)),
        k if k == archforge_uml::ENUMERATION_LITERAL && store.class_of(c).ok() == Some(archforge_uml::ENUMERATION) => Some((c, MemberKind::Literal)),
        _ => None,
    }
}

/// A classifier's panel: its name and owner (AR-132), then a class's Abstract, No subclasses (final) and
/// No redefinition (leaf) for every kind but a built-in type (AR-137, AR-138), then a class's, an interface's or a data type's
/// attributes and operations, or an enumeration's literals, each with its editor; a primitive type
/// has none.
pub(crate) fn class_body(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect, y: f32, subject: &Subject) {
    let (Some(class), Some(class_id)) = (subject.obj, subject.id) else { return };
    let kind = shell.session.as_ref().and_then(|s| Classifier::of(s.doc(), class)).unwrap_or_default();
    // An association class's panel is its line's with its members under its ends.
    if kind == Classifier::AssociationClass {
        return crate::uml_relate::body(shell, ui, p, r, y, subject);
    }
    let width = r.width() - 2.0 * PAD;
    let half = ((width - 8.0) / 2.0).max(0.0);
    let (rename, key) = kind.rename_command();
    crate::panels::name_field(shell, ui, p, pos2(r.left() + PAD, y), half, subject, rename, key);
    // Its owner, a package or a classifier: a pick moves it there (AR-132, D2 and D3).
    let field = crate::uml_nesting::owner_field(shell, ui, p, pos2(r.left() + PAD + half + 8.0, y), half, class);
    let mut y = y + 17.0 + FIELD + 12.0;
    // A nested classifier's owner explained under it (D2).
    if let Some(help) = shell.session.as_ref().and_then(|s| crate::uml_nesting::owner_help(s.doc(), class)) {
        let paint = ui.painter().clone();
        let g = ux::galley_rows(&paint, &help, types::CAPTION, p.text_muted, Some(width.max(0.0)), 4);
        let tall = g.size().y;
        paint.galley(pos2(r.left() + PAD, y), g, p.text_muted);
        y += tall + 14.0;
    }
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
    y = final_check(shell, ui, p, r, y, class, class_id);
    y = leaf_check(shell, ui, p, r, y, class, class_id);
    // The classifiers it nests, a class's or an interface's (AR-132, D1).
    if shell.session.as_ref().is_some_and(|s| crate::uml_nesting::owns(s.doc(), class)) {
        y = crate::uml_nesting::nested_section(shell, ui, p, r, y, class, class_id);
    }
    // Its template parameters, a class's, an interface's or a data type's (AR-130, C1).
    if shell.session.as_ref().is_some_and(|s| s.doc().class_of(class).is_ok_and(archforge_uml::templates::can_be_template)) {
        y = crate::uml_templates::section_of(shell, ui, p, r, y, class, class_id);
    }
    if kind == Classifier::Enumeration {
        y = members_section(shell, ui, p, r, y, class, class_id, MemberKind::Literal);
    }
    if kind.takes_members() {
        let below = members_section(shell, ui, p, r, y, class, class_id, MemberKind::Attribute);
        y = members_section(shell, ui, p, r, below + 4.0, class, class_id, MemberKind::Operation);
    }
    // Its constraints, a built-in type's aside (AR-133).
    if shell.session.as_ref().is_some_and(|s| archforge_uml::constraints::is_context(s.doc(), class)) {
        crate::uml_constraints::section_of(shell, ui, p, r, y + 4.0, class, class_id);
    }
    // The owner menu last, over the sections.
    crate::uml_nesting::owner_menu(shell, ui, p, field, class, class_id);
}

/// The id of No subclasses (final), the design's own (AR-137).
pub(crate) const FINAL_ID: &str = "workspace.properties.classifier.isFinalSpecialization";

/// No subclasses (final) at `y` with its explanation under it (AR-137): ticked, nothing may
/// specialize the classifier and its card says `{final}` after its name (`uml.classifier.set`, one
/// undo step). Ticked while something specializes it, the box stays clear, nothing is written, and a
/// refusal under it names those classifiers, with a button that selects their generalizations so
/// they can be removed. Left out for a built-in type, which is never changed. Gives the y under it.
fn final_check(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect, mut y: f32, class: Obj, class_id: Uuid) -> f32 {
    let Some(store) = shell.session.as_ref().map(|s| s.doc()) else { return y };
    if archforge_uml::members::is_built_in(store, class) {
        return y;
    }
    let own = name(store, class);
    let was = archforge_uml::relations::is_final(store, class);
    let below: Vec<(Uuid, String)> = archforge_uml::relations::specializations(store, class)
        .into_iter()
        .filter_map(|(g, s)| Some((store.uuid(g).ok()?, name(store, s))))
        .collect();
    let width = (r.width() - 2.0 * PAD).max(0.0);
    let left = r.left() + PAD;
    let explained = fmt(&tr("Nothing may specialise {name}. Shown as {final} after its name."), &[("name", &own)]);
    let refused = (shell.panels.final_refused == Some(class_id) && !was && !below.is_empty()).then(|| final_refusal(&own, &below));
    let mut on = was;
    check(shell, ui, p, pos2(left, y), FINAL_ID, &tr("No subclasses (final)"), &mut on);
    // Its explanation describes it, and a refusal joins it; its state is said as a check box's.
    let described = match &refused {
        Some(why) => format!("{explained} {why}"),
        None => explained.clone(),
    };
    ui.ctx().accesskit_node_builder(crate::widgets::egui_id(FINAL_ID), |node| {
        node.set_role(egui::accesskit::Role::CheckBox);
        node.set_toggled(if was { egui::accesskit::Toggled::True } else { egui::accesskit::Toggled::False });
        node.set_description(described);
        if refused.is_some() {
            node.set_invalid(egui::accesskit::Invalid::True);
        }
    });
    let paint = ui.painter().clone();
    let said = ux::galley_rows(&paint, &explained, types::CAPTION, p.text_muted, Some((width - 24.0).max(0.0)), 3);
    let tall = said.size().y;
    paint.galley(pos2(left + 24.0, y + 22.0), said, p.text_muted);
    y += 22.0 + tall + 12.0;
    if on != was {
        if on && !below.is_empty() {
            // Refused before anything is written: the box stays clear and the history untouched.
            shell.panels.final_refused = Some(class_id);
            ui.ctx().request_repaint();
        } else {
            match run(shell, "uml.classifier.set", json!({ "classifier": class_id.to_string(), "final": on })) {
                Ok(_) => {
                    shell.panels.notice = None;
                    shell.panels.final_refused = None;
                }
                Err(e) => shell.panels.notice = Some(e),
            }
        }
    }
    if let Some(why) = refused {
        // The refusal on the danger tint, its warning mark at its top left.
        let text = ux::galley_rows(&paint, &why, types::SMALL, p.text, Some((width - 42.0).max(0.0)), 8);
        let card = Rect::from_min_size(pos2(left, y), vec2(width, text.size().y + 20.0));
        paint.rect_filled(card, 6.0, p.danger_tint);
        ux::icon_in(&paint, Rect::from_min_size(pos2(left + 10.0, y + 11.5), vec2(14.0, 14.0)), &icons::WARNING, p.danger);
        paint.galley(pos2(left + 32.0, y + 10.0), text, p.text);
        crate::widgets::record_visible(ui, &mut shell.registry, &format!("{FINAL_ID}.refusal"), "text", card);
        y = card.bottom() + 12.0;
        // A button that selects their generalizations, for Delete to remove them.
        let n = u64::try_from(below.len()).unwrap_or(u64::MAX);
        let label = trn("Select the generalization", "Select the {n} generalizations", n);
        let g = ux::galley(&paint, &label, types::UI, p.text, None);
        let b = Rect::from_min_size(pos2(left, y), vec2((g.size().x + 24.0).min(width), FIELD));
        paint.rect_stroke(b, 6.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
        paint.galley(pos2(b.left() + 12.0, b.center().y - g.size().y / 2.0), g, p.text);
        if control(shell, ui, &format!("{FINAL_ID}.select"), "button", b, true).accessible_name(label).clicked() {
            let ids: Vec<String> = below.iter().map(|(g, _)| g.to_string()).collect();
            shell.panels.final_refused = None;
            if let Err(e) = run(shell, "view.select", json!({ "ids": ids })) {
                shell.panels.notice = Some(e);
            }
        }
        y = b.bottom() + 12.0;
    }
    y
}

/// Why `name` may not be made final while the classifiers `below` specialize it, in the user's
/// words: each named, the generalizations to be removed first.
pub(crate) fn final_refusal(name: &str, below: &[(Uuid, String)]) -> String {
    let names: Vec<&str> = below.iter().map(|(_, n)| n.as_str()).collect();
    let listed = match names.split_last() {
        Some((last, rest)) if !rest.is_empty() => fmt(&tr("{names} and {last}"), &[("names", &rest.join(", ")), ("last", last)]),
        _ => names.join(", "),
    };
    let n = u64::try_from(below.len()).unwrap_or(u64::MAX);
    let has = fmt(
        &trn("{name} has {n} subclass: {subclasses}.", "{name} has {n} subclasses: {subclasses}.", n),
        &[("name", name), ("subclasses", &listed)],
    );
    let remove = trn("Remove that generalization before making it final.", "Remove those generalizations before making it final.", n);
    format!("{has} {remove}")
}

/// The id of a classifier's No redefinition (leaf), the design's own (AR-138).
pub(crate) const LEAF_ID: &str = "workspace.properties.element.isLeaf";
/// The id of a member editor's No redefinition (leaf).
pub(crate) const MEMBER_LEAF_ID: &str = "workspace.properties.member.isLeaf";

/// A check box at `at` labelled `label`, its explanation under it `width` wide, which also describes
/// it, and `refused`, when given, on the danger tint under that with its warning mark, joining its
/// description; its state is said as a check box's. Gives what a click made of `was` and the y under
/// what it drew.
#[allow(clippy::too_many_arguments, reason = "a check box is drawn with its explanation and refusal")]
pub(crate) fn explained_check(
    shell: &mut Shell,
    ui: &mut egui::Ui,
    p: &Palette,
    at: egui::Pos2,
    width: f32,
    id: &str,
    label: &str,
    was: bool,
    explained: &str,
    refused: Option<&str>,
) -> (bool, f32) {
    let mut on = was;
    check(shell, ui, p, at, id, label, &mut on);
    let described = match refused {
        Some(why) => format!("{explained} {why}"),
        None => explained.to_string(),
    };
    ui.ctx().accesskit_node_builder(crate::widgets::egui_id(id), |node| {
        node.set_role(egui::accesskit::Role::CheckBox);
        node.set_toggled(if was { egui::accesskit::Toggled::True } else { egui::accesskit::Toggled::False });
        node.set_description(described);
        if refused.is_some() {
            node.set_invalid(egui::accesskit::Invalid::True);
        }
    });
    let paint = ui.painter().clone();
    let said = ux::galley_rows(&paint, explained, types::CAPTION, p.text_muted, Some((width - 24.0).max(0.0)), 3);
    let tall = said.size().y;
    paint.galley(pos2(at.x + 24.0, at.y + 22.0), said, p.text_muted);
    let mut y = at.y + 22.0 + tall + 12.0;
    if let Some(why) = refused {
        let text = ux::galley_rows(&paint, why, types::SMALL, p.text, Some((width - 42.0).max(0.0)), 8);
        let card = Rect::from_min_size(pos2(at.x, y), vec2(width, text.size().y + 20.0));
        paint.rect_filled(card, 6.0, p.danger_tint);
        ux::icon_in(&paint, Rect::from_min_size(pos2(at.x + 10.0, y + 11.5), vec2(14.0, 14.0)), &icons::WARNING, p.danger);
        paint.galley(pos2(at.x + 32.0, y + 10.0), text, p.text);
        crate::widgets::record_visible(ui, &mut shell.registry, &format!("{id}.refusal"), "text", card);
        y = card.bottom() + 12.0;
    }
    (on, y)
}

/// Why an element may not be made leaf while `by` redefine it, in the user's words: each named
/// (`CardPayment::authorise()`), their redefinitions to be removed first (AR-138).
pub(crate) fn leaf_refusal(by: &[String]) -> String {
    let listed = match by.split_last() {
        Some((last, rest)) if !rest.is_empty() => fmt(&tr("{names} and {last}"), &[("names", &rest.join(", ")), ("last", last)]),
        _ => by.join(", "),
    };
    let n = u64::try_from(by.len()).unwrap_or(u64::MAX);
    fmt(
        &trn("{names} redefines it. Remove that redefinition first.", "{names} redefine it. Remove those redefinitions first.", n),
        &[("names", &listed)],
    )
}

/// What redefines `o`, by name, as [`leaf_refusal`] names them.
pub(crate) fn redefiner_names(store: &Store, o: Obj) -> Vec<String> {
    archforge_uml::relations::redefiners(store, o).into_iter().map(|r| archforge_uml::relations::redefiner_name(store, r)).collect()
}

/// No redefinition (leaf) at `y`, after No subclasses (final), with its explanation under it
/// (AR-138): ticked, nothing may redefine the classifier and its card says `{leaf}` after its name
/// (`uml.classifier.set`, one undo step). Ticked while something redefines it, the box stays clear,
/// nothing is written, and a refusal under it names what does. Left out for a built-in type, which
/// is never changed. Gives the y under it.
fn leaf_check(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect, y: f32, class: Obj, class_id: Uuid) -> f32 {
    let Some(store) = shell.session.as_ref().map(|s| s.doc()) else { return y };
    if archforge_uml::members::is_built_in(store, class) {
        return y;
    }
    let was = archforge_uml::relations::is_leaf(store, class);
    let by = redefiner_names(store, class);
    let explained = fmt(&tr("A subclass could not redefine {name} where it is inherited. Shown as {leaf}."), &[("name", &name(store, class))]);
    let refused = (shell.panels.leaf_refused == Some(class_id) && !was && !by.is_empty()).then(|| leaf_refusal(&by));
    let width = (r.width() - 2.0 * PAD).max(0.0);
    let (on, y) =
        explained_check(shell, ui, p, pos2(r.left() + PAD, y), width, LEAF_ID, &tr("No redefinition (leaf)"), was, &explained, refused.as_deref());
    if on != was {
        if on && !by.is_empty() {
            // Refused before anything is written: the box stays clear and the history untouched.
            shell.panels.leaf_refused = Some(class_id);
            ui.ctx().request_repaint();
        } else {
            match run(shell, "uml.classifier.set", json!({ "classifier": class_id.to_string(), "leaf": on })) {
                Ok(_) => {
                    shell.panels.notice = None;
                    shell.panels.leaf_refused = None;
                }
                Err(e) => shell.panels.notice = Some(e),
            }
        }
    }
    y
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
            let here = archforge_uml::commands::package_of(store, package).map(|p| package_parts(store, p)).unwrap_or_default();
            let choices: Vec<(Uuid, Vec<String>)> = archforge_uml::commands::all_packages(store)
                .into_iter()
                .filter(|o| !archforge_uml::commands::is_within(store, *o, package))
                .filter_map(|o| Some((store.uuid(o).ok()?, package_parts(store, o))))
                .collect();
            (here, choices)
        }
        None => (Vec::new(), Vec::new()),
    };
    let parent_id = "workspace.properties.package.parent";
    let field = crate::panels::path_select_field(
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
    // The profiles it applies (AR-134, F5), then its constraints (AR-133).
    if let Some(id) = subject.id {
        let below = crate::uml_profiles::applied_section(shell, ui, p, r, field.bottom() + 16.0, package, id);
        crate::uml_constraints::section_of(shell, ui, p, r, below, package, id);
    }
    if shell.panels.menu == Some(Menu::PackageParent) {
        let paths: Vec<Vec<String>> = choices.iter().map(|(_, n)| n.clone()).collect();
        if let Some(i) = crate::panels::menu_paths(shell, ui, p, field, parent_id, &paths)
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

/// A package's path below the root package by its names, `sales › orders`, or the root's name for
/// the root:
/// short enough for a note, and still telling packages apart, since every one sits under the root.
fn path_below_root(store: &Store, package: Obj) -> Vec<String> {
    let root = archforge_uml::commands::root_package(store);
    let parent = |p: Obj| archforge_uml::commands::package_of(store, p).filter(|up| Some(*up) != root);
    path_parts(package, parent, |p| name(store, p))
}

/// The names on a package's path from the top, its own last: each name one part, whatever it
/// holds, every ancestor up to the root however deep it sits.
pub(crate) fn package_parts(store: &Store, package: Obj) -> Vec<String> {
    path_parts(package, |p| archforge_uml::commands::package_of(store, p), |p| name(store, p))
}

/// The names of `start` and of every ancestor `parent` leads to, from the top down. A walk that
/// comes back to where it has been (a cycle a damaged file could hold) stops there, so it ends
/// after at most one step per element the model holds, however deep the path is.
pub(crate) fn path_parts<T: Copy + Eq + std::hash::Hash>(start: T, parent: impl Fn(T) -> Option<T>, name: impl Fn(T) -> String) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut names = Vec::new();
    let mut at = Some(start);
    while let Some(p) = at.filter(|p| seen.insert(*p)) {
        names.push(name(p));
        at = parent(p);
    }
    names.reverse();
    names
}

/// The Attributes, Operations or Literals section with its rows and the editor; gives the y under
/// it. An enumeration without literals says so under its title.
#[allow(clippy::too_many_arguments, reason = "a section is drawn into the panel for its class and kind")]
pub(crate) fn members_section(
    shell: &mut Shell,
    ui: &mut egui::Ui,
    p: &Palette,
    r: Rect,
    mut y: f32,
    class: Obj,
    class_id: Uuid,
    kind: MemberKind,
) -> f32 {
    let paint = ui.painter().clone();
    let width = (r.width() - 2.0 * PAD).max(0.0);
    let Some(store) = shell.session.as_ref().map(|s| s.doc()) else { return y };
    let plain = ux::Marks::default();
    let rows: Vec<(Uuid, String, ux::Marks)> = match kind {
        MemberKind::Attribute => attributes(store, class)
            .into_iter()
            .filter_map(|a| {
                let marks = ux::Marks { slanted: false, underlined: archforge_uml::figures::is_static(store, a) };
                Some((store.uuid(a).ok()?, archforge_uml::figures::attribute_text(store, a), marks))
            })
            .collect(),
        MemberKind::Operation => operations(store, class)
            .into_iter()
            .filter_map(|o| {
                let marks = ux::Marks {
                    slanted: flag(store, o, archforge_uml::behavioral_feature::IS_ABSTRACT),
                    underlined: archforge_uml::figures::is_static(store, o),
                };
                Some((store.uuid(o).ok()?, archforge_uml::figures::operation_text(store, o), marks))
            })
            .collect(),
        MemberKind::Literal => {
            archforge_uml::commands::literals(store, class).into_iter().filter_map(|l| Some((store.uuid(l).ok()?, name(store, l), plain))).collect()
        }
    };
    let (title, add, hint, row_id) = match kind {
        MemberKind::Attribute => (tr("Attributes · {n}"), "workspace.properties.attributes.add", "Add attribute", "workspace.properties.attribute"),
        MemberKind::Operation => (tr("Operations · {n}"), "workspace.properties.operations.add", "Add operation", "workspace.properties.operation"),
        MemberKind::Literal => (tr("Literals · {n}"), "workspace.properties.literals.add", "Add literal", "workspace.properties.literal"),
    };
    // A stereotype's attributes are its properties (AR-134, F1).
    let stereotype = archforge_uml::profiles::is_stereotype(store, class);
    let (title, hint) = if stereotype && kind == MemberKind::Attribute { (tr("Properties · {n}"), "Add property") } else { (title, hint) };
    let title = fmt(&title, &[("n", &rows.len().to_string())]);
    let Some(next) = section(shell, ui, p, r, y, &title, Some((add, hint))) else {
        open(shell, class_id, kind, None);
        return y + 50.0;
    };
    y = next - 6.0;
    let editing = |shell: &Shell| shell.panels.member.as_ref().filter(|e| e.kind == kind && e.class == class_id).map(|e| e.target);
    let chosen = crate::panels::selected_id(shell);
    if rows.is_empty() && kind == MemberKind::Literal && editing(shell) != Some(None) {
        ux::text(&paint, pos2(r.left() + PAD + 10.0, y + 9.0), &tr("No literals yet"), types::UI, p.text_muted, Some(width));
        y += ROW;
    }
    for (id, text, marks) in &rows {
        if editing(shell) == Some(Some(*id)) {
            y = editor(shell, ui, p, r, y);
            continue;
        }
        let row = Rect::from_min_size(pos2(r.left() + PAD, y), vec2(width, ROW));
        let key = format!("{row_id}.{id}");
        // Its name for assistive technology, and a literal's on hover too: the row's text, whole.
        let resp = control(shell, ui, &key, "row", row, true).accessible_name(text.as_str());
        let resp = if kind == MemberKind::Literal { resp.on_hover_text(text.as_str()) } else { resp };
        if Some(*id) == chosen {
            paint.rect_filled(row, 6.0, p.accent_tint);
        } else if resp.contains_pointer() {
            paint.rect_filled(row, 6.0, p.page);
        }
        // The row as the card lists it, an abstract operation slanted, a static member underlined.
        let at = pos2(row.left() + 10.0, row.top() + 9.0);
        let room = Some((width - 50.0).max(0.0));
        ux::text_marked(&paint, at, text, types::UI, p.text, room, *marks);
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

/// What a type menu offers, each as it reads and as the commands take it: None, the template
/// parameters of `within` (the classifier whose member is typed) when it is a template, nearest
/// first (AR-130), every built-in type by its name, which the commands always resolve to the one
/// in Types, then the model's own by name. A type of the model's own named like a built-in one
/// never hides it: both are offered, told apart by [`type_notes`].
pub(crate) fn type_choices(store: &Store, within: Option<Obj>) -> Vec<(String, String)> {
    std::iter::once((tr("None"), String::new()))
        .chain(parameter_types(store, within))
        .chain(BUILT_IN.iter().map(|b| (b.to_string(), b.to_string())))
        .chain(model_types(store).into_iter().map(|(id, n)| (n, id)))
        .collect()
}

/// The muted note after each of `choices` (from [`type_choices`]) in a type menu, empty for most:
/// a template parameter always says so (AR-130); where several choices share a name, whatever the
/// case, each says where it is from, "built-in"
/// for a built-in type and its package's path below the root package (every package's top) for
/// one of the model's own, so the entries are told apart. Each is given by its parts, the path's
/// names, so a long one folds from its front.
pub(crate) fn type_notes(store: &Store, choices: &[(String, String)]) -> Vec<Vec<String>> {
    let mut counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for (label, value) in choices {
        if !value.is_empty() {
            *counts.entry(label.to_lowercase()).or_default() += 1;
        }
    }
    choices
        .iter()
        .map(|(label, value)| {
            if Uuid::parse(value).and_then(|u| store.by_uuid(u)).is_some_and(|t| archforge_uml::templates::is_parameter_element(store, t)) {
                return vec![tr("template parameter")];
            }
            if value.is_empty() || counts.get(&label.to_lowercase()).copied().unwrap_or(0) < 2 {
                return Vec::new();
            }
            match Uuid::parse(value).and_then(|u| store.by_uuid(u)) {
                // A nested type is told apart by its owners too (AR-132): `sales › Order`.
                Some(t) => {
                    let mut parts = archforge_uml::nesting::home_package(store, t).map(|p| path_below_root(store, p)).unwrap_or_default();
                    parts.extend(archforge_uml::nesting::owner_chain(store, t).into_iter().map(|o| name(store, o)));
                    parts
                }
                None => vec![tr("built-in")],
            }
        })
        .collect()
}

/// Every type named `name`, ignoring case, as the commands take it: the built-in type of that
/// name, and each of the model's own types, whichever package holds it. Typed where several share
/// the name, it is refused and the menu, which tells them apart, picks one: a built-in type is
/// never taken for one of the model's own nor one of those for it. Inside a template, `within`, its
/// parameters are types too (AR-130).
pub(crate) fn types_named(store: &Store, name: &str, within: Option<Obj>) -> Vec<String> {
    let universe = store.universe();
    let own = store
        .alive()
        .filter(|o| {
            store.class_of(*o).is_ok_and(|c| {
                universe.conforms(c, TypeRef::Class(archforge_uml::TYPE)) && !universe.conforms(c, TypeRef::Class(archforge_uml::ASSOCIATION))
            })
        })
        .filter(|t| !built_in(store, *t) && !archforge_uml::templates::is_parameter_element(store, *t))
        .filter(|t| self::name(store, *t).eq_ignore_ascii_case(name))
        .filter(|t| !archforge_uml::profiles::is_stereotype(store, *t) && !archforge_uml::profiles::in_library(store, *t))
        .filter_map(|t| store.uuid(t).ok().map(|u| u.to_string()));
    let builtin = BUILT_IN.iter().find(|b| b.eq_ignore_ascii_case(name)).map(|b| b.to_string());
    let params = parameter_types(store, within).into_iter().filter(|(n, _)| n.eq_ignore_ascii_case(name)).map(|(_, id)| id);
    params.chain(builtin).chain(own).collect()
}

/// The open type menu's choices for a member of `within`, built by [`type_choices`] only when the
/// project, its state, the classifier or the interface language changed since they were last built;
/// taken out of the shell for the frame, to be handed back with [`keep_choices`].
pub(crate) fn choices_for_menu(shell: &mut Shell, within: Option<Uuid>) -> crate::panels::TypeChoices {
    let Some(session) = shell.session.as_ref() else { return crate::panels::TypeChoices::default() };
    let store = session.doc();
    let project = archforge_uml::commands::model(store).ok().and_then(|m| store.uuid(m).ok());
    let revision = session.revision();
    let language = crate::i18n::language();
    match shell.panels.type_choices.take() {
        Some(kept) if kept.project == project && kept.revision == revision && kept.language == language && kept.within == within => kept,
        _ => {
            let choices = type_choices(store, within.and_then(|w| store.by_uuid(w)));
            let note_parts = type_notes(store, &choices);
            let notes = note_parts.iter().map(|parts| parts.join(crate::panels::PATH_SEP)).collect();
            let (labels, values) = choices.into_iter().unzip();
            crate::panels::TypeChoices { project, revision, language, within, labels, values, notes, note_parts }
        }
    }
}

/// Hands the type menu's choices back to the shell for the next frame.
pub(crate) fn keep_choices(shell: &mut Shell, choices: crate::panels::TypeChoices) {
    shell.panels.type_choices = Some(choices);
}

/// The menu of a type select for a member of `within`, when it is open; sets `ty` to the type
/// picked.
#[allow(clippy::too_many_arguments, reason = "a menu is drawn for its field, id, value and the classifier typed in")]
pub(crate) fn type_menu(shell: &mut Shell, ui: &egui::Ui, p: &Palette, field: Rect, id: &str, ty: &mut String, which: Menu, within: Option<Uuid>) {
    if shell.panels.menu != Some(which) {
        return;
    }
    let choices = choices_for_menu(shell, within);
    // A stereotype's property is typed by a primitive type, an enumeration or a data type alone
    // (AR-134): the menu offers those.
    let stereotype = shell
        .panels
        .member
        .as_ref()
        .and_then(|e| shell.session.as_ref().and_then(|s| s.doc().by_uuid(e.class).map(|c| archforge_uml::profiles::is_stereotype(s.doc(), c))))
        .unwrap_or(false);
    let offered: Vec<usize> = (0..choices.values.len())
        .filter(|i| {
            !stereotype
                || choices.values.get(*i).is_none_or(|v| {
                    shell.session.as_ref().is_none_or(|s| match Uuid::parse(v).and_then(|u| s.doc().by_uuid(u)) {
                        Some(t) => s
                            .doc()
                            .class_of(t)
                            .is_ok_and(|c| c == archforge_uml::PRIMITIVE_TYPE || c == archforge_uml::ENUMERATION || c == archforge_uml::DATA_TYPE),
                        None => true,
                    })
                })
        })
        .collect();
    let pick = |v: &[String]| offered.iter().filter_map(|i| v.get(*i).cloned()).collect::<Vec<String>>();
    let (labels, notes) = (pick(&choices.labels), pick(&choices.notes));
    let note_parts: Vec<Vec<String>> = offered.iter().filter_map(|i| choices.note_parts.get(*i).cloned()).collect();
    if let Some(i) = crate::panels::menu_noted(shell, ui, p, field, id, &labels, &notes, &note_parts) {
        *ty = offered.get(i).and_then(|i| choices.values.get(*i)).cloned().unwrap_or_default();
    }
    keep_choices(shell, choices);
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
    // A literal's editor holds its name alone, as wide as the card.
    let literal = edit.kind == MemberKind::Literal;
    // Visibility narrow, the name wide.
    let narrow = 112.0_f32.min(inner / 2.0);
    let wide = (inner - narrow - 8.0).max(0.0);
    // An interface's members are public: its menu offers Public alone.
    let public_only = shell
        .session
        .as_ref()
        .and_then(|s| s.doc().by_uuid(edit.class).and_then(|c| Classifier::of(s.doc(), c)))
        .is_some_and(|k| k == Classifier::Interface);
    let offered = if public_only { &VISIBILITIES[..1] } else { &VISIBILITIES[..] };
    let words: Vec<String> = offered.iter().map(|v| visibility_word(v)).collect();
    edit.visibility = edit.visibility.min(words.len().saturating_sub(1));
    let vis_id = "workspace.properties.member.visibility";
    let shown = words.get(edit.visibility).cloned().unwrap_or_default();
    let vis = if literal {
        Rect::NOTHING
    } else {
        select_field(shell, ui, p, at(0.0, cy), narrow, vis_id, &tr("Visibility"), &shown, Menu::MemberVisibility)
    };
    let name_id = "workspace.properties.member.name";
    let (name_x, name_w) = if literal { (0.0, inner) } else { (narrow + 8.0, wide) };
    enter |= text_field(shell, ui, p, at(name_x, cy), name_w, name_id, &tr("Name"), &mut edit.name, archforge_uml::commands::MAX_NAME, take, false);
    cy += row;
    // Type selects, whose menus open over what is under them, drawn last.
    let mut menus: Vec<(Rect, String, Menu)> = Vec::new();
    let mut modifier_menus: Option<ModifierMenus> = None;
    let type_id = "workspace.properties.member.type";
    match edit.kind {
        MemberKind::Literal => {}
        MemberKind::Attribute => {
            let half = ((inner - 8.0) / 2.0).max(0.0);
            let value = shell.session.as_ref().map(|s| type_label(s.doc(), &edit.ty)).unwrap_or_default();
            let field = select_field(shell, ui, p, at(0.0, cy), half, type_id, &tr("Type"), &value, Menu::MemberType);
            menus.push((field, type_id.into(), Menu::MemberType));
            let m_id = "workspace.properties.member.multiplicity";
            enter |=
                text_field(shell, ui, p, at(half + 8.0, cy), half, m_id, &tr("Multiplicity"), &mut edit.multiplicity, MAX_MULTIPLICITY, false, false);
            cy += row;
            cy = refusal_under(&paint, p, at(0.0, cy), inner, &edit, &["type", "multiplicity"]) - y;
            let drawn = modifiers(shell, ui, p, at(0.0, cy), inner, &mut edit);
            enter |= drawn.enter;
            modifier_menus = Some(drawn.menus);
            cy += drawn.height;
        }
        MemberKind::Operation => {
            let value = shell.session.as_ref().map(|s| type_label(s.doc(), &edit.ty)).unwrap_or_default();
            let field = select_field(shell, ui, p, at(0.0, cy), inner, type_id, &tr("Return type"), &value, Menu::MemberType);
            menus.push((field, type_id.into(), Menu::MemberType));
            cy += row;
            let right = check(shell, ui, p, at(0.0, cy), "workspace.properties.member.static", &tr("Static"), &mut edit.is_static);
            check(shell, ui, p, pos2(right + 4.0, y + cy), "workspace.properties.member.abstract", &tr("Abstract"), &mut edit.is_abstract);
            cy += 26.0;
            cy = member_leaf(shell, ui, p, at(0.0, cy), inner, &mut edit) - y + 4.0;
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
    if let Some(why) = edit.refused.as_ref().filter(|_| edit.refused_at.is_none()) {
        let g = ux::text(&paint, at(0.0, cy - 4.0), why, types::CAPTION, p.amber, Some(inner));
        cy += g.height() + 8.0;
    }
    let card = Rect::from_min_size(pos2(left, y), vec2(width, cy));
    paint.set(background, egui::Shape::rect_filled(card, 8.0, p.surface));
    paint.rect_stroke(card, 8.0, Stroke::new(1.0, p.accent), StrokeKind::Inside);
    crate::panels::reveal(shell, ui, card);
    // The menus last, over the card.
    if shell.panels.menu == Some(Menu::MemberVisibility)
        && !literal
        && let Some(i) = menu(shell, ui, p, vis, vis_id, &words)
    {
        edit.visibility = i;
    }
    let within = edit.class;
    for (field, id, which) in menus {
        let slot = match which {
            Menu::ParamType(i) => edit.params.get_mut(i).map(|p| &mut p.ty),
            _ => Some(&mut edit.ty),
        };
        if let Some(ty) = slot {
            type_menu(shell, ui, p, field, &id, ty, which, Some(within));
        }
    }
    if let Some(m) = modifier_menus {
        modifier_menu_picks(shell, ui, p, m, &mut edit);
    }
    shell.panels.member = Some(edit.clone());
    if cancel || (shell.panels.menu.is_none() && ui.input(|i| i.key_pressed(egui::Key::Escape))) {
        shell.panels.member = None;
    } else if remove {
        let target = edit.target.map(|t| t.to_string()).unwrap_or_default();
        let said = crate::uml_constraints::before_removal(shell, std::slice::from_ref(&target));
        match run(shell, "element.remove", json!({ "ids": [target] })) {
            Ok(_) => {
                shell.panels.member = None;
                reselect(shell, edit.class);
                crate::uml_constraints::say_dropped(shell, said, &fmt(&tr("{name} deleted"), &[("name", &edit.name)]));
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

/// The member editor's No redefinition (leaf) at `at`, `width` wide, with its explanation under it
/// (AR-138): ticked, the member is to be leaf when applied. Ticked on a member something redefines
/// already, the box stays clear and a refusal under it names what does. Gives the y under it.
fn member_leaf(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, at: egui::Pos2, width: f32, edit: &mut MemberEdit) -> f32 {
    let refused = edit.refused.clone().filter(|_| edit.refused_at == Some("leaf"));
    let explained = tr("A subclass could not redefine this member. Shown as {leaf} after it.");
    let label = tr("No redefinition (leaf)");
    let (on, y) = explained_check(shell, ui, p, at, width, MEMBER_LEAF_ID, &label, edit.is_leaf, &explained, refused.as_deref());
    if on != edit.is_leaf {
        match shell.session.as_ref().and_then(|s| member_leaf_refusal(s.doc(), edit, on)) {
            // Refused before anything is written: the box stays clear.
            Some(why) => {
                edit.refused = Some(why);
                edit.refused_at = Some("leaf");
                ui.ctx().request_repaint();
            }
            None => {
                edit.is_leaf = on;
                if edit.refused_at == Some("leaf") {
                    edit.refused = None;
                    edit.refused_at = None;
                }
            }
        }
    }
    y
}

/// Why the member as edited may not be made leaf (`leaf`), in the user's words: something redefines
/// it already, named; the rule the commands keep too (AR-138). None when it is leaf already, or
/// may be made so.
pub(crate) fn member_leaf_refusal(store: &Store, edit: &MemberEdit, leaf: bool) -> Option<String> {
    if !leaf || edit.kind == MemberKind::Literal {
        return None;
    }
    let o = edit.target.and_then(|t| store.by_uuid(t))?;
    if archforge_uml::relations::is_leaf(store, o) {
        return None;
    }
    let by = redefiner_names(store, o);
    (!by.is_empty()).then(|| leaf_refusal(&by))
}

/// The refusal of `edit` when it sits under one of `fields`, wrapped under the field at `at`;
/// gives the y under it.
fn refusal_under(paint: &egui::Painter, p: &Palette, at: egui::Pos2, width: f32, edit: &MemberEdit, fields: &[&str]) -> f32 {
    match (&edit.refused, edit.refused_at) {
        (Some(why), Some(field)) if fields.contains(&field) => {
            let said = ux::galley_rows(paint, why, types::CAPTION, p.amber, Some(width), 4);
            let height = said.size().y;
            paint.galley(at - vec2(0.0, 4.0), said, p.amber);
            at.y + height + 6.0
        }
        _ => at.y,
    }
}

/// The menus the modifiers' selects open, drawn over the card last: the default's literals (for
/// an enumeration's type) and the Subsets and Redefines choices, each with its select's place.
pub(crate) struct ModifierMenus {
    default: Option<Rect>,
    subsets: (Rect, TargetChoices),
    redefines: Rect,
    /// The name being typed, which Subsets leaves out of its kept list.
    name: String,
}

/// What [`modifiers`] drew: its height, whether Enter was pressed in its field, and its menus.
struct Drawn {
    height: f32,
    enter: bool,
    menus: ModifierMenus,
}

/// The attribute editor's modifiers under its type, at `at`, `width` wide (AR-126): Default value,
/// a text field or, for an enumeration's type, a select of its literals; Static, Derived, Read
/// only, Ordered and Unique; Subsets and Redefines, which list only the properties the rules
/// allow, with a muted line under each that has none. A refusal sits under its field.
fn modifiers(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, at: egui::Pos2, width: f32, edit: &mut MemberEdit) -> Drawn {
    let paint = ui.painter().clone();
    let half = ((width - 8.0) / 2.0).max(0.0);
    let row = 17.0 + FIELD + 10.0;
    let mut y = at.y;
    let mut enter = false;
    // The choices, built once per state of the project and shape of the attribute (its name aside,
    // which Subsets drops after): Subsets', Redefines' and, for an enumeration's type, its literals,
    // among which its default is picked.
    let key = format!("attribute|{}|{:?}|{}|{}", edit.class, edit.target, edit.ty, edit.multiplicity.trim());
    let (class, target, ty, multiplicity) = (edit.class, edit.target, edit.ty.clone(), edit.multiplicity.clone());
    let ty_for_literals = ty.clone();
    let targets = take_targets(
        shell,
        &key,
        |store| {
            let bounds = archforge_uml::members::parse_multiplicity(&multiplicity).ok()?;
            let edit = MemberEdit { class, target, ty, multiplicity, ..edit_of(None, class, MemberKind::Attribute, target) };
            edit_shape(store, &edit, bounds)
        },
        |store| edit_type(store, &ty_for_literals).0,
    );
    let default_id = "workspace.properties.member.default";
    let default = match &targets.literals {
        Some(_) => {
            let shown = if edit.default.trim().is_empty() { tr("None") } else { edit.default.clone() };
            Some(select_field(shell, ui, p, pos2(at.x, y), width, default_id, &tr("Default value"), &shown, Menu::MemberDefault))
        }
        None => {
            let limit = archforge_uml::modifiers::MAX_DEFAULT;
            enter |= text_field(shell, ui, p, pos2(at.x, y), width, default_id, &tr("Default value"), &mut edit.default, limit, false, false);
            None
        }
    };
    y += row;
    y = refusal_under(&paint, p, pos2(at.x, y), width, edit, &["default"]);
    // The flags in two columns, Static first as before.
    let right = at.x + half + 8.0;
    check(shell, ui, p, pos2(at.x, y), "workspace.properties.member.static", &tr("Static"), &mut edit.is_static);
    check(shell, ui, p, pos2(right, y), "workspace.properties.member.derived", &tr("Derived"), &mut edit.derived);
    y += 26.0;
    check(shell, ui, p, pos2(at.x, y), "workspace.properties.member.read-only", &tr("Read only"), &mut edit.read_only);
    check(shell, ui, p, pos2(right, y), "workspace.properties.member.ordered", &tr("Ordered"), &mut edit.ordered);
    y += 26.0;
    check(shell, ui, p, pos2(at.x, y), "workspace.properties.member.unique", &tr("Unique"), &mut edit.unique);
    y += 26.0;
    y = member_leaf(shell, ui, p, pos2(at.x, y), width, edit) + 4.0;
    // Subsets and Redefines.
    let (subsets_shown, redefines_shown) = match shell.session.as_ref() {
        Some(s) => (chosen_names(s.doc(), &edit.subsets), chosen_names(s.doc(), &edit.redefines)),
        None => (tr("None"), tr("None")),
    };
    let subsets =
        select_field(shell, ui, p, pos2(at.x, y), half, "workspace.properties.member.subsets", &tr("Subsets"), &subsets_shown, Menu::MemberSubsets);
    let redefines = select_field(
        shell,
        ui,
        p,
        pos2(right, y),
        half,
        "workspace.properties.member.redefines",
        &tr("Redefines"),
        &redefines_shown,
        Menu::MemberRedefines,
    );
    y += 17.0 + FIELD + 6.0;
    let mut said = 0.0_f32;
    let no_subsets = offered_subsets(&targets, &edit.name).is_empty();
    for (empty, x, text) in [
        (no_subsets, at.x, tr("No property to subset")),
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
    y = refusal_under(&paint, p, pos2(at.x, y + 4.0), width, edit, &["subsets", "redefines"]);
    let name = edit.name.clone();
    Drawn { height: y - at.y + 4.0, enter, menus: ModifierMenus { default, subsets: (subsets, targets), redefines, name } }
}

/// The names of the properties `ids` names, joined, or None when it names none.
fn chosen_names(store: &Store, ids: &[Uuid]) -> String {
    if ids.is_empty() {
        return tr("None");
    }
    ids.iter().filter_map(|u| store.by_uuid(*u)).map(|o| name(store, o)).collect::<Vec<_>>().join(", ")
}

/// The open modifier menu, when one is: a pick sets the default's literal, or the one property
/// Subsets or Redefines names (None names none); the choices go back to the shell.
fn modifier_menu_picks(shell: &mut Shell, ui: &egui::Ui, p: &Palette, m: ModifierMenus, edit: &mut MemberEdit) {
    let ModifierMenus { default, subsets: (subsets_field, targets), redefines: redefines_field, name } = m;
    match shell.panels.menu {
        Some(Menu::MemberDefault) => {
            if let (Some(field), Some(names)) = (default, targets.literals.as_ref())
                && let Some(pick) = literal_menu(shell, ui, p, field, "workspace.properties.member.default", names)
            {
                edit.default = pick;
            }
        }
        Some(Menu::MemberSubsets) => {
            if let Some(pick) = target_menu(shell, ui, p, subsets_field, "workspace.properties.member.subsets", &offered_subsets(&targets, &name)) {
                edit.subsets = pick;
            }
        }
        Some(Menu::MemberRedefines) => match redefines_menu(shell, ui, p, redefines_field, "workspace.properties.member.redefines", &targets) {
            Some(Ok(pick)) => {
                edit.redefines = pick;
                if edit.refused_at == Some("redefines") {
                    edit.refused = None;
                    edit.refused_at = None;
                }
            }
            // A leaf property: nothing changes, and why sits under Redefines.
            Some(Err(why)) => {
                edit.refused = Some(why);
                edit.refused_at = Some("redefines");
            }
            None => {}
        },
        _ => {}
    }
    keep_targets(shell, targets);
}

/// A literal select's open menu, None first then `names`: the literal's name picked, or empty for
/// None.
pub(crate) fn literal_menu(shell: &mut Shell, ui: &egui::Ui, p: &Palette, field: Rect, id: &str, names: &[String]) -> Option<String> {
    let items: Vec<String> = std::iter::once(tr("None")).chain(names.iter().cloned()).collect();
    let i = menu(shell, ui, p, field, id, &items)?;
    Some(i.checked_sub(1).and_then(|i| names.get(i)).cloned().unwrap_or_default())
}

/// A Subsets or Redefines select's open menu, None first then `offered` by their labels: the one
/// property picked, or none for None.
pub(crate) fn target_menu(shell: &mut Shell, ui: &egui::Ui, p: &Palette, field: Rect, id: &str, offered: &[&Target]) -> Option<Vec<Uuid>> {
    let items: Vec<String> = std::iter::once(tr("None")).chain(offered.iter().map(|t| t.label.clone())).collect();
    let i = menu(shell, ui, p, field, id, &items)?;
    Some(i.checked_sub(1).and_then(|i| offered.get(i)).map(|t| vec![t.id]).unwrap_or_default())
}

/// Redefines' open menu (AR-126, AR-138): None first, the properties it offers by their labels, then
/// those it may not take for being leaf, each with a muted `{leaf}` after it. Gives the one property
/// picked, none for None, or, for a leaf one, why it may not be picked.
pub(crate) fn redefines_menu(
    shell: &mut Shell,
    ui: &egui::Ui,
    p: &Palette,
    field: Rect,
    id: &str,
    targets: &TargetChoices,
) -> Option<Result<Vec<Uuid>, String>> {
    let items: Vec<String> = std::iter::once(tr("None")).chain(targets.redefines.iter().chain(&targets.leaf).map(|t| t.label.clone())).collect();
    let notes: Vec<String> = std::iter::repeat_n(String::new(), 1 + targets.redefines.len())
        .chain(std::iter::repeat_n("{leaf}".to_string(), targets.leaf.len()))
        .collect();
    let i = crate::panels::menu_noted(shell, ui, p, field, id, &items, &notes, &[])?;
    let Some(i) = i.checked_sub(1) else { return Some(Ok(Vec::new())) };
    if let Some(t) = targets.redefines.get(i) {
        return Some(Ok(vec![t.id]));
    }
    let t = targets.leaf.get(i.saturating_sub(targets.redefines.len()))?;
    Some(Err(target_refusal(&t.name, archforge_uml::modifiers::TargetProblem::Leaf, true)))
}

/// The properties Subsets offers a property named `name`: the kept list, less a property named
/// alike whatever the case, which a subset never is.
pub(crate) fn offered_subsets<'a>(targets: &'a TargetChoices, name: &str) -> Vec<&'a Target> {
    let named = name.trim().to_lowercase();
    targets.subsets.iter().filter(|t| t.name.to_lowercase() != named).collect()
}

/// A property Subsets or Redefines offers: its id, its name and its label (`name · Class`).
#[derive(Debug, Clone, PartialEq)]
pub struct Target {
    pub id: Uuid,
    pub name: String,
    pub label: String,
}

/// The properties Subsets and Redefines offer for one shape of one property, in the order of their
/// labels, Subsets' without the name rule, which [`offered_subsets`] applies to the name being
/// typed; and the literals of the property's type when it is an enumeration. Kept for the state of
/// the project and the interface language they were built in (AR-126).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TargetChoices {
    pub key: String,
    pub subsets: Vec<Target>,
    pub redefines: Vec<Target>,
    /// The properties Redefines would offer but for their being leaf (AR-138): listed after the
    /// others, each marked `{leaf}`, and refused with why when picked.
    pub leaf: Vec<Target>,
    /// At most [`archforge_uml::members::MAX_LITERALS`], as an enumeration holds.
    pub literals: Option<Vec<String>>,
}

/// The most target lists kept: the attribute editor's and both ends' fit.
const KEPT_TARGETS: usize = 4;

/// The choices for `what` (the property's shape as text, its name aside), taken out of the shell
/// for the frame: the kept ones when the project, its state and the language are the same, else
/// built once from `shape` and `enumeration` (the property's type when it is one) and kept from
/// then on. Hand them back with [`keep_targets`].
pub(crate) fn take_targets(
    shell: &mut Shell,
    what: &str,
    shape: impl FnOnce(&Store) -> Option<archforge_uml::modifiers::Shape>,
    enumeration: impl FnOnce(&Store) -> Option<Obj>,
) -> TargetChoices {
    let Some(session) = shell.session.as_ref() else { return TargetChoices::default() };
    let store = session.doc();
    let project = archforge_uml::commands::model(store).ok().and_then(|m| store.uuid(m).ok());
    let key = format!("{project:?}|{}|{}|{what}", session.revision(), crate::i18n::language());
    if let Some(i) = shell.panels.targets.iter().position(|t| t.key == key) {
        return shell.panels.targets.remove(i);
    }
    let labelled = |objs: Vec<Obj>| -> Vec<Target> {
        let mut out: Vec<Target> = objs
            .into_iter()
            .filter_map(|t| {
                let owner = archforge_uml::modifiers::context_of(store, t).first().map(|c| name(store, *c)).unwrap_or_default();
                let named = name(store, t);
                Some(Target { id: store.uuid(t).ok()?, label: format!("{named} · {owner}"), name: named })
            })
            .collect();
        out.sort_by_key(|t| t.label.to_lowercase());
        out
    };
    let literals = enumeration(store).filter(|e| store.class_of(*e).ok() == Some(archforge_uml::ENUMERATION)).map(|e| {
        archforge_uml::commands::literals(store, e).into_iter().take(archforge_uml::members::MAX_LITERALS).map(|l| name(store, l)).collect()
    });
    match shape(store) {
        Some(sh) => TargetChoices {
            key,
            subsets: labelled(archforge_uml::modifiers::subsets_candidates_named(store, &sh, false)),
            redefines: labelled(archforge_uml::modifiers::redefines_candidates(store, &sh)),
            leaf: labelled(archforge_uml::modifiers::redefines_leaf(store, &sh)),
            literals,
        },
        None => TargetChoices { key, literals, ..TargetChoices::default() },
    }
}

/// Hands target choices back to the shell for the next frame, the oldest dropped past
/// [`KEPT_TARGETS`].
pub(crate) fn keep_targets(shell: &mut Shell, targets: TargetChoices) {
    shell.panels.targets.push(targets);
    if shell.panels.targets.len() > KEPT_TARGETS {
        shell.panels.targets.remove(0);
    }
}

/// The type an editor's type value names, without making it, with the form its default reads in:
/// a built-in type by its name (none while it is not made yet), any other by its id.
pub(crate) fn edit_type(store: &Store, ty: &str) -> (Option<Obj>, archforge_uml::modifiers::DefaultForm) {
    use archforge_uml::modifiers::DefaultForm;
    if ty.is_empty() {
        return (None, DefaultForm::Opaque);
    }
    if let Some(form) = DefaultForm::built_in(ty) {
        let made = archforge_uml::members::types_package(store)
            .and_then(|p| archforge_uml::commands::primitive_types(store, p).into_iter().find(|t| built_in(store, *t) && name(store, *t) == ty));
        return (made, form);
    }
    match Uuid::parse(ty).and_then(|u| store.by_uuid(u)) {
        Some(t) => (Some(t), DefaultForm::of(store, Some(t))),
        None => (None, DefaultForm::Opaque),
    }
}

/// The shape of the attribute as edited, with its typed `bounds` (none: one), as Subsets and
/// Redefines check it.
fn edit_shape(store: &Store, edit: &MemberEdit, bounds: Option<(u64, Option<u64>)>) -> Option<archforge_uml::modifiers::Shape> {
    let class = store.by_uuid(edit.class)?;
    let (ty, _) = edit_type(store, &edit.ty);
    let (lower, upper) = bounds.unwrap_or((1, Some(1)));
    Some(archforge_uml::modifiers::Shape {
        itself: edit.target.and_then(|t| store.by_uuid(t)),
        context: vec![class],
        ty,
        lower,
        upper,
        composite: false,
        name: edit.name.trim().to_string(),
    })
}

/// Why the attribute as edited cannot be written, in the user's words, with the field it sits
/// under: a multiplicity that does not read; a default its type does not read (a type change the
/// kept default does not fit said under the type, naming the default); a property it subsets or
/// redefines that the rules refuse, named with why. The commands keep the same rules. None for any
/// other member.
pub(crate) fn modifier_refusal(store: &Store, edit: &MemberEdit) -> Option<(&'static str, String)> {
    if edit.kind != MemberKind::Attribute {
        return None;
    }
    let bounds = match archforge_uml::members::parse_multiplicity(&edit.multiplicity) {
        Ok(b) => b,
        Err(e) => return Some(("multiplicity", crate::uml_relate::multiplicity_problem(&e))),
    };
    let (_, form) = edit_type(store, &edit.ty);
    if let Err(e) = archforge_uml::modifiers::parse_default(store, form, &edit.default) {
        let was = edit.target.and_then(|t| store.by_uuid(t)).map(|o| (type_value(store, o), archforge_uml::modifiers::default_text(store, o)));
        if let Some((was_ty, was_default)) = was
            && was_ty != edit.ty
            && !was_default.trim().is_empty()
            && was_default.trim() == edit.default.trim()
        {
            let ty = type_label(store, &edit.ty);
            let said = fmt(
                &tr("The default value {value} does not fit {type}; change or clear it first."),
                &[("value", edit.default.trim()), ("type", &ty)],
            );
            return Some(("type", said));
        }
        return Some(("default", default_refusal(store, &e)));
    }
    let shape = edit_shape(store, edit, bounds)?;
    let current = edit.target.and_then(|t| store.by_uuid(t));
    let ids = |ts: Vec<Obj>| -> Vec<Uuid> { ts.into_iter().filter_map(|t| store.uuid(t).ok()).collect() };
    let had_subsets = current.map(|c| ids(archforge_uml::modifiers::subsets(store, c))).unwrap_or_default();
    let had_redefines = current.map(|c| ids(archforge_uml::modifiers::redefines(store, c))).unwrap_or_default();
    let was = current.map(|c| archforge_uml::modifiers::Shape::of(store, c));
    target_refusals(store, &shape, was.as_ref(), (&edit.subsets, &had_subsets), (&edit.redefines, &had_redefines))
}

/// Why `shape` may not subset one of `subsets` or redefine one of `redefines` (each with what the
/// property has already), in the user's words, with the field it sits under; none when the rules
/// allow each. As the commands do, every target is checked against the new shape except one the
/// property had (`was` its shape in the model) that the rules no longer allowed already, which never
/// blocks another edit.
pub(crate) fn target_refusals(
    store: &Store,
    shape: &archforge_uml::modifiers::Shape,
    was: Option<&archforge_uml::modifiers::Shape>,
    subsets: (&[Uuid], &[Uuid]),
    redefines: (&[Uuid], &[Uuid]),
) -> Option<(&'static str, String)> {
    let problem_of = |sh: &archforge_uml::modifiers::Shape, t: Obj, redefining: bool| {
        if redefining { archforge_uml::modifiers::redefines_problem(store, sh, t) } else { archforge_uml::modifiers::subsets_problem(store, sh, t) }
    };
    for ((ids, had), redefining, field) in [(subsets, false, "subsets"), (redefines, true, "redefines")] {
        for id in ids {
            let Some(t) = store.by_uuid(*id).filter(|t| store.is_alive(*t)) else {
                return Some((field, tr("A property chosen here is gone; choose again.")));
            };
            let stale = had.contains(id) && was.is_some_and(|w| problem_of(w, t, redefining).is_some());
            if let Some(problem) = problem_of(shape, t, redefining).filter(|_| !stale) {
                return Some((field, target_refusal(&name(store, t), problem, redefining)));
            }
        }
    }
    None
}

/// Why a default does not read, in the user's words: the value and the form its type takes.
pub(crate) fn default_refusal(store: &Store, e: &archforge_uml::modifiers::DefaultError) -> String {
    use archforge_uml::modifiers::{DefaultError, DefaultForm};
    let (value, form) = match e {
        DefaultError::TooLong => {
            return fmt(&tr("A default value is at most {n} characters."), &[("n", &archforge_uml::modifiers::MAX_DEFAULT.to_string())]);
        }
        DefaultError::Malformed(value, form) => (value.as_str(), *form),
    };
    let said = match form {
        DefaultForm::Integer => tr("{value} is not a whole number, which an Integer's default is, such as 42."),
        DefaultForm::Real => tr("{value} is not a number, which a Real's default is, such as 2.5."),
        DefaultForm::Boolean => tr("{value} is neither true nor false, which a Boolean's default is."),
        DefaultForm::UnlimitedNatural => tr("{value} is neither a whole number of 0 or more nor *, which an UnlimitedNatural's default is."),
        DefaultForm::Enumeration(en) => {
            return fmt(&tr("{value} is not a literal of {enumeration}."), &[("value", value), ("enumeration", &name(store, en))]);
        }
        DefaultForm::String | DefaultForm::Opaque => tr("{value} cannot be read as a default."),
    };
    fmt(&said, &[("value", value)])
}

/// Why `name` cannot be subset (or, `redefining`, redefined), in the user's words.
pub(crate) fn target_refusal(name: &str, problem: archforge_uml::modifiers::TargetProblem, redefining: bool) -> String {
    use archforge_uml::modifiers::TargetProblem as P;
    let said = match (problem, redefining) {
        (P::Unnamed, _) => return tr("A property without a name cannot be named here."),
        (P::NotAProperty | P::Itself | P::OutOfReach, false) => tr("{name} is not a property this one can subset."),
        (P::NotAProperty | P::Itself | P::OutOfReach, true) => tr("{name} is not a property this one can redefine."),
        (P::SameName, _) => tr("{name} has this property's name; a subset takes another."),
        (P::Cycle, _) => tr("{name} subsets this property already."),
        (P::Type, _) => tr("{name} has a type this property's type does not conform to."),
        (P::Upper, _) => tr("{name} holds fewer values than this property."),
        (P::Multiplicity, _) => tr("This property's multiplicity does not lie within {name}'s."),
        (P::Composite, _) => tr("{name} is composite; only a composite end redefines it."),
        (P::Leaf, _) => tr("{name} is leaf: nothing may redefine it. Clear \"No redefinition\" on it first."),
    };
    fmt(&said, &[("name", name)])
}

/// Shows why the editor's command was refused, at the card's foot, or clears it.
fn refuse(shell: &mut Shell, why: Option<String>) {
    if let Some(e) = shell.panels.member.as_mut() {
        e.refused = why;
        e.refused_at = None;
    }
}

/// Selects the class: what the panel shows after its member went or was written.
fn reselect(shell: &mut Shell, class: Uuid) {
    if let Err(e) = run(shell, "view.select", json!({ "ids": [class.to_string()] })) {
        shell.panels.notice = Some(e);
    }
}

/// Moves the member above its neighbour before it, or below the one after it, among the class's
/// attributes or operations, or the enumeration's literals.
fn move_member(shell: &mut Shell, edit: &MemberEdit, up: bool) {
    let Some(target) = edit.target else { return };
    let at = shell.session.as_ref().and_then(|s| {
        let store = s.doc();
        let (c, o) = (store.by_uuid(edit.class)?, store.by_uuid(target)?);
        let (list, prop) = match edit.kind {
            MemberKind::Attribute => (attributes(store, c), archforge_uml::members::member_lists(store, c)?.0),
            MemberKind::Operation => (operations(store, c), archforge_uml::members::member_lists(store, c)?.1),
            MemberKind::Literal => (archforge_uml::commands::literals(store, c), archforge_uml::enumeration::OWNED_LITERAL),
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
        MemberKind::Literal => ("uml.literal.move", "literal"),
    };
    match run(shell, cmd, json!({ key: target.to_string(), "at": at })) {
        Ok(_) => refuse(shell, None),
        Err(e) => refuse(shell, Some(e)),
    }
}

/// Writes the member through its command; a modifier, a type or a multiplicity the rules refuse is
/// said under its field in the user's words, and nothing is written.
fn commit(shell: &mut Shell, edit: &MemberEdit) {
    let refusal = shell.session.as_ref().and_then(|s| {
        let store = s.doc();
        member_leaf_refusal(store, edit, edit.is_leaf).map(|why| ("leaf", why)).or_else(|| modifier_refusal(store, edit))
    });
    if let Some((field, why)) = refusal {
        refuse(shell, Some(why));
        if let Some(e) = shell.panels.member.as_mut() {
            e.refused_at = Some(field);
        }
        return;
    }
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

/// The command that writes the member as edited, with its parameters: the member's id under its
/// key when it is edited, else its classifier's under `class` (an enumeration's under
/// `enumeration`).
pub(crate) fn command_of(edit: &MemberEdit) -> (&'static str, serde_json::Value) {
    let visibility = VISIBILITIES[edit.visibility.min(VISIBILITIES.len() - 1)];
    let (cmd, mut params, key) = match edit.kind {
        MemberKind::Attribute => (
            if edit.target.is_some() { "uml.attribute.set" } else { "uml.attribute.add" },
            json!({
                "name": edit.name.trim(), "visibility": visibility, "type": edit.ty,
                "multiplicity": edit.multiplicity.trim(), "static": edit.is_static, "leaf": edit.is_leaf,
                "default": edit.default, "derived": edit.derived, "read_only": edit.read_only, "ordered": edit.ordered, "unique": edit.unique,
                "subsets": edit.subsets.iter().map(Uuid::to_string).collect::<Vec<_>>(),
                "redefines": edit.redefines.iter().map(Uuid::to_string).collect::<Vec<_>>(),
            }),
            "attribute",
        ),
        MemberKind::Operation => (
            if edit.target.is_some() { "uml.operation.set" } else { "uml.operation.add" },
            json!({
                "name": edit.name.trim(), "visibility": visibility, "returns": edit.ty, "static": edit.is_static, "abstract": edit.is_abstract,
                "leaf": edit.is_leaf,
                "parameters": edit.params.iter().map(|p| json!({ "name": p.name.trim(), "type": p.ty, "multiplicity": p.multiplicity.trim() })).collect::<Vec<_>>(),
            }),
            "operation",
        ),
        MemberKind::Literal => {
            (if edit.target.is_some() { "uml.literal.rename" } else { "uml.literal.add" }, json!({ "name": edit.name.trim() }), "literal")
        }
    };
    let (key, id) = match (edit.target, edit.kind) {
        (Some(t), _) => (key, t.to_string()),
        (None, MemberKind::Literal) => ("enumeration", edit.class.to_string()),
        (None, _) => ("class", edit.class.to_string()),
    };
    if let Some(fields) = params.as_object_mut() {
        fields.insert(key.to_string(), json!(id));
    }
    (cmd, params)
}

/// Why the literal as edited cannot be written, in the user's words: a new one in an enumeration
/// that holds the most literals it may, or a name another literal of its enumeration has, whatever
/// the case; the rules the commands keep too. None for any other member.
pub(crate) fn literal_refusal(store: &Store, edit: &MemberEdit) -> Option<String> {
    if edit.kind != MemberKind::Literal {
        return None;
    }
    let enumeration = store.by_uuid(edit.class)?;
    let max = archforge_uml::members::MAX_LITERALS;
    if edit.target.is_none() && archforge_uml::commands::literals(store, enumeration).len() >= max {
        return Some(fmt(&tr("An enumeration holds at most {n} literals."), &[("n", &max.to_string())]));
    }
    let except = edit.target.and_then(|t| store.by_uuid(t));
    let clash = archforge_uml::members::literal_clash(store, enumeration, edit.name.trim(), except)?;
    Some(fmt(&tr("There is a literal named {name} already."), &[("name", &name(store, clash))]))
}

/// Why the attribute as edited cannot be written, in the user's words: in an association class its
/// name is one of its ends' roles, whatever the case, and they share its names; the rule the
/// commands keep too. None for any other member.
pub(crate) fn attribute_refusal(store: &Store, edit: &MemberEdit) -> Option<String> {
    if edit.kind != MemberKind::Attribute {
        return None;
    }
    let class = store.by_uuid(edit.class)?;
    let end = archforge_uml::members::end_role_clash(store, class, edit.name.trim())?;
    Some(fmt(&tr("An end of this association class has the role {name}; an attribute takes another name."), &[("name", &name(store, end))]))
}

/// Runs the member's command and, in the same undo step, grows every card of its class that would
/// then hide a member, on every diagram, keeping its width (owner decision: a card the user made
/// shorter stays so until a member of its class is added or edited); an enumeration's grows to
/// show a literal added the same way. A literal's name that clashes, an association class's
/// attribute named as one of its ends' roles, or a modifier the rules refuse (AR-126), is refused
/// first, in the user's words, and nothing is run.
pub(crate) fn write_in_sight(shell: &mut Shell, edit: &MemberEdit, cmd: &str, params: serde_json::Value) -> Result<serde_json::Value, String> {
    let Some(session) = shell.session.as_mut() else { return Err(tr("The model could not be opened")) };
    let store = session.doc();
    if let Some(why) = literal_refusal(store, edit)
        .or_else(|| attribute_refusal(store, edit))
        .or_else(|| member_leaf_refusal(store, edit, edit.is_leaf))
        .or_else(|| modifier_refusal(store, edit).map(|(_, w)| w))
    {
        return Err(why);
    }
    let more = match (edit.target, edit.kind) {
        (None, MemberKind::Attribute | MemberKind::Literal) => (1, 0),
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
        "uml.literal.add" => "Add literal",
        "uml.literal.rename" => "Rename literal",
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
    ["Add attribute", "Add operation", "Add literal", "Remove", "Remove parameter", "Move up", "Move down", "Cancel", "Apply"].into_iter()
}
