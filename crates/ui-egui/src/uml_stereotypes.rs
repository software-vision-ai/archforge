//! Stereotypes applied in the shell (AR-135, design page 08, frames F3 and F4): a Stereotypes
//! section in the Properties of every element a stereotype may extend (a classifier or the member
//! selected on its panel, a package, an association, a generalization), its `+` opening the picker
//! of every stereotype of the model, those that apply first, those applied ticked, then after a line
//! the rest, disabled with why (`properties only`, `Logging not applied`); typing filters it, the
//! arrows move, Enter applies, Escape closes. Each applied stereotype is a row with its profile and
//! a bin that takes it off (its definition stays), then a control per property fitting its type: a
//! select of true and false or of an enumeration's literals, or a field for any other value, its
//! default shown with a `default` tag while it is unset, a value set kept apart from it even when it
//! says the same; a refused value stays in its field, ringed, with the reason under it. Under them,
//! what values and Remove mean, and, while a class diagram is open, Show stereotype values on this
//! diagram. The rules are the model's (`archforge_uml::applications`): the shell asks them and says
//! their answers in the user's words.

use archforge_mof::{Obj, Store, Uuid};
use archforge_uml::applications::{self, Offer, ValueProblem};
use archforge_uml::modifiers::{DefaultError, DefaultForm};
use archforge_uml::profiles;
use egui::{Rect, Sense, Stroke, StrokeKind, pos2, vec2};
use serde_json::json;

use crate::Shell;
use crate::home::control;
use crate::i18n::{fmt, tr, tr_ctx, trn};
use crate::panels::{FIELD, Menu, PAD, ROW, run, section};
use crate::ux::{self, Palette, icons, types};

/// The ids of the Stereotypes section (F3, F4).
pub(crate) const PREFIX: &str = "workspace.properties.stereotypes";
#[cfg(test)]
pub(crate) const ADD_ID: &str = "workspace.properties.stereotypes.add";
#[cfg(test)]
pub(crate) const APPLIED_ID: &str = "workspace.properties.stereotypes.applied";
pub(crate) const SHOW_ID: &str = "workspace.properties.stereotypes.showValues";

/// What values and Remove mean, under the section (F3), and where stereotypes come from while none
/// is applied: too long for a line of code, so named here and listed for the catalogue in [`texts`].
const VALUES_HELP: &str = "Values you set are stored on {name}; a default shows the tag until you change it. Remove takes «{stereotype}» off {name} and keeps its definition.";
const NONE_HELP: &str = "None applied yet. + offers the stereotypes of the profiles applied to {package} or a package around it.";

/// The words a picker row's reason uses for the metaclasses a stereotype extends, by UML's names.
fn kind_words(metaclass: &str) -> String {
    match metaclass {
        "Class" => tr("classes"),
        "Interface" => tr("interfaces"),
        "Enumeration" => tr("enumerations"),
        "DataType" => tr("data types"),
        "Property" => tr("properties"),
        "Operation" => tr("operations"),
        "Association" => tr("associations"),
        "Generalization" => tr("generalizations"),
        "Package" => tr("packages"),
        other => other.to_string(),
    }
}

fn name(store: &Store, o: Obj) -> String {
    archforge_core::name_of(store, o)
}

/// A value being typed into a property's field: where (the element, the stereotype and the
/// property), the text so far, and why it was refused, when it was.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValueEdit {
    pub element: Uuid,
    pub stereotype: Uuid,
    pub property: Uuid,
    pub text: String,
    pub refused: Option<String>,
}

/// The Stereotypes section's state: the picker's filter as typed, and a change refused, said
/// under the section for its element.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Stereotypes {
    pub value: Option<ValueEdit>,
    pub filter: String,
    pub refused: Option<(Uuid, String)>,
    /// The element whose section's picker or value menu is open: an association and each of its
    /// ends have a section of their own on one panel.
    pub menu_for: Option<Uuid>,
}

/// Which section a panel draws: the panel's own element's, a member's selected on it, or an
/// association end's (`i`, its place among the ends), each with ids of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Heading {
    Own,
    Member,
    End(usize),
}

impl Heading {
    /// The start of the section's ids.
    fn prefix(self) -> String {
        match self {
            Heading::Own | Heading::Member => PREFIX.to_string(),
            Heading::End(i) => format!("workspace.properties.end.{i}.stereotypes"),
        }
    }
}

/// Drops what the store no longer has.
pub(crate) fn retain_valid(state: &mut Stereotypes, store: &Store) {
    if state.value.as_ref().is_some_and(|v| store.by_uuid(v.element).is_none() || store.by_uuid(v.property).is_none()) {
        state.value = None;
    }
    if state.refused.as_ref().is_some_and(|(e, _)| store.by_uuid(*e).is_none()) {
        state.refused = None;
    }
}

/// Whether `element` has a Stereotypes section: whether a stereotype may extend its metaclass.
pub(crate) fn takes(store: &Store, element: Obj) -> bool {
    applications::element_problem(store, element).is_none()
}

/// The UML metaclass a picker row says the element is: the one Archforge draws it as, by UML's name.
fn metaclass_word(store: &Store, element: Obj) -> &'static str {
    let Ok(class) = store.class_of(element) else { return "" };
    profiles::METACLASSES
        .iter()
        .find(|(_, k)| *k == class)
        .or_else(|| profiles::METACLASSES.iter().find(|(_, k)| store.universe().conforms(class, archforge_mof::TypeRef::Class(*k))))
        .map_or("", |(n, _)| n)
}

/// A row of the picker: the stereotype, its label, the detail at its right (its profile, or the
/// short reason), whether it may be picked, whether it is applied, and the whole reason.
struct Pick {
    stereotype: Uuid,
    label: String,
    detail: String,
    live: bool,
    ticked: bool,
    why: Option<String>,
}

/// The picker's rows for `element`, filtered by `filter` (any part of the name, whatever the case):
/// those that apply and those applied first, then the rest, each group by profile.
fn picks(store: &Store, element: Obj, filter: &str) -> Vec<Pick> {
    let wanted = filter.trim().to_lowercase();
    let mut open = Vec::new();
    let mut shut = Vec::new();
    for (s, offer) in applications::choices(store, element) {
        let label = name(store, s);
        if !wanted.is_empty() && !label.to_lowercase().contains(&wanted) {
            continue;
        }
        let Ok(id) = store.uuid(s) else { continue };
        let profile = profiles::profile_of(store, s).map(|p| name(store, p)).unwrap_or_default();
        let pick = |live, ticked, detail, why| Pick { stereotype: id, label: label.clone(), detail, live, ticked, why };
        match offer {
            Offer::Open => open.push(pick(true, false, profile, None)),
            Offer::Applied => open.push(pick(false, true, profile, Some(tr("Applied already")))),
            Offer::Extends(metaclasses) => {
                let short = match metaclasses.as_slice() {
                    [] => tr("extends nothing yet"),
                    [one] => fmt(&tr("{kinds} only"), &[("kinds", &kind_words(one))]),
                    [a, b] => fmt(
                        &tr("{kinds} only"),
                        &[("kinds", &fmt(&tr("{first} and {second}"), &[("first", &kind_words(a)), ("second", &kind_words(b))]))],
                    ),
                    many => fmt(&tr("{kinds} only"), &[("kinds", &many.iter().map(|m| kind_words(m)).collect::<Vec<_>>().join(", "))]),
                };
                let why = fmt(
                    &tr("«{stereotype}» does not extend {metaclass}: it extends {metaclasses}."),
                    &[("stereotype", &label), ("metaclass", metaclass_word(store, element)), ("metaclasses", &metaclasses.join(", "))],
                );
                let why =
                    if metaclasses.is_empty() { fmt(&tr("«{stereotype}» extends no metaclass yet."), &[("stereotype", &label)]) } else { why };
                shut.push(pick(false, false, short, Some(why)));
            }
            Offer::OutOfScope => {
                let home = profiles::home_of(store, element).map(|p| name(store, p)).unwrap_or_default();
                let why = fmt(
                    &tr("Its profile {profile} is not applied to {package} or a package around it."),
                    &[("profile", &profile), ("package", &home)],
                );
                shut.push(pick(false, false, fmt(&tr("{profile} not applied"), &[("profile", &profile)]), Some(why)));
            }
        }
    }
    open.into_iter().chain(shut).collect()
}

/// The Stereotypes section of `element` at `y` (F3): its title with the count and the picker's
/// `+`, a row per applied stereotype with its profile and its bin, a control per property, what
/// values mean, a refusal, and Show stereotype values on this diagram. `heading` says whose section it
/// is: the panel's own element's, a member's selected there, which the title names, or an association
/// end's, with ids of its own and no diagram check. Gives the y under it.
#[allow(clippy::too_many_arguments, reason = "a section is drawn into the panel for its element")]
pub(crate) fn section_of(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect, mut y: f32, element: Obj, heading: Heading) -> f32 {
    let prefix = heading.prefix();
    let add_id = format!("{prefix}.add");
    let Some(store) = shell.session.as_ref().map(|s| s.doc()) else { return y };
    let Ok(element_id) = store.uuid(element) else { return y };
    let own = name(store, element);
    let applied: Vec<(Obj, Obj)> = applications::applied_to(store, element);
    let n = applied.len().to_string();
    let title = match heading {
        Heading::Own => fmt(&tr("Stereotypes · {n}"), &[("n", &n)]),
        Heading::End(_) if own.is_empty() => fmt(&tr("Stereotypes of this end · {n}"), &[("n", &n)]),
        Heading::Member | Heading::End(_) => fmt(&tr("Stereotypes of {name} · {n}"), &[("name", &own), ("n", &n)]),
    };
    let add = Rect::from_min_size(pos2(r.right() - PAD - 28.0, y + 11.0), vec2(28.0, 28.0));
    let picking = shell.panels.menu == Some(Menu::StereotypePick) && shell.panels.stereotypes.menu_for == Some(element_id);
    match section(shell, ui, p, r, y, &title, Some((&add_id, "Apply stereotype"))) {
        Some(next) => y = next - 6.0,
        None => {
            shell.panels.menu = if picking { None } else { Some(Menu::StereotypePick) };
            shell.panels.stereotypes.menu_for = Some(element_id);
            shell.panels.stereotypes.filter.clear();
            shell.panels.menu_reached = None;
            y += 44.0;
        }
    }
    let width = (r.width() - 2.0 * PAD).max(0.0);
    let left = r.left() + PAD;
    let mut row_index = 0usize;
    for (s, instance) in &applied {
        y = applied_rows(shell, ui, p, (left, width), y, &prefix, element, element_id, *s, *instance, &mut row_index);
    }
    if let Some((_, why)) = shell.panels.stereotypes.refused.clone().filter(|(e, _)| *e == element_id) {
        y = refusal(shell, ui, p, left, y + 4.0, width, &why, &format!("{prefix}.refusal"));
    }
    let Some(store) = shell.session.as_ref().map(|s| s.doc()) else { return y };
    let said = match applied.first() {
        Some((s, _)) => fmt(&tr(VALUES_HELP), &[("name", &own), ("stereotype", &name(store, *s))]),
        None => {
            let home = profiles::home_of(store, element).map(|p| name(store, p)).unwrap_or_default();
            fmt(&tr(NONE_HELP), &[("package", &home)])
        }
    };
    y = help(ui, p, left, y + 4.0, width, &said);
    // The diagram's choice once, under the panel's own section, not under each end's.
    if !matches!(heading, Heading::End(_)) {
        y = show_values_check(shell, ui, p, left, y);
    }
    if shell.panels.menu == Some(Menu::StereotypePick) && shell.panels.stereotypes.menu_for == Some(element_id) {
        picker(shell, ui, p, add, &add_id, element, element_id);
    }
    y + 6.0
}

/// The Stereotypes section on `owner`'s panel: the selected member's when a member of it is
/// selected (its row marked), else `owner`'s own; none for what takes no stereotype, nor while the
/// model has no stereotype to apply. Gives the y under it.
pub(crate) fn section_for(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect, y: f32, owner: Obj) -> f32 {
    let Some(store) = shell.session.as_ref().map(|s| s.doc()) else { return y };
    let selected = crate::panels::selected_id(shell).and_then(|id| store.by_uuid(id));
    let member = selected.filter(|m| {
        *m != owner
            && store.class_of(*m).is_ok_and(|c| c == archforge_uml::PROPERTY || c == archforge_uml::OPERATION)
            && store.container(*m).ok().flatten().is_some_and(|(c, _)| c == owner)
    });
    let target = member.unwrap_or(owner);
    // Only what works is shown: a model without a stereotype has nothing to apply.
    if !takes(store, target) || (applications::all_stereotypes(store).is_empty() && applications::applied_to(store, target).is_empty()) {
        return y;
    }
    section_of(shell, ui, p, r, y, target, if member.is_some() { Heading::Member } else { Heading::Own })
}

/// The Stereotypes section of association end `end`, the `i`th, in its part of the association's
/// Properties, as an attribute's is; none while the model has no stereotype to apply. Gives the y
/// under it.
pub(crate) fn end_section(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect, y: f32, end: Obj, i: usize) -> f32 {
    let Some(store) = shell.session.as_ref().map(|s| s.doc()) else { return y };
    if !takes(store, end) || (applications::all_stereotypes(store).is_empty() && applications::applied_to(store, end).is_empty()) {
        return y;
    }
    section_of(shell, ui, p, r, y, end, Heading::End(i))
}

/// A muted line of help at `y`, `width` wide; gives the y under it.
fn help(ui: &egui::Ui, p: &Palette, left: f32, y: f32, width: f32, text: &str) -> f32 {
    let paint = ui.painter();
    let g = ux::galley_rows(paint, text, types::CAPTION, p.text_muted, Some(width.max(0.0)), 6);
    let tall = g.size().y;
    paint.galley(pos2(left, y), g, p.text_muted);
    y + tall + 12.0
}

/// A refusal on the danger's tint with its warning; gives the y under it.
#[allow(clippy::too_many_arguments, reason = "a refusal is drawn at its place with its id")]
fn refusal(shell: &mut Shell, ui: &egui::Ui, p: &Palette, left: f32, y: f32, width: f32, why: &str, id: &str) -> f32 {
    let paint = ui.painter().clone();
    let text = ux::galley_rows(&paint, why, types::SMALL, p.text, Some((width - 42.0).max(0.0)), 8);
    let card = Rect::from_min_size(pos2(left, y), vec2(width, text.size().y + 20.0));
    paint.rect_filled(card, 6.0, p.danger_tint);
    ux::icon_in(&paint, Rect::from_min_size(pos2(left + 10.0, y + 11.5), vec2(14.0, 14.0)), &icons::WARNING, p.danger);
    paint.galley(pos2(left + 32.0, y + 10.0), text, p.text);
    crate::widgets::record_visible(ui, &mut shell.registry, id, "text", card);
    card.bottom() + 12.0
}

/// An applied stereotype's row, `«Entity»` and its profile muted, its bin at the right end; then a
/// line per property, its name at the left and its control at the right. Gives the y under them.
#[allow(clippy::too_many_arguments, reason = "the rows are drawn for their element, stereotype and place")]
fn applied_rows(
    shell: &mut Shell,
    ui: &mut egui::Ui,
    p: &Palette,
    (left, width): (f32, f32),
    mut y: f32,
    prefix: &str,
    element: Obj,
    element_id: Uuid,
    s: Obj,
    instance: Obj,
    row_index: &mut usize,
) -> f32 {
    let Some(store) = shell.session.as_ref().map(|x| x.doc()) else { return y };
    let Ok(s_id) = store.uuid(s) else { return y };
    let label = format!("«{}»", name(store, s));
    let profile = profiles::profile_of(store, s).map(|x| name(store, x)).unwrap_or_default();
    let properties: Vec<Obj> = profiles::properties(store, s);
    let key = format!("{prefix}.applied.{s_id}");
    let paint = ui.painter().clone();
    let r = Rect::from_min_size(pos2(left, y), vec2(width, ROW));
    crate::widgets::record_visible(ui, &mut shell.registry, &key, "row", r);
    ui.ctx().accesskit_node_builder(crate::widgets::egui_id(&key), |node| {
        node.set_role(egui::accesskit::Role::Row);
        node.set_label(format!("{label} {profile}"));
    });
    let g = ux::text_mid(&paint, r.left() + 10.0, r.center().y, &label, types::SMALL_STRONG, p.text, Some((width - 140.0).max(0.0)));
    ux::text_mid(
        &paint,
        g.right() + 10.0,
        r.center().y,
        &profile,
        types::CAPTION,
        p.text_muted,
        Some((r.right() - 40.0 - g.right() - 10.0).max(0.0)),
    );
    // The bin takes the stereotype off; its definition stays.
    let bin = Rect::from_center_size(pos2(r.right() - 16.0, r.center().y), vec2(24.0, 24.0));
    ux::icon_in(&paint, Rect::from_center_size(bin.center(), vec2(14.0, 14.0)), &icons::TRASH, p.text_secondary);
    let hint = fmt(&tr("Remove «{stereotype}» from {name}"), &[("stereotype", &name(store, s)), ("name", &name(store, element))]);
    if control(shell, ui, &format!("{key}.remove"), "button", bin, true).on_hover_text(hint.as_str()).accessible_name(hint.as_str()).clicked() {
        match run(shell, "uml.stereotype.unapply", json!({ "element": element_id.to_string(), "stereotype": s_id.to_string() })) {
            Ok(_) => {
                shell.panels.notice = None;
                shell.panels.stereotypes.refused = None;
            }
            Err(e) => shell.panels.stereotypes.refused = Some((element_id, e)),
        }
        return y + ROW;
    }
    y += ROW;
    for property in properties {
        y = value_line(shell, ui, p, (left, width), y, &key, element_id, s_id, instance, property, *row_index);
        *row_index = row_index.saturating_add(1);
    }
    y + 4.0
}

/// The `default` tag beside a value left to its default.
fn default_tag(paint: &egui::Painter, p: &Palette, right: f32, mid: f32) -> f32 {
    let word = tr_ctx("value", "default");
    let g = ux::galley(paint, &word, types::CAPTION, p.text_muted, None);
    let tag = Rect::from_min_size(pos2(right - g.size().x - 12.0, mid - 9.0), vec2(g.size().x + 12.0, 18.0));
    paint.rect_stroke(tag, 4.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
    paint.galley(pos2(tag.left() + 6.0, tag.center().y - g.size().y / 2.0), g, p.text_muted);
    tag.left()
}

/// One property's line: its name at the left, its control at the right, and a refusal under it.
/// A Boolean or an enumeration of one value is a select; any other value is typed. Gives the y
/// under it.
#[allow(clippy::too_many_arguments, reason = "a value's line is drawn for its element, stereotype and property")]
fn value_line(
    shell: &mut Shell,
    ui: &mut egui::Ui,
    p: &Palette,
    (left, width): (f32, f32),
    y: f32,
    row_key: &str,
    element_id: Uuid,
    s_id: Uuid,
    instance: Obj,
    property: Obj,
    index: usize,
) -> f32 {
    let Some(store) = shell.session.as_ref().map(|x| x.doc()) else { return y };
    let Ok(p_id) = store.uuid(property) else { return y };
    let paint = ui.painter().clone();
    let label_w = (width * 0.38).max(0.0);
    let field = Rect::from_min_size(pos2(left + label_w, y + 4.0), vec2((width - label_w).max(0.0), FIELD));
    let p_name = name(store, property);
    ux::text_mid(&paint, left + 10.0, field.center().y, &p_name, types::SMALL, p.text_secondary, Some((label_w - 14.0).max(0.0)));
    let id = format!("{row_key}.value.{p_id}");
    let set = applications::is_set(store, instance, property);
    let typed = applications::value_texts(store, instance, property);
    let default = archforge_uml::modifiers::default_text(store, property);
    let form = applications::form_of(store, property);
    let (_, upper) = archforge_uml::modifiers::bounds(store, property);
    let single = upper == Some(1);
    let choices: Option<Vec<String>> = match form {
        DefaultForm::Boolean if single => Some(vec!["true".into(), "false".into()]),
        DefaultForm::Enumeration(e) if single => Some(archforge_uml::commands::literals(store, e).into_iter().map(|l| name(store, l)).collect()),
        _ => None,
    };
    let mut below = field.bottom() + 6.0;
    match choices {
        Some(choices) => {
            let shown = if set { typed.join(", ") } else { default.clone() };
            paint.rect_filled(field, 6.0, p.field);
            let mut room = field.right() - 28.0;
            if !set && !default.is_empty() {
                room = default_tag(&paint, p, room, field.center().y) - 6.0;
            }
            let ink = if set { p.text } else { p.text_secondary };
            if shown.is_empty() {
                ux::text_mid(&paint, field.left() + 8.0, field.center().y, "—", types::SMALL, p.text_muted, None);
            } else {
                ux::text_mid(&paint, field.left() + 8.0, field.center().y, &shown, types::SMALL, ink, Some((room - field.left() - 8.0).max(0.0)));
            }
            ux::icon_mid(&paint, pos2(field.right() - 14.0, field.center().y), &icons::CHEVRON_DOWN, p.text_secondary);
            let state =
                if set { shown.clone() } else { fmt(&tr("{value} (default)"), &[("value", if default.is_empty() { "—" } else { &default })]) };
            let menu = Menu::StereotypeValue(index);
            let open = shell.panels.menu == Some(menu) && shell.panels.stereotypes.menu_for == Some(element_id);
            if control(shell, ui, &id, "select", field, true).accessible_name(format!("{p_name}: {state}")).clicked() {
                shell.panels.menu = if open { None } else { Some(menu) };
                shell.panels.stereotypes.menu_for = Some(element_id);
                shell.panels.menu_reached = None;
            }
            if open {
                let mut items = vec![if default.is_empty() { tr("No value") } else { fmt(&tr("Default ({value})"), &[("value", &default)]) }];
                items.extend(choices.iter().cloned());
                if let Some(i) = crate::panels::menu(shell, ui, p, field, &id, &items) {
                    let values: Vec<String> = if i == 0 { Vec::new() } else { choices.get(i - 1).cloned().into_iter().collect() };
                    write_values(shell, element_id, s_id, p_id, values, None);
                }
            }
        }
        None => {
            let editing = shell.panels.stereotypes.value.clone().filter(|v| v.element == element_id && v.stereotype == s_id && v.property == p_id);
            let current = typed.join(", ");
            let mut text = editing.as_ref().map_or_else(|| current.clone(), |v| v.text.clone());
            let refused = editing.as_ref().and_then(|v| v.refused.clone());
            paint.rect_filled(field, 6.0, p.field);
            let eid = crate::widgets::egui_id(&id);
            let focused = ui.memory(|m| m.has_focus(eid));
            let mut inner_right = field.right() - 8.0;
            if !set && text.is_empty() && !focused && !default.is_empty() {
                inner_right = default_tag(&paint, p, field.right() - 6.0, field.center().y) - 6.0;
            }
            crate::widgets::record_visible(ui, &mut shell.registry, &id, "field", field);
            let inner = crate::panels::field_inner(ui, field);
            let inner = Rect::from_min_max(inner.min, pos2(inner_right.max(inner.left()), inner.bottom()));
            let hint = if default.is_empty() { "—".to_string() } else { default.clone() };
            let edit = egui::TextEdit::singleline(&mut text)
                .id(eid)
                .frame(egui::Frame::NONE)
                .margin(egui::Margin::ZERO)
                .font(types::SMALL.font())
                .text_color(p.text)
                .hint_text(egui::RichText::new(hint).color(p.text_muted))
                .char_limit(archforge_uml::modifiers::MAX_DEFAULT)
                .desired_width(inner.width());
            let out = edit.show(&mut ui.new_child(egui::UiBuilder::new().max_rect(inner)));
            let state =
                if set { current.clone() } else { fmt(&tr("{value} (default)"), &[("value", if default.is_empty() { "—" } else { &default })]) };
            let _ = (*out.response).clone().accessible_name(format!("{p_name}: {state}"));
            if refused.is_some() {
                paint.rect_stroke(field, 6.0, Stroke::new(1.0, p.danger), StrokeKind::Inside);
            } else if out.response.has_focus() {
                paint.rect_stroke(field, 6.0, Stroke::new(1.0, p.focus), StrokeKind::Inside);
            }
            let enter = (out.response.has_focus() || out.response.lost_focus()) && ui.input(|i| i.key_pressed(egui::Key::Enter));
            let committed = enter || out.response.lost_focus();
            if committed && editing.is_some() {
                let values: Vec<String> = if text.trim().is_empty() {
                    Vec::new()
                } else if single {
                    vec![text.clone()]
                } else {
                    text.split(',').map(|v| v.trim().to_string()).collect()
                };
                if values == typed && refused.is_none() {
                    shell.panels.stereotypes.value = None;
                } else {
                    write_values(shell, element_id, s_id, p_id, values, Some(text));
                }
            } else if text != current || refused.is_some() {
                let changed = editing.as_ref().is_none_or(|v| v.text != text);
                shell.panels.stereotypes.value = Some(ValueEdit {
                    element: element_id,
                    stereotype: s_id,
                    property: p_id,
                    text,
                    refused: if changed && editing.is_some() { None } else { refused.clone() },
                });
            } else if editing.is_some() {
                shell.panels.stereotypes.value = None;
            }
            if let Some(why) = refused {
                let g = ux::galley_rows(&paint, &why, types::CAPTION, p.danger, Some(field.width()), 4);
                let tall = g.size().y;
                paint.galley(pos2(field.left(), field.bottom() + 4.0), g, p.danger);
                crate::widgets::record_visible(
                    ui,
                    &mut shell.registry,
                    &format!("{id}.refusal"),
                    "text",
                    Rect::from_min_size(pos2(field.left(), field.bottom() + 4.0), vec2(field.width(), tall)),
                );
                below = field.bottom() + 4.0 + tall + 6.0;
            }
        }
    }
    below
}

/// Why typed values do not fit, in the user's words.
fn value_refusal(store: &Store, property: Obj, problem: &ValueProblem) -> String {
    let p_name = name(store, property);
    match problem {
        ValueProblem::TooMany => fmt(&tr("{name} holds at most {n} values."), &[("name", &p_name), ("n", &applications::MAX_VALUES.to_string())]),
        ValueProblem::Empty => tr("A value is empty. Clear the field to show the default."),
        ValueProblem::Malformed(DefaultError::TooLong) => {
            fmt(&tr("A value is longer than {n} characters."), &[("n", &archforge_uml::modifiers::MAX_DEFAULT.to_string())])
        }
        ValueProblem::Malformed(DefaultError::Malformed(v, form)) => {
            let accepted = match form {
                DefaultForm::Integer => tr("A whole number, such as 42 or -7."),
                DefaultForm::Real => tr("A number, such as 2.5 or -1e3."),
                DefaultForm::Boolean => tr("true or false."),
                DefaultForm::UnlimitedNatural => tr("A whole number of 0 or more, or *."),
                DefaultForm::String | DefaultForm::Opaque => tr("Any text."),
                DefaultForm::Enumeration(e) => {
                    let names: Vec<String> = archforge_uml::commands::literals(store, *e).into_iter().map(|l| name(store, l)).collect();
                    fmt(&tr("One of {literals}."), &[("literals", &names.join(", "))])
                }
            };
            fmt(&tr("{value} is no value of {name}. {accepted}"), &[("value", v), ("name", &p_name), ("accepted", &accepted)])
        }
        ValueProblem::Count(n, lower, upper) => {
            let bounds = applications::bounds_text(*lower, *upper);
            let given = u64::try_from(*n).unwrap_or(u64::MAX);
            fmt(
                &trn("{name} holds [{bounds}] values; {n} is given.", "{name} holds [{bounds}] values; {n} are given.", given),
                &[("name", &p_name), ("bounds", &bounds)],
            )
        }
        ValueProblem::Twice(v) => fmt(&tr("{name} holds each value once, and {value} is there twice."), &[("name", &p_name), ("value", v)]),
    }
}

/// Writes `values` of property `p_id` on the element's stereotype `s_id` in one undo step, whose
/// command grows the cards that show them on diagrams that show values (AR-241); refused, the text
/// typed stays in its field with the reason, the model untouched.
fn write_values(shell: &mut Shell, element: Uuid, s_id: Uuid, p_id: Uuid, values: Vec<String>, typed: Option<String>) {
    let problem = shell.session.as_ref().and_then(|s| {
        let store = s.doc();
        let property = store.by_uuid(p_id)?;
        applications::read_values(store, property, &values).err().map(|e| value_refusal(store, property, &e))
    });
    if let Some(why) = problem {
        shell.panels.stereotypes.value =
            Some(ValueEdit { element, stereotype: s_id, property: p_id, text: typed.unwrap_or_else(|| values.join(", ")), refused: Some(why) });
        return;
    }
    let params = json!({ "element": element.to_string(), "stereotype": s_id.to_string(), "property": p_id.to_string(), "values": values });
    let Some(session) = shell.session.as_mut() else { return };
    let out = session.execute("uml.stereotype.set-value", &params);
    match out {
        Ok(_) => {
            shell.panels.stereotypes.value = None;
            shell.panels.notice = None;
        }
        Err(e) => {
            shell.panels.stereotypes.value = Some(ValueEdit {
                element,
                stereotype: s_id,
                property: p_id,
                text: typed.unwrap_or_else(|| values.join(", ")),
                refused: Some(e.to_string()),
            });
        }
    }
}

/// The open class diagram, when one is open.
fn open_class_diagram(shell: &Shell) -> Option<(Uuid, Obj)> {
    let id = shell.canvas.active?;
    let store = shell.session.as_ref()?.doc();
    let d = store.by_uuid(id)?;
    let kind = store.get(d, archforge_diagram::diagram::KIND).ok().and_then(|v| v.as_str().map(str::to_string));
    (kind.as_deref() == Some(archforge_uml::CLASS_DIAGRAM)).then_some((id, d))
}

/// Show stereotype values on this diagram, while a class diagram is open: ticked, its cards list
/// the values set on their stereotypes, each diagram its own choice; turned on, the cards that would
/// hide a value grow, in the same undo step. Gives the y under it.
fn show_values_check(shell: &mut Shell, ui: &egui::Ui, p: &Palette, left: f32, y: f32) -> f32 {
    let Some((d_id, d)) = open_class_diagram(shell) else { return y };
    let was = shell.session.as_ref().is_some_and(|s| applications::values_shown(s.doc(), d));
    let mut on = was;
    crate::panels::check(shell, ui, p, pos2(left, y), SHOW_ID, &tr("Show stereotype values on this diagram"), &mut on);
    if on != was
        && let Err(e) = show_values(shell, d_id, on)
    {
        shell.panels.notice = Some(e);
    }
    y + 28.0
}

/// Shows or hides the stereotype values on diagram `d` in one undo step, whose command grows the
/// cards that would hide a value (AR-241).
pub(crate) fn show_values(shell: &mut Shell, d: Uuid, on: bool) -> Result<(), String> {
    let session = shell.session.as_mut().ok_or_else(|| tr("The model could not be opened"))?;
    session
        .execute("uml.stereotype.show-values", &json!({ "diagram": d.to_string(), "show": on }))
        .map(|_| shell.panels.notice = None)
        .map_err(|e| e.to_string())
}

/// The picker under `anchor` (F4): headed `APPLY TO ORDER`, then the filter as typed, the
/// stereotypes that apply and those applied (ticked), a line, and the rest, muted, their short
/// reason at the right and the whole one on hover and for screen readers. Typing filters, the arrows
/// walk the rows that apply, Enter applies, Escape or a press elsewhere closes.
fn picker(shell: &mut Shell, ui: &egui::Ui, p: &Palette, anchor: Rect, add_id: &str, element: Obj, element_id: Uuid) {
    // Typing filters the rows; Backspace takes the last character back.
    let typed: String = ui.input(|i| i.events.iter().filter_map(|e| if let egui::Event::Text(t) = e { Some(t.clone()) } else { None }).collect());
    let back = ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Backspace));
    if !typed.is_empty() || back {
        let f = &mut shell.panels.stereotypes.filter;
        if back {
            f.pop();
        }
        if f.chars().count() < 64 {
            f.push_str(&typed);
        }
        shell.panels.menu_reached = None;
    }
    let Some(store) = shell.session.as_ref().map(|s| s.doc()) else { return };
    let filter = shell.panels.stereotypes.filter.clone();
    let rows = picks(store, element, &filter);
    let heading = fmt(&tr("Apply to {name}"), &[("name", &name(store, element))]);
    let split = rows.iter().position(|r| !r.live && !r.ticked);
    let window = ui.ctx().content_rect();
    let width = 280.0_f32.max(anchor.width()).min((window.width() - 16.0).max(anchor.width()));
    let head = if filter.is_empty() { 28.0 } else { 46.0 };
    let gap = if split.is_some_and(|s| s > 0) { 9.0 } else { 0.0 };
    let empty = if rows.is_empty() { 30.0 } else { 0.0 };
    let full = 8.0 + head + rows.len() as f32 * 30.0 + gap + empty;
    let below = window.bottom() - 8.0 - (anchor.bottom() + 4.0);
    let height = full.min(below.max(8.0 + head + 30.0));
    let left = (anchor.right() - width).min(window.right() - 8.0 - width).max(window.left() + 8.0);
    let m = Rect::from_min_size(pos2(left, anchor.bottom() + 4.0), vec2(width, height));
    let live: Vec<usize> = rows.iter().enumerate().filter(|(_, r)| r.live).map(|(i, _)| i).collect();
    let picked = ux::overlay(ui, crate::widgets::egui_id(&format!("{add_id}.layer")), m, |ui| {
        let paint = ui.painter().clone();
        ux::shadow(&paint, m, 8.0, p.shadow);
        paint.rect_filled(m, 8.0, p.surface);
        paint.rect_stroke(m, 8.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
        ux::text(
            &paint,
            pos2(m.left() + 14.0, m.top() + 12.0),
            &crate::canvas::eyebrow_case(&heading),
            types::EYEBROW,
            p.text_muted,
            Some(m.width() - 28.0),
        );
        if !filter.is_empty() {
            let said = fmt(&tr("Filter: {text}"), &[("text", &filter)]);
            ux::text(&paint, pos2(m.left() + 14.0, m.top() + 30.0), &said, types::CAPTION, p.text_secondary, Some(m.width() - 28.0));
        }
        crate::widgets::record_visible(ui, &mut shell.registry, &format!("{add_id}.menu"), "menu", m);
        let mut reached = shell.panels.menu_reached.as_ref().filter(|(of, _)| of == add_id).map(|(_, i)| *i);
        let (down, up, enter) = ui.input_mut(|i| {
            (
                i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown),
                i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp),
                i.consume_key(egui::Modifiers::NONE, egui::Key::Enter),
            )
        });
        if !live.is_empty() && (down || up) {
            let at = reached.and_then(|r| live.iter().position(|l| *l == r));
            let next = match at {
                None => 0,
                Some(i) if down => (i + 1).min(live.len() - 1),
                Some(i) => i.saturating_sub(1),
            };
            reached = live.get(next).copied();
        }
        // Enter with nothing reached applies the first that applies, as the filter left it.
        let reached_for_enter = reached.or_else(|| live.first().copied());
        shell.panels.menu_reached = reached.map(|i| (add_id.to_string(), i));
        let list = Rect::from_min_max(pos2(m.left(), m.top() + 4.0 + head), pos2(m.right(), m.bottom() - 4.0));
        let clipped = paint.with_clip_rect(list);
        if rows.is_empty() {
            let said = if filter.is_empty() {
                tr("No stereotype yet. Make one in a profile from Models ▸ New ▸ Stereotype.")
            } else {
                tr("No stereotype matches.")
            };
            ux::text_mid(&clipped, list.left() + 14.0, list.top() + 15.0, &said, types::SMALL, p.text_muted, Some(list.width() - 28.0));
        }
        let mut picked = None;
        for (i, row) in rows.iter().enumerate() {
            let past = if split.is_some_and(|s| s > 0 && i >= s) { gap } else { 0.0 };
            if split == Some(i) && i > 0 {
                let line_y = list.top() + i as f32 * 30.0 + gap / 2.0;
                clipped.rect_filled(Rect::from_min_size(pos2(m.left() + 12.0, line_y), vec2(m.width() - 24.0, 1.0)), 0.0, p.border);
            }
            let r = Rect::from_min_size(pos2(m.left() + 4.0, list.top() + i as f32 * 30.0 + past), vec2(m.width() - 8.0, 30.0));
            if !list.intersects(r) {
                continue;
            }
            let rid = format!("{add_id}.{}", row.stereotype);
            let resp = if row.live {
                control(shell, ui, &rid, "menu-item", r.intersect(list), true).accessible_name(format!("{} {}", row.label, row.detail))
            } else {
                crate::widgets::record_visible(ui, &mut shell.registry, &rid, "menu-item", r.intersect(list));
                let resp = ui.interact(r.intersect(list), crate::widgets::egui_id(&rid), Sense::hover());
                let why = row.why.clone().unwrap_or_else(|| row.detail.clone());
                let ticked = row.ticked;
                ui.ctx().accesskit_node_builder(crate::widgets::egui_id(&rid), |node| {
                    node.set_role(egui::accesskit::Role::MenuItem);
                    node.set_disabled();
                    if ticked {
                        node.set_toggled(egui::accesskit::Toggled::True);
                    }
                    node.set_description(why.clone());
                });
                resp.on_hover_text(why).accessible_name(row.label.as_str())
            };
            if row.live && (resp.hovered() || reached == Some(i)) {
                clipped.rect_filled(r, 5.0, p.page);
            } else if row.ticked {
                clipped.rect_filled(r, 5.0, p.accent_tint);
            }
            let ink = if row.ticked {
                p.accent
            } else if row.live {
                p.text
            } else {
                p.text_muted
            };
            let tick = if row.ticked { 22.0 } else { 0.0 };
            let detail = ux::galley(&clipped, &row.detail, types::CAPTION, p.text_muted, Some(m.width() / 2.0));
            let dw = detail.size().x;
            clipped.galley(pos2(r.right() - 10.0 - tick - dw, r.center().y - detail.size().y / 2.0), detail, p.text_muted);
            if row.ticked {
                ux::icon_mid(&clipped, pos2(r.right() - 16.0, r.center().y), &icons::TICK, p.accent);
            }
            ux::text_mid(&clipped, r.left() + 10.0, r.center().y, &row.label, types::SMALL, ink, Some((r.width() - 30.0 - tick - dw).max(0.0)));
            if row.live && (resp.clicked() || (enter && reached_for_enter == Some(i))) {
                picked = Some(row.stereotype);
            }
        }
        picked
    });
    let outside = ui.input(|i| i.pointer.any_pressed() && !i.pointer.interact_pos().is_some_and(|pos| m.contains(pos) || anchor.contains(pos)));
    if picked.is_some() || outside || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        shell.panels.menu = None;
        shell.panels.menu_reached = None;
        shell.panels.stereotypes.filter.clear();
    }
    if let Some(s) = picked {
        match run(shell, "uml.stereotype.apply", json!({ "element": element_id.to_string(), "stereotype": s.to_string() })) {
            Ok(_) => {
                shell.panels.notice = None;
                shell.panels.stereotypes.refused = None;
            }
            Err(e) => shell.panels.stereotypes.refused = Some((element_id, e)),
        }
    }
}

/// What a stereotype application on `element`, and on what it holds and takes along, says in the
/// delete dialog: `«Entity» on Order`, `«Id» on id`, each application once.
pub(crate) fn going_along(store: &Store, element: Obj) -> Vec<String> {
    let mut going = vec![element];
    if store.class_of(element).is_ok_and(archforge_uml::is_classifier) {
        going.extend(archforge_uml::commands::going_with(store, &[element]));
    }
    // A nested package hands what it holds up to its parent: only its own applications go.
    let hands_up = store.class_of(element).ok() == Some(archforge_uml::PACKAGE) && archforge_uml::commands::package_of(store, element).is_some();
    profiles::applications_on(store, &going)
        .into_iter()
        .filter_map(|i| {
            let s = applications::stereotype_of(store, i)?;
            let e = profiles::element_of(store, i).filter(|e| !hands_up || *e == element)?;
            Some(fmt(&tr("«{stereotype}» on {elements}"), &[("stereotype", &name(store, s)), ("elements", &name(store, e))]))
        })
        .collect()
}

/// In the sample class model with Persistence applied and used (F6), the values of F3: Order's
/// table set to `orders`, cached left to its default; the profile Auditing (Auditable, extending
/// Class) applied to Commerce, and Logging (Tracked, extending Class) applied nowhere, so the picker
/// (F4) shows each kind of row; the open diagram showing the values. Gives Order's id.
#[cfg(any(test, feature = "snapshot"))]
pub(crate) fn put_values(shell: &mut Shell) -> Result<Uuid, String> {
    let (commerce, _, entity) = crate::uml_profiles::put_profiles(shell, true)?;
    let session = shell.session.as_mut().ok_or("the sample needs a session")?;
    let order = {
        let st = session.doc();
        let o = st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_uml::CLASS) && name(st, *o) == "Order").ok_or("Order")?;
        st.uuid(o).map_err(|e| e.to_string())?
    };
    let id = |v: serde_json::Value| v.get("id").and_then(|i| i.as_str()).unwrap_or_default().to_string();
    let mut run = |cmd: &str, p: serde_json::Value| session.execute(cmd, &p).map(|o| id(o.value)).map_err(|e| format!("{cmd}: {e}"));
    run("uml.stereotype.set-value", json!({ "element": order.to_string(), "stereotype": entity, "property": "table", "values": ["orders"] }))?;
    let auditing = run("uml.profile.add", json!({ "name": "Auditing" }))?;
    run("uml.stereotype.add", json!({ "profile": auditing, "name": "Auditable", "extends": ["Class"] }))?;
    run("uml.profile.apply", json!({ "package": commerce, "profile": auditing }))?;
    let logging = run("uml.profile.add", json!({ "name": "Logging" }))?;
    run("uml.stereotype.add", json!({ "profile": logging, "name": "Tracked", "extends": ["Class"] }))?;
    if let Some(d) = shell.canvas.active {
        show_values(shell, d, true)?;
    }
    if let Some(s) = shell.session.as_mut() {
        s.mark_saved().map_err(|e| e.to_string())?;
    }
    Ok(order)
}

/// The words this module hands to `tr` by a name rather than a literal, for the catalogue's
/// coverage test: the section's `+` hint and its helps.
#[cfg(test)]
pub(crate) fn texts() -> impl Iterator<Item = &'static str> {
    ["Apply stereotype", VALUES_HELP, NONE_HELP].into_iter()
}
