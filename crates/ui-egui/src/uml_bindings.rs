//! Template bindings in Properties and on the canvas (AR-131, design page 08, frames C2 and C3).
//!
//! Drawing: the Relationship tool's Template binding is dragged from the bound class to the
//! template. Releasing it on a template commits nothing: a draft opens in Properties, headed New
//! template binding, with its Bound element and Template, and `Substitutions · n`, one row per
//! parameter with a selector of the arguments that fit it, a parameter with a default showing it
//! with a `default` tag. While the draft is open its line is drawn dashed in the accent colour
//! between the two cards. Apply adds the binding, its substitutions, its line and its label as one
//! undo step and selects it; a parameter without a default and without an argument keeps the
//! draft open and is named on the danger tint ("T needs an argument: pick a class."). Cancel or
//! Escape drops the draft and leaves the model and its history as they were, and so does selecting
//! something else.
//!
//! A binding selected shows its Bound element and Template as selects, and its substitutions as
//! selectors, each written as one undo step when picked (`uml.template-binding.set`); another
//! template opens the draft over the binding, its arguments to pick before Apply.

use archforge_mof::{Obj, Store, Uuid};
use archforge_uml::bindings;
use archforge_uml::templates::{self, Kind};
use egui::{Pos2, Rect, Stroke, StrokeKind, Vec2, pos2, vec2};
use serde_json::{Map, Value as Json, json};

use crate::Shell;
use crate::i18n::{fmt, tr, trn};
use crate::panels::{FIELD, Menu, PAD, Subject, run, section, select_field};
use crate::ux::{self, Palette, icons, types};

/// The ids' stem: `workspace.properties.templateBinding.bound`, `.template`,
/// `.substitution.<k>.argument`, `.cancel`, `.apply`, `.refusal`.
const ID: &str = "workspace.properties.templateBinding";

/// The draft's help under its buttons, and the binding's under its substitutions: too long for
/// one line of code, so the catalogue test reads them through [`texts`].
const DRAFT_HELP: &str = "Releasing the line on a template opens this. The line stays dashed in the accent colour until Apply; one undo step then adds the binding with its arguments. Cancel or Esc removes the draft and leaves nothing.";
const SELECTOR_HELP: &str = "Each selector lists only the arguments that fit its parameter: its kind, and its constraint or what specialises it. A parameter on its default follows the template's default.";
const LINE_HELP: &str = "The line reads «bind» with each parameter and its argument, and points at the template. The binding keeps its arguments in the model; it is neither a dependency nor a realization.";

/// A binding before it is written: a new one drawn on the canvas, or another template picked for an
/// existing one. Nothing reaches the model until Apply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingDraft {
    /// The binding whose template is being changed, or none for a new one.
    pub binding: Option<Uuid>,
    /// The diagram and the two nodes the new binding's line joins, from the bound element's to the
    /// template's.
    pub diagram: Option<Uuid>,
    pub nodes: Option<(Uuid, Uuid)>,
    pub bound: Uuid,
    pub template: Uuid,
    /// Each parameter of the template with the argument picked, as the commands take it (a built-in
    /// type's name or a classifier's id), empty for none or the default.
    pub args: Vec<(Uuid, String)>,
    /// Why Apply was refused, shown on the danger tint until the arguments change.
    pub refused: Option<String>,
    /// The selection the draft was opened over: selecting anything else drops it.
    pub opened_over: Option<Uuid>,
}

fn name_of(store: &Store, o: Obj) -> String {
    archforge_core::name_of(store, o)
}

/// The template's parameters, each with no argument yet.
fn empty_args(store: &Store, template: Obj) -> Vec<(Uuid, String)> {
    templates::parameters(store, template).into_iter().filter_map(|p| Some((store.uuid(p).ok()?, String::new()))).collect()
}

/// Opens the draft of a new binding from the class of `nodes.0` to the template of `nodes.1` on
/// diagram `d`, in Properties.
pub(crate) fn open_new(shell: &mut Shell, d: Obj, nodes: (Uuid, Uuid), bound: Obj, template: Obj) {
    let Some(store) = shell.session.as_ref().map(|s| s.doc()) else { return };
    let (Ok(b), Ok(t)) = (store.uuid(bound), store.uuid(template)) else { return };
    let draft = BindingDraft {
        binding: None,
        diagram: store.uuid(d).ok(),
        nodes: Some(nodes),
        bound: b,
        template: t,
        args: empty_args(store, template),
        refused: None,
        opened_over: crate::panels::selected_id(shell),
    };
    shell.panels.binding = Some(draft);
    shell.panels.menu = None;
    shell.state.show_panel(crate::state::Panel::Properties);
}

/// Opens the draft over binding `b` for another template, `template`, its arguments to pick.
fn open_for_template(shell: &mut Shell, b: Obj, template: Obj) {
    let Some(store) = shell.session.as_ref().map(|s| s.doc()) else { return };
    let (Ok(id), Ok(t)) = (store.uuid(b), store.uuid(template)) else { return };
    let Some(bound) = templates::bound_element(store, b).and_then(|o| store.uuid(o).ok()) else { return };
    shell.panels.binding = Some(BindingDraft {
        binding: Some(id),
        diagram: None,
        nodes: None,
        bound,
        template: t,
        args: empty_args(store, template),
        refused: None,
        opened_over: Some(id),
    });
}

/// The line of a new binding's draft on diagram `d`, from the bound class's node to the template's:
/// the nodes it joins, while one is open there.
pub(crate) fn draft_line(shell: &Shell, d: Obj) -> Option<(Uuid, Uuid)> {
    let draft = shell.panels.binding.as_ref().filter(|b| b.binding.is_none())?;
    let store = shell.session.as_ref()?.doc();
    (draft.diagram == store.uuid(d).ok()).then_some(draft.nodes).flatten()
}

/// Draws the draft's line between the rectangles `from` and `to` on screen: dashed in the accent
/// colour, an open arrow at the template, as the binding will draw once applied.
pub(crate) fn paint_draft(paint: &egui::Painter, p: &Palette, from: Rect, to: Rect, z: f32) {
    let start = crate::canvas::exit_point(from, to.center());
    let end = crate::canvas::exit_point(to, from.center());
    let stroke = Stroke::new((1.2 * z).clamp(1.0, 2.5), p.accent);
    paint.extend(egui::Shape::dashed_line(&[start, end], stroke, 5.0 * z.max(0.5), 4.0 * z.max(0.5)));
    crate::canvas::line_mark(paint, archforge_diagram::figure::EndMark::OpenArrow, end, start, stroke, z, p.surface);
}

/// The argument a parameter shows, as the commands take it: the draft's pick, or the binding's
/// substitution (empty for its default).
fn value_now(store: &Store, binding: Option<Obj>, draft: Option<&BindingDraft>, p: Obj) -> String {
    if let Some(d) = draft {
        let id = store.uuid(p).ok();
        return d.args.iter().find(|(q, _)| Some(*q) == id).map(|(_, v)| v.clone()).unwrap_or_default();
    }
    let actual = binding.and_then(|b| bindings::substitution_for(store, b, p)).and_then(|s| templates::actual_of(store, s));
    crate::uml_templates::value_of(store, actual)
}

/// An argument of `kind`, in the user's words, as the placeholder of a parameter still to be given
/// one says it.
fn choose(kind: Option<Kind>) -> String {
    match kind {
        Some(Kind::Interface) => tr("Choose an interface"),
        Some(Kind::DataType) => tr("Choose a data type"),
        Some(Kind::Enumeration) => tr("Choose an enumeration"),
        Some(Kind::PrimitiveType) => tr("Choose a primitive type"),
        _ => tr("Choose a class"),
    }
}

/// A parameter's kind as its row says it after its name: class, data type.
fn kind_note(kind: Option<Kind>) -> String {
    match kind {
        Some(Kind::Interface) => tr("interface"),
        Some(Kind::DataType) => tr("data type"),
        Some(Kind::Enumeration) => tr("enumeration"),
        Some(Kind::PrimitiveType) => tr("primitive type"),
        _ => tr("class"),
    }
}

/// What a parameter still needs, in the user's words: `T needs an argument: pick a class.`
pub(crate) fn needs(name: &str, kind: Option<Kind>) -> String {
    let line = match kind {
        Some(Kind::Interface) => tr("{name} needs an argument: pick an interface."),
        Some(Kind::DataType) => tr("{name} needs an argument: pick a data type."),
        Some(Kind::Enumeration) => tr("{name} needs an argument: pick an enumeration."),
        Some(Kind::PrimitiveType) => tr("{name} needs an argument: pick a primitive type."),
        _ => tr("{name} needs an argument: pick a class."),
    };
    fmt(&line, &[("name", name)])
}

/// What the parameters `params`, each with its kind, need, none having a default, in the user's
/// words: one as [`needs`] says it, several listed with their kinds, `T (class) and K (data type)
/// have no default, so each needs an argument.`; None for none.
pub(crate) fn lacking(params: &[(&str, Kind)]) -> Option<String> {
    match params {
        [] => None,
        [(name, kind)] => Some(needs(name, Some(*kind))),
        _ => {
            let named: Vec<String> =
                params.iter().map(|(name, kind)| fmt(&tr("{name} ({kind})"), &[("name", name), ("kind", &kind_note(Some(*kind)))])).collect();
            Some(fmt(&tr("{names} have no default, so each needs an argument."), &[("names", &crate::uml_sets::and_list(&named))]))
        }
    }
}

/// The arguments parameter `p` may take, each as it reads and as the commands take it: Default
/// (its default) first when it has one, then the classifiers of its kind that fit its constraint,
/// the built-in types among them where its kind takes them.
fn argument_choices(store: &Store, p: Obj) -> Vec<(String, String)> {
    let Some(kind) = templates::kind_of(store, p) else { return Vec::new() };
    let mut out: Vec<(String, String)> = Vec::new();
    if let Some(d) = templates::default_of(store, p) {
        out.push((fmt(&tr("Default ({name})"), &[("name", &name_of(store, d))]), String::new()));
    }
    out.extend(
        crate::uml_templates::of_kind(store, kind)
            .into_iter()
            .filter(|(_, v)| bindings::arg_of(store, "", v).is_ok_and(|a| bindings::arg_fits(store, p, a))),
    );
    out
}

/// The classifiers that may bind `template` in place of the bound element of binding `b`.
fn bound_choices(store: &Store, b: Obj, template: Obj) -> Vec<Obj> {
    let mut out: Vec<Obj> = store
        .alive()
        .filter(|c| store.class_of(*c).is_ok_and(bindings::can_bind))
        .filter(|c| bindings::problem(store, *c, template, Some(b)).is_none() || templates::bound_element(store, b) == Some(*c))
        .collect();
    out.sort_by_key(|c| name_of(store, *c).to_lowercase());
    out
}

/// The templates `bound` may bind in place of binding `b`'s.
fn template_choices(store: &Store, b: Obj, bound: Obj) -> Vec<Obj> {
    let mut out: Vec<Obj> = store
        .alive()
        .filter(|t| templates::is_template(store, *t))
        .filter(|t| bindings::problem(store, bound, *t, Some(b)).is_none() || bindings::template_of(store, b) == Some(*t))
        .collect();
    out.sort_by_key(|c| name_of(store, *c).to_lowercase());
    out
}

/// A read-only field with its label, as the draft shows the two ends its line joins.
fn fixed_field(shell: &mut Shell, ui: &egui::Ui, p: &Palette, at: Pos2, width: f32, id: &str, label: &str, value: &str) {
    let paint = ui.painter();
    ux::text(paint, at, label, types::CAPTION, p.text_secondary, Some(width.max(0.0)));
    let field = Rect::from_min_size(at + vec2(0.0, 17.0), vec2(width.max(0.0), FIELD));
    paint.rect_filled(field, 6.0, p.field);
    ux::text_mid(paint, field.left() + 8.0, field.center().y, value, types::SMALL, p.text, Some((width - 16.0).max(0.0)));
    let _ = crate::home::control(shell, ui, id, "field", field, false).accessible_name(format!("{label}: {value}"));
}

/// The refusal on the danger tint, with its icon; gives its height.
fn refusal_box(shell: &mut Shell, ui: &egui::Ui, p: &Palette, at: Pos2, width: f32, why: &str) -> f32 {
    let paint = ui.painter();
    let g = ux::galley_rows(paint, why, types::SMALL, p.text, Some((width - 42.0).max(0.0)), 8);
    let card = Rect::from_min_size(at, vec2(width.max(0.0), g.size().y + 20.0));
    paint.rect_filled(card, 6.0, p.danger_tint);
    ux::icon_in(paint, Rect::from_min_size(card.min + vec2(10.0, 11.5), Vec2::splat(14.0)), &icons::WARNING, p.danger);
    paint.galley(card.min + vec2(32.0, 10.0), g, p.text);
    crate::widgets::record_visible(ui, &mut shell.registry, &format!("{ID}.refusal"), "text", card);
    card.height()
}

/// What the draft still needs, in the user's words, or none: each parameter without a default and
/// without an argument named, and those that may keep their defaults.
fn missing_words(store: &Store, draft: &BindingDraft) -> Option<String> {
    let template = store.by_uuid(draft.template)?;
    let params = templates::parameters(store, template);
    let picked = |p: Obj| store.uuid(p).ok().is_some_and(|id| draft.args.iter().any(|(q, v)| *q == id && !v.is_empty()));
    let lacking: Vec<String> = params
        .iter()
        .filter(|p| templates::default_of(store, **p).is_none() && !picked(**p))
        .map(|p| needs(&templates::parameter_name(store, *p), templates::kind_of(store, *p)))
        .collect();
    if lacking.is_empty() {
        return None;
    }
    let defaults: Vec<String> =
        params.iter().filter(|p| templates::default_of(store, **p).is_some() && !picked(**p)).map(|p| templates::parameter_name(store, *p)).collect();
    let mut said = lacking.join(" ");
    if !defaults.is_empty() {
        let n = u64::try_from(defaults.len()).unwrap_or(u64::MAX);
        said.push(' ');
        said.push_str(&fmt(&trn("{names} can keep its default.", "{names} can keep their defaults.", n), &[("names", &defaults.join(", "))]));
    }
    Some(said)
}

/// The Binding section and the substitutions of a binding (`binding`) or of the open draft; gives
/// the y under them.
#[allow(clippy::too_many_arguments, reason = "the section is drawn into the panel for one binding or its draft")]
fn sections(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect, y: f32, binding: Option<Obj>, draft: bool, heading: bool) -> f32 {
    let paint = ui.painter().clone();
    let width = (r.width() - 2.0 * PAD).max(0.0);
    let half = ((width - 8.0) / 2.0).max(0.0);
    let left = r.left() + PAD;
    let Some(store) = shell.session.as_ref().map(|s| s.doc()) else { return y };
    let d = if draft { shell.panels.binding.clone() } else { None };
    let (bound, template) = match (&d, binding) {
        (Some(d), _) => (store.by_uuid(d.bound), store.by_uuid(d.template)),
        (None, Some(b)) => (templates::bound_element(store, b), bindings::template_of(store, b)),
        _ => (None, None),
    };
    let (Some(bound), Some(template)) = (bound, template) else { return y };
    let params = templates::parameters(store, template);
    // Each parameter with its name, kind, argument as the commands take it, default and what its
    // selector shows, read before anything is drawn.
    let rows: Vec<(Obj, String, Option<Kind>, String, Option<String>, String)> = params
        .iter()
        .map(|q| {
            let value = value_now(store, binding, d.as_ref(), *q);
            let default = templates::default_of(store, *q).map(|x| name_of(store, x));
            let kind = templates::kind_of(store, *q);
            let shown = if value.is_empty() {
                default.clone().unwrap_or_else(|| choose(kind))
            } else {
                crate::uml_templates::classifier_named(store, &value).map_or_else(|| value.clone(), |c| name_of(store, c))
            };
            (*q, templates::parameter_name(store, *q), kind, value, default, shown)
        })
        .collect();
    let current_template = binding.and_then(|b| bindings::template_of(store, b));
    let (bound_name, template_name) = (name_of(store, bound), name_of(store, template));
    // Properties titles a binding's own panel Binding; the draft's heading is its own.
    let mut y = if heading { section(shell, ui, p, r, y, &tr("Binding"), None).unwrap_or(y + 50.0) } else { y };
    let (bound_id, template_id) = (format!("{ID}.bound"), format!("{ID}.template"));
    // A new binding's ends are the line just drawn; an existing one's are picked here.
    let (mut bound_field, mut template_field) = (None, None);
    if d.as_ref().is_some_and(|d| d.binding.is_none()) {
        fixed_field(shell, ui, p, pos2(left, y), half, &bound_id, &tr("Bound element"), &bound_name);
        fixed_field(shell, ui, p, pos2(left + half + 8.0, y), half, &template_id, &tr("Template"), &template_name);
    } else {
        bound_field = Some(select_field(shell, ui, p, pos2(left, y), half, &bound_id, &tr("Bound element"), &bound_name, Menu::BindingBound));
        template_field =
            Some(select_field(shell, ui, p, pos2(left + half + 8.0, y), half, &template_id, &tr("Template"), &template_name, Menu::BindingTemplate));
    }
    y += 17.0 + FIELD + 16.0;
    let title = fmt(&tr("Substitutions · {n}"), &[("n", &rows.len().to_string())]);
    y = section(shell, ui, p, r, y, &title, None).unwrap_or(y + 50.0);
    let refused_now = d.as_ref().and_then(|d| d.refused.clone());
    let mut fields: Vec<Rect> = Vec::new();
    // The selectors take what the parameters' names leave, at least half the width.
    let wide = (width - 96.0).max(half);
    let narrow = (width - wide - 8.0).max(0.0);
    for (k, (_, name, kind, value, default, shown)) in rows.iter().enumerate() {
        let row_top = y;
        let g = ux::text(&paint, pos2(left, row_top + 4.0), name, types::UI_STRONG, p.text, Some(narrow));
        ux::text(
            &paint,
            pos2(g.right() + 6.0, row_top + 6.0),
            &kind_note(*kind),
            types::CAPTION,
            p.text_muted,
            Some((narrow - g.width() - 6.0).max(0.0)),
        );
        let field = Rect::from_min_size(pos2(left + narrow + 8.0, row_top), vec2(wide, FIELD));
        let on_default = value.is_empty() && default.is_some();
        let lacking = value.is_empty() && default.is_none();
        paint.rect_filled(field, 6.0, p.field);
        if lacking && refused_now.is_some() {
            paint.rect_stroke(field, 6.0, Stroke::new(1.0, p.danger), StrokeKind::Inside);
        }
        let tag = on_default.then(|| ux::galley(&paint, &tr("default"), types::CAPTION, p.text_secondary, None));
        let tag_w = tag.as_ref().map_or(0.0, |t| t.size().x + 12.0);
        let ink = if lacking { p.text_secondary } else { p.text };
        ux::text_mid(&paint, field.left() + 8.0, field.center().y, shown, types::SMALL, ink, Some((wide - 36.0 - tag_w).max(0.0)));
        if let Some(t) = tag {
            let pill = Rect::from_min_size(pos2(field.right() - 28.0 - tag_w, field.center().y - 9.0), vec2(tag_w, 18.0));
            paint.rect_filled(pill, 4.0, p.surface);
            paint.rect_stroke(pill, 4.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
            paint.galley(pos2(pill.left() + 6.0, pill.center().y - t.size().y / 2.0), t, p.text_secondary);
        }
        ux::icon_mid(&paint, pos2(field.right() - 14.0, field.center().y), &icons::CHEVRON_DOWN, p.text_secondary);
        let id = format!("{ID}.substitution.{k}.argument");
        let said = if on_default { format!("{name}: {shown} ({})", tr("default")) } else { format!("{name}: {shown}") };
        let resp = crate::home::control(shell, ui, &id, "select", field, true).accessible_name(said);
        let resp = match default {
            Some(dflt) => resp.on_hover_text(fmt(&tr("Default ({name})"), &[("name", dflt)])),
            None => resp,
        };
        if resp.clicked() {
            let m = Menu::BindingArgument(k);
            shell.panels.menu = if shell.panels.menu == Some(m) { None } else { Some(m) };
        }
        fields.push(field);
        y += FIELD + 12.0;
    }
    // The menus over everything drawn so far.
    let store_now = shell.session.as_ref().map(|s| s.doc());
    if let (Some(Menu::BindingArgument(k)), Some(store)) = (shell.panels.menu, store_now)
        && let (Some((q, ..)), Some(field)) = (rows.get(k), fields.get(k))
    {
        let choices = argument_choices(store, *q);
        let items: Vec<String> = choices.iter().map(|(l, _)| l.clone()).collect();
        let id = format!("{ID}.substitution.{k}.argument");
        if let Some((_, v)) = crate::panels::menu(shell, ui, p, *field, &id, &items).and_then(|i| choices.get(i)).cloned() {
            pick_argument(shell, binding, *q, v);
        }
    }
    if let (Some(Menu::BindingBound), Some(field), Some(b), Some(store)) =
        (shell.panels.menu, bound_field, binding, shell.session.as_ref().map(|s| s.doc()))
    {
        let choices = bound_choices(store, b, template);
        let items: Vec<String> = choices.iter().map(|c| name_of(store, *c)).collect();
        if let Some(c) = crate::panels::menu(shell, ui, p, field, &bound_id, &items).and_then(|i| choices.get(i)).copied() {
            set_bound(shell, b, c);
        }
    }
    if let (Some(Menu::BindingTemplate), Some(field), Some(b), Some(store)) =
        (shell.panels.menu, template_field, binding, shell.session.as_ref().map(|s| s.doc()))
    {
        let choices = template_choices(store, b, bound);
        let items: Vec<String> = choices.iter().map(|c| name_of(store, *c)).collect();
        if let Some(t) = crate::panels::menu(shell, ui, p, field, &template_id, &items).and_then(|i| choices.get(i)).copied() {
            if Some(t) == current_template {
                shell.panels.binding = None;
            } else {
                open_for_template(shell, b, t);
            }
        }
    }
    y
}

/// The argument `value` picked for parameter `q`: into the draft, or written to binding `binding`
/// as one undo step.
fn pick_argument(shell: &mut Shell, binding: Option<Obj>, q: Obj, value: String) {
    let Some(store) = shell.session.as_ref().map(|s| s.doc()) else { return };
    let Ok(qid) = store.uuid(q) else { return };
    let binding_id = binding.and_then(|b| store.uuid(b).ok());
    // A draft over this binding, or a new one, takes the pick; nothing is written.
    if let Some(d) = shell.panels.binding.as_mut().filter(|d| binding.is_none() || d.binding == binding_id) {
        match d.args.iter_mut().find(|(x, _)| *x == qid) {
            Some(slot) => slot.1 = value,
            None => d.args.push((qid, value)),
        }
        d.refused = None;
        return;
    }
    let Some(b) = binding_id else { return };
    let mut args = Map::new();
    args.insert(qid.to_string(), Json::String(value));
    if let Err(e) = run(shell, "uml.template-binding.set", json!({ "binding": b.to_string(), "arguments": args })) {
        shell.panels.notice = Some(e);
    }
}

/// Binding `b` moved to the bound element `c`, its arguments kept, as one undo step.
fn set_bound(shell: &mut Shell, b: Obj, c: Obj) {
    let Some(store) = shell.session.as_ref().map(|s| s.doc()) else { return };
    let (Ok(b), Ok(c)) = (store.uuid(b), store.uuid(c)) else { return };
    if let Err(e) = run(shell, "uml.template-binding.set", json!({ "binding": b.to_string(), "bound": c.to_string() })) {
        shell.panels.notice = Some(e);
    }
}

/// The draft's foot: what it still needs on the danger tint, Cancel and Apply, and the help under
/// them; gives the y under it.
fn draft_foot(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect, mut y: f32) -> f32 {
    let width = (r.width() - 2.0 * PAD).max(0.0);
    let left = r.left() + PAD;
    let Some(draft) = shell.panels.binding.clone() else { return y };
    if let Some(why) = draft.refused.as_deref() {
        y += refusal_box(shell, ui, p, pos2(left, y), width, why) + 12.0;
    }
    let apply_w = {
        let paint = ui.painter();
        ux::galley(paint, &tr("Apply"), types::UI, p.text, None).size().x + 20.0
    };
    let cancel_w = {
        let paint = ui.painter();
        ux::galley(paint, &tr("Cancel"), types::UI, p.text, None).size().x + 20.0
    };
    let right = r.right() - PAD;
    let (_, apply) = crate::settings::button(
        shell,
        ui,
        p,
        &format!("{ID}.apply"),
        pos2(right - apply_w, y),
        &tr("Apply"),
        crate::settings::Look::Primary,
        true,
        None,
    );
    let (_, cancel) = crate::settings::button(
        shell,
        ui,
        p,
        &format!("{ID}.cancel"),
        pos2(right - apply_w - 8.0 - cancel_w, y),
        &tr("Cancel"),
        crate::settings::Look::Secondary,
        true,
        None,
    );
    y += FIELD + 16.0;
    let help = if draft.binding.is_none() {
        tr(DRAFT_HELP)
    } else {
        tr("Another template takes arguments of its own: pick them, then Apply. Cancel or Esc keeps the binding as it is.")
    };
    let paint = ui.painter();
    let g = ux::galley_rows(paint, &help, types::CAPTION, p.text_muted, Some(width), 8);
    let h = g.size().y;
    paint.galley(pos2(left, y), g, p.text_muted);
    y += h;
    let escape = shell.panels.menu.is_none() && ui.input(|i| i.key_pressed(egui::Key::Escape));
    if cancel.clicked() || escape {
        shell.panels.binding = None;
    } else if apply.clicked() {
        apply_draft(shell);
    }
    y
}

/// Writes the draft as one undo step, or says what it still needs and keeps it open.
fn apply_draft(shell: &mut Shell) {
    let Some(draft) = shell.panels.binding.clone() else { return };
    let Some(store) = shell.session.as_ref().map(|s| s.doc()) else { return };
    if let Some(why) = missing_words(store, &draft) {
        if let Some(d) = shell.panels.binding.as_mut() {
            d.refused = Some(why);
        }
        return;
    }
    let mut args = Map::new();
    for (q, v) in draft.args.iter().filter(|(_, v)| !v.is_empty()) {
        args.insert(q.to_string(), Json::String(v.clone()));
    }
    let out = match (draft.binding, draft.diagram, draft.nodes) {
        (Some(b), _, _) => {
            run(shell, "uml.template-binding.set", json!({ "binding": b.to_string(), "template": draft.template.to_string(), "arguments": args }))
                .map(|_| Some(b))
        }
        (None, Some(d), Some((from, to))) => {
            let Some(session) = shell.session.as_mut() else { return };
            let (bound, template) = (draft.bound.to_string(), draft.template.to_string());
            session
                .batch(&tr("Add template binding"), |b| {
                    let made = b.execute("uml.template-binding.add", &json!({ "bound": bound, "template": template, "arguments": args }))?;
                    let element = made.get("id").and_then(|i| i.as_str()).unwrap_or_default().to_string();
                    let e = b.execute(
                        "diagram.connect",
                        &json!({ "diagram": d.to_string(), "element": element, "source": from.to_string(), "target": to.to_string() }),
                    )?;
                    let edge = e.get("id").and_then(|i| i.as_str()).unwrap_or_default().to_string();
                    b.execute("diagram.label", &json!({ "edge": edge, "kind": "name", "at": "middle" }))?;
                    Ok(())
                })
                .map(|o| o.value.get(0).and_then(|v| v.get("id")).and_then(|v| v.as_str()).and_then(Uuid::parse))
                .map_err(|e| crate::panels::refusal(&e))
        }
        _ => return,
    };
    match out {
        Ok(made) => {
            shell.panels.binding = None;
            if let Some(b) = made {
                crate::panels::select_element(shell, b);
            }
        }
        Err(e) => {
            if let Some(d) = shell.panels.binding.as_mut() {
                d.refused = Some(e);
            }
        }
    }
}

/// Properties while a new binding's draft is open (C3): its heading, the Binding section, the
/// substitutions and the draft's foot. True when it drew, so the selection's panel is not drawn.
pub(crate) fn draft_panel(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect, y: f32) -> bool {
    drop_stale(shell);
    if shell.panels.binding.as_ref().is_none_or(|d| d.binding.is_some()) {
        return false;
    }
    let paint = ui.painter().clone();
    let width = (r.width() - 2.0 * PAD).max(0.0);
    ux::text(&paint, r.min + vec2(PAD, y), &tr("New template binding"), types::CARD_TITLE, p.text, Some(width));
    ux::text(&paint, r.min + vec2(PAD, y + 26.0), &tr("Drawing · not in the model until you apply"), types::CAPTION, p.text_secondary, Some(width));
    let mut clip = ui.new_child(egui::UiBuilder::new().max_rect(Rect::from_min_max(pos2(r.left(), r.top() + y + 51.0), r.max)));
    clip.set_clip_rect(Rect::from_min_max(pos2(r.left(), r.top() + y + 51.0), r.max));
    let below = sections(shell, &mut clip, p, r, r.top() + y + 51.0, None, true, true);
    draft_foot(shell, &mut clip, p, r, below + 4.0);
    true
}

/// Drops a draft whose elements are gone, whose diagram is no longer the open one, or whose
/// selection has moved on: a draft belongs to the gesture that opened it.
pub(crate) fn drop_stale(shell: &mut Shell) {
    let Some(d) = shell.panels.binding.clone() else { return };
    let store = shell.session.as_ref().map(|s| s.doc());
    let gone = store.is_none_or(|st| {
        st.by_uuid(d.bound).is_none_or(|o| !st.is_alive(o))
            || st.by_uuid(d.template).is_none_or(|o| !st.is_alive(o))
            || d.binding.is_some_and(|b| st.by_uuid(b).is_none_or(|o| !st.is_alive(o)))
    });
    let moved = crate::panels::selected_id(shell) != d.opened_over || (d.binding.is_none() && d.diagram != shell.canvas.active);
    if gone || moved {
        shell.panels.binding = None;
    }
}

/// A binding's Properties (C2): its Bound element and Template, then its substitutions; with
/// another template picked, the draft over it.
pub(crate) fn body(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect, y: f32, subject: &Subject) {
    let Some(o) = subject.obj else { return };
    drop_stale(shell);
    let draft = shell.panels.binding.as_ref().is_some_and(|d| d.binding == subject.id && d.binding.is_some());
    let below = sections(shell, ui, p, r, y, Some(o), draft, false);
    if draft {
        draft_foot(shell, ui, p, r, below + 4.0);
        return;
    }
    let width = (r.width() - 2.0 * PAD).max(0.0);
    let said = format!("{} {}", tr(SELECTOR_HELP), tr(LINE_HELP));
    let paint = ui.painter();
    let g = ux::galley_rows(paint, &said, types::CAPTION, p.text_muted, Some(width), 10);
    paint.galley(pos2(r.left() + PAD, below + 4.0), g, p.text_muted);
}

/// The kind line of a binding's Properties: `Binding · OrderRepository binds Repository · model, in
/// every diagram`.
pub(crate) fn kind_line(store: &Store, b: Obj) -> String {
    let bound = templates::bound_element(store, b).map(|o| name_of(store, o)).unwrap_or_default();
    let template = bindings::template_of(store, b).map(|o| name_of(store, o)).unwrap_or_default();
    fmt(&tr("Binding · {bound} binds {template} · model, in every diagram"), &[("bound", &bound), ("template", &template)])
}

/// What Models writes after a class that binds templates: `binds Repository`.
pub(crate) fn binds_note(store: &Store, c: Obj) -> Option<String> {
    let names: Vec<String> =
        bindings::bindings_of(store, c).into_iter().filter_map(|b| bindings::template_of(store, b)).map(|t| name_of(store, t)).collect();
    (!names.is_empty()).then(|| fmt(&tr("binds {template}"), &[("template", &names.join(", "))]))
}

/// Why the Template binding tool starts no line at `element`, in the user's words.
pub(crate) fn start_problem(store: &Store, element: Obj) -> Option<String> {
    let ok = store.class_of(element).is_ok_and(bindings::can_bind) && !archforge_uml::members::is_built_in(store, element);
    (!ok).then(|| tr("Only a class, an interface or a data type binds a template"))
}

/// Why `bound` may not bind `template`, in the user's words, when it may not: the model's rule said
/// as a refusal of the engine is. `except` is a binding being reconnected.
pub(crate) fn refusal(store: &Store, bound: Obj, template: Obj, except: Option<Obj>) -> Option<String> {
    let problem = bindings::problem(store, bound, template, except)?;
    Some(crate::panels::said(&problem.reason(store, bound, template)).unwrap_or_else(|| problem.said(store, bound, template)))
}

/// Whether moving binding `b`'s template end onto `template` needs arguments first: a parameter
/// of it has no default. Said at the canvas's top, in the user's words.
pub(crate) fn needs_arguments(store: &Store, template: Obj) -> Option<String> {
    templates::parameters(store, template)
        .into_iter()
        .any(|p| templates::default_of(store, p).is_none())
        .then(|| tr("Pick another template in the binding's Properties, where its arguments are chosen."))
}

/// Drops the draft whose bound element, template or binding is gone.
pub(crate) fn retain_valid(draft: &mut Option<BindingDraft>, store: &Store) {
    if draft.as_ref().is_some_and(|d| {
        store.by_uuid(d.bound).is_none() || store.by_uuid(d.template).is_none() || d.binding.is_some_and(|b| store.by_uuid(b).is_none())
    }) {
        *draft = None;
    }
}

/// The open draft's Apply pressed, as a sample to look at.
#[cfg(any(test, feature = "snapshot"))]
pub(crate) fn apply_sample(shell: &mut Shell) {
    apply_draft(shell);
}

/// Every word this module says through `tr` at run time, for the catalogue.
#[cfg(test)]
pub(crate) fn texts() -> impl Iterator<Item = &'static str> {
    [DRAFT_HELP, SELECTOR_HELP, LINE_HELP].into_iter()
}
