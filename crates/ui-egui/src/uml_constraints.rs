//! A classifier's or a package's constraints in Properties (AR-133): the section `Constraints · n`
//! with its `+`, a row per constraint reading `name {expression}` with its pencil (a double click
//! opens it too), and the editor: Name and Language (an editable select: plain text, OCL, English
//! or any label typed), the Expression in a field of several lines, the Constrained elements as
//! chips with a `+` that picks more from the model, and, while a class diagram is open, how that
//! diagram shows it, Linked note or Braces. The editor says the text is stored as written and
//! never evaluated. Nothing is written before Apply, which is one undo step: the model's
//! constraint (`uml.constraint.add` or `.set`) with how the open diagram shows it
//! (`uml.constraint.show`). An empty expression is refused under the constrained elements, the
//! field ringed, the editor kept open and the model untouched. The bin removes the constraint, the
//! arrows move it among its context's.

use archforge_engine::{EngineError, Reason};
use archforge_mof::{Obj, Store, Uuid};
use archforge_uml::constraints::{self, Form};
use egui::{Rect, Stroke, StrokeKind, Vec2, pos2, vec2};
use serde_json::json;

use crate::Shell;
use crate::home::control;
use crate::i18n::{fmt, tr};
use crate::panels::{Combo, ComboOut, FIELD, Menu, PAD, ROW, icon_button, section, text_field};
use crate::ux::{self, Palette, icons, types};
use crate::widgets;

/// The longest language typed, and the longest expression.
const MAX_LANGUAGE: usize = 64;
const MAX_EXPRESSION: usize = 16 * 1024;
/// The languages the Language select offers, kept as written like any language typed: they are
/// the model's text, not the interface's, so they are not translated.
const LANGUAGES: [&str; 2] = ["OCL", "English"];

/// A constraint being edited: the constraint, or none for one about to be added to `context`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstraintEdit {
    pub target: Option<Uuid>,
    pub context: Uuid,
    pub name: String,
    pub language: String,
    pub expression: String,
    /// The constrained elements' ids, in order.
    pub elements: Vec<Uuid>,
    /// How the open diagram shows it, and whether the user chose that: until then a new
    /// constraint's form follows its text.
    pub form: Form,
    pub form_chosen: bool,
    pub refused: Option<String>,
    /// Where the refusal sits: under the constrained elements for the expression, under the form for
    /// the diagram, else at the card's foot.
    pub refused_at: Option<&'static str>,
    pub opened: bool,
}

fn name_of(store: &Store, o: Obj) -> String {
    archforge_core::name_of(store, o)
}

/// The class diagram open in the centre.
fn open_diagram(shell: &Shell) -> Option<Obj> {
    let store = shell.session.as_ref()?.doc();
    let d = store.by_uuid(shell.canvas.active?)?;
    let kind = store.get(d, archforge_diagram::diagram::KIND).ok().and_then(|v| v.as_str().map(str::to_string));
    (store.is_alive(d) && kind.as_deref() == Some(archforge_uml::CLASS_DIAGRAM)).then_some(d)
}

/// A fresh name for a new constraint of `context`: `Constraint 1`, or the next free number.
fn fresh_name(store: &Store, context: Obj) -> String {
    let taken: Vec<String> = constraints::constraints_of(store, context).into_iter().map(|c| name_of(store, c).to_lowercase()).collect();
    let word = tr("Constraint");
    (1..=1000).map(|n| format!("{word} {n}")).find(|n| !taken.contains(&n.to_lowercase())).unwrap_or(word)
}

/// Opens the editor on `target`, or on a new constraint of `context`.
pub(crate) fn open(shell: &mut Shell, context: Uuid, target: Option<Uuid>) {
    let Some(store) = shell.session.as_ref().map(|s| s.doc()) else { return };
    let Some(ctx) = store.by_uuid(context) else { return };
    let d = open_diagram(shell);
    let mut edit = ConstraintEdit {
        target,
        context,
        name: fresh_name(store, ctx),
        language: String::new(),
        expression: String::new(),
        elements: vec![context],
        form: Form::Braces,
        form_chosen: false,
        refused: None,
        refused_at: None,
        opened: true,
    };
    if let Some(c) = target.and_then(|t| store.by_uuid(t)) {
        edit.name = name_of(store, c);
        edit.language = constraints::language(store, c);
        edit.expression = constraints::expression(store, c);
        edit.elements = constraints::constrained(store, c).into_iter().filter_map(|e| store.uuid(e).ok()).collect();
        match d.and_then(|d| constraints::shown_on(store, d, c)) {
            Some((_, form)) => {
                edit.form = form;
                edit.form_chosen = true;
            }
            None => edit.form = constraints::suggested_form(store, c),
        }
    }
    shell.panels.constraint = Some(edit);
    shell.panels.menu = None;
}

/// The row a constraint reads as: its name and its expression in braces, on one line.
pub(crate) fn row_text(store: &Store, c: Obj) -> String {
    let braces = constraints::braces(store, c).replace('\n', " ");
    let name = name_of(store, c);
    if name.is_empty() { braces } else { format!("{name} {braces}") }
}

/// The section `Constraints · n` of `context`'s panel at `y`, with its rows and the editor; gives
/// the y under it.
pub(crate) fn section_of(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect, mut y: f32, context: Obj, context_id: Uuid) -> f32 {
    let paint = ui.painter().clone();
    let width = (r.width() - 2.0 * PAD).max(0.0);
    let Some(store) = shell.session.as_ref().map(|s| s.doc()) else { return y };
    let rows: Vec<(Uuid, String)> =
        constraints::constraints_of(store, context).into_iter().filter_map(|c| Some((store.uuid(c).ok()?, row_text(store, c)))).collect();
    let title = fmt(&tr("Constraints · {n}"), &[("n", &rows.len().to_string())]);
    let Some(next) = section(shell, ui, p, r, y, &title, Some(("workspace.properties.constraints.add", "Add constraint"))) else {
        open(shell, context_id, None);
        return y + 50.0;
    };
    y = next - 6.0;
    let editing = |shell: &Shell| shell.panels.constraint.as_ref().filter(|e| e.context == context_id).map(|e| e.target);
    let chosen = crate::panels::selected_id(shell);
    for (id, text) in &rows {
        if editing(shell) == Some(Some(*id)) {
            y = editor(shell, ui, p, r, y);
            continue;
        }
        let row = Rect::from_min_size(pos2(r.left() + PAD, y), vec2(width, ROW));
        let key = format!("workspace.properties.constraint.{id}");
        let resp = control(shell, ui, &key, "row", row, true).accessible_name(text.as_str()).on_hover_text(text.as_str());
        if Some(*id) == chosen {
            paint.rect_filled(row, 6.0, p.accent_tint);
        } else if resp.contains_pointer() {
            paint.rect_filled(row, 6.0, p.page);
        }
        ux::text(&paint, pos2(row.left() + 10.0, row.top() + 9.0), text, types::UI, p.text, Some((width - 50.0).max(0.0)));
        if crate::panels::edit_button(shell, ui, p, row, &key, resp.contains_pointer()) || resp.double_clicked() {
            crate::panels::select_element(shell, *id);
            open(shell, context_id, Some(*id));
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

/// What the element menu offers: every element a constraint may name that the editor does not
/// name yet, as its name reads (a member after its classifier, `Order.total`), with its kind.
fn offered(store: &Store, chosen: &[Uuid]) -> Vec<(Uuid, String, String)> {
    let mut out: Vec<(Uuid, String, String)> = store
        .alive()
        .filter(|o| constraints::is_constrainable(store, *o) && !archforge_uml::members::is_built_in(store, *o))
        .filter_map(|o| {
            let id = store.uuid(o).ok()?;
            if chosen.contains(&id) {
                return None;
            }
            let label = element_label(store, o);
            (!label.is_empty()).then(|| (id, label, kind_word(store, o)))
        })
        .collect();
    out.sort_by_key(|(_, label, _)| label.to_lowercase());
    out
}

/// An element as a chip or the menu names it: a member after its owner, `Order.total`.
fn element_label(store: &Store, o: Obj) -> String {
    let name = name_of(store, o);
    let owner = store.container(o).ok().flatten().map(|(c, _)| c).filter(|c| store.class_of(*c).is_ok_and(archforge_uml::is_classifier));
    match owner {
        Some(c) if !name.is_empty() => format!("{}.{name}", name_of(store, c)),
        _ => name,
    }
}

/// The kind of `o` in a word, as the element menu notes it.
fn kind_word(store: &Store, o: Obj) -> String {
    let Ok(class) = store.class_of(o) else { return String::new() };
    match class {
        c if c == archforge_uml::PACKAGE => tr("Package"),
        c if c == archforge_uml::PROPERTY => tr("Attribute"),
        c if c == archforge_uml::OPERATION => tr("Operation"),
        c if c == archforge_uml::ENUMERATION_LITERAL => tr("Literal"),
        c if c == archforge_uml::PARAMETER => tr("Parameter"),
        c if archforge_uml::is_relationship(c) && !archforge_uml::is_classifier(c) => tr("Relationship"),
        _ => crate::uml_classifier::Classifier::of(store, o).map(crate::uml_classifier::Classifier::word).unwrap_or_default(),
    }
}

/// A field of several lines with its label; gives whether it has the keyboard.
fn expression_field(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, at: egui::Pos2, width: f32, text: &mut String, refused: bool) -> f32 {
    let paint = ui.painter().clone();
    let id = "workspace.properties.constraint.expression";
    ux::text(&paint, at, &tr("Expression"), types::CAPTION, p.text_secondary, Some(width.max(0.0)));
    let lines = text.lines().count().clamp(2, 10) as f32 + if text.ends_with('\n') { 1.0 } else { 0.0 };
    let height = (lines * 17.0 + 16.0).max(56.0);
    let field = Rect::from_min_size(at + vec2(0.0, 17.0), vec2(width.max(0.0), height));
    paint.rect_filled(field, 6.0, p.field);
    widgets::record_visible(ui, &mut shell.registry, id, "field", field);
    let inner = field.shrink2(vec2(8.0, 8.0));
    let label = tr("Expression");
    let out = egui::TextEdit::multiline(text)
        .id(widgets::egui_id(id))
        .frame(egui::Frame::NONE)
        .margin(egui::Margin::ZERO)
        .font(types::SMALL.font())
        .text_color(p.text)
        .char_limit(MAX_EXPRESSION)
        .hint_text(egui::RichText::new(tr("Describe the rule…")).color(p.text_muted).font(types::SMALL.font()))
        .desired_width(inner.width())
        .desired_rows(2)
        .show(&mut ui.new_child(egui::UiBuilder::new().max_rect(inner)));
    let _ = (*out.response).clone().accessible_name(label.as_str());
    let ring = if refused { Some(p.danger) } else { out.response.has_focus().then_some(p.focus) };
    if let Some(ink) = ring {
        paint.rect_stroke(field, 6.0, Stroke::new(1.0, ink), StrokeKind::Inside);
    }
    field.bottom() - at.y
}

/// The constrained elements' chips, each with its ✕ while more than one is left, and the `+` that
/// opens the element menu; gives the height drawn and the `+`'s rectangle.
fn chips(shell: &mut Shell, ui: &egui::Ui, p: &Palette, at: egui::Pos2, width: f32, edit: &mut ConstraintEdit) -> (f32, Rect) {
    let paint = ui.painter().clone();
    ux::text(&paint, at, &tr("Constrained elements"), types::CAPTION, p.text_secondary, Some(width.max(0.0)));
    let labels: Vec<String> = shell
        .session
        .as_ref()
        .map(|s| {
            let store = s.doc();
            edit.elements.iter().map(|id| store.by_uuid(*id).map(|o| element_label(store, o)).unwrap_or_default()).collect()
        })
        .unwrap_or_default();
    let (mut x, mut y) = (at.x, at.y + 19.0);
    let mut gone = None;
    let removable = edit.elements.len() > 1;
    for (i, label) in labels.iter().enumerate() {
        let g = ux::galley(&paint, label, types::SMALL, p.text, Some((width - 40.0).max(0.0)));
        let w = 8.0 + g.size().x + if removable { 22.0 } else { 8.0 };
        if x + w > at.x + width && x > at.x {
            x = at.x;
            y += 29.0;
        }
        let chip = Rect::from_min_size(pos2(x, y + 2.0), vec2(w, 23.0));
        paint.rect_filled(chip, 6.0, p.field);
        let text_y = chip.center().y - g.size().y / 2.0;
        paint.galley(pos2(chip.left() + 8.0, text_y), g, p.text);
        widgets::record_visible(ui, &mut shell.registry, &format!("workspace.properties.constraint.element.{i}"), "chip", chip);
        if removable {
            let b = Rect::from_center_size(pos2(chip.right() - 11.0, chip.center().y), Vec2::splat(16.0));
            ux::icon_in(&paint, b.shrink(4.0), &icons::CLOSE, p.text_secondary);
            let hint = fmt(&tr("Remove {name} from the constrained elements"), &[("name", label)]);
            if control(shell, ui, &format!("workspace.properties.constraint.element.{i}.remove"), "button", b, true)
                .on_hover_text(hint.as_str())
                .accessible_name(hint.as_str())
                .clicked()
            {
                gone = Some(i);
            }
        }
        x += w + 6.0;
    }
    if let Some(i) = gone {
        edit.elements.remove(i);
    }
    if x + FIELD > at.x + width && x > at.x {
        x = at.x;
        y += 29.0;
    }
    let add = Rect::from_min_size(pos2(x, y), Vec2::splat(FIELD));
    ux::icon_in(&paint, Rect::from_center_size(add.center(), Vec2::splat(14.0)), &icons::PLUS, p.text_secondary);
    let hint = tr("Add a constrained element");
    if control(shell, ui, "workspace.properties.constraint.elements.add", "button", add, true)
        .on_hover_text(hint.as_str())
        .accessible_name(hint.as_str())
        .clicked()
    {
        shell.panels.menu = if shell.panels.menu == Some(Menu::ConstraintElement) { None } else { Some(Menu::ConstraintElement) };
    }
    (y + FIELD - at.y, add)
}

/// The segmented control of how the open diagram shows the constraint; gives its height.
fn presentation(shell: &mut Shell, ui: &egui::Ui, p: &Palette, at: egui::Pos2, width: f32, edit: &mut ConstraintEdit) -> f32 {
    let paint = ui.painter().clone();
    ux::text(&paint, at, &tr("On this diagram"), types::CAPTION, p.text_secondary, Some(width.max(0.0)));
    let words = [(Form::Note, tr("Linked note"), "note"), (Form::Braces, tr("Braces"), "braces")];
    let sizes: Vec<f32> = words.iter().map(|(_, w, _)| ux::galley(&paint, w, types::SMALL, p.text, None).size().x + 22.0).collect();
    let total: f32 = sizes.iter().sum::<f32>() + 4.0;
    let frame = Rect::from_min_size(at + vec2(0.0, 17.0), vec2(total.min(width.max(0.0)), FIELD));
    paint.rect_filled(frame, 7.0, p.field);
    let mut x = frame.left() + 2.0;
    for ((form, word, key), w) in words.iter().zip(sizes) {
        let seg = Rect::from_min_size(pos2(x, frame.top() + 2.0), vec2(w, FIELD - 4.0));
        let on = edit.form == *form;
        if on {
            paint.rect_filled(seg, 6.0, p.surface);
            paint.rect_stroke(seg, 6.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
        }
        ux::text_mid(&paint, seg.left() + 11.0, seg.center().y, word, types::SMALL, if on { p.text } else { p.text_secondary }, None);
        let resp = control(shell, ui, &format!("workspace.properties.constraint.show.{key}"), "radio", seg, true).accessible_name(word.as_str());
        ui.ctx().accesskit_node_builder(resp.id, |node| node.set_selected(on));
        if resp.clicked() {
            edit.form = *form;
            edit.form_chosen = true;
            if edit.refused_at == Some("show") {
                edit.refused = None;
                edit.refused_at = None;
            }
        }
        x += w;
    }
    17.0 + FIELD
}

/// The refusal box under a field: the danger's tint, its icon and the reason; gives its height.
fn refusal_box(paint: &egui::Painter, p: &Palette, at: egui::Pos2, width: f32, why: &str) -> f32 {
    let g = ux::galley_rows(paint, why, types::SMALL, p.text, Some((width - 40.0).max(0.0)), 6);
    let r = Rect::from_min_size(at, vec2(width.max(0.0), g.size().y + 20.0));
    paint.rect_filled(r, 6.0, p.danger.gamma_multiply(0.1));
    ux::icon_in(paint, Rect::from_min_size(r.min + vec2(10.0, 11.0), Vec2::splat(14.0)), &icons::ISSUE, p.danger);
    paint.galley(r.min + vec2(32.0, 10.0), g, p.text);
    r.height()
}

/// The constraint editor card; gives the y under it.
fn editor(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect, y: f32) -> f32 {
    let paint = ui.painter().clone();
    let width = (r.width() - 2.0 * PAD).max(0.0);
    let Some(mut edit) = shell.panels.constraint.clone() else { return y };
    let background = paint.add(egui::Shape::Noop);
    let left = r.left() + PAD;
    let inner = (width - 24.0).max(0.0);
    let at = |x: f32, dy: f32| pos2(left + 12.0 + x, y + dy);
    let row = 17.0 + FIELD + 10.0;
    let take = std::mem::take(&mut edit.opened);
    let half = ((inner - 8.0) / 2.0).max(0.0);
    let mut cy = 12.0;
    let mut enter = false;
    enter |= text_field(
        shell,
        ui,
        p,
        at(0.0, cy),
        half,
        "workspace.properties.constraint.name",
        &tr("Name"),
        &mut edit.name,
        archforge_uml::commands::MAX_NAME,
        take,
        false,
    );
    // The language: plain text when empty, a preset picked, or any label typed.
    let current = if edit.language.is_empty() { None } else { LANGUAGES.iter().position(|l| *l == edit.language) };
    let (label, placeholder, pick, pick_name, hint, clear) =
        (tr("Language"), tr("Plain text"), tr("Pick a language"), tr("Pick the constraint's language"), tr("Or type any language"), tr("Plain text"));
    let combo = Combo {
        id: "workspace.properties.constraint.language",
        label: &label,
        name: &label,
        placeholder: &placeholder,
        options: &LANGUAGES,
        current,
        pick: &pick,
        pick_name: &pick_name,
        hint: &hint,
        clear: &clear,
        refused: None,
        menu: Menu::ConstraintLanguage,
        limit: MAX_LANGUAGE,
    };
    match crate::panels::editable_select(shell, ui, p, at(half + 8.0, cy), half, &mut edit.language, &combo) {
        ComboOut::Pick(i) => edit.language = LANGUAGES.get(i).map(|l| (*l).to_string()).unwrap_or_default(),
        ComboOut::Commit => {}
        _ => {}
    }
    cy += row;
    let refused_here = edit.refused_at == Some("expression");
    cy += expression_field(shell, ui, p, at(0.0, cy), inner, &mut edit.expression, refused_here) + 10.0;
    if refused_here && constraints::expression_problem(&edit.expression).is_none() {
        edit.refused = None;
        edit.refused_at = None;
    }
    let (chips_height, add) = chips(shell, ui, p, at(0.0, cy), inner, &mut edit);
    cy += chips_height + 12.0;
    if let (Some(why), Some("expression")) = (edit.refused.clone(), edit.refused_at) {
        cy += refusal_box(&paint, p, at(0.0, cy), inner, &why) + 12.0;
    }
    // A new constraint's form follows its text until the user picks one.
    if !edit.form_chosen {
        edit.form = constraints::form_for(&edit.expression, edit.elements.len());
    }
    let diagram = open_diagram(shell);
    if diagram.is_some() {
        cy += presentation(shell, ui, p, at(0.0, cy), inner, &mut edit) + 10.0;
        if let (Some(why), Some("show")) = (edit.refused.clone(), edit.refused_at) {
            cy += refusal_box(&paint, p, at(0.0, cy), inner, &why) + 10.0;
        }
    }
    let said = ux::galley_rows(
        &paint,
        &tr("Stored as written. Archforge keeps the text and its references but does not evaluate it: nothing is checked against the model."),
        types::CAPTION,
        p.text_secondary,
        Some(inner),
        6,
    );
    let said_height = said.size().y;
    paint.galley(at(0.0, cy), said, p.text_secondary);
    cy += said_height + 10.0;
    if diagram.is_some() {
        let g = ux::galley_rows(
            &paint,
            &tr("Long or multi-element constraints read best as a linked note; the choice is per diagram."),
            types::CAPTION,
            p.text_muted,
            Some(inner),
            6,
        );
        let h = g.size().y;
        paint.galley(at(0.0, cy), g, p.text_muted);
        cy += h + 12.0;
    }
    let (mut remove, mut up, mut down) = (false, false, false);
    if edit.target.is_some() {
        remove = icon_button(shell, ui, p, at(0.0, cy), "workspace.properties.constraint.remove", &icons::TRASH, p.danger, None, "Remove");
        up = icon_button(shell, ui, p, at(FIELD + 8.0, cy), "workspace.properties.constraint.up", &icons::MOVE_UP, p.text_secondary, None, "Move up");
        down = icon_button(
            shell,
            ui,
            p,
            at(2.0 * (FIELD + 8.0), cy),
            "workspace.properties.constraint.down",
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
        "workspace.properties.constraint.cancel",
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
        "workspace.properties.constraint.apply",
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
    // The element menu last, over the card.
    if shell.panels.menu == Some(Menu::ConstraintElement)
        && let Some(store) = shell.session.as_ref().map(|s| s.doc())
    {
        let choices = offered(store, &edit.elements);
        let items: Vec<String> = choices.iter().map(|(_, l, _)| l.clone()).collect();
        let notes: Vec<String> = choices.iter().map(|(_, _, k)| k.clone()).collect();
        if items.is_empty() {
            shell.panels.menu = None;
        } else if let Some(i) = crate::panels::menu_noted(shell, ui, p, add, "workspace.properties.constraint.elements.menu", &items, &notes, &[])
            && let Some((id, _, _)) = choices.get(i)
        {
            edit.elements.push(*id);
        }
    }
    shell.panels.constraint = Some(edit.clone());
    // Enter in the name writes; in the expression it is a new line.
    if cancel || (shell.panels.menu.is_none() && ui.input(|i| i.key_pressed(egui::Key::Escape))) {
        shell.panels.constraint = None;
    } else if remove {
        remove_constraint(shell, &edit);
    } else if up || down {
        move_constraint(shell, &edit, up);
    } else if apply || enter {
        commit(shell, &edit);
    }
    card.bottom() + 8.0
}

fn refuse(shell: &mut Shell, why: String, at: Option<&'static str>) {
    if let Some(e) = shell.panels.constraint.as_mut() {
        e.refused = Some(why);
        e.refused_at = at;
    }
}

/// Selects the context again once the editor has done its work.
fn reselect(shell: &mut Shell, context: Uuid) {
    shell.panels.constraint = None;
    let _ = crate::panels::run(shell, "view.select", json!({ "ids": [context.to_string()] }));
}

fn remove_constraint(shell: &mut Shell, edit: &ConstraintEdit) {
    let target = edit.target.map(|t| t.to_string()).unwrap_or_default();
    match crate::panels::run(shell, "element.remove", json!({ "ids": [target] })) {
        Ok(_) => reselect(shell, edit.context),
        Err(e) => refuse(shell, e, None),
    }
}

fn move_constraint(shell: &mut Shell, edit: &ConstraintEdit, up: bool) {
    let Some(target) = edit.target else { return };
    let Some(store) = shell.session.as_ref().map(|s| s.doc()) else { return };
    let (Some(c), Some(ctx)) = (store.by_uuid(target), store.by_uuid(edit.context)) else { return };
    let all = constraints::constraints_of(store, ctx);
    let Some(i) = all.iter().position(|x| *x == c) else { return };
    let to = if up { i.checked_sub(1) } else { Some(i + 1).filter(|j| *j < all.len()) };
    let Some(to) = to else { return };
    if let Err(e) = crate::panels::run(shell, "uml.constraint.move", json!({ "constraint": target.to_string(), "at": to })) {
        refuse(shell, e, None);
    }
}

/// Writes the editor as one undo step: the constraint, and how the open diagram shows it (a new
/// one always, one already there or one whose form was picked), or says why not.
fn commit(shell: &mut Shell, edit: &ConstraintEdit) {
    if constraints::expression_problem(&edit.expression).is_some() {
        refuse(shell, tr("Type an expression. A constraint without one states no rule."), Some("expression"));
        return;
    }
    let d = open_diagram(shell);
    let Some(store) = shell.session.as_ref().map(|s| s.doc()) else { return };
    let shown = edit.target.and_then(|t| store.by_uuid(t)).zip(d).and_then(|(c, d)| constraints::shown_on(store, d, c));
    let show = d.filter(|_| edit.target.is_none() || shown.is_some() || edit.form_chosen).and_then(|d| store.uuid(d).ok());
    let elements: Vec<String> = edit.elements.iter().map(Uuid::to_string).collect();
    let fields = json!({
        "name": edit.name,
        "expression": edit.expression,
        "language": edit.language,
        "constrained": elements,
    });
    let (target, context, form) = (edit.target, edit.context, edit.form);
    let label = if target.is_some() { tr("Edit constraint") } else { tr("Add constraint") };
    let Some(session) = shell.session.as_mut() else { return };
    let result = session.batch(&label, |b| {
        let mut params = fields.clone();
        let id = match target {
            Some(t) => {
                params["constraint"] = json!(t.to_string());
                b.execute("uml.constraint.set", &params)?;
                t.to_string()
            }
            None => {
                params["context"] = json!(context.to_string());
                b.execute("uml.constraint.add", &params)?["id"].as_str().unwrap_or_default().to_string()
            }
        };
        if let Some(d) = show {
            b.execute("uml.constraint.show", &json!({ "diagram": d.to_string(), "constraint": id, "as": form.word() }))?;
        }
        Ok(())
    });
    match result {
        Ok(_) => reselect(shell, context),
        Err(e) => {
            // Braces with nothing to sit by are said under the form's buttons, any other refusal under Apply.
            let braces = matches!(&e, EngineError::Failed { error, .. } if error.reason == Some(Reason::BracesNoElement));
            refuse(shell, crate::panels::refusal(&e), braces.then_some("show"));
        }
    }
}

/// Drops an editor whose context or constraint is gone.
pub(crate) fn retain_valid(edit: &mut Option<ConstraintEdit>, store: &Store) {
    if edit.as_ref().is_some_and(|e| store.by_uuid(e.context).is_none() || e.target.is_some_and(|t| store.by_uuid(t).is_none())) {
        *edit = None;
    }
}

/// What a removal says of the constraints it changes, after `lead` (`Payment deleted`): the one it
/// changed and what it constrains now, the one that went with it, or how many changed; nothing
/// when it changes none (AR-133).
pub(crate) fn dropped_text(store: &Store, dropped: &[constraints::Dropped]) -> Option<String> {
    let name = |c: Obj| {
        let n = name_of(store, c);
        if n.is_empty() { tr("a constraint") } else { n }
    };
    match dropped {
        [] => None,
        [one] if one.remaining.is_empty() => {
            Some(fmt(&tr("{constraint} constrained nothing else and went with it"), &[("constraint", &name(one.constraint))]))
        }
        [one] => {
            let rest: Vec<String> = one.remaining.iter().map(|e| element_label(store, *e)).collect();
            Some(fmt(&tr("{constraint} now constrains {elements} only"), &[("constraint", &name(one.constraint)), ("elements", &rest.join(", "))]))
        }
        many => Some(crate::i18n::trn("{n} constraint changed", "{n} constraints changed", u64::try_from(many.len()).unwrap_or(u64::MAX))),
    }
}

/// The ids a removal names, as `element.remove` takes them, worked out into what it does to the
/// constraints; to be said after the removal by [`say_dropped`].
pub(crate) fn before_removal(shell: &Shell, ids: &[String]) -> Option<String> {
    let store = shell.session.as_ref()?.doc();
    let objs: Vec<Obj> = ids.iter().filter_map(|i| Uuid::parse(i).and_then(|u| store.by_uuid(u))).collect();
    dropped_text(store, &constraints::dropped_by(store, &objs))
}

/// Says what a removal that went through did to the constraints: after the toast it made, or in a
/// toast of its own led by `lead`, with Undo.
pub(crate) fn say_dropped(shell: &mut Shell, said: Option<String>, lead: &str) {
    let Some(said) = said else { return };
    let undo_label = shell.session.as_ref().and_then(|s| s.undo_label().map(str::to_string));
    let text = match shell.canvas.toast.take() {
        Some(t) if t.until > shell.frames => format!("{} · {said}", t.text),
        _ => format!("{lead} · {said}"),
    };
    shell.canvas.toast = Some(crate::connect::Toast { text, until: shell.frames.saturating_add(400), undo_label });
}
