//! Profiles and stereotypes in the shell (AR-134, design page 08, frames F1, F2, F5 and F6):
//! Models' New ▸ on a package (Package, Profile, then the classifiers) and on a profile
//! (Stereotype), each made as one undo step and named in place; a profile's and a stereotype's rows
//! with what they are muted after them (`profile`, `extends Class`), and a package's applied
//! profiles after its name; a profile's Properties with its stereotypes and its diagram; a
//! stereotype's with its name and profile, Extends (the metaclasses Archforge draws, the others
//! listed disabled with why) and its properties through the member editor; and a package's Applied
//! profiles with `+`, Unapply refused while elements use the profile, the uses counted and a button
//! that selects them. The rules are the model's (`archforge_uml::profiles`): the shell asks them
//! and says their answers in the user's words.

use archforge_engine::{InUse, Used};
use archforge_mof::{Obj, Store, Uuid};
use archforge_uml::profiles::{self, Uses};
use egui::{Rect, Sense, Stroke, StrokeKind, pos2, vec2};
use serde_json::json;

use crate::Shell;
use crate::explorer::{Row, Tree, fresh_name};
use crate::home::control;
use crate::i18n::{fmt, tr, tr_ctx, trn};
use crate::panels::{FIELD, Menu, PAD, ROW, Subject, icon_button, run, section, select_field};
use crate::uml_classifier::Classifier;
use crate::ux::{self, Palette, icons, types};

/// The ids of a stereotype's Properties (F1).
pub(crate) const STEREOTYPE_PROFILE_ID: &str = "workspace.properties.stereotype.profile";
pub(crate) const EXTENDS_ADD_ID: &str = "workspace.properties.stereotype.extends.add";
pub(crate) const EXTENDS_ROW_ID: &str = "workspace.properties.stereotype.extends";
pub(crate) const EXTENSION_ID: &str = "workspace.properties.stereotype.extension";
/// The ids of a profile's Properties.
pub(crate) const STEREOTYPES_ADD_ID: &str = "workspace.properties.profile.stereotypes.add";
pub(crate) const STEREOTYPE_ROW_ID: &str = "workspace.properties.profile.stereotype";
pub(crate) const DIAGRAM_ID: &str = "workspace.properties.profile.diagram";
/// The ids of a package's Applied profiles (F5, F6).
pub(crate) const APPLIED_ADD_ID: &str = "workspace.properties.package.profiles.add";
pub(crate) const APPLIED_ROW_ID: &str = "workspace.properties.package.profile";
pub(crate) const APPLIED_EDIT_ID: &str = "workspace.properties.package.profileEditor";
pub(crate) const USES_ID: &str = "workspace.properties.package.profileRefusal";

/// What applying a profile means, and what a stereotype's properties take, under their sections:
/// too long for a line of code, so named here and listed for the catalogue in [`texts`].
const APPLIED_HELP: &str = "Elements inside {name}, and inside its packages, can use the stereotypes of the applied profiles. Applying is one undo step; it adds no stereotype by itself.";
const TYPES_HELP: &str = "Types: primitive types, enumerations and data types of the model. Renaming {name} keeps every application; removing it is refused while elements use it.";

/// The kinds Models' New ▸ makes in a package (F2): a package and a profile, then the classifiers.
const PACKAGE_KINDS: [&str; 2] = ["package", "profile"];

fn name(store: &Store, o: Obj) -> String {
    archforge_core::name_of(store, o)
}

/// An editor open in a stereotype's Extends or a package's Applied profiles: the element whose
/// section it is, the row it edits (a metaclass's name, a profile's id), what it is set to, and
/// whether it was just opened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileEdit {
    pub owner: Uuid,
    pub target: Option<String>,
    pub choice: String,
    pub opened: bool,
}

/// A refusal shown under a section of Profiles' panels (F6): the element whose section it is, the
/// reason in the user's words, and the elements in use, which a button selects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileRefusal {
    pub owner: Uuid,
    pub why: String,
    pub uses: Vec<Uuid>,
}

// What the shell says of uses.

/// The elements a stereotype is used on, in words: one name, two names joined, or how many, `n`,
/// when none is named.
fn on_text(names: &[String], n: usize) -> String {
    match names {
        [one] => one.clone(),
        [a, b] => fmt(&tr("{first} and {second}"), &[("first", a), ("second", b)]),
        _ => trn("{n} element", "{n} elements", u64::try_from(n).unwrap_or(u64::MAX)),
    }
}

/// What is in use and its uses, in the user's words, as `Persistence is in use in Commerce: «Entity»
/// on Order and Customer, «Id» on 2 elements. Remove those first.`: what Profiles' panels say before
/// asking, and what a refusal of the engine's says (AR-437).
pub(crate) fn in_use(used: &InUse) -> String {
    let on = |(s, names, n): &(String, Vec<String>, usize)| {
        fmt(&tr("«{stereotype}» on {elements}"), &[("stereotype", s), ("elements", &on_text(names, *n))])
    };
    let uses = used.uses.iter().map(on).collect::<Vec<_>>().join(", ");
    // A sentence names its two parts by its own keys, so each part is given under every key it goes by.
    let (template, first, second) = match &used.what {
        Used::Element(name) | Used::Moved(name) => (tr("{name} is in use: {uses}. Remove those first."), name, name),
        Used::ProfileIn { profile, package } => (tr("{profile} is in use in {package}: {uses}. Remove those first."), profile, package),
        Used::Extension { stereotype, metaclass } => {
            (tr("«{stereotype}» on {metaclass} is in use: {uses}. Remove those first."), stereotype, metaclass)
        }
        Used::HeldProfile { package, profile } => {
            (tr("{name} holds the profile {profile}, which is in use: {uses}. Remove those first."), profile, package)
        }
        Used::AppliedProfiles { package } => (tr("{name} applies the profiles its elements use: {uses}. Remove those first."), package, package),
    };
    fmt(&template, &[("profile", first), ("stereotype", first), ("name", second), ("package", second), ("metaclass", second), ("uses", &uses)])
}

/// `uses` of `what`, in the user's words.
fn uses_said(store: &Store, what: Used, uses: &Uses) -> String {
    in_use(&InUse { what, uses: profiles::uses_named(store, uses) })
}

/// The ids of the elements in use.
fn use_ids(store: &Store, uses: &Uses) -> Vec<Uuid> {
    uses.elements().into_iter().filter_map(|e| store.uuid(e).ok()).collect()
}

/// Why `obj` may not be removed for its uses, in the user's words; none when the uses allow it.
pub(crate) fn removal_refusal(store: &Store, obj: Obj) -> Option<String> {
    match profiles::removal_problem(store, obj)?.reason? {
        archforge_engine::Reason::InUse(used) => Some(in_use(&used)),
        _ => None,
    }
}

// Models.

/// What a profile's or a stereotype's row says after its name: `profile`, `extends Class,
/// Property`, or `extends nothing yet`.
fn stereotype_note(store: &Store, s: Obj) -> String {
    let extended = profiles::extended(store, s);
    if extended.is_empty() { tr("extends nothing yet") } else { fmt(&tr("extends {metaclasses}"), &[("metaclasses", &extended.join(", "))]) }
}

/// What a package's row says after its name when it applies profiles: `applies Persistence`.
pub(crate) fn package_note(store: &Store, package: Obj) -> Option<String> {
    let applied: Vec<String> = profiles::applied(store, package).into_iter().map(|(_, p)| name(store, p)).collect();
    (!applied.is_empty()).then(|| fmt(&tr("applies {profiles}"), &[("profiles", &applied.join(", "))]))
}

/// A profile's rows at `depth`: the profile, `profile` after its name, then, when open, its
/// stereotypes, each `extends …` after its name with its properties under it.
pub(crate) fn profile_rows(store: &Store, profile: Obj, depth: u8, t: &Tree<'_>) -> Vec<Row> {
    let mut under = Vec::new();
    for s in profiles::stereotypes(store, profile) {
        under.extend(stereotype_rows(store, s, depth.saturating_add(1), t));
    }
    let label = name(store, profile);
    if !t.matches(&label) && under.is_empty() {
        return Vec::new();
    }
    let has = !profiles::stereotypes(store, profile).is_empty();
    let expandable = if t.filtering() { !under.is_empty() } else { has };
    let expanded = expandable && t.open(profile);
    let mut out: Vec<Row> = t
        .row(profile, depth, "profile", &icons::PROFILE, label)
        .map(|row| Row { expandable, expanded, note: Some(tr("profile")), ..row })
        .into_iter()
        .collect();
    if expanded {
        out.extend(under);
    }
    out
}

/// A stereotype's rows: itself, draggable onto its profile's diagram, then, when open, its
/// properties with their types.
fn stereotype_rows(store: &Store, s: Obj, depth: u8, t: &Tree<'_>) -> Vec<Row> {
    let mut members = Vec::new();
    if t.filtering() || t.open(s) {
        for a in profiles::properties(store, s) {
            let label = name(store, a);
            if t.matches(&label) {
                let ty = archforge_uml::figures::attribute_text(store, a);
                let detail = ty.split_once(" : ").map(|(_, rest)| rest.to_string());
                members.extend(t.row(a, depth.saturating_add(1), "attribute", &icons::ATTRIBUTE, label).map(|row| Row { detail, ..row }));
            }
        }
    }
    let label = name(store, s);
    if !t.matches(&label) && members.is_empty() {
        return Vec::new();
    }
    let has = !profiles::properties(store, s).is_empty();
    let expandable = if t.filtering() { !members.is_empty() } else { has };
    let expanded = expandable && t.open(s);
    let mut out: Vec<Row> = t
        .row(s, depth, "stereotype", &icons::STEREOTYPE, label)
        .map(|row| Row { expandable, expanded, draggable: true, note: Some(stereotype_note(store, s)), ..row })
        .into_iter()
        .collect();
    if expanded {
        out.extend(members);
    }
    out
}

/// The rows of a profile and its stereotypes that open, for Expand all.
pub(crate) fn foldable(store: &Store, profile: Obj) -> Vec<Obj> {
    let stereotypes = profiles::stereotypes(store, profile);
    let mut out: Vec<Obj> = if stereotypes.is_empty() { Vec::new() } else { vec![profile] };
    out.extend(stereotypes.into_iter().filter(|s| !profiles::properties(store, *s).is_empty()));
    out
}

/// What Models' New ▸ offers on a package (F2) or a profile: a package, a profile and the
/// classifiers in a package; a stereotype in a profile.
pub(crate) fn inside(store: &Store, o: Obj) -> Option<crate::notation_ui::Inside> {
    let owner = name(store, o);
    let heading = fmt(&tr("New in {name}"), &[("name", &owner)]);
    if profiles::is_profile(store, o) {
        return Some(crate::notation_ui::Inside {
            word: tr("New"),
            heading,
            kinds: vec![("stereotype", tr("Stereotype"), &icons::STEREOTYPE)],
            refused: Vec::new(),
            why_not: None,
            split: None,
        });
    }
    if store.class_of(o).ok() != Some(archforge_uml::PACKAGE) {
        return None;
    }
    let mut kinds: Vec<(&'static str, String, &'static ux::Icon)> =
        vec![(PACKAGE_KINDS[0], tr("Package"), &icons::PACKAGE), (PACKAGE_KINDS[1], tr_ctx("uml", "Profile"), &icons::PROFILE)];
    kinds.extend(Classifier::ALL.iter().map(|k| (k.key(), k.word(), k.icon())));
    Some(crate::notation_ui::Inside { word: tr("New"), heading, kinds, refused: Vec::new(), why_not: None, split: Some(PACKAGE_KINDS.len()) })
}

/// Makes what New ▸ `key` names inside the package or profile `o`, named afresh among what it holds
/// of that kind, as one undo step; Models opens the rows above it and its name for typing.
pub(crate) fn add_inside(shell: &mut Shell, key: &str, o: Obj) -> Option<Result<Uuid, String>> {
    let store = shell.session.as_ref()?.doc();
    let id = store.uuid(o).ok()?.to_string();
    let taken = |class: archforge_mof::ClassId| {
        archforge_uml::nesting::names_under(store, o)
            .into_iter()
            .filter(|x| store.class_of(*x).is_ok_and(|c| archforge_uml::names::kind(c) == archforge_uml::names::kind(class)))
            .map(|x| name(store, x))
            .collect::<Vec<_>>()
    };
    let (stem, cmd, params): (String, &str, Box<dyn Fn(&str) -> serde_json::Value>) = if profiles::is_profile(store, o) {
        if key != "stereotype" {
            return None;
        }
        let names = taken(archforge_uml::STEREOTYPE);
        let fresh = fresh_name("Stereotype", names.into_iter(), false);
        (fresh.unwrap_or_default(), "uml.stereotype.add", Box::new(move |n| json!({ "profile": id, "name": n })))
    } else {
        match key {
            "package" => {
                let fresh = fresh_name("Package", taken(archforge_uml::PACKAGE).into_iter(), false);
                (fresh.unwrap_or_default(), "uml.package.add", Box::new(move |n| json!({ "parent": id, "name": n })))
            }
            "profile" => {
                let fresh = fresh_name("Profile", taken(archforge_uml::PROFILE).into_iter(), false);
                (fresh.unwrap_or_default(), "uml.profile.add", Box::new(move |n| json!({ "package": id, "name": n })))
            }
            key => {
                let kind = Classifier::ALL.into_iter().find(|k| k.key() == key)?;
                let fresh = fresh_name(kind.stem(), taken(archforge_uml::CLASS).into_iter(), false);
                (fresh.unwrap_or_default(), kind.add_command(), Box::new(move |n| json!({ "package": id, "name": n })))
            }
        }
    };
    if stem.is_empty() {
        return Some(Err(tr("No free name is left")));
    }
    Some(made_in_place(shell, cmd, params(&stem), stem))
}

/// Runs `cmd` and opens the new element's name for typing in Models, its rows above open.
fn made_in_place(shell: &mut Shell, cmd: &str, params: serde_json::Value, fresh: String) -> Result<Uuid, String> {
    let out = run(shell, cmd, params)?;
    let id = out.get("id").and_then(|v| v.as_str()).and_then(Uuid::parse).ok_or_else(|| tr("The model could not be opened"))?;
    if let Some(store) = shell.session.as_ref().map(|s| s.doc())
        && let Some(c) = store.by_uuid(id)
        && let Some(ui) = crate::notation_ui::of(shell.notation_uis, store, c)
    {
        let parents: Vec<Uuid> = ui.parents(store, c).into_iter().filter_map(|p| store.uuid(p).ok()).collect();
        shell.explorer.expanded.extend(parents);
    }
    shell.explorer.filter.clear();
    shell.explorer.rename = Some(crate::explorer::Rename::fresh(id, fresh));
    shell.explorer.focused = true;
    shell.state.show_explorer(crate::state::Explorer::Models);
    Ok(id)
}

// Properties.

/// A pickable or refused row of a profile menu: its words, the detail at its right (a reason, the
/// profile it is from), whether it may be picked, whether it is ticked, and the whole reason.
#[derive(Clone)]
struct Pick {
    label: String,
    detail: String,
    live: bool,
    ticked: bool,
    why: Option<String>,
}

/// A menu under `anchor` of `rows` headed `heading`, the refused ones muted with their reason on
/// hover and for screen readers; the arrows walk the live rows, Enter picks, Escape or a press
/// elsewhere closes. Gives the row picked.
fn pick_menu(shell: &mut Shell, ui: &egui::Ui, p: &Palette, anchor: Rect, id: &str, heading: &str, rows: &[Pick]) -> Option<usize> {
    let window = ui.ctx().content_rect();
    let width = 260.0_f32.max(anchor.width()).min((window.width() - 16.0).max(anchor.width()));
    let head = 28.0;
    let full = 8.0 + head + rows.len() as f32 * 30.0;
    let below = window.bottom() - 8.0 - (anchor.bottom() + 4.0);
    let height = full.min(below.max(8.0 + head + 30.0));
    let left = (anchor.right() - width).min(window.right() - 8.0 - width).max(window.left() + 8.0);
    let m = Rect::from_min_size(pos2(left, anchor.bottom() + 4.0), vec2(width, height));
    let live: Vec<usize> = rows.iter().enumerate().filter(|(_, r)| r.live).map(|(i, _)| i).collect();
    let picked = ux::overlay(ui, crate::widgets::egui_id(&format!("{id}.layer")), m, |ui| {
        let paint = ui.painter().clone();
        ux::shadow(&paint, m, 8.0, p.shadow);
        paint.rect_filled(m, 8.0, p.surface);
        paint.rect_stroke(m, 8.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
        ux::text(
            &paint,
            pos2(m.left() + 14.0, m.top() + 12.0),
            &crate::canvas::eyebrow_case(heading),
            types::EYEBROW,
            p.text_muted,
            Some(m.width() - 28.0),
        );
        let mut reached = shell.panels.menu_reached.as_ref().filter(|(of, _)| of == id).map(|(_, i)| *i);
        let (down, up, enter) = ui.input_mut(|i| {
            (
                i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown),
                i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp),
                reached.is_some() && i.consume_key(egui::Modifiers::NONE, egui::Key::Enter),
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
        shell.panels.menu_reached = reached.map(|i| (id.to_string(), i));
        let list = Rect::from_min_max(pos2(m.left(), m.top() + 4.0 + head), pos2(m.right(), m.bottom() - 4.0));
        let most = (rows.len() as f32 * 30.0 - list.height()).max(0.0);
        let mut offset = shell.panels.menu_scroll.as_ref().filter(|(of, _)| of == id).map_or(0.0, |(_, o)| *o);
        if ui.input(|i| i.pointer.hover_pos().is_some_and(|at| m.contains(at))) {
            offset -= ui.input(|i| i.smooth_scroll_delta.y);
        }
        let offset = offset.clamp(0.0, most);
        shell.panels.menu_scroll = Some((id.to_string(), offset));
        let clipped = paint.with_clip_rect(list);
        let mut picked = None;
        for (i, row) in rows.iter().enumerate() {
            let r = Rect::from_min_size(pos2(m.left() + 4.0, list.top() - offset + i as f32 * 30.0), vec2(m.width() - 8.0, 30.0));
            if !list.intersects(r) {
                continue;
            }
            let rid = format!("{id}.{i}");
            let resp = if row.live {
                control(shell, ui, &rid, "menu-item", r.intersect(list), true).accessible_name(row.label.as_str())
            } else {
                crate::widgets::record_visible(ui, &mut shell.registry, &rid, "menu-item", r.intersect(list));
                let resp = ui.interact(r.intersect(list), crate::widgets::egui_id(&rid), Sense::hover());
                let why = row.why.clone().unwrap_or_else(|| row.detail.clone());
                ui.ctx().accesskit_node_builder(crate::widgets::egui_id(&rid), |node| {
                    node.set_role(egui::accesskit::Role::MenuItem);
                    node.set_disabled();
                    node.set_description(why.clone());
                });
                resp.on_hover_text(why).accessible_name(row.label.as_str())
            };
            if row.live && (resp.hovered() || reached == Some(i)) {
                clipped.rect_filled(r, 5.0, p.page);
            }
            let ink = if !row.live {
                p.text_muted
            } else if row.ticked {
                p.accent
            } else {
                p.text
            };
            let tick = if row.ticked { 22.0 } else { 0.0 };
            let detail = ux::galley(&clipped, &row.detail, types::CAPTION, p.text_muted, Some(m.width() / 2.0));
            let dw = detail.size().x;
            clipped.galley(pos2(r.right() - 10.0 - tick - dw, r.center().y - detail.size().y / 2.0), detail, p.text_muted);
            if row.ticked {
                ux::icon_mid(&clipped, pos2(r.right() - 18.0, r.center().y), &icons::TICK, p.accent);
            }
            ux::text_mid(&clipped, r.left() + 10.0, r.center().y, &row.label, types::SMALL, ink, Some((r.width() - 30.0 - tick - dw).max(0.0)));
            if row.live && (resp.clicked() || (enter && reached == Some(i))) {
                picked = Some(i);
            }
        }
        picked
    });
    let outside = ui.input(|i| i.pointer.any_pressed() && !i.pointer.interact_pos().is_some_and(|pos| m.contains(pos) || anchor.contains(pos)));
    if picked.is_some() || outside || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        shell.panels.menu = None;
        shell.panels.menu_reached = None;
    }
    picked
}

/// A muted line of help at `y`, `width` wide; gives the y under it.
fn help(ui: &egui::Ui, p: &Palette, left: f32, y: f32, width: f32, text: &str) -> f32 {
    let paint = ui.painter();
    let g = ux::galley_rows(paint, text, types::CAPTION, p.text_muted, Some(width.max(0.0)), 6);
    let tall = g.size().y;
    paint.galley(pos2(left, y), g, p.text_muted);
    y + tall + 14.0
}

/// A row of a section: its icon, its label and its edit button, a click selecting `select`; gives
/// whether the edit button was pressed.
#[allow(clippy::too_many_arguments, reason = "a row is drawn for its section, label and marks")]
fn row(
    shell: &mut Shell,
    ui: &egui::Ui,
    p: &Palette,
    at: egui::Pos2,
    width: f32,
    id: &str,
    label: &str,
    icon: &'static ux::Icon,
    marked: bool,
    edit: bool,
) -> (bool, bool) {
    let paint = ui.painter().clone();
    let r = Rect::from_min_size(at, vec2(width, ROW));
    let resp = control(shell, ui, id, "row", r, true).accessible_name(label);
    if marked {
        paint.rect_filled(r, 6.0, p.accent_tint);
    } else if resp.contains_pointer() {
        paint.rect_filled(r, 6.0, p.page);
    }
    ux::icon_mid(&paint, pos2(r.left() + 17.0, r.center().y), icon, p.text_secondary);
    ux::text_mid(&paint, r.left() + 34.0, r.center().y, label, types::UI, p.text, Some((width - 80.0).max(0.0)));
    let edited = edit && crate::panels::edit_button(shell, ui, p, r, id, resp.contains_pointer());
    (resp.clicked(), edited || (edit && resp.double_clicked()))
}

/// The refusal under a section (F6): the danger's tint, its warning and the reason, then a button
/// that selects the elements in use; gives the y under it.
fn refusal(shell: &mut Shell, ui: &egui::Ui, p: &Palette, left: f32, y: f32, width: f32, r: &ProfileRefusal) -> f32 {
    let paint = ui.painter().clone();
    let text = ux::galley_rows(&paint, &r.why, types::SMALL, p.text, Some((width - 42.0).max(0.0)), 8);
    let card = Rect::from_min_size(pos2(left, y), vec2(width, text.size().y + 20.0));
    paint.rect_filled(card, 6.0, p.danger_tint);
    ux::icon_in(&paint, Rect::from_min_size(pos2(left + 10.0, y + 11.5), vec2(14.0, 14.0)), &icons::WARNING, p.danger);
    paint.galley(pos2(left + 32.0, y + 10.0), text, p.text);
    crate::widgets::record_visible(ui, &mut shell.registry, USES_ID, "text", card);
    let mut y = card.bottom() + 12.0;
    if !r.uses.is_empty() {
        let n = u64::try_from(r.uses.len()).unwrap_or(u64::MAX);
        let label = trn("Show the use", "Show the {n} uses", n);
        let g = ux::galley(&paint, &label, types::UI, p.text, None);
        let b = Rect::from_min_size(pos2(left, y), vec2((g.size().x + 24.0).min(width), FIELD));
        paint.rect_stroke(b, 6.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
        paint.galley(pos2(b.left() + 12.0, b.center().y - g.size().y / 2.0), g, p.text);
        if control(shell, ui, &format!("{USES_ID}.show"), "button", b, true).accessible_name(label).clicked() {
            let ids: Vec<String> = r.uses.iter().map(Uuid::to_string).collect();
            shell.panels.profile_refused = None;
            if let Err(e) = run(shell, "view.select", json!({ "ids": ids })) {
                shell.panels.notice = Some(e);
            }
        }
        y = b.bottom() + 12.0;
    }
    y
}

/// A package's Applied profiles at `y` (F5, F6): its title with the count and a `+` whose menu
/// lists every profile of the model, those applied ticked and disabled; a row per applied profile
/// with its edit button, whose editor unapplies it; a refusal under the rows, the uses counted, and
/// what applying means. Gives the y under it.
pub(crate) fn applied_section(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect, mut y: f32, package: Obj, package_id: Uuid) -> f32 {
    let Some(store) = shell.session.as_ref().map(|s| s.doc()) else { return y };
    let applied: Vec<(Uuid, String)> =
        profiles::applied(store, package).into_iter().filter_map(|(_, x)| Some((store.uuid(x).ok()?, name(store, x)))).collect();
    let all: Vec<(Uuid, String)> = profiles::all_profiles(store).into_iter().filter_map(|x| Some((store.uuid(x).ok()?, name(store, x)))).collect();
    let own = name(store, package);
    let title = fmt(&tr("Applied profiles · {n}"), &[("n", &applied.len().to_string())]);
    let add = Rect::from_min_size(pos2(r.right() - PAD - 28.0, y + 11.0), vec2(28.0, 28.0));
    match section(shell, ui, p, r, y, &title, Some((APPLIED_ADD_ID, "Apply profile"))) {
        Some(next) => y = next - 6.0,
        None => {
            shell.panels.menu = if shell.panels.menu == Some(Menu::AppliedProfile) { None } else { Some(Menu::AppliedProfile) };
            y += 44.0;
        }
    }
    let width = (r.width() - 2.0 * PAD).max(0.0);
    let left = r.left() + PAD;
    let editing = shell.panels.profile_edit.as_ref().filter(|e| e.owner == package_id).cloned();
    for (id, label) in &applied {
        let key = format!("{APPLIED_ROW_ID}.{id}");
        let target = id.to_string();
        let open = editing.as_ref().is_some_and(|e| e.target.as_deref() == Some(target.as_str()));
        let refused = shell.panels.profile_refused.as_ref().is_some_and(|x| x.owner == package_id);
        let (clicked, edit) = row(shell, ui, p, pos2(left, y), width, &key, label, &icons::PROFILE, open || refused, true);
        y += ROW;
        if clicked {
            crate::panels::select_element(shell, *id);
        }
        if edit {
            shell.panels.profile_edit = Some(ProfileEdit { owner: package_id, target: Some(target.clone()), choice: target.clone(), opened: true });
            shell.panels.profile_refused = None;
        }
        if open {
            y = unapply_editor(shell, ui, p, left, y, width, package_id, *id, label);
        }
    }
    if let Some(refusal_shown) = shell.panels.profile_refused.clone().filter(|x| x.owner == package_id) {
        y = refusal(shell, ui, p, left, y + 4.0, width, &refusal_shown);
    }
    let said = fmt(&tr(APPLIED_HELP), &[("name", &own)]);
    y = help(ui, p, left, y + 4.0, width, &said);
    if shell.panels.menu == Some(Menu::AppliedProfile) {
        let rows: Vec<Pick> = if all.is_empty() {
            vec![Pick {
                label: tr("No profile yet"),
                detail: String::new(),
                live: false,
                ticked: false,
                why: Some(tr("Make one with New ▸ Profile in Models.")),
            }]
        } else {
            all.iter()
                .map(|(id, n)| {
                    let on = applied.iter().any(|(a, _)| a == id);
                    Pick {
                        label: n.clone(),
                        detail: if on { tr("applied") } else { String::new() },
                        live: !on,
                        ticked: on,
                        why: on.then(|| tr("Applied already")),
                    }
                })
                .collect()
        };
        let heading = fmt(&tr("Apply to {name}"), &[("name", &own)]);
        if let Some(i) = pick_menu(shell, ui, p, add, APPLIED_ADD_ID, &heading, &rows)
            && let Some((profile, _)) = all.get(i)
        {
            match run(shell, "uml.profile.apply", json!({ "package": package_id.to_string(), "profile": profile.to_string() })) {
                Ok(_) => shell.panels.notice = None,
                Err(e) => shell.panels.notice = Some(e),
            }
        }
    }
    y + 6.0
}

/// The editor of an applied profile's row: its name, Unapply (the bin) and Cancel. Unapply refused
/// for uses closes the editor and shows the refusal under the rows (F6). Gives the y under it.
#[allow(clippy::too_many_arguments, reason = "an editor is drawn for its package and profile")]
fn unapply_editor(shell: &mut Shell, ui: &egui::Ui, p: &Palette, left: f32, y: f32, width: f32, package: Uuid, profile: Uuid, label: &str) -> f32 {
    let paint = ui.painter().clone();
    let card = Rect::from_min_size(pos2(left, y + 4.0), vec2(width, 12.0 + FIELD + 12.0));
    paint.rect_filled(card, 8.0, p.surface);
    paint.rect_stroke(card, 8.0, Stroke::new(1.0, p.accent), StrokeKind::Inside);
    let at = |x: f32| pos2(card.left() + 12.0 + x, card.top() + 12.0);
    let inner = (width - 24.0).max(0.0);
    ux::text_mid(&paint, card.left() + 12.0, at(0.0).y + FIELD / 2.0, label, types::UI, p.text, Some((inner - 2.0 * FIELD - 16.0).max(0.0)));
    let unapply = icon_button(
        shell,
        ui,
        p,
        at(inner - 2.0 * FIELD - 8.0),
        &format!("{APPLIED_EDIT_ID}.unapply"),
        &icons::TRASH,
        p.danger,
        None,
        "Unapply profile",
    );
    let cancel = icon_button(shell, ui, p, at(inner - FIELD), &format!("{APPLIED_EDIT_ID}.cancel"), &icons::CLOSE, p.text_secondary, None, "Cancel");
    if let Some(e) = shell.panels.profile_edit.as_mut() {
        e.opened = false;
    }
    if cancel || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        shell.panels.profile_edit = None;
    } else if unapply {
        shell.panels.profile_edit = None;
        unapply_profile(shell, package, profile);
    }
    card.bottom() + 8.0
}

/// Unapplies `profile` from `package`, or, while elements inside use it, shows why with the uses
/// counted (F6), changing nothing.
pub(crate) fn unapply_profile(shell: &mut Shell, package: Uuid, profile: Uuid) {
    let refused = shell.session.as_ref().and_then(|s| {
        let store = s.doc();
        let (pk, pr) = (store.by_uuid(package)?, store.by_uuid(profile)?);
        let uses = profiles::unapply_uses(store, pk, pr);
        (!uses.is_empty()).then(|| ProfileRefusal {
            owner: package,
            why: uses_said(store, Used::ProfileIn { profile: name(store, pr), package: name(store, pk) }, &uses),
            uses: use_ids(store, &uses),
        })
    });
    if let Some(r) = refused {
        shell.panels.profile_refused = Some(r);
        return;
    }
    match run(shell, "uml.profile.unapply", json!({ "package": package.to_string(), "profile": profile.to_string() })) {
        Ok(_) => {
            shell.panels.notice = None;
            shell.panels.profile_refused = None;
        }
        Err(e) => shell.panels.notice = Some(e),
    }
}

/// A profile's panel: its name, its stereotypes with a `+` that makes one named in place in
/// Models, its diagram's button, and its applied profiles where it is a package's too.
pub(crate) fn profile_body(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect, y: f32, subject: &Subject) {
    let (Some(profile), Some(profile_id)) = (subject.obj, subject.id) else { return };
    let width = (r.width() - 2.0 * PAD).max(0.0);
    let left = r.left() + PAD;
    crate::panels::name_field(shell, ui, p, pos2(left, y), width, subject, "uml.profile.rename", "profile");
    let mut y = y + 17.0 + FIELD + 16.0;
    let Some(store) = shell.session.as_ref().map(|s| s.doc()) else { return };
    let rows: Vec<(Uuid, String, String)> = profiles::stereotypes(store, profile)
        .into_iter()
        .filter_map(|s| Some((store.uuid(s).ok()?, name(store, s), stereotype_note(store, s))))
        .collect();
    let title = fmt(&tr("Stereotypes · {n}"), &[("n", &rows.len().to_string())]);
    match section(shell, ui, p, r, y, &title, Some((STEREOTYPES_ADD_ID, "Add stereotype"))) {
        Some(next) => y = next - 6.0,
        None => {
            if let Some(Err(e)) = add_inside(shell, "stereotype", profile) {
                shell.panels.notice = Some(e);
            }
            y += 44.0;
        }
    }
    let chosen = crate::panels::selected_id(shell);
    for (id, label, note) in &rows {
        let (clicked, _) = row(
            shell,
            ui,
            p,
            pos2(left, y),
            width,
            &format!("{STEREOTYPE_ROW_ID}.{id}"),
            &format!("{label} · {note}"),
            &icons::STEREOTYPE,
            Some(*id) == chosen,
            false,
        );
        if clicked {
            crate::panels::select_element(shell, *id);
        }
        y += ROW;
    }
    if rows.is_empty() {
        y = help(ui, p, left, y + 4.0, width, &tr("None yet. Add one here or from Models ▸ New ▸ Stereotype."));
    }
    y += 8.0;
    if crate::panels::ghost_button(shell, ui, p, pos2(left - 6.0, y), DIAGRAM_ID, &icons::CLASS_DIAGRAM, &tr("Open profile diagram"))
        && let Err(e) = open_profile_diagram(shell, profile_id)
    {
        shell.panels.notice = Some(e);
    }
    y += 36.0;
    help(
        ui,
        p,
        left,
        y,
        width,
        &tr("A package applies the profile through Applied profiles in its Properties; its elements may then use the profile's stereotypes."),
    );
}

/// The metaclasses a stereotype may extend as the menu offers them: those Archforge draws, by
/// UML's names, then those it does not draw yet, refused.
fn extends_rows(extended: &[String], current: Option<&str>) -> Vec<(String, Pick)> {
    let drawn = profiles::METACLASSES.iter().map(|(n, _)| {
        let taken = extended.iter().any(|e| e == n) && Some(*n) != current;
        let on = Some(*n) == current || (current.is_none() && extended.iter().any(|e| e == n));
        (
            n.to_string(),
            Pick {
                label: n.to_string(),
                detail: if on { tr("extended") } else { String::new() },
                live: !taken && !on,
                ticked: on,
                why: (taken || on).then(|| tr("Extended already")),
            },
        )
    });
    let not_drawn = profiles::NOT_DRAWN.iter().map(|n| {
        (
            n.to_string(),
            Pick {
                label: n.to_string(),
                detail: tr("not drawn by Archforge yet"),
                live: false,
                ticked: false,
                why: Some(tr("not drawn by Archforge yet")),
            },
        )
    });
    drawn.chain(not_drawn).collect()
}

/// A stereotype's panel (F1): Name and Profile side by side, Extends · n with its `+` and its rows,
/// each with an editor, the metaclasses explained; then Properties · n through the member editor,
/// the types explained.
pub(crate) fn stereotype_body(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect, y: f32, subject: &Subject) {
    let (Some(s), Some(s_id)) = (subject.obj, subject.id) else { return };
    let width = (r.width() - 2.0 * PAD).max(0.0);
    let half = ((width - 8.0) / 2.0).max(0.0);
    let left = r.left() + PAD;
    crate::panels::name_field(shell, ui, p, pos2(left, y), half, subject, "uml.stereotype.set", "stereotype");
    let Some(store) = shell.session.as_ref().map(|s| s.doc()) else { return };
    let profile = profiles::profile_of(store, s);
    let all: Vec<(Uuid, String)> = profiles::all_profiles(store).into_iter().filter_map(|x| Some((store.uuid(x).ok()?, name(store, x)))).collect();
    let here = profile.map(|x| name(store, x)).unwrap_or_default();
    let field = select_field(
        shell,
        ui,
        p,
        pos2(left + half + 8.0, y),
        half,
        STEREOTYPE_PROFILE_ID,
        &tr_ctx("uml", "Profile"),
        &here,
        Menu::StereotypeProfile,
    );
    let mut y = y + 17.0 + FIELD + 16.0;
    // Extends.
    let Some(store) = shell.session.as_ref().map(|s| s.doc()) else { return };
    let extended = profiles::extended(store, s);
    let own = name(store, s);
    let title = fmt(&tr("Extends · {n}"), &[("n", &extended.len().to_string())]);
    let add = Rect::from_min_size(pos2(r.right() - PAD - 28.0, y + 11.0), vec2(28.0, 28.0));
    match section(shell, ui, p, r, y, &title, Some((EXTENDS_ADD_ID, "Add extended metaclass"))) {
        Some(next) => y = next - 6.0,
        None => {
            shell.panels.menu = if shell.panels.menu == Some(Menu::StereotypeExtends) { None } else { Some(Menu::StereotypeExtends) };
            y += 44.0;
        }
    }
    let editing = shell.panels.profile_edit.clone().filter(|e| e.owner == s_id);
    for m in &extended {
        let open = editing.as_ref().is_some_and(|e| e.target.as_deref() == Some(m.as_str()));
        let (_, edit) = row(shell, ui, p, pos2(left, y), width, &format!("{EXTENDS_ROW_ID}.{m}"), m, &icons::CLASS, open, true);
        y += ROW;
        if edit {
            shell.panels.profile_edit = Some(ProfileEdit { owner: s_id, target: Some(m.clone()), choice: m.clone(), opened: true });
            shell.panels.profile_refused = None;
        }
        if open {
            y = extension_editor(shell, ui, p, left, y, width, s_id, &extended);
        }
    }
    if let Some(shown) = shell.panels.profile_refused.clone().filter(|x| x.owner == s_id) {
        y = refusal(shell, ui, p, left, y + 4.0, width, &shown);
    }
    y = help(
        ui,
        p,
        left,
        y + 4.0,
        width,
        &tr("Offered: the metaclasses Archforge draws. Other metaclasses are listed, disabled, as not drawn by Archforge yet."),
    );
    // Properties, through the member editor.
    let below = crate::uml_panel::members_section(shell, ui, p, r, y, s, s_id, crate::uml_panel::MemberKind::Attribute);
    let said = fmt(&tr(TYPES_HELP), &[("name", &own)]);
    help(ui, p, left, below, width, &said);
    // The menus last, over the sections.
    if shell.panels.menu == Some(Menu::StereotypeProfile) {
        let names: Vec<String> = all.iter().map(|(_, n)| n.clone()).collect();
        if let Some(i) = crate::panels::menu(shell, ui, p, field, STEREOTYPE_PROFILE_ID, &names)
            && let Some((to, _)) = all.get(i)
            && profile.and_then(|x| shell.session.as_ref()?.doc().uuid(x).ok()) != Some(*to)
        {
            move_stereotype(shell, s_id, *to);
        }
    }
    if shell.panels.menu == Some(Menu::StereotypeExtends) {
        let rows = extends_rows(&extended, None);
        let picks: Vec<Pick> = rows.iter().map(|(_, pick)| pick.clone()).collect();
        let heading = fmt(&tr("{name} extends"), &[("name", &own)]);
        if let Some(i) = pick_menu(shell, ui, p, add, EXTENDS_ADD_ID, &heading, &picks)
            && let Some((m, _)) = rows.get(i)
        {
            let mut all = extended.clone();
            all.push(m.clone());
            set_extends(shell, s_id, &all, None);
        }
    }
}

/// The editor of an extension's row: the metaclass (a select of those offered), the bin that drops
/// it, Cancel and Apply. Gives the y under it.
#[allow(clippy::too_many_arguments, reason = "an editor is drawn for its stereotype and its row")]
fn extension_editor(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, left: f32, y: f32, width: f32, s: Uuid, extended: &[String]) -> f32 {
    let Some(mut edit) = shell.panels.profile_edit.clone() else { return y };
    let paint = ui.painter().clone();
    let background = paint.add(egui::Shape::Noop);
    let inner = (width - 24.0).max(0.0);
    let at = |x: f32, dy: f32| pos2(left + 12.0 + x, y + 4.0 + dy);
    let field = select_field(
        shell,
        ui,
        p,
        at(0.0, 12.0),
        inner,
        &format!("{EXTENSION_ID}.metaclass"),
        &tr("Metaclass"),
        &edit.choice,
        Menu::ExtensionMetaclass,
    );
    let foot = 12.0 + 17.0 + FIELD + 12.0;
    let remove = icon_button(shell, ui, p, at(0.0, foot), &format!("{EXTENSION_ID}.remove"), &icons::TRASH, p.danger, None, "Remove");
    let cancel = icon_button(
        shell,
        ui,
        p,
        at(inner - 2.0 * FIELD - 8.0, foot),
        &format!("{EXTENSION_ID}.cancel"),
        &icons::CLOSE,
        p.text_secondary,
        None,
        "Cancel",
    );
    let apply =
        icon_button(shell, ui, p, at(inner - FIELD, foot), &format!("{EXTENSION_ID}.apply"), &icons::TICK, p.on_accent, Some(p.accent_fill), "Apply");
    let card = Rect::from_min_size(pos2(left, y + 4.0), vec2(width, foot + FIELD + 12.0));
    paint.set(background, egui::Shape::rect_filled(card, 8.0, p.surface));
    paint.rect_stroke(card, 8.0, Stroke::new(1.0, p.accent), StrokeKind::Inside);
    if shell.panels.menu == Some(Menu::ExtensionMetaclass) {
        let rows = extends_rows(extended, edit.target.as_deref());
        let picks: Vec<Pick> = rows.iter().map(|(_, pick)| pick.clone()).collect();
        if let Some(i) = pick_menu(shell, ui, p, field, &format!("{EXTENSION_ID}.metaclass"), &tr("Metaclass"), &picks)
            && let Some((m, _)) = rows.get(i)
        {
            edit.choice = m.clone();
        }
    }
    edit.opened = false;
    shell.panels.profile_edit = Some(edit.clone());
    let target = edit.target.clone().unwrap_or_default();
    if cancel || (shell.panels.menu.is_none() && ui.input(|i| i.key_pressed(egui::Key::Escape))) {
        shell.panels.profile_edit = None;
    } else if remove {
        let kept: Vec<String> = extended.iter().filter(|m| **m != target).cloned().collect();
        shell.panels.profile_edit = None;
        set_extends(shell, s, &kept, Some(&target));
    } else if apply || ui.input(|i| i.key_pressed(egui::Key::Enter)) && shell.panels.menu.is_none() {
        let kept: Vec<String> = extended.iter().map(|m| if *m == target { edit.choice.clone() } else { m.clone() }).collect();
        shell.panels.profile_edit = None;
        set_extends(shell, s, &kept, Some(&target));
    }
    card.bottom() + 8.0
}

/// Sets what stereotype `s` extends to `names`, or, when dropping `dropped` strands uses, shows why
/// with the uses counted, changing nothing.
fn set_extends(shell: &mut Shell, s: Uuid, names: &[String], dropped: Option<&str>) {
    let refused = dropped.and_then(|m| {
        if names.iter().any(|n| n == m) {
            return None;
        }
        let store = shell.session.as_ref()?.doc();
        let st = store.by_uuid(s)?;
        let metaclass = profiles::metaclass(store, m)?;
        let uses = profiles::extension_uses(store, st, metaclass);
        (!uses.is_empty()).then(|| ProfileRefusal {
            owner: s,
            why: uses_said(store, Used::Extension { stereotype: name(store, st), metaclass: m.to_string() }, &uses),
            uses: use_ids(store, &uses),
        })
    });
    if let Some(r) = refused {
        shell.panels.profile_refused = Some(r);
        return;
    }
    match run(shell, "uml.stereotype.set", json!({ "stereotype": s.to_string(), "extends": names })) {
        Ok(_) => {
            shell.panels.notice = None;
            shell.panels.profile_refused = None;
        }
        Err(e) => shell.panels.notice = Some(e),
    }
}

/// Moves stereotype `s` to profile `to`, or, while elements use it, shows why.
fn move_stereotype(shell: &mut Shell, s: Uuid, to: Uuid) {
    let refused = shell.session.as_ref().and_then(|session| {
        let store = session.doc();
        let st = store.by_uuid(s)?;
        let uses = profiles::stereotype_uses(store, st);
        (!uses.is_empty()).then(|| ProfileRefusal {
            owner: s,
            why: uses_said(store, Used::Moved(name(store, st)), &uses),
            uses: use_ids(store, &uses),
        })
    });
    if let Some(r) = refused {
        shell.panels.profile_refused = Some(r);
        return;
    }
    match run(shell, "uml.stereotype.set", json!({ "stereotype": s.to_string(), "profile": to.to_string() })) {
        Ok(_) => shell.panels.notice = None,
        Err(e) => shell.panels.notice = Some(e),
    }
}

// The profile's diagram.

/// Opens the profile's diagram, made when it has none and filled with every stereotype, every
/// metaclass they extend and the extensions between them, what it lacked placed as one undo step
/// (`uml.profile.diagram`).
pub(crate) fn open_profile_diagram(shell: &mut Shell, profile: Uuid) -> Result<(), String> {
    let out = run(shell, "uml.profile.diagram", json!({ "profile": profile.to_string() }))?;
    let d = out.get("id").and_then(|v| v.as_str()).and_then(Uuid::parse).ok_or_else(|| tr("The model could not be opened"))?;
    crate::canvas::open(shell, d);
    Ok(())
}

/// A stereotype dropped on its profile's diagram: its card, the cards of the metaclasses it extends
/// the diagram lacks, and the extensions, as one undo step; on any other diagram it is refused.
pub(crate) fn drop_stereotype(shell: &mut Shell, d: Obj, s: Obj, at: (f64, f64)) {
    let Some(store) = shell.session.as_ref().map(|x| x.doc()) else { return };
    let of = store.get(d, archforge_diagram::diagram::ELEMENT).ok().and_then(|v| v.as_ref());
    if of.is_none() || of != profiles::profile_of(store, s) {
        shell.canvas.notice = Some(tr("A stereotype is shown on its profile's diagram"));
        return;
    }
    if crate::editing::on_diagram(store, d, s) {
        shell.canvas.notice = Some(tr("Already on this diagram"));
        return;
    }
    let shown = |e: Obj| archforge_diagram::views_of(store, e).2.iter().any(|n| archforge_diagram::diagram_of(store, *n) == Some(d));
    let metaclasses: Vec<Uuid> =
        profiles::extensions(store, s).into_iter().map(|e| e.metaclass).filter(|m| !shown(*m)).filter_map(|m| store.uuid(m).ok()).collect();
    let (Ok(d_id), Ok(s_id)) = (store.uuid(d), store.uuid(s)) else { return };
    let rows = profiles::properties(store, s).len().max(1);
    let (w, h) = (archforge_uml::CLASS_SIZE.0, 66.0 + 20.0 * rows as f64 + 16.0);
    let Some(session) = shell.session.as_mut() else { return };
    let out = session.batch(&tr("Place stereotype"), |b| {
        b.execute("diagram.place", &json!({ "diagram": d_id.to_string(), "element": s_id.to_string(), "x": at.0 - w / 2.0, "y": at.1 - h / 2.0, "width": w, "height": h }))?;
        for (i, m) in metaclasses.iter().enumerate() {
            let x = at.0 - w / 2.0 + i as f64 * (w + 50.0);
            b.execute("diagram.place", &json!({ "diagram": d_id.to_string(), "element": m.to_string(), "x": x, "y": at.1 - h / 2.0 - 280.0, "width": w, "height": archforge_uml::PRIMITIVE_TYPE_SIZE.1 }))?;
        }
        b.execute("diagram.connect-all", &json!({ "diagram": d_id.to_string() }))?;
        Ok(())
    });
    if let Err(e) = out {
        shell.canvas.notice = Some(crate::panels::refusal(&e));
    }
}

/// The words this module hands to `tr` by a name rather than a literal, for the catalogue's
/// coverage test: the sections' `+` hints and the editors' buttons.
#[cfg(test)]
pub(crate) fn texts() -> impl Iterator<Item = &'static str> {
    ["Apply profile", "Add stereotype", "Add extended metaclass", "Unapply profile", APPLIED_HELP, TYPES_HELP].into_iter()
}

/// In the sample class model, the profile Persistence of F1 in the root package beside Commerce:
/// Entity extending Class with `table : String` and `cached : Boolean = false`, Id extending
/// Property and Reference extending Association with `lazy : Boolean = true`; Commerce applies it.
/// With `uses`, the stereotypes are applied (AR-135): «Entity» on Order and Customer, «Id» on their
/// id attributes, «Reference» on places (F6). Gives (Commerce, Persistence, Entity) ids.
#[cfg(any(test, feature = "snapshot"))]
pub(crate) fn put_profiles(shell: &mut Shell, uses: bool) -> Result<(String, String, String), String> {
    let id = |v: serde_json::Value| v.get("id").and_then(|i| i.as_str()).unwrap_or_default().to_string();
    let store = shell.session.as_ref().ok_or("the profiles need a session")?.doc();
    let commerce = store
        .alive()
        .find(|o| store.class_of(*o).ok() == Some(archforge_uml::PACKAGE) && name(store, *o) == "Commerce")
        .and_then(|o| store.uuid(o).ok())
        .ok_or("the sample has no Commerce")?
        .to_string();
    let session = shell.session.as_mut().ok_or("the profiles need a session")?;
    let mut run = |cmd: &str, p: serde_json::Value| session.execute(cmd, &p).map(|o| id(o.value)).map_err(|e| format!("{cmd}: {e}"));
    let persistence = run("uml.profile.add", json!({ "name": "Persistence" }))?;
    let entity = run("uml.stereotype.add", json!({ "profile": persistence, "name": "Entity", "extends": ["Class"] }))?;
    run("uml.attribute.add", json!({ "class": entity, "name": "table", "type": "String" }))?;
    run("uml.attribute.add", json!({ "class": entity, "name": "cached", "type": "Boolean", "default": "false" }))?;
    run("uml.stereotype.add", json!({ "profile": persistence, "name": "Id", "extends": ["Property"] }))?;
    let reference = run("uml.stereotype.add", json!({ "profile": persistence, "name": "Reference", "extends": ["Association"] }))?;
    run("uml.attribute.add", json!({ "class": reference, "name": "lazy", "type": "Boolean", "default": "true" }))?;
    run("uml.profile.apply", json!({ "package": commerce, "profile": persistence }))?;
    if uses {
        apply_samples(shell)?;
    }
    if let Some(s) = shell.session.as_mut() {
        s.mark_saved().map_err(|e| e.to_string())?;
    }
    Ok((commerce, persistence, entity))
}

/// The sample's stereotype applications (F6), made as the window makes them
/// (`uml.stereotype.apply`): «Entity» on Order and Customer, «Id» on their id attributes,
/// «Reference» on places.
#[cfg(any(test, feature = "snapshot"))]
fn apply_samples(shell: &mut Shell) -> Result<(), String> {
    let session = shell.session.as_mut().ok_or("the sample needs a session")?;
    let uses: Vec<(String, String)> = {
        let st = session.doc();
        let find = |class: archforge_mof::ClassId, n: &str| st.alive().find(|o| st.class_of(*o).ok() == Some(class) && name(st, *o) == n);
        let id = |o: Obj| st.uuid(o).map(|u| u.to_string()).map_err(|e| e.to_string());
        let (order, customer) = (find(archforge_uml::CLASS, "Order").ok_or("Order")?, find(archforge_uml::CLASS, "Customer").ok_or("Customer")?);
        let places = find(archforge_uml::ASSOCIATION, "places").ok_or("places")?;
        let stereo = |n: &str| find(archforge_uml::STEREOTYPE, n).ok_or_else(|| format!("the stereotype {n}"));
        let mut uses = vec![(id(stereo("Entity")?)?, id(order)?), (id(stereo("Entity")?)?, id(customer)?)];
        for c in [order, customer] {
            if let Some(a) = archforge_uml::members::attributes(st, c).into_iter().find(|a| name(st, *a) == "id") {
                uses.push((id(stereo("Id")?)?, id(a)?));
            }
        }
        uses.push((id(stereo("Reference")?)?, id(places)?));
        uses
    };
    for (stereotype, element) in uses {
        session.execute("uml.stereotype.apply", &json!({ "element": element, "stereotype": stereotype })).map_err(|e| e.to_string())?;
    }
    Ok(())
}
