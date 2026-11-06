//! Nested classifiers in the shell (AR-132, design page 08, frames D1 to D4): Properties' Owner
//! select in Package's place on every classifier, which moves the classifier under a package or a
//! classifier (`uml.classifier.set-owner`, one undo step) and lists every owner that cannot take
//! it disabled with the reason, those nested in it never; the Nested classifiers section of a
//! class or an interface, whose `+` offers the kinds Models' New inside ▸ offers; and the making
//! of one, named in place in Models. The rules are the model's (`archforge_uml::nesting`): the
//! shell only asks them and says their answers in the user's words.

use archforge_mof::{Obj, Store, Uuid};
use archforge_uml::nesting::{self, OwnerProblem};
use egui::{Rect, Sense, Stroke, StrokeKind, pos2, vec2};
use serde_json::json;

use crate::Shell;
use crate::explorer::fresh_name;
use crate::home::control;
use crate::i18n::{fmt, tr};
use crate::panels::{Menu, PAD, PATH_SEP, ROW, run, section};
use crate::uml_classifier::Classifier;
use crate::ux::{self, Palette, icons, types};

/// The Owner select's id; its menu's rows add `.{i}` (D3).
pub(crate) const OWNER_ID: &str = "workspace.properties.class.owner";
/// The Nested classifiers section's `+`, its menu's rows `.{kind}`, and its rows `.{id}` under
/// [`NESTED_ROW_ID`] (D1).
pub(crate) const NESTED_ADD_ID: &str = "workspace.properties.class.nested.add";
pub(crate) const NESTED_ROW_ID: &str = "workspace.properties.class.nested";

/// The kinds a classifier may nest, in the menus' order: those Models' New inside ▸ and the
/// section's `+` offer.
pub(crate) const KINDS: [Classifier; 4] = [Classifier::Class, Classifier::Interface, Classifier::Enumeration, Classifier::DataType];

/// The key `uml.nested.add` takes for a kind.
fn command_kind(kind: Classifier) -> &'static str {
    match kind {
        Classifier::Interface => "interface",
        Classifier::Enumeration => "enumeration",
        Classifier::DataType => "data-type",
        _ => "class",
    }
}

fn name(store: &Store, o: Obj) -> String {
    archforge_core::name_of(store, o)
}

/// Whether `o` owns nested classifiers: a class, an interface or an association class.
pub(crate) fn owns(store: &Store, o: Obj) -> bool {
    store.class_of(o).is_ok_and(nesting::can_own)
}

/// An owner's path as the Owner select shows it, by its names from the top: a package's from the
/// root, a classifier's the classifiers that nest it and its own name (D3 names a class alone, its
/// kind beside it), so a path folded from its front keeps the owner's own name.
pub(crate) fn owner_parts(store: &Store, o: Obj) -> Vec<String> {
    if store.class_of(o).ok() == Some(archforge_uml::PACKAGE) {
        return crate::uml_panel::package_parts(store, o);
    }
    let mut parts: Vec<String> = nesting::owner_chain(store, o).into_iter().map(|c| name(store, c)).collect();
    parts.push(name(store, o));
    parts
}

/// What an owner is, as the Owner menu says it at its row's right (package, class, interface),
/// and what an element its name clashes with is (attribute, operation, constraint).
fn kind_word(store: &Store, o: Obj) -> String {
    match store.class_of(o).ok() {
        Some(c) if c == archforge_uml::PACKAGE => return tr("package"),
        Some(c) if c == archforge_uml::PROPERTY => return tr("attribute"),
        Some(c) if c == archforge_uml::OPERATION => return tr("operation"),
        Some(c) if c == archforge_uml::CONSTRAINT => return tr("constraint"),
        Some(c) if c == archforge_uml::GENERALIZATION_SET => return tr("generalization set"),
        Some(c) if !archforge_uml::is_classifier(c) => return tr("element"),
        _ => {}
    }
    match Classifier::of(store, o) {
        Some(Classifier::Interface) => tr("interface"),
        Some(Classifier::Enumeration) => tr("enumeration"),
        Some(Classifier::DataType) => tr("data type"),
        Some(Classifier::PrimitiveType) => tr("primitive type"),
        Some(Classifier::AssociationClass) => tr("association class"),
        _ => tr("class"),
    }
}

/// Why `c` may not go under `owner`, short for the menu row's right and whole for its tooltip and
/// its description, in the user's words.
pub(crate) fn reason(store: &Store, c: Obj, owner: Obj, problem: &OwnerProblem) -> (String, String) {
    let (own, to) = (name(store, c), name(store, owner));
    match problem {
        OwnerProblem::Itself => (tr("itself"), fmt(&tr("{name} cannot own itself."), &[("name", &own)])),
        OwnerProblem::Inside => {
            (tr("inside it"), fmt(&tr("{owner} is inside {name}: owning {name} would make a cycle."), &[("owner", &to), ("name", &own)]))
        }
        OwnerProblem::OwnsNone => (
            tr("owns none"),
            fmt(&tr("{owner} owns no classifiers: only a package, a class, an interface or an association class does."), &[("owner", &to)]),
        ),
        OwnerProblem::NotNestable => (
            tr("not inside a class"),
            fmt(&tr("{name} stays in a package: a primitive type or an association class is never nested in a classifier."), &[("name", &own)]),
        ),
        OwnerProblem::NameTaken(s) => {
            let taken = name(store, *s);
            // The clashing element said by its kind, `class Priority`, for a package owner too.
            let short = fmt(&tr("{kind} {name} already holds that name"), &[("kind", &kind_word(store, *s)), ("name", &taken)]);
            (short, fmt(&tr("{owner} already owns a member named {name}."), &[("owner", &to), ("name", &taken)]))
        }
        OwnerProblem::Reserved => (
            tr("kept for a built-in type"),
            fmt(&tr("The package {owner} keeps the name {name} for a built-in type."), &[("owner", &to), ("name", &own)]),
        ),
        OwnerProblem::BuiltIn => (tr("built-in type"), fmt(&tr("{name} is a built-in type and stays in Types."), &[("name", &own)])),
    }
}

/// One row of the Owner menu: the owner, its path, its kind, whether it owns the classifier now,
/// and why it may not, short and whole, when it may not.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct OwnerRow {
    pub owner: Obj,
    pub id: Uuid,
    pub parts: Vec<String>,
    pub kind: String,
    pub current: bool,
    pub why: Option<(String, String)>,
}

/// The Owner menu's rows for `c` (D3): every owner that may take it, in the model's order, then,
/// after them, those that may not, each with its reason; those nested in `c` never.
pub(crate) fn owner_rows(store: &Store, c: Obj) -> Vec<OwnerRow> {
    let current = nesting::owner_of(store, c);
    let rows: Vec<OwnerRow> = nesting::candidates(store, c)
        .into_iter()
        .filter_map(|(o, problem)| {
            Some(OwnerRow {
                owner: o,
                id: store.uuid(o).ok()?,
                parts: owner_parts(store, o),
                kind: kind_word(store, o),
                current: Some(o) == current,
                why: problem.as_ref().map(|p| reason(store, c, o, p)),
            })
        })
        .collect();
    let (open, shut): (Vec<OwnerRow>, Vec<OwnerRow>) = rows.into_iter().partition(|r| r.why.is_none());
    open.into_iter().chain(shut).collect()
}

/// The Owner select at `at`, `width` wide (D2, D3): the owner's path, folded from its front so its
/// own name shows, the whole path its tooltip and accessible name; a click opens its menu.
pub(crate) fn owner_field(shell: &mut Shell, ui: &egui::Ui, p: &Palette, at: egui::Pos2, width: f32, class: Obj) -> Rect {
    let here = shell.session.as_ref().and_then(|s| nesting::owner_of(s.doc(), class).map(|o| owner_parts(s.doc(), o))).unwrap_or_default();
    crate::panels::path_select_field(shell, ui, p, at, width, OWNER_ID, &tr("Owner"), &here, Menu::ClassOwner)
}

/// What the Owner explains under its field for a nested classifier (D2).
pub(crate) fn owner_help(store: &Store, class: Obj) -> Option<String> {
    nesting::is_nested(store, class).then(|| {
        fmt(
            &tr("Owner is a package or a classifier. Changing it keeps {name}'s identity, members, relationships and every type that uses it."),
            &[("name", &name(store, class))],
        )
    })
}

/// The Owner menu under `field` while it is open (D3): its OWNER heading, the owners that may take
/// the classifier with their kind at the right and the current one ticked, a line, then those that
/// may not, muted, each with its short reason, the whole reason on hover and for screen readers.
/// The arrow keys walk the rows that may be picked, Enter picks, Escape or a press elsewhere
/// closes; a pick moves the classifier there as one undo step.
pub(crate) fn owner_menu(shell: &mut Shell, ui: &egui::Ui, p: &Palette, field: Rect, class: Obj, class_id: Uuid) {
    if shell.panels.menu != Some(Menu::ClassOwner) {
        return;
    }
    let Some(store) = shell.session.as_ref().map(|s| s.doc()) else { return };
    let rows = owner_rows(store, class);
    let open = rows.iter().filter(|r| r.why.is_none()).count();
    let heading = 28.0;
    let gap = if open < rows.len() && open > 0 { 9.0 } else { 0.0 };
    let window = ui.ctx().content_rect();
    let width = {
        let paint = ui.painter();
        let measure = |t: &str| ux::galley(paint, t, types::SMALL, p.text, None).size().x;
        rows.iter()
            .take(256)
            .map(|r| 20.0 + measure(&r.parts.join(PATH_SEP)) + 16.0 + measure(r.why.as_ref().map_or(&r.kind, |w| &w.0)) + 30.0)
            .fold(field.width(), f32::max)
    }
    .min((window.width() - 16.0).max(field.width()));
    let full = 8.0 + heading + rows.len() as f32 * 30.0 + gap;
    let below = window.bottom() - 8.0 - (field.bottom() + 4.0);
    let height = full.min(below.max(8.0 + heading + 30.0));
    let left = field.left().min(window.right() - 8.0 - width).max(window.left() + 8.0);
    let m = Rect::from_min_size(pos2(left, field.bottom() + 4.0), vec2(width, height));
    let picked =
        ux::overlay(ui, crate::widgets::egui_id(&format!("{OWNER_ID}.layer")), m, |ui| owner_menu_body(shell, ui, p, m, &rows, open, heading));
    let outside = ui.input(|i| i.pointer.any_pressed() && !i.pointer.interact_pos().is_some_and(|pos| m.contains(pos) || field.contains(pos)));
    if picked.is_some() || outside || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        shell.panels.menu = None;
        shell.panels.menu_reached = None;
    }
    let Some(row) = picked.and_then(|i| rows.get(i)) else { return };
    if row.current {
        return;
    }
    match run(shell, "uml.classifier.set-owner", json!({ "classifier": class_id.to_string(), "owner": row.id.to_string() })) {
        Ok(_) => shell.panels.notice = None,
        Err(e) => shell.panels.notice = Some(e),
    }
}

/// The Owner menu's frame and rows; gives the row picked.
fn owner_menu_body(shell: &mut Shell, ui: &egui::Ui, p: &Palette, m: Rect, rows: &[OwnerRow], open: usize, heading: f32) -> Option<usize> {
    let paint = ui.painter().clone();
    ux::shadow(&paint, m, 8.0, p.shadow);
    paint.rect_filled(m, 8.0, p.surface);
    paint.rect_stroke(m, 8.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
    ux::text(
        &paint,
        pos2(m.left() + 14.0, m.top() + 12.0),
        &crate::canvas::eyebrow_case(&tr("Owner")),
        types::EYEBROW,
        p.text_muted,
        Some(m.width() - 28.0),
    );
    // The arrows walk the rows that may be picked; Enter picks.
    let mut reached = shell.panels.menu_reached.as_ref().filter(|(of, _)| of == OWNER_ID).map(|(_, i)| *i);
    let (down, up, enter) = ui.input_mut(|i| {
        (
            i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown),
            i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp),
            reached.is_some() && i.consume_key(egui::Modifiers::NONE, egui::Key::Enter),
        )
    });
    if open > 0 && (down || up) {
        reached = Some(match reached {
            None => 0,
            Some(i) if down => (i + 1).min(open - 1),
            Some(i) => i.saturating_sub(1),
        });
    }
    shell.panels.menu_reached = reached.map(|i| (OWNER_ID.to_string(), i));
    let list = Rect::from_min_max(pos2(m.left(), m.top() + 4.0 + heading), pos2(m.right(), m.bottom() - 4.0));
    // Its rows scroll with the wheel over it, and a row the arrows reach comes into sight.
    let gap = |i: usize| if i >= open && open > 0 && open < rows.len() { 9.0 } else { 0.0 };
    let total = rows.len() as f32 * 30.0 + gap(rows.len());
    let most = (total - list.height()).max(0.0);
    let mut offset = shell.panels.menu_scroll.as_ref().filter(|(of, _)| of == OWNER_ID).map_or(0.0, |(_, o)| *o);
    if ui.input(|i| i.pointer.hover_pos().is_some_and(|at| m.contains(at))) {
        offset -= ui.input(|i| i.smooth_scroll_delta.y);
    }
    if let Some(i) = reached.filter(|_| down || up) {
        let top = i as f32 * 30.0 + gap(i);
        if top < offset {
            offset = top;
        } else if top + 30.0 > offset + list.height() {
            offset = top + 30.0 - list.height();
        }
    }
    let offset = offset.clamp(0.0, most);
    shell.panels.menu_scroll = Some((OWNER_ID.to_string(), offset));
    let clipped = paint.with_clip_rect(list);
    let mut picked = None;
    let mut y = list.top() - offset;
    for (i, row) in rows.iter().enumerate() {
        if i == open && i > 0 {
            clipped.rect_filled(Rect::from_min_size(pos2(m.left() + 12.0, y + 4.0), vec2(m.width() - 24.0, 1.0)), 0.0, p.border);
            y += 9.0;
        }
        let r = Rect::from_min_size(pos2(m.left() + 4.0, y), vec2(m.width() - 8.0, 30.0));
        y += 30.0;
        if !list.intersects(r) {
            continue;
        }
        let id = format!("{OWNER_ID}.{i}");
        let path = row.parts.join(PATH_SEP);
        let resp = match &row.why {
            None => control(shell, ui, &id, "menu-item", r.intersect(list), true).accessible_name(format!("{path} · {}", row.kind)),
            Some((_, whole)) => {
                // Disabled, it says why on hover and to a screen reader.
                crate::widgets::record_visible(ui, &mut shell.registry, &id, "menu-item", r.intersect(list));
                let resp = ui.interact(r.intersect(list), crate::widgets::egui_id(&id), Sense::hover());
                ui.ctx().accesskit_node_builder(crate::widgets::egui_id(&id), |node| {
                    node.set_role(egui::accesskit::Role::MenuItem);
                    node.set_disabled();
                    node.set_description(whole.clone());
                });
                resp.on_hover_text(whole.as_str()).accessible_name(path.as_str())
            }
        };
        let live = row.why.is_none();
        if live && (resp.hovered() || reached == Some(i)) || row.current {
            clipped.rect_filled(r, 5.0, if row.current { p.accent_tint } else { p.page });
        }
        let ink = if !live {
            p.text_muted
        } else if row.current {
            p.accent
        } else {
            p.text
        };
        let detail = row.why.as_ref().map_or(row.kind.as_str(), |w| w.0.as_str());
        let tick = if row.current { 22.0 } else { 0.0 };
        let detail_g = ux::galley(&clipped, detail, types::CAPTION, p.text_muted, Some(m.width() / 2.0));
        let dw = detail_g.size().x;
        clipped.galley(pos2(r.right() - 10.0 - tick - dw, r.center().y - detail_g.size().y / 2.0), detail_g, p.text_muted);
        if row.current {
            ux::icon_mid(&clipped, pos2(r.right() - 18.0, r.center().y), &icons::TICK, p.accent);
        }
        let room = (r.width() - 36.0 - tick - dw).max(0.0);
        let fits = |t: &str| ux::galley(&clipped, t, types::SMALL, ink, None).size().x <= room;
        let (shown, folded) = ux::cut_front(&row.parts, PATH_SEP, fits);
        ux::text_mid(&clipped, r.left() + 10.0, r.center().y, &shown, types::SMALL, ink, Some(room));
        // A path folded from its front says itself whole on hover.
        let resp = if folded && live { resp.on_hover_text(path.as_str()) } else { resp };
        if live && (resp.clicked() || (enter && reached == Some(i))) {
            picked = Some(i);
        }
    }
    picked
}

/// Makes a classifier of `kind` nested in `owner`, named afresh among the owner's nested ones
/// (`Enumeration 1`), as one undo step, selected; Models opens the rows above it and its name for
/// typing there (D1), so it is named in place whichever way it was asked for.
pub(crate) fn add_nested(shell: &mut Shell, owner: Uuid, kind: Classifier) -> Result<Uuid, String> {
    let store = shell.session.as_ref().map(|s| s.doc()).ok_or_else(|| tr("The model could not be opened"))?;
    let o = store.by_uuid(owner).ok_or_else(|| tr("The model could not be opened"))?;
    let taken = nesting::nested(store, o).into_iter().map(|c| name(store, c));
    let fresh = fresh_name(kind.stem(), taken, false).ok_or_else(|| tr("No free name is left"))?;
    let out = run(shell, "uml.nested.add", json!({ "owner": owner.to_string(), "kind": command_kind(kind), "name": fresh }))?;
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

/// The Nested classifiers section of a class or an interface at `y` (D1): its title with the
/// count and a `+` whose menu offers the kinds Models' New inside ▸ does, then a row a nested
/// classifier, its icon and name, a click selecting it; none yet says where to add one. Gives the
/// y under it.
pub(crate) fn nested_section(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect, mut y: f32, class: Obj, class_id: Uuid) -> f32 {
    let Some(store) = shell.session.as_ref().map(|s| s.doc()) else { return y };
    let own = name(store, class);
    let rows: Vec<(Uuid, String, Classifier)> =
        nesting::nested(store, class).into_iter().filter_map(|c| Some((store.uuid(c).ok()?, name(store, c), Classifier::of(store, c)?))).collect();
    let title = fmt(&tr("Nested classifiers · {n}"), &[("n", &rows.len().to_string())]);
    let add = Rect::from_min_size(pos2(r.right() - PAD - 28.0, y + 11.0), vec2(28.0, 28.0));
    match section(shell, ui, p, r, y, &title, Some((NESTED_ADD_ID, "Add nested classifier"))) {
        Some(next) => y = next - 6.0,
        None => {
            shell.panels.menu = if shell.panels.menu == Some(Menu::NestedKind) { None } else { Some(Menu::NestedKind) };
            y += 44.0;
        }
    }
    let paint = ui.painter().clone();
    let width = (r.width() - 2.0 * PAD).max(0.0);
    if rows.is_empty() {
        let none = fmt(&tr("None yet. Add one here or from Models ▸ New inside {name}; both offer the same kinds."), &[("name", &own)]);
        let g = ux::galley_rows(&paint, &none, types::CAPTION, p.text_muted, Some(width), 3);
        let tall = g.size().y;
        paint.galley(pos2(r.left() + PAD, y + 4.0), g, p.text_muted);
        y += tall + 14.0;
    }
    let chosen = crate::panels::selected_id(shell);
    for (id, label, kind) in &rows {
        let row = Rect::from_min_size(pos2(r.left() + PAD, y), vec2(width, ROW));
        let resp = control(shell, ui, &format!("{NESTED_ROW_ID}.{id}"), "row", row, true).accessible_name(format!("{label} · {}", kind.word()));
        if Some(*id) == chosen {
            paint.rect_filled(row, 6.0, p.accent_tint);
        } else if resp.contains_pointer() {
            paint.rect_filled(row, 6.0, p.page);
        }
        ux::icon_mid(&paint, pos2(row.left() + 17.0, row.center().y), kind.icon(), p.text_secondary);
        ux::text_mid(&paint, row.left() + 34.0, row.center().y, label, types::UI, p.text, Some((width - 44.0).max(0.0)));
        if resp.clicked() {
            crate::panels::select_element(shell, *id);
        }
        y += ROW;
    }
    if shell.panels.menu == Some(Menu::NestedKind) {
        let words: Vec<String> = KINDS.iter().map(|k| k.word()).collect();
        if let Some(i) = crate::panels::menu(shell, ui, p, add, NESTED_ADD_ID, &words)
            && let Some(kind) = KINDS.get(i)
            && let Err(e) = add_nested(shell, class_id, *kind)
        {
            shell.panels.notice = Some(e);
        }
    }
    y + 10.0
}

/// What Models' New inside ▸ offers on `o` (D1): the kinds it may nest, live; or, on an enumeration,
/// a data type or a primitive type, the row disabled with why. None on anything that is no
/// classifier.
pub(crate) fn inside(store: &Store, o: Obj) -> Option<crate::notation_ui::Inside> {
    let kind = Classifier::of(store, o)?;
    if archforge_uml::members::is_built_in(store, o) {
        return None;
    }
    let owner = name(store, o);
    let why_not = (!owns(store, o)).then(|| {
        fmt(
            &tr("{name} is {kind}, which owns no classifiers: only a class, an interface or an association class does."),
            &[("name", &owner), ("kind", &a_kind(kind))],
        )
    });
    Some(crate::notation_ui::Inside {
        word: fmt(&tr("New inside {name}"), &[("name", &owner)]),
        heading: fmt(&tr("New inside {name}"), &[("name", &owner)]),
        kinds: KINDS.iter().map(|k| (k.key(), k.word(), k.icon())).collect(),
        refused: vec![("package", tr("Package"), tr("not inside a class"))],
        why_not,
    })
}

/// A kind with its article, as a sentence says it: an enumeration, a data type.
fn a_kind(kind: Classifier) -> String {
    match kind {
        Classifier::Enumeration => tr("an enumeration"),
        Classifier::DataType => tr("a data type"),
        Classifier::PrimitiveType => tr("a primitive type"),
        Classifier::Interface => tr("an interface"),
        Classifier::AssociationClass => tr("an association class"),
        Classifier::Class => tr("a class"),
    }
}

/// Makes the kind `key` inside `o`, as New inside ▸ picks it.
pub(crate) fn add_inside(shell: &mut Shell, key: &str, o: Obj) -> Option<Result<Uuid, String>> {
    let kind = KINDS.into_iter().find(|k| k.key() == key)?;
    let id = shell.session.as_ref()?.doc().uuid(o).ok()?;
    Some(add_nested(shell, id, kind))
}

/// The words this module hands to `tr` by a name rather than a literal, for the catalogue's
/// coverage test: the section's `+` hint.
#[cfg(test)]
pub(crate) fn texts() -> impl Iterator<Item = &'static str> {
    ["Add nested classifier"].into_iter()
}
