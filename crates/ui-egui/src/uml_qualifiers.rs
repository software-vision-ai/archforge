//! An association end's qualifiers in Properties (AR-127, design page 08, frames A1 and A6): the
//! section `Qualifiers · n` in the end's part of the panel, its `+` opening the row editor for a
//! new qualifier, a row per qualifier reading `lineNo : Integer` with its pencil (a double click
//! opens it too), and the editor: Name and Type (the shared type selector), then Multiplicity, a
//! line saying what the qualifier picks, the bin and the moves at its foot on the left, Cancel and
//! Apply on the right. Apply is one undo step (`uml.qualifier.add` or `.set`); the arrows move the
//! qualifier among its end's (`uml.qualifier.move`) and the bin removes it (`element.remove`). A
//! name another qualifier of the end has, whatever the case, is refused in place with the reason
//! in a tinted box, the name field ringed, the editor kept open and nothing written.

use archforge_mof::{Obj, Store, Uuid};
use archforge_uml::qualifiers;
use egui::{Rect, Stroke, StrokeKind, Vec2, pos2, vec2};
use serde_json::json;

use crate::Shell;
use crate::home::control;
use crate::i18n::{fmt, tr};
use crate::panels::{FIELD, Menu, PAD, ROW, icon_button, run, section, select_field, text_field};
use crate::ux::{self, Palette, icons, types};

/// The longest multiplicity typed.
const MAX_MULTIPLICITY: usize = 24;

/// A qualifier being edited: the qualifier, or none for one about to be added to end `end` of
/// `association`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QualifierEdit {
    pub association: Uuid,
    pub end: usize,
    pub target: Option<Uuid>,
    pub name: String,
    /// The type as the commands take it: empty, a built-in type's name or an element's id.
    pub ty: String,
    pub multiplicity: String,
    pub refused: Option<String>,
    /// The field the refusal sits under (`name`, `multiplicity`), or none for the card's foot.
    pub refused_at: Option<&'static str>,
    /// True until the editor has been shown once: the name field takes the keyboard then.
    pub opened: bool,
}

fn name_of(store: &Store, o: Obj) -> String {
    archforge_core::name_of(store, o)
}

fn type_of(store: &Store, o: Obj) -> Option<Obj> {
    store.get(o, archforge_uml::typed_element::TYPE).ok().and_then(|v| v.as_ref()).filter(|t| store.is_alive(*t))
}

/// The type of `q` as the commands take it: a built-in type by its name, any other by its id.
fn type_value(store: &Store, q: Obj) -> String {
    match type_of(store, q) {
        Some(t) if archforge_uml::members::is_built_in(store, t) => name_of(store, t),
        Some(t) => store.uuid(t).map(|u| u.to_string()).unwrap_or_default(),
        None => String::new(),
    }
}

/// End `i` of association `a`.
fn end_of(store: &Store, a: Obj, i: usize) -> Option<Obj> {
    archforge_uml::figures::member_ends(store, a)?.get(i).copied()
}

/// Opens the editor on qualifier `target`, or on a new one of end `end` of `association`.
pub(crate) fn open(shell: &mut Shell, association: Uuid, end: usize, target: Option<Uuid>) {
    let mut edit = QualifierEdit {
        association,
        end,
        target,
        name: String::new(),
        ty: String::new(),
        multiplicity: String::new(),
        refused: None,
        refused_at: None,
        opened: true,
    };
    if let Some(store) = shell.session.as_ref().map(|s| s.doc())
        && let Some(q) = target.and_then(|t| store.by_uuid(t))
    {
        edit.name = name_of(store, q);
        edit.ty = type_value(store, q);
        edit.multiplicity = archforge_uml::figures::multiplicity(store, q);
    }
    shell.panels.qualifier = Some(edit);
    shell.panels.menu = None;
}

/// The ids of end `i`'s qualifier controls: its section's add, a row's (and its editor's fields)
/// by the qualifier's place.
fn row_key(i: usize, k: usize) -> String {
    format!("workspace.properties.end.{i}.qualifier.{k}")
}

/// What end `i` of association `a` says once it is qualified: where the box is drawn.
pub(crate) fn qualified_note(store: &Store, end: Obj) -> Option<String> {
    if qualifiers::qualifiers(store, end).is_empty() {
        return None;
    }
    let class = type_of(store, end).map(|t| name_of(store, t)).unwrap_or_default();
    Some(fmt(&tr("Qualified end: the qualifier box is drawn here, against {class}."), &[("class", &class)]))
}

/// What the other end's multiplicity counts when end `end` is qualified (AR-127): one value per
/// qualified instance and qualifier value, shown beside its class; none while `end` has no
/// qualifier.
pub(crate) fn per_value_note(store: &Store, end: Obj, other: Obj) -> Option<String> {
    if qualifiers::qualifiers(store, end).is_empty() {
        return None;
    }
    let class = type_of(store, other).map(|t| name_of(store, t)).unwrap_or_default();
    Some(fmt(&tr("Counted per qualifier value of the other end; shown next to {class}."), &[("class", &class)]))
}

/// The section `Qualifiers · n` of end `i` (`end`) of association `a`, whose id is `a_id`, at `y`,
/// with its rows and the editor; gives the y under it.
#[allow(clippy::too_many_arguments, reason = "the section is drawn into the end's part of the panel")]
pub(crate) fn section_of(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect, mut y: f32, a_id: Uuid, i: usize, end: Obj) -> f32 {
    let paint = ui.painter().clone();
    let width = (r.width() - 2.0 * PAD).max(0.0);
    let Some(store) = shell.session.as_ref().map(|s| s.doc()) else { return y };
    let rows: Vec<(Uuid, String)> =
        qualifiers::qualifiers(store, end).into_iter().filter_map(|q| Some((store.uuid(q).ok()?, qualifiers::qualifier_text(store, q)))).collect();
    let note = qualified_note(store, end);
    let title = fmt(&tr("Qualifiers · {n}"), &[("n", &rows.len().to_string())]);
    let add = format!("workspace.properties.end.{i}.qualifiers.add");
    let Some(next) = section(shell, ui, p, r, y, &title, Some((&add, "Add qualifier"))) else {
        open(shell, a_id, i, None);
        return y + 50.0;
    };
    y = next - 6.0;
    if let Some(note) = note {
        let g = ux::galley_rows(&paint, &note, types::CAPTION, p.text_secondary, Some(width), 3);
        let h = g.size().y;
        paint.galley(pos2(r.left() + PAD, y), g, p.text_secondary);
        y += h + 8.0;
    }
    let editing = |shell: &Shell| shell.panels.qualifier.as_ref().filter(|e| e.association == a_id && e.end == i).map(|e| e.target);
    let chosen = crate::panels::selected_id(shell);
    for (k, (id, text)) in rows.iter().enumerate() {
        if editing(shell) == Some(Some(*id)) {
            y = editor(shell, ui, p, r, y, k);
            continue;
        }
        let row = Rect::from_min_size(pos2(r.left() + PAD, y), vec2(width, ROW));
        let key = row_key(i, k);
        let resp = control(shell, ui, &key, "row", row, true).accessible_name(text.as_str()).on_hover_text(text.as_str());
        if Some(*id) == chosen {
            paint.rect_filled(row, 6.0, p.accent_tint);
        } else if resp.contains_pointer() {
            paint.rect_filled(row, 6.0, p.page);
        }
        ux::text(&paint, pos2(row.left() + 10.0, row.top() + 9.0), text, types::UI, p.text, Some((width - 50.0).max(0.0)));
        if crate::panels::edit_button(shell, ui, p, row, &key, resp.contains_pointer()) || resp.double_clicked() {
            crate::panels::select_element(shell, *id);
            open(shell, a_id, i, Some(*id));
        } else if resp.clicked() {
            crate::panels::select_element(shell, *id);
        }
        y += ROW;
    }
    if editing(shell) == Some(None) {
        y = editor(shell, ui, p, r, y, rows.len());
    }
    y + 10.0
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

/// What the qualifier as edited picks, in the user's words: `One order and one lineNo pick 0..1
/// line.`, the qualified end and the other end each named by its role, else its class, the other
/// end's multiplicity one when it names none.
fn picks(store: &Store, edit: &QualifierEdit) -> Option<String> {
    let a = store.by_uuid(edit.association)?;
    let (end, other) = (end_of(store, a, edit.end)?, end_of(store, a, 1 - edit.end.min(1))?);
    let called = |e: Obj| {
        let role = name_of(store, e);
        if role.is_empty() { type_of(store, e).map(|t| name_of(store, t)).unwrap_or_default() } else { role }
    };
    let target = edit.target.and_then(|t| store.by_uuid(t));
    let mut names: Vec<String> = qualifiers::qualifiers(store, end)
        .into_iter()
        .map(|q| if Some(q) == target { edit.name.trim().to_string() } else { name_of(store, q) })
        .collect();
    if target.is_none() {
        names.push(edit.name.trim().to_string());
    }
    names.retain(|n| !n.is_empty());
    if names.is_empty() {
        return None;
    }
    let m = archforge_uml::figures::multiplicity(store, other);
    let m = if m.is_empty() { "1".to_string() } else { m };
    Some(fmt(
        &tr("One {owner} and one {qualifiers} pick {multiplicity} {target}."),
        &[("owner", &called(end)), ("qualifiers", &names.join(", ")), ("multiplicity", &m), ("target", &called(other))],
    ))
}

/// The qualifier editor card for the qualifier at place `k` (its count for a new one); gives the y
/// under it.
fn editor(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect, y: f32, k: usize) -> f32 {
    let paint = ui.painter().clone();
    let width = (r.width() - 2.0 * PAD).max(0.0);
    let Some(mut edit) = shell.panels.qualifier.clone() else { return y };
    let background = paint.add(egui::Shape::Noop);
    let left = r.left() + PAD;
    let inner = (width - 24.0).max(0.0);
    let at = |x: f32, dy: f32| pos2(left + 12.0 + x, y + dy);
    let row = 17.0 + FIELD + 10.0;
    let take = std::mem::take(&mut edit.opened);
    let half = ((inner - 8.0) / 2.0).max(0.0);
    let key = row_key(edit.end, k);
    let mut cy = 12.0;
    let mut enter = false;
    let typed = edit.name.clone();
    let name_id = format!("{key}.name");
    enter |= text_field(shell, ui, p, at(0.0, cy), half, &name_id, &tr("Name"), &mut edit.name, archforge_uml::commands::MAX_NAME, take, false);
    if edit.refused_at == Some("name") {
        if edit.name != typed {
            // Typing again takes the reason away until the next Apply.
            edit.refused = None;
            edit.refused_at = None;
        } else {
            let field = Rect::from_min_size(at(0.0, cy + 17.0), vec2(half, FIELD));
            paint.rect_stroke(field, 6.0, Stroke::new(1.0, p.danger), StrokeKind::Inside);
        }
    }
    let type_id = format!("{key}.type");
    let value = shell.session.as_ref().map(|s| crate::uml_panel::type_label(s.doc(), &edit.ty)).unwrap_or_default();
    let type_field = select_field(shell, ui, p, at(half + 8.0, cy), half, &type_id, &tr("Type"), &value, Menu::QualifierType);
    cy += row;
    let m_id = format!("{key}.multiplicity");
    enter |= text_field(shell, ui, p, at(0.0, cy), half, &m_id, &tr("Multiplicity"), &mut edit.multiplicity, MAX_MULTIPLICITY, false, false);
    cy += row;
    if let (Some(why), Some(_)) = (edit.refused.clone(), edit.refused_at) {
        cy += refusal_box(&paint, p, at(0.0, cy - 4.0), inner, &why) + 8.0;
    } else if let Some(said) = shell.session.as_ref().and_then(|s| picks(s.doc(), &edit)) {
        let g = ux::galley_rows(&paint, &said, types::CAPTION, p.text_secondary, Some(inner), 4);
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
    // The type menu last, over the card.
    crate::uml_panel::type_menu(shell, ui, p, type_field, &type_id, &mut edit.ty, Menu::QualifierType, None);
    shell.panels.qualifier = Some(edit.clone());
    if cancel || (shell.panels.menu.is_none() && ui.input(|i| i.key_pressed(egui::Key::Escape))) {
        shell.panels.qualifier = None;
    } else if remove {
        let target = edit.target.map(|t| t.to_string()).unwrap_or_default();
        match run(shell, "element.remove", json!({ "ids": [target] })) {
            Ok(_) => {
                shell.panels.qualifier = None;
                let _ = run(shell, "view.select", json!({ "ids": [edit.association.to_string()] }));
            }
            Err(e) => refuse(shell, e, None),
        }
    } else if up || down {
        move_qualifier(shell, &edit, up);
    } else if apply || enter {
        commit(shell, &edit);
    }
    card.bottom() + 8.0
}

fn refuse(shell: &mut Shell, why: String, at: Option<&'static str>) {
    if let Some(e) = shell.panels.qualifier.as_mut() {
        e.refused = Some(why);
        e.refused_at = at;
    }
}

fn move_qualifier(shell: &mut Shell, edit: &QualifierEdit, up: bool) {
    let Some(target) = edit.target else { return };
    let Some(store) = shell.session.as_ref().map(|s| s.doc()) else { return };
    let Some(q) = store.by_uuid(target) else { return };
    let Some(end) = qualifiers::qualified_end(store, q) else { return };
    let all = qualifiers::qualifiers(store, end);
    let Some(i) = all.iter().position(|x| *x == q) else { return };
    let to = if up { i.checked_sub(1) } else { Some(i + 1).filter(|j| *j < all.len()) };
    let Some(to) = to else { return };
    match run(shell, "uml.qualifier.move", json!({ "qualifier": target.to_string(), "at": to })) {
        Ok(_) => {
            if let Some(e) = shell.panels.qualifier.as_mut() {
                e.refused = None;
                e.refused_at = None;
            }
        }
        Err(e) => refuse(shell, e, None),
    }
}

/// Why the qualifier as edited cannot be written, in the user's words, with the field it sits
/// under: an empty name, a name another qualifier of its end has whatever the case, or a
/// multiplicity that does not read. The commands keep the same rules.
pub(crate) fn refusal(store: &Store, edit: &QualifierEdit) -> Option<(String, &'static str)> {
    let name = edit.name.trim();
    if name.is_empty() {
        return Some((tr("Type a name: a qualifier without one picks nothing."), "name"));
    }
    let end = store.by_uuid(edit.association).and_then(|a| end_of(store, a, edit.end))?;
    let target = edit.target.and_then(|t| store.by_uuid(t));
    if let Some(other) = qualifiers::name_clash(store, end, name, target) {
        let said = fmt(&tr("This end already has a qualifier named {name}. Pick another name."), &[("name", &name_of(store, other))]);
        return Some((said, "name"));
    }
    if let Err(e) = archforge_uml::members::parse_multiplicity(&edit.multiplicity) {
        return Some((crate::uml_relate::multiplicity_problem(&e), "multiplicity"));
    }
    None
}

/// Writes the editor as one undo step, or says why not, leaving the model and its history as
/// they were.
fn commit(shell: &mut Shell, edit: &QualifierEdit) {
    if let Some((why, at)) = shell.session.as_ref().and_then(|s| refusal(s.doc(), edit)) {
        refuse(shell, why, Some(at));
        return;
    }
    let (cmd, params) = match edit.target {
        Some(t) => (
            "uml.qualifier.set",
            json!({ "qualifier": t.to_string(), "name": edit.name.trim(), "type": edit.ty, "multiplicity": edit.multiplicity.trim() }),
        ),
        None => (
            "uml.qualifier.add",
            json!({
                "association": edit.association.to_string(),
                "end": edit.end,
                "name": edit.name.trim(),
                "type": edit.ty,
                "multiplicity": edit.multiplicity.trim(),
            }),
        ),
    };
    match run(shell, cmd, params) {
        Ok(_) => {
            shell.panels.qualifier = None;
            shell.panels.notice = None;
        }
        Err(e) => refuse(shell, e, None),
    }
}

/// Drops an editor whose association or qualifier is gone.
pub(crate) fn retain_valid(edit: &mut Option<QualifierEdit>, store: &Store) {
    if edit.as_ref().is_some_and(|e| store.by_uuid(e.association).is_none() || e.target.is_some_and(|t| store.by_uuid(t).is_none())) {
        *edit = None;
    }
}

/// `typed` as a new qualifier's name on end `end` of association `association`, applied and
/// refused, as a sample to look at.
#[cfg(any(test, feature = "snapshot"))]
pub(crate) fn refuse_sample(shell: &mut Shell, association: Uuid, end: usize, typed: &str, ty: &str) {
    open(shell, association, end, None);
    if let Some(e) = shell.panels.qualifier.as_mut() {
        e.name = typed.to_string();
        e.ty = ty.to_string();
        e.multiplicity = "1".to_string();
    }
    let Some(edit) = shell.panels.qualifier.clone() else { return };
    commit(shell, &edit);
}

/// Every word this module says through `tr` at run time, for the catalogue.
#[cfg(test)]
pub(crate) fn texts() -> impl Iterator<Item = &'static str> {
    ["Add qualifier"].into_iter()
}
