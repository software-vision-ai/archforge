//! Generalization sets on a class diagram (AR-129, design page 08, frames B1 to B4). There is no
//! tool: selected generalizations (Shift-click) are grouped from their context menu, Create
//! generalization set… (⌥⌘G, Ctrl Alt G elsewhere) or Add to set ▸ a set of their general class,
//! and from the same actions in Properties for the keyboard. A selection whose generalizations
//! specialise different classes, or that holds anything else, shows both actions disabled with the
//! reason, on hover, for screen readers and under the menu.
//!
//! A set's Properties hold its name and general class, Completeness (Complete | Incomplete) and
//! Disjointness (Disjoint | Overlapping), segmented and moved with the arrow keys, and its members,
//! `+` offering only generalizations to its general class. A generalization's Properties list the
//! sets it is in, each with its braces, `+` joining another or starting one. Every change is one
//! undo step through the `uml.generalization-set.*` commands; a reconnection of a member is refused
//! with the reason in Properties and at the canvas's top.

use archforge_mof::{Obj, Store, Uuid};
use archforge_uml::generalization_sets as sets;
use egui::{Pos2, Rect, Sense, Stroke, StrokeKind, Vec2, pos2, vec2};
use serde_json::json;

use crate::canvas::{Drawing, ExtraLabel, edge_points, element_of, refs};
use crate::cx::{Ctx, CtxRef, Request};
use crate::i18n::{fmt, tr, trn};
use crate::panels::{FIELD, FieldOut, PAD, ROW, Subject, control_cx, edit_field_cx, menu_cx, run_cx, section_cx};
use crate::state::Panel;
use crate::uml_state::menus;
use crate::uml_state::{uml, uml_mut};
use crate::ux::{self, Palette, icons, types};

/// The automation ids the design gives.
pub(crate) const CREATE_ID: &str = "workspace.canvas.contextMenu.generalizationSet.create";
pub(crate) const PANEL_CREATE_ID: &str = "workspace.properties.selection.createGeneralizationSet";
const MENU_WIDTH: f32 = 288.0;
const MENU_ROW: f32 = 30.0;

fn name(store: &Store, o: Obj) -> String {
    archforge_core::name_of(store, o)
}

/// The elements a selection stands for: a line selected as a line counts as its element; each once.
pub(crate) fn elements(store: &Store, selection: &[Obj]) -> Vec<Obj> {
    let mut out: Vec<Obj> = Vec::new();
    for o in selection.iter().copied().filter(|o| store.is_alive(*o)) {
        let e = if store.class_of(o).ok() == Some(archforge_diagram::EDGE) {
            store.get(o, archforge_diagram::edge::ELEMENT).ok().and_then(|v| v.as_ref())
        } else {
            Some(o)
        };
        if let Some(e) = e.filter(|e| !out.contains(e)) {
            out.push(e);
        }
    }
    out
}

/// What a selection of `elements` may do with sets: its general class and its generalizations, or
/// why it may not form a set, in the user's words: the short reason a menu row gives and the whole
/// sentence.
pub(crate) enum Grouping {
    Shared { general: Obj, gens: Vec<Obj> },
    Refused { short: String, short_add: String, why: String },
}

/// How `elements` group, as the menu, the shortcut and Properties say it.
pub(crate) fn grouping(store: &Store, elements: &[Obj]) -> Grouping {
    match sets::shared_general(store, elements) {
        Ok(general) => Grouping::Shared { general, gens: elements.to_vec() },
        Err(sets::Grouping::ParentsDiffer(gens)) => {
            let ends: Vec<(String, String)> = gens
                .iter()
                .map(|g| {
                    let (s, p) = (sets::specific(store, *g), sets::general(store, *g));
                    (s.map(|o| name(store, o)).unwrap_or_default(), p.map(|o| name(store, o)).unwrap_or_default())
                })
                .collect();
            Grouping::Refused { short: tr("parents differ"), short_add: tr("no shared parent"), why: parents_differ(&ends) }
        }
        Err(sets::Grouping::NotAGeneralization(o)) => {
            let what = archforge_core::name_of(store, o);
            let what = if what.is_empty() { tr("Something selected") } else { what };
            let why =
                format!("{} {}", fmt(&tr("{name} is not a generalization."), &[("name", &what)]), tr("A set groups generalizations of one class."));
            Grouping::Refused { short: tr("not all generalizations"), short_add: tr("not all generalizations"), why }
        }
        Err(sets::Grouping::Empty) => {
            let why = tr("Select generalizations to one class first.");
            Grouping::Refused { short: tr("nothing selected"), short_add: tr("nothing selected"), why }
        }
    }
}

/// Why generalizations to different classifiers form no set, each given by its specific and its
/// general classifier's names: `A specialises B and C specialises D. A set groups ...`.
pub(crate) fn parents_differ(ends: &[(String, String)]) -> String {
    let said: Vec<String> =
        ends.iter().map(|(s, g)| fmt(&tr("{specific} specialises {general}"), &[("specific", s.as_str()), ("general", g.as_str())])).collect();
    format!("{}. {}", and_list(&said), tr("A set groups generalizations of one class."))
}

/// `a`, `a and b`, `a, b and c`.
pub(crate) fn and_list(items: &[String]) -> String {
    match items.split_last() {
        Some((last, rest)) if !rest.is_empty() => fmt(&tr("{names} and {last}"), &[("names", &rest.join(", ")), ("last", last)]),
        _ => items.join(", "),
    }
}

/// The selection's elements, when it holds a generalization: what the menu, the shortcut and
/// Properties' selection panel act on.
pub(crate) fn selected(cx: &CtxRef) -> Option<Vec<Obj>> {
    let s = cx.session.as_ref()?;
    let store = s.doc();
    let all = elements(store, &s.view().selection);
    all.iter().any(|o| sets::is_generalization(store, *o)).then_some(all)
}

fn ids(store: &Store, objs: &[Obj]) -> Vec<String> {
    objs.iter().filter_map(|o| store.uuid(*o).ok()).map(|u| u.to_string()).collect()
}

/// Groups `gens`, generalizations to one class, into a new set with a fresh name, as one undo step;
/// the set is selected and Properties opens with its name ready for typing.
pub(crate) fn create(cx: &mut Ctx, gens: &[Obj], general: Obj) {
    let Some(store) = cx.session.as_ref().map(|s| s.doc()) else { return };
    let taken: Vec<String> = archforge_uml::nesting::home_package(store, general)
        .map(|pkg| {
            store
                .list(pkg, archforge_uml::package::PACKAGED_ELEMENT)
                .unwrap_or_default()
                .iter()
                .filter_map(archforge_mof::Value::as_ref)
                .filter(|o| sets::is_set(store, *o))
                .map(|o| name(store, o))
                .collect()
        })
        .unwrap_or_default();
    let Some(fresh) = crate::explorer::fresh_name("GeneralizationSet", taken.into_iter(), false) else { return };
    let params = json!({ "generalizations": ids(store, gens), "name": fresh });
    match run_cx(cx, "uml.generalization-set.add", params) {
        Ok(out) => {
            cx.canvas.notice = None;
            cx.requests.push(Request::ShowPanel(Panel::Properties));
            if let Some(uml) = uml_mut(cx.notation_states) {
                uml.set_name_focus = out.get("id").and_then(|v| v.as_str()).and_then(Uuid::parse);
            }
        }
        Err(e) => cx.canvas.notice = Some(e),
    }
}

/// Adds `gens` to the set `set`, as one undo step.
pub(crate) fn add_to(cx: &mut Ctx, set: Obj, gens: &[Obj]) {
    let Some(store) = cx.session.as_ref().map(|s| s.doc()) else { return };
    let (Ok(id), gens) = (store.uuid(set), ids(store, gens)) else { return };
    if let Err(e) = run_cx(cx, "uml.generalization-set.set", json!({ "set": id.to_string(), "add": gens })) {
        cx.canvas.notice = Some(e);
    }
}

/// Takes `g` out of the set `set`, as one undo step; a set left with no member goes with it.
fn remove_from(cx: &mut Ctx, set: Obj, g: Obj) {
    let Some(store) = cx.session.as_ref().map(|s| s.doc()) else { return };
    let (Ok(s), Ok(g)) = (store.uuid(set), store.uuid(g)) else { return };
    if let Err(e) = run_cx(cx, "uml.generalization-set.set", json!({ "set": s.to_string(), "remove": [g.to_string()] })) {
        cx.panels.notice = Some(e);
    }
}

/// ⌥⌘G (Ctrl Alt G): the selection's generalizations into a new set, or why not at the canvas's top.
pub(crate) fn shortcut(cx: &mut Ctx) -> bool {
    let Some(all) = selected(&cx.cx_ref()) else { return false };
    let Some(store) = cx.session.as_ref().map(|s| s.doc()) else { return false };
    match grouping(store, &all) {
        Grouping::Shared { general, gens } => create(cx, &gens, general),
        Grouping::Refused { why, .. } => cx.canvas.notice = Some(why),
    }
    true
}

/// ⌥⌘G (Ctrl Alt G elsewhere): the selected generalizations into a new set, or why they cannot
/// form one at the canvas's top; true when pressed with a generalization selected, which takes the
/// keys this frame.
pub(crate) fn chord_pressed(cx: &mut Ctx, ui: &egui::Ui) -> bool {
    ui.input(|i| crate::actions::chord_press(i, egui::Key::G, egui::Modifiers::COMMAND | egui::Modifiers::ALT)) && shortcut(cx)
}

/// A right click on a generalization's line, away from any card: the set actions' menu opened
/// there, the line selected first when the selection did not hold its generalization (AR-129);
/// none for anything else.
pub(crate) fn secondary_click(press: &crate::kinds::Press<'_>) -> Option<crate::editing::Action> {
    let (store, at) = (press.store, press.at?);
    if (press.node_at)(at).is_some() {
        return None;
    }
    let g = (press.line_at)(at).and_then(|e| store.get(e, archforge_diagram::edge::ELEMENT).ok().and_then(|v| v.as_ref()))?;
    if !sets::is_generalization(store, g) {
        return None;
    }
    let first = if elements(store, press.selection).contains(&g) { None } else { store.uuid(g).ok() };
    Some(crate::editing::Action::Own(Box::new(move |shell| {
        if let Some(id) = first {
            crate::editing::select(shell, vec![id]);
        }
        if let Some(uml) = uml_mut(&mut shell.notation_states) {
            uml.line_menu = Some(at);
        }
    })))
}

/// The generalization sets the generalizations among `lines` belong to: what Select all takes in
/// beside a diagram's lines, as a set's label is drawn whether a node holds it or not (AR-129).
pub(crate) fn sets_of_lines(store: &Store, lines: &[Obj]) -> Vec<Obj> {
    lines.iter().filter(|g| sets::is_generalization(store, **g)).flat_map(|g| sets::sets_of(store, *g)).collect()
}

/// The shortcut as the menu writes it.
pub(crate) fn chord(ui: &egui::Ui) -> &'static str {
    if ui.ctx().os() == egui::os::OperatingSystem::Mac { "⌥⌘G" } else { "Ctrl Alt G" }
}

/// The selected set, removed from the model with Delete, its generalizations kept, with a toast
/// offering Undo; false when no set is selected.
pub(crate) fn remove_selected_set(cx: &mut Ctx) -> bool {
    let Some(session) = cx.session.as_ref() else { return false };
    let store = session.doc();
    let Some(set) = session.view().selection.iter().copied().find(|o| sets::is_set(store, *o)) else { return false };
    let (Ok(id), said) = (store.uuid(set), name(store, set)) else { return false };
    match run_cx(cx, "element.remove", json!({ "ids": [id.to_string()] })) {
        Ok(_) => {
            let text = fmt(&tr("{name} removed; its generalizations stay"), &[("name", &said)]);
            let undo_label = cx.session.as_ref().and_then(|s| s.undo_label().map(str::to_string));
            cx.canvas.toast = Some(crate::canvas::Toast { text, until: cx.frames.saturating_add(crate::canvas::TOAST_FRAMES), undo_label });
        }
        Err(e) => cx.canvas.notice = Some(e),
    }
    true
}

/// The names of the sets removing `objs` leaves empty, which go with them: for the toast.
pub(crate) fn emptied_names(store: &Store, objs: &[Obj]) -> Vec<String> {
    sets::emptied_by(store, objs).into_iter().map(|s| name(store, s)).collect()
}

/// Why a reconnection of generalization `g` onto `target` is refused while it is in sets, in the
/// user's words (B4), when it is.
pub(crate) fn reconnect_refusal(store: &Store, g: Obj, target: Obj) -> Option<String> {
    let in_sets = sets::sets_of(store, g);
    if in_sets.is_empty() {
        return None;
    }
    let names: Vec<String> = in_sets.iter().map(|s| name(store, *s)).collect();
    let general = sets::general(store, g).map(|o| name(store, o)).unwrap_or_default();
    #[rustfmt::skip]
    let text = if in_sets.len() == 1 {
        tr("{generalization} is in {sets}, which groups the specialisations of {general}. Remove it from the set before reconnecting it to {target}.")
    } else {
        tr("{generalization} is in {sets}, which group the specialisations of {general}. Remove it from those sets before reconnecting it to {target}.")
    };
    Some(fmt(
        &text,
        &[
            ("generalization", &sets::generalization_name(store, g)),
            ("sets", &and_list(&names)),
            ("general", &general),
            ("target", &name(store, target)),
        ],
    ))
}

// The context menu.

/// One row of the line menu: its key, label, the detail at its right (a shortcut or why it is
/// disabled), whether it can be picked, and what it does.
struct Item {
    id: String,
    label: String,
    detail: String,
    live: bool,
    act: Act,
}

#[derive(Clone, Copy)]
enum Act {
    Create,
    Add(Obj),
    Remove,
    Delete,
    Nothing,
}

/// The context menu of selected generalization lines, open at `at` (B1, B3): its heading, Create
/// generalization set…, Add to set ▸ each set of their class, Remove from this diagram and Delete
/// from the model. A mixed selection shows the set actions disabled with the reason, and the
/// reason in full on the danger tint under the menu.
pub(crate) fn line_menu(cx: &mut Ctx, ui: &egui::Ui, p: &Palette, r: Rect, at: Pos2, d: Obj) {
    let Some(all) = selected(&cx.cx_ref()) else {
        if let Some(uml) = uml_mut(cx.notation_states) {
            uml.line_menu = None;
        }
        return;
    };
    let Some(store) = cx.session.as_ref().map(|s| s.doc()) else { return };
    let paint = ui.painter().with_clip_rect(r);
    let mac = ui.ctx().os() == egui::os::OperatingSystem::Mac;
    let n = u64::try_from(all.len()).unwrap_or(u64::MAX);
    let grouping = grouping(store, &all);
    let mut items: Vec<Item> = Vec::new();
    let mut reason = None;
    let heading = match &grouping {
        Grouping::Shared { general, gens } => {
            items.push(Item {
                id: CREATE_ID.into(),
                label: tr("Create generalization set…"),
                detail: chord(ui).into(),
                live: true,
                act: Act::Create,
            });
            let own = sets::sets_for(store, *general);
            if own.is_empty() {
                let detail = fmt(&tr("none for {general} yet"), &[("general", &name(store, *general))]);
                items.push(Item {
                    id: "workspace.canvas.contextMenu.generalizationSet.add".into(),
                    label: tr("Add to set"),
                    detail,
                    live: false,
                    act: Act::Nothing,
                });
            }
            for (i, s) in own.iter().enumerate() {
                let members = sets::members(store, *s);
                let all_in = gens.iter().all(|g| members.contains(g));
                let label = fmt(&tr("Add to set ▸ {name}"), &[("name", &name(store, *s))]);
                let detail = if all_in { tr("already in it") } else { String::new() };
                items.push(Item {
                    id: format!("workspace.canvas.contextMenu.generalizationSet.add.{i}"),
                    label,
                    detail,
                    live: !all_in,
                    act: Act::Add(*s),
                });
            }
            fmt(&trn("{n} generalization · to {general}", "{n} generalizations · to {general}", n), &[("general", &name(store, *general))])
        }
        Grouping::Refused { short, short_add, why } => {
            items.push(Item {
                id: CREATE_ID.into(), label: tr("Create generalization set…"), detail: short.clone(), live: false, act: Act::Nothing
            });
            items.push(Item {
                id: "workspace.canvas.contextMenu.generalizationSet.add".into(),
                label: tr("Add to set"),
                detail: short_add.clone(),
                live: false,
                act: Act::Nothing,
            });
            reason = Some(why.clone());
            let k = u64::try_from(all.iter().filter(|o| sets::is_generalization(store, **o)).count()).unwrap_or(u64::MAX);
            if k == n { trn("{n} generalization", "{n} generalizations", n) } else { trn("{n} element", "{n} elements", n) }
        }
    };
    let separator = items.len();
    let (delete_chord, _) = if mac { ("⌫", "⇧⌫") } else { ("Del", "Shift Del") };
    items.push(Item {
        id: "workspace.canvas.contextMenu.remove".into(),
        label: tr("Remove from this diagram"),
        detail: String::new(),
        live: true,
        act: Act::Remove,
    });
    items.push(Item {
        id: "workspace.canvas.contextMenu.delete".into(),
        label: tr("Delete from the model"),
        detail: delete_chord.into(),
        live: true,
        act: Act::Delete,
    });
    let heading_h = 26.0;
    let height = 8.0 + heading_h + items.len() as f32 * MENU_ROW + 1.0;
    let x = at.x.min(r.right() - MENU_WIDTH - 8.0).max(r.left() + 8.0);
    let y = at.y.min(r.bottom() - height - 8.0).max(r.top() + 8.0);
    let m = Rect::from_min_size(pos2(x, y), vec2(MENU_WIDTH, height));
    ux::shadow(&paint, m, 8.0, p.shadow);
    paint.rect_filled(m, 8.0, p.surface);
    paint.rect_stroke(m, 8.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
    let eyebrow = crate::canvas::eyebrow_case(&heading);
    ux::text(&paint, pos2(m.left() + 12.0, m.top() + 10.0), &eyebrow, types::EYEBROW, p.text_muted, Some(MENU_WIDTH - 24.0));
    let mut picked = None;
    let mut ry = m.top() + 4.0 + heading_h;
    for (i, item) in items.iter().enumerate() {
        if i == separator {
            paint.rect_filled(Rect::from_min_size(pos2(m.left() + 8.0, ry), vec2(MENU_WIDTH - 16.0, 1.0)), 0.0, p.border);
            ry += 1.0;
        }
        let row = Rect::from_min_size(pos2(m.left() + 4.0, ry), vec2(MENU_WIDTH - 8.0, MENU_ROW));
        let resp = if item.live {
            control_cx(cx, ui, &item.id, "menu-item", row, true).accessible_name(item.label.as_str())
        } else {
            // Disabled, it says why on hover and to a screen reader.
            crate::widgets::record_visible(ui, cx.registry, &item.id, "menu-item", row);
            let resp = ui.interact(row, crate::widgets::egui_id(&item.id), Sense::hover());
            let why = if item.detail.is_empty() { item.label.clone() } else { format!("{}: {}", item.label, item.detail) };
            ui.ctx().accesskit_node_builder(crate::widgets::egui_id(&item.id), |node| {
                node.set_role(egui::accesskit::Role::MenuItem);
                node.set_disabled();
                node.set_description(reason.clone().unwrap_or_else(|| why.clone()));
            });
            resp.on_hover_text(reason.clone().unwrap_or(why)).accessible_name(item.label.as_str())
        };
        if item.live && resp.hovered() {
            paint.rect_filled(row, 5.0, p.page);
        }
        let ink = if !item.live {
            p.text_muted
        } else if matches!(item.act, Act::Delete) {
            p.danger
        } else {
            p.text
        };
        let detail = ux::galley(&paint, &item.detail, types::CAPTION, p.text_muted, Some(MENU_WIDTH / 2.0));
        ux::text(&paint, row.min + vec2(10.0, 7.5), &item.label, types::SMALL, ink, Some((row.width() - 24.0 - detail.size().x).max(0.0)));
        paint.galley(pos2(row.right() - 10.0 - detail.size().x, row.top() + 8.0), detail, p.text_muted.into());
        if item.live && resp.clicked() {
            picked = Some(item.act);
        }
        ry += MENU_ROW;
    }
    // The reason in full under the menu, on the danger tint (B3).
    let mut area = m;
    if let Some(why) = &reason {
        let text = ux::galley_rows(&paint, why, types::SMALL, p.text, Some(MENU_WIDTH - 42.0), 6);
        let card = Rect::from_min_size(pos2(m.left(), m.bottom() + 8.0), vec2(MENU_WIDTH, text.size().y + 20.0));
        paint.rect_filled(card, 6.0, p.danger_tint);
        ux::icon_in(&paint, Rect::from_min_size(pos2(card.left() + 10.0, card.top() + 11.5), vec2(14.0, 14.0)), &icons::WARNING, p.danger);
        paint.galley(pos2(card.left() + 32.0, card.top() + 10.0), text, p.text.into());
        crate::widgets::record_visible(ui, cx.registry, "workspace.canvas.contextMenu.generalizationSet.reason", "text", card);
        area = area.union(card);
    }
    let outside = ui.input(|i| i.pointer.any_pressed() && !i.pointer.interact_pos().is_some_and(|pos| area.contains(pos)));
    if (outside || ui.input(|i| i.key_pressed(egui::Key::Escape)))
        && let Some(uml) = uml_mut(cx.notation_states)
    {
        uml.line_menu = None;
    }
    let Some(act) = picked else { return };
    if let Some(uml) = uml_mut(cx.notation_states) {
        uml.line_menu = None;
    }
    let gens = match &grouping {
        Grouping::Shared { gens, .. } => gens.clone(),
        Grouping::Refused { .. } => Vec::new(),
    };
    match (act, &grouping) {
        (Act::Create, Grouping::Shared { general, .. }) => create(cx, &gens, *general),
        (Act::Add(s), _) => add_to(cx, s, &gens),
        (Act::Remove, _) => remove_lines(cx, d, &all),
        (Act::Delete, _) => delete(cx, &all),
        _ => {}
    }
}

/// Removes the lines of `elements` from diagram `d`, as one undo step, the model untouched.
fn remove_lines(cx: &mut Ctx, d: Obj, elements: &[Obj]) {
    let Some(store) = cx.session.as_ref().map(|s| s.doc()) else { return };
    let edges: Vec<String> = elements
        .iter()
        .flat_map(|e| archforge_diagram::views_of(store, *e).1)
        .filter(|e| archforge_diagram::diagram_of(store, *e) == Some(d))
        .filter_map(|e| store.uuid(e).ok().map(|u| u.to_string()))
        .collect();
    let Some(session) = cx.session.as_mut() else { return };
    let out = session.batch(&tr("Remove from this diagram"), |b| {
        for edge in &edges {
            b.execute("diagram.disconnect", &json!({ "edge": edge }))?;
        }
        Ok(())
    });
    if let Err(e) = out {
        cx.canvas.notice = Some(crate::panels::refusal(&e));
    }
}

/// Deletes `elements` from the model, as one undo step, with a toast offering Undo that names the
/// sets left empty, which went with them.
fn delete(cx: &mut Ctx, elements: &[Obj]) {
    let Some(store) = cx.session.as_ref().map(|s| s.doc()) else { return };
    let gone = emptied_names(store, elements);
    let n = u64::try_from(elements.len()).unwrap_or(u64::MAX);
    let word = trn("{n} element", "{n} elements", n);
    let what = match elements {
        [one] => crate::notation_ui::of(cx.notation_uis, store, *one).and_then(|ui| ui.words(store, *one)).map(|w| w.word).unwrap_or(word),
        _ => word,
    };
    let ids = ids(store, elements);
    match run_cx(cx, "element.remove", json!({ "ids": ids })) {
        Ok(_) => {
            let text = removed_text(&what, &gone);
            let undo_label = cx.session.as_ref().and_then(|s| s.undo_label().map(str::to_string));
            cx.canvas.toast = Some(crate::canvas::Toast { text, until: cx.frames.saturating_add(crate::canvas::TOAST_FRAMES), undo_label });
        }
        Err(e) => cx.canvas.notice = Some(e),
    }
}

/// The toast's text for `what` removed, naming the sets that went with it, left with no member.
pub(crate) fn removed_text(what: &str, gone: &[String]) -> String {
    let removed = fmt(&tr("{what} removed"), &[("what", what)]);
    if gone.is_empty() {
        return removed;
    }
    let n = u64::try_from(gone.len()).unwrap_or(u64::MAX);
    let sets = fmt(&trn("{sets} had no other member and went too", "{sets} had no other members and went too", n), &[("sets", &and_list(gone))]);
    format!("{removed} · {sets}")
}

// Properties.

/// A segmented pair of choices (Complete | Incomplete), its ids `{id}.{first key}` and
/// `{id}.{second key}`; a click or the arrow keys while it has the keyboard flip it. Gives the new
/// value when it changed.
#[allow(clippy::too_many_arguments, reason = "a segmented control is drawn with its label, choices and ids")]
fn segmented(cx: &mut Ctx, ui: &egui::Ui, p: &Palette, at: Pos2, id: &str, label: &str, choices: [(&str, String); 2], first: bool) -> Option<bool> {
    let paint = ui.painter().clone();
    ux::text(&paint, at, label, types::CAPTION, p.text_secondary, None);
    let galleys = choices.clone().map(|(_, w)| ux::galley(&paint, &w, types::SMALL, p.text, None));
    let frame = Rect::from_min_size(at + vec2(0.0, 17.0), vec2(galleys.iter().map(|g| g.size().x + 24.0).sum::<f32>() + 4.0, FIELD));
    paint.rect_filled(frame, 6.0, p.field);
    let mut x = frame.left() + 2.0;
    let mut out = None;
    let mut focused = false;
    for (i, ((key, word), g)) in choices.into_iter().zip(galleys).enumerate() {
        let on = (i == 0) == first;
        let b = Rect::from_min_size(pos2(x, frame.top() + 2.0), vec2(g.size().x + 24.0, FIELD - 4.0));
        let sid = format!("{id}.{key}");
        let resp = control_cx(cx, ui, &sid, "toggle", b, true).accessible_name(format!("{label}: {word}"));
        ui.ctx().accesskit_node_builder(crate::widgets::egui_id(&sid), |node| {
            node.set_role(egui::accesskit::Role::RadioButton);
            node.set_toggled(if on { egui::accesskit::Toggled::True } else { egui::accesskit::Toggled::False });
        });
        focused |= resp.has_focus();
        if on {
            paint.rect_filled(b, 5.0, p.surface);
            paint.rect_stroke(b, 5.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
        }
        let ink = if on { p.text } else { p.text_secondary };
        paint.galley(pos2(b.left() + 12.0, b.center().y - g.size().y / 2.0), g, ink.into());
        if resp.clicked() && !on {
            out = Some(i == 0);
        }
        x = b.right();
    }
    if focused && ui.input(|i| [egui::Key::ArrowLeft, egui::Key::ArrowRight].into_iter().any(|key| crate::actions::plain_press(i, key))) {
        out = Some(!first);
    }
    out
}

/// What `complete` and `disjoint` mean for the general class, said under the choices.
fn meaning(braces: &str, general: &str, complete: bool, disjoint: bool) -> String {
    let text = match (complete, disjoint) {
        (true, true) => tr("Shown beside the lines as {braces}: every {general} is exactly one of these kinds."),
        (true, false) => tr("Shown beside the lines as {braces}: every {general} is at least one of these kinds."),
        (false, true) => tr("Shown beside the lines as {braces}: a {general} is at most one of these kinds."),
        (false, false) => tr("Shown beside the lines as {braces}: a {general} may be any number of these kinds."),
    };
    fmt(&text, &[("braces", braces), ("general", general)])
}

/// Muted help text at `at`, wrapped to `width`; gives the y under it.
fn help(ui: &egui::Ui, p: &Palette, at: Pos2, width: f32, text: &str) -> f32 {
    let paint = ui.painter();
    let g = ux::galley_rows(paint, text, types::CAPTION, p.text_muted, Some(width), 6);
    let h = g.size().y;
    paint.galley(at, g, p.text_muted.into());
    at.y + h
}

/// A row of a list in Properties with its text, its edit button and a remove button; gives what was
/// pressed: (edit, remove).
fn list_row(cx: &mut Ctx, ui: &egui::Ui, p: &Palette, row: Rect, id: &str, text: &str, remove_hint: &str) -> (bool, bool) {
    let paint = ui.painter().clone();
    let resp = control_cx(cx, ui, id, "row", row, true).accessible_name(text);
    if resp.contains_pointer() {
        paint.rect_filled(row, 6.0, p.page);
    }
    ux::text(&paint, pos2(row.left() + 10.0, row.top() + 9.0), text, types::UI, p.text, Some((row.width() - 76.0).max(0.0)));
    let edit_row = Rect::from_min_max(row.min, pos2(row.right() - 28.0, row.bottom()));
    let edit = crate::panels::edit_button_cx(cx, ui, p, edit_row, id, resp.contains_pointer()) || resp.double_clicked();
    let b = Rect::from_center_size(pos2(row.right() - 14.0, row.center().y), vec2(24.0, 24.0));
    let r = control_cx(cx, ui, &format!("{id}.remove"), "button", b, true).on_hover_text(tr(remove_hint)).accessible_name(tr(remove_hint));
    if r.hovered() {
        paint.rect_filled(b, 6.0, p.border);
    }
    ux::icon_mid(&paint, b.center(), &icons::CLOSE, if r.hovered() { p.danger } else { p.text_muted });
    (edit, r.clicked())
}

/// A set's Properties (B2): Name and its General class, Completeness and Disjointness with what they
/// mean, then its members with `+` and, per member, edit (its generalization) and remove.
pub(crate) fn set_body(cx: &mut Ctx, ui: &mut egui::Ui, p: &Palette, r: Rect, mut y: f32, subject: &Subject) {
    let (Some(s), Some(id)) = (subject.obj, subject.id) else { return };
    let Some(store) = cx.session.as_ref().map(|s| s.doc()) else { return };
    let width = (r.width() - 2.0 * PAD).max(0.0);
    let left = r.left() + PAD;
    let half = ((width - 8.0) / 2.0).max(0.0);
    let general = sets::general_of(store, s);
    let general_name = general.map(|g| name(store, g)).unwrap_or_default();
    let (complete, disjoint) = sets::flags(store, s);
    let braces = sets::braces(store, s);
    let members: Vec<Obj> = sets::members(store, s);
    let candidates: Vec<Obj> = general
        .map(|g| archforge_uml::relations::specializations(store, g).into_iter().map(|(m, _)| m).filter(|m| !members.contains(m)).collect())
        .unwrap_or_default();
    let candidate_names: Vec<String> = candidates.iter().map(|g| sets::generalization_name(store, *g)).collect();
    let member_rows: Vec<(Obj, Uuid, String)> =
        members.iter().filter_map(|g| Some((*g, store.uuid(*g).ok()?, sets::generalization_name(store, *g)))).collect();
    // Name, written on Enter or leaving the field, given up on Escape, never sent empty; the name
    // field takes the keyboard when the set was just made.
    let focus = uml_mut(cx.notation_states).and_then(|uml| uml.set_name_focus.take()) == Some(id);
    let mut text = match &cx.panels.name {
        Some(n) if n.target == id => n.text.clone(),
        _ => subject.name.clone(),
    };
    let set = "uml.generalization-set.set";
    let out = edit_field_cx(
        cx,
        ui,
        p,
        pos2(left, y),
        half,
        "workspace.properties.name",
        &tr("Name"),
        &mut text,
        archforge_uml::commands::MAX_NAME,
        focus,
        true,
    );
    if out == FieldOut::Cancel {
        cx.panels.name = None;
    } else if out == FieldOut::Commit {
        cx.panels.name = None;
        if text.trim() != subject.name && !crate::panels::refuse_empty_name_cx(cx, set, &text) {
            match run_cx(cx, set, json!({ "set": id.to_string(), "name": text.trim() })) {
                Ok(_) => cx.panels.notice = None,
                Err(e) => cx.panels.notice = Some(e),
            }
        }
    } else if text != subject.name {
        cx.panels.name = Some(crate::panels::NameEdit { target: id, text });
    } else if cx.panels.name.as_ref().is_some_and(|n| n.target == id) {
        cx.panels.name = None;
    }
    // Its general class, read only: it follows the members.
    let paint = ui.painter().clone();
    let at = pos2(left + half + 8.0, y);
    ux::text(&paint, at, &tr("General class"), types::CAPTION, p.text_secondary, Some(half));
    let field = Rect::from_min_size(at + vec2(0.0, 17.0), vec2(half, FIELD));
    paint.rect_filled(field, 6.0, p.field);
    ux::text_mid(&paint, field.left() + 8.0, field.center().y, &general_name, types::SMALL, p.text_secondary, Some((half - 16.0).max(0.0)));
    control_cx(cx, ui, "workspace.properties.generalizationSet.general", "field", field, false)
        .accessible_name(format!("{}: {general_name}", tr("General class")));
    y += 17.0 + FIELD + 14.0;
    let set_flag = |cx: &mut Ctx, key: &str, v: bool| {
        if let Err(e) = run_cx(cx, "uml.generalization-set.set", json!({ "set": id.to_string(), key: v })) {
            cx.panels.notice = Some(e);
        }
    };
    let choices = [("complete", tr("Complete")), ("incomplete", tr("Incomplete"))];
    if let Some(v) = segmented(cx, ui, p, pos2(left, y), "workspace.properties.generalizationSet", &tr("Completeness"), choices, complete) {
        set_flag(cx, "complete", v);
    }
    y += 17.0 + FIELD + 12.0;
    let choices = [("disjoint", tr("Disjoint")), ("overlapping", tr("Overlapping"))];
    if let Some(v) = segmented(cx, ui, p, pos2(left, y), "workspace.properties.generalizationSet", &tr("Disjointness"), choices, disjoint) {
        set_flag(cx, "disjoint", v);
    }
    y += 17.0 + FIELD + 12.0;
    y = help(ui, p, pos2(left, y), width, &meaning(&braces, &general_name, complete, disjoint)) + 18.0;
    // Members · n, `+` offering the generalizations to the general class not in it yet.
    let title = fmt(&tr("Members · {n}"), &[("n", &member_rows.len().to_string())]);
    let add_id = "workspace.properties.generalizationSet.members.add";
    let plus = Rect::from_min_size(pos2(r.right() - PAD - 28.0, y + 11.0), vec2(28.0, 28.0));
    match section_cx(cx, ui, p, r, y, &title, Some((add_id, "Add member"))) {
        Some(next) => y = next - 6.0,
        None => {
            cx.panels.menu = Some(menus::SET_MEMBER);
            y += 44.0;
        }
    }
    if cx.panels.menu == Some(menus::SET_MEMBER) {
        let items = if candidate_names.is_empty() {
            vec![fmt(&tr("No other generalization to {general}"), &[("general", &general_name)])]
        } else {
            candidate_names.clone()
        };
        if let Some(i) = menu_cx(cx, ui, p, plus, add_id, &items)
            && let Some(g) = candidates.get(i)
        {
            add_to(cx, s, &[*g]);
        }
    }
    for (g, gid, text) in &member_rows {
        let row = Rect::from_min_size(pos2(left, y), vec2(width, ROW));
        let (edit, remove) = list_row(cx, ui, p, row, &format!("workspace.properties.generalizationSet.member.{gid}"), text, "Remove from set");
        if remove {
            remove_from(cx, s, *g);
        } else if edit {
            crate::panels::select_element_cx(cx, *gid);
        }
        y += ROW;
    }
    y += 6.0;
    let said = fmt(&tr("Add member offers only generalizations to {general}. Edit opens that generalization."), &[("general", &general_name)]);
    y = help(ui, p, pos2(left, y), width, &said) + 20.0;
    paint.rect_filled(Rect::from_min_size(pos2(r.left(), y), vec2(r.width(), 1.0)), 0.0, p.border);
    #[rustfmt::skip]
    let said = tr("Deleting the set keeps its generalizations; only the grouping goes. The set line is drawn across its members; drag its label to move it, a diagram-only change.");
    let bottom = help(ui, p, pos2(left, y + 16.0), width, &said);
    crate::widgets::record_visible(
        ui,
        cx.registry,
        "workspace.properties.generalizationSet.help",
        "text",
        Rect::from_min_max(pos2(left, y), pos2(left + width, bottom)),
    );
}

/// A generalization's sets in its Properties (B4): `Generalization sets · n` with `+` (a new set of
/// this generalization, or another set of its general class to join), a row per set with its
/// braces, edit (the set) and remove (out of the set); a refused reconnection on the danger tint.
/// Gives the y under it.
pub(crate) fn generalization_section(cx: &mut Ctx, ui: &egui::Ui, p: &Palette, r: Rect, mut y: f32, g: Obj, gid: Uuid) -> f32 {
    let Some(store) = cx.session.as_ref().map(|s| s.doc()) else { return y };
    let width = (r.width() - 2.0 * PAD).max(0.0);
    let left = r.left() + PAD;
    let in_sets: Vec<(Obj, Uuid, String)> =
        sets::sets_of(store, g).into_iter().filter_map(|s| Some((s, store.uuid(s).ok()?, sets::label(store, s)))).collect();
    let general = sets::general(store, g);
    let joinable: Vec<Obj> =
        general.map(|c| sets::sets_for(store, c)).unwrap_or_default().into_iter().filter(|s| !in_sets.iter().any(|(o, _, _)| o == s)).collect();
    let mut offers = vec![tr("New set from this generalization")];
    offers.extend(joinable.iter().map(|s| fmt(&tr("Add to {name}"), &[("name", &sets::label(store, *s))])));
    let title = fmt(&tr("Generalization sets · {n}"), &[("n", &in_sets.len().to_string())]);
    let add_id = "workspace.properties.generalization.sets.add";
    let plus = Rect::from_min_size(pos2(r.right() - PAD - 28.0, y + 11.0), vec2(28.0, 28.0));
    match section_cx(cx, ui, p, r, y, &title, Some((add_id, "Add to a generalization set"))) {
        Some(next) => y = next - 6.0,
        None => {
            cx.panels.menu = Some(menus::SET_JOIN);
            y += 44.0;
        }
    }
    if cx.panels.menu == Some(menus::SET_JOIN)
        && let Some(i) = menu_cx(cx, ui, p, plus, add_id, &offers)
    {
        match i.checked_sub(1).and_then(|k| joinable.get(k)) {
            Some(s) => add_to(cx, *s, &[g]),
            None => {
                if let Some(general) = general {
                    create(cx, &[g], general);
                }
            }
        }
    }
    if in_sets.is_empty() {
        ux::text(ui.painter(), pos2(left + 10.0, y + 9.0), &tr("Not in a set yet."), types::UI, p.text_muted, Some(width));
        y += ROW;
    }
    for (s, sid, text) in &in_sets {
        let row = Rect::from_min_size(pos2(left, y), vec2(width, ROW));
        let (edit, remove) = list_row(cx, ui, p, row, &format!("workspace.properties.generalization.set.{sid}"), text, "Remove from set");
        if remove {
            remove_from(cx, *s, g);
        } else if edit {
            crate::panels::select_element_cx(cx, *sid);
        }
        y += ROW;
    }
    y += 6.0;
    y = help(ui, p, pos2(left, y), width, &tr("Membership is listed here for every set, and each set draws its own line and label on the canvas."))
        + 14.0;
    if let Some(why) =
        uml(cx.notation_states).and_then(|uml| uml.set_refused.as_ref()).filter(|(of, _)| *of == gid && !in_sets.is_empty()).map(|(_, w)| w.clone())
    {
        let paint = ui.painter().clone();
        let text = ux::galley_rows(&paint, &why, types::SMALL, p.text, Some((width - 42.0).max(0.0)), 8);
        let card = Rect::from_min_size(pos2(left, y), vec2(width, text.size().y + 20.0));
        paint.rect_filled(card, 6.0, p.danger_tint);
        ux::icon_in(&paint, Rect::from_min_size(pos2(left + 10.0, y + 11.5), vec2(14.0, 14.0)), &icons::WARNING, p.danger);
        paint.galley(pos2(left + 32.0, y + 10.0), text, p.text.into());
        crate::widgets::record_visible(ui, cx.registry, "workspace.properties.generalization.sets.refusal", "text", card);
        y = card.bottom() + 12.0;
    }
    if !in_sets.is_empty() {
        y = help(
            ui,
            p,
            pos2(left, y),
            width,
            &tr("Deleting this generalization takes it out of its sets in the same undo step; a set left with no members goes too."),
        ) + 14.0;
    }
    y
}

/// Properties for a selection holding generalizations (B1): its heading, how they relate, and the
/// set actions as buttons, reachable from the keyboard, disabled with the reason for a mixed one.
pub(crate) fn selection_body(cx: &mut Ctx, ui: &mut egui::Ui, p: &Palette, r: Rect, y: f32, all: &[Obj]) {
    let Some(store) = cx.session.as_ref().map(|s| s.doc()) else { return };
    let paint = ui.painter().clone();
    let width = (r.width() - 2.0 * PAD).max(0.0);
    let left = r.left() + PAD;
    let n = u64::try_from(all.len()).unwrap_or(u64::MAX);
    let generalizations = all.iter().all(|o| sets::is_generalization(store, *o));
    let heading = if generalizations { trn("{n} generalization", "{n} generalizations", n) } else { trn("{n} element", "{n} elements", n) };
    let grouping = grouping(store, all);
    let kind = match &grouping {
        Grouping::Shared { general, .. } => fmt(&tr("Selection · all specialise {general}"), &[("general", &name(store, *general))]),
        Grouping::Refused { .. } => tr("Selection · mixed"),
    };
    let in_any = all.iter().any(|g| !sets::sets_of(store, *g).is_empty());
    let (live, why, general, gens) = match &grouping {
        Grouping::Shared { general, gens } => (true, None, Some(*general), gens.clone()),
        Grouping::Refused { why, .. } => (false, Some(why.clone()), None, Vec::new()),
    };
    // Each set of their class, with whether they are all in it already.
    let own: Vec<(Obj, String, bool)> = general
        .map(|g| sets::sets_for(store, g))
        .unwrap_or_default()
        .into_iter()
        .map(|s| {
            let members = sets::members(store, s);
            (s, fmt(&tr("Add to {name}"), &[("name", &name(store, s))]), gens.iter().all(|g| members.contains(g)))
        })
        .collect();
    ux::text(&paint, r.min + vec2(PAD, y), &heading, types::CARD_TITLE, p.text, Some(width));
    ux::text(&paint, r.min + vec2(PAD, y + 26.0), &kind, types::CAPTION, p.text_secondary, Some(width));
    let mut y = r.top() + y + 51.0;
    y = section_cx(cx, ui, p, r, y, &tr("Generalization sets"), None).unwrap_or(y + 50.0);
    let line = if in_any { tr("Some of these are in a set already.") } else { tr("None of these is in a set yet.") };
    ux::text(&paint, pos2(left, y), &line, types::UI, p.text, Some(width));
    y += 30.0;
    let button = |cx: &mut Ctx, at: Pos2, id: &str, label: &str, live: bool| {
        let g = ux::galley(&paint, label, types::UI, if live { p.text } else { p.text_muted }, None);
        let b = Rect::from_min_size(at, vec2((g.size().x + 24.0).min(width), FIELD));
        paint.rect_stroke(b, 6.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
        paint.galley(pos2(b.left() + 12.0, b.center().y - g.size().y / 2.0), g, if live { p.text.into() } else { p.text_muted.into() });
        let resp = if live {
            control_cx(cx, ui, id, "button", b, true)
        } else {
            crate::widgets::record_visible(ui, cx.registry, id, "button", b);
            let resp = ui.interact(b, crate::widgets::egui_id(id), Sense::hover());
            let reason = why.clone().unwrap_or_default();
            ui.ctx().accesskit_node_builder(crate::widgets::egui_id(id), |node| {
                node.set_role(egui::accesskit::Role::Button);
                node.set_disabled();
                node.set_description(reason.clone());
            });
            resp.on_hover_text(reason)
        };
        (resp.accessible_name(label).clicked() && live, b.bottom())
    };
    let (clicked, bottom) = button(cx, pos2(left, y), PANEL_CREATE_ID, &tr("Create set from selection"), live);
    y = bottom + 8.0;
    if clicked && let Some(general) = general {
        create(cx, &gens, general);
        return;
    }
    for (i, (s, label, all_in)) in own.iter().enumerate() {
        let (clicked, bottom) = button(cx, pos2(left, y), &format!("workspace.properties.selection.addToGeneralizationSet.{i}"), label, !all_in);
        y = bottom + 8.0;
        if clicked {
            add_to(cx, *s, &gens);
            return;
        }
    }
    y += 6.0;
    if let Some(why) = &why {
        let text = ux::galley_rows(&paint, why, types::SMALL, p.text, Some((width - 42.0).max(0.0)), 8);
        let card = Rect::from_min_size(pos2(left, y), vec2(width, text.size().y + 20.0));
        paint.rect_filled(card, 6.0, p.danger_tint);
        ux::icon_in(&paint, Rect::from_min_size(pos2(left + 10.0, y + 11.5), vec2(14.0, 14.0)), &icons::WARNING, p.danger);
        paint.galley(pos2(left + 32.0, y + 10.0), text, p.text.into());
        crate::widgets::record_visible(ui, cx.registry, "workspace.properties.selection.generalizationSet.reason", "text", card);
        y = card.bottom() + 12.0;
    }
    #[rustfmt::skip]
    let said = fmt(&tr("Same action as the context menu's Create generalization set… ({chord}). Only generalizations to one class form a set; a mixed selection disables it and says why."), &[("chord", chord(ui))]);
    help(ui, p, pos2(left, y), width, &said);
}

/// Where a generalization set shows on screen: its dashed line across its members' lines and its
/// label's rectangle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SetPlace {
    pub line: [Pos2; 2],
    pub label: Rect,
}

/// Where a set shows (AR-129), from its members' routes on screen, each from the specific class
/// to the general one: the set line runs square across them at a level along their common
/// direction, `nth` steps of 26 px towards the general class for the `nth` set of the same lines,
/// and the label of `size` sits on the general's side of it, beside the middle member. A label
/// moved there (`placed`) sets the level instead: the line runs just past its edge.
pub(crate) fn set_place(routes: &[Vec<Pos2>], nth: usize, size: Vec2, placed: Option<Rect>, z: f32) -> Option<SetPlace> {
    let routes: Vec<&Vec<Pos2>> = routes.iter().filter(|r| r.len() >= 2 && r.iter().all(|p| p.is_finite())).collect();
    if routes.is_empty() {
        return None;
    }
    let sum = routes.iter().fold(Vec2::ZERO, |acc, r| match (r.first(), r.last()) {
        (Some(a), Some(b)) => acc + (*b - *a),
        _ => acc,
    });
    // Square to the axis the lines mostly run along, as their right-angled routes are.
    let dir = match (sum.x.abs() > sum.y.abs(), sum.x < 0.0, sum.y > 0.0) {
        (true, true, _) => vec2(-1.0, 0.0),
        (true, false, _) => vec2(1.0, 0.0),
        (false, _, true) => vec2(0.0, 1.0),
        (false, _, false) => vec2(0.0, -1.0),
    };
    let side = vec2(-dir.y, dir.x);
    let along = |p: Pos2| p.to_vec2().dot(dir);
    let across = |p: Pos2| p.to_vec2().dot(side);
    // The extents of the label along the line's direction and across it.
    let ext_d = dir.y.abs() * size.y + dir.x.abs() * size.x;
    let ext_n = side.x.abs() * size.x + side.y.abs() * size.y;
    let gap = 4.0 * z;
    let level = match placed {
        Some(r) => along(r.center()) - ext_d / 2.0 - gap,
        None => {
            let spans: Vec<(f32, f32)> = routes
                .iter()
                .filter_map(|r| {
                    let (a, b) = (along(*r.first()?), along(*r.last()?));
                    Some((a.min(b), a.max(b)))
                })
                .collect();
            let lo = spans.iter().map(|s| s.0).fold(f32::MIN, f32::max);
            let hi = spans.iter().map(|s| s.1).fold(f32::MAX, f32::min);
            let base = if lo < hi { lo + 0.3 * (hi - lo) } else { spans.iter().map(|s| (s.0 + s.1) / 2.0).sum::<f32>() / spans.len().max(1) as f32 };
            base + nth.min(16) as f32 * 26.0 * z
        }
    };
    // Where each member's route crosses the level, or its point nearest it.
    let mut crossings: Vec<f32> = routes
        .iter()
        .map(|r| {
            let hit = r.windows(2).find_map(|w| {
                let (a, b) = (along(w[0]), along(w[1]));
                if (a - level) * (b - level) <= 0.0 && (a - b).abs() > 1e-6 {
                    let t = (level - a) / (b - a);
                    Some(across(w[0] + (w[1] - w[0]) * t))
                } else {
                    None
                }
            });
            hit.unwrap_or_else(|| {
                let near = r.iter().copied().fold(r[0], |m, p| if (along(p) - level).abs() < (along(m) - level).abs() { p } else { m });
                across(near)
            })
        })
        .collect();
    crossings.sort_by(f32::total_cmp);
    let (first, last) = (*crossings.first()?, *crossings.last()?);
    let pad = if crossings.len() == 1 { 36.0 * z } else { 24.0 * z };
    let at = |t: f32| (dir * level + side * t).to_pos2();
    let line = [at(first - pad), at(last + pad)];
    let label = match placed {
        Some(r) => r,
        None => {
            let middle = crossings.get(crossings.len() / 2).copied().unwrap_or(first);
            let anchor = at(middle) + side * 8.0 * z + dir * gap;
            Rect::from_center_size(anchor + dir * (ext_d / 2.0) + side * (ext_n / 2.0), size)
        }
    };
    (line.iter().all(|p| p.is_finite()) && label.is_finite()).then_some(SetPlace { line, label })
}

/// The generalization sets of the diagram's lines (AR-129): for each set with a member line here,
/// a dashed line across its members' lines and its label, `name {complete, disjoint}`, beside it,
/// worked out from the model each frame; muted, the accent while the set is selected. A label moved
/// on this diagram sits where its node holds it. Gives the labels drawn, for the next frame's press,
/// each saying "Generalization set" and its whole text on hover.
pub(crate) fn draw_sets(paint: &egui::Painter, d: &Drawing<'_>, registry: &mut crate::widgets::Registry, ui: &egui::Ui) -> Vec<ExtraLabel> {
    let (p, store, z) = (d.p, d.store, d.view.zoom);
    let mut by_set: std::collections::BTreeMap<Obj, Vec<Obj>> = std::collections::BTreeMap::new();
    for e in refs(store, d.diagram, archforge_diagram::diagram::EDGE) {
        let Some(g) = element_of(store, e, archforge_diagram::edge::ELEMENT).filter(|g| sets::is_generalization(store, *g)) else { continue };
        for s in sets::sets_of(store, g) {
            by_set.entry(s).or_default().push(e);
        }
    }
    let mut out = Vec::new();
    // The sets of one general classifier's lines take a level each.
    let mut nth_of: std::collections::BTreeMap<Option<Obj>, usize> = std::collections::BTreeMap::new();
    for (s, edges) in by_set {
        let routes: Vec<Vec<Pos2>> = edges.iter().filter_map(|e| edge_points(d, *e)).collect();
        let nth = nth_of.entry(sets::general_of(store, s)).or_insert(0);
        let k = *nth;
        *nth += 1;
        let name = archforge_core::name_of(store, s);
        let braces = sets::braces(store, s);
        let selected = d.selection.contains(&s);
        let (name_ink, braces_ink, line_ink) = if selected { (p.accent, p.accent, p.accent) } else { (p.text_secondary, p.text_muted, p.text_muted) };
        let name_g = ux::galley(paint, &name, types::CAPTION_STRONG.scaled(z), name_ink, None);
        let braces_g = ux::galley(paint, &braces, types::CAPTION.scaled(z), braces_ink, None);
        let pad = vec2(6.0, 2.0) * z;
        let gap = if name.is_empty() { 0.0 } else { 5.0 * z };
        let size = vec2(name_g.size().x + gap + braces_g.size().x, name_g.size().y.max(braces_g.size().y)) + pad * 2.0;
        let node = sets::label_node(store, d.diagram, s);
        let placed = node.map(|n| Rect::from_min_size(d.node_rect(n).min, size));
        let id = store.uuid(s).ok();
        let placed = match (d.label_drag, id) {
            (Some((dragged, by)), Some(id)) if dragged == id => {
                set_place(&routes, k, size, placed, z).map(|at| Rect::from_min_size(at.label.min + by, size))
            }
            _ => placed,
        };
        let Some(at) = set_place(&routes, k, size, placed, z) else { continue };
        let stroke = Stroke::new((1.2 * z).clamp(1.0, 2.5), line_ink);
        paint.extend(egui::Shape::dashed_line(&at.line, stroke, (5.0 * z).max(2.0), (4.0 * z).max(2.0)));
        let label = at.label;
        paint.rect_filled(label, 4.0 * z, p.canvas);
        if selected {
            paint.rect_stroke(label, 4.0 * z, Stroke::new(1.0, p.accent), StrokeKind::Inside);
        }
        let mut x = label.left() + pad.x;
        let y = label.center().y;
        let name_w = name_g.size().x;
        paint.galley(pos2(x, y - name_g.size().y / 2.0), name_g, name_ink.into());
        x += name_w + gap;
        paint.galley(pos2(x, y - braces_g.size().y / 2.0), braces_g, braces_ink.into());
        let Some(id) = id else { continue };
        crate::widgets::record_visible(ui, registry, &format!("workspace.canvas.generalizationSet.{id}"), "label", label);
        crate::widgets::record_visible(
            ui,
            registry,
            &format!("workspace.canvas.generalizationSet.{id}.line"),
            "line",
            Rect::from_two_pos(at.line[0], at.line[1]).expand(3.0),
        );
        let text = if name.is_empty() { braces } else { format!("{name} {braces}") };
        let hover = fmt(&tr("Generalization set {label}"), &[("label", &text)]);
        out.push(ExtraLabel { element: id, rect: label, node: node.and_then(|n| store.uuid(n).ok()), hover });
    }
    out
}

/// A set's label that no node holds yet, let go on diagram `d` with its top left corner at
/// `(x, y)`: placed there on this diagram alone, a node made to hold it, as one undo step
/// (`uml.generalization-set.place`); a refusal is said at the canvas's top.
pub(crate) fn place_label(cx: &mut Ctx, d: Obj, set: Uuid, x: f64, y: f64) {
    let Some(diagram) = cx.session.as_ref().and_then(|s| s.doc().uuid(d).ok()) else { return };
    let params = json!({ "diagram": diagram.to_string(), "set": set.to_string(), "x": x.round(), "y": y.round() });
    if let Err(e) = run_cx(cx, "uml.generalization-set.place", params) {
        cx.canvas.notice = Some(e);
    }
}

/// Every text that reaches `tr` through a variable, for the catalogue tests.
#[cfg(test)]
pub(crate) fn texts() -> impl Iterator<Item = &'static str> {
    ["Add member", "Remove from set", "Add to a generalization set"].into_iter()
}
