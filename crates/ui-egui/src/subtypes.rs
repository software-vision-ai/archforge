//! A supertype's subtypes in Properties (slice S6 of archforge-gm2.34, from the design's panel
//! state "Subtypes"): the Subtypes section lists the tables that are kinds of the selected one with
//! an `ST` badge, the mapping and the key they share; `+` offers the schema's tables that can become
//! a subtype; a row selects its table, its cross removes the generalization with the key it made.
//! Under the rows: Complete, Disjoint, the Discriminator and the Mapping of the whole set, each
//! applied at once through `er.entity.set-subtyping`. A selected generalization shows its
//! supertype's section.

use archforge_mof::{Obj, Store, Uuid, Value};
use egui::{Rect, Vec2, pos2, vec2};
use serde_json::json;

use crate::Shell;
use crate::checks::badge;
use crate::home::control;
use crate::i18n::{fmt, tr};
use crate::panels::{FIELD, Menu, PAD, check, menu, run, section, select_field};
use crate::ux::{self, Palette, icons, types};

/// The mappings as the select offers them: the engine's word and the label.
pub(crate) const MAPPINGS: [(archforge_er::SubtypeMapping, &str); 3] = [
    (archforge_er::SubtypeMapping::TablePerType, "Table per type"),
    (archforge_er::SubtypeMapping::SingleTable, "Single table"),
    (archforge_er::SubtypeMapping::TablePerConcreteType, "Table per concrete type"),
];

/// One subtype as the section lists it.
pub struct SubtypeRow {
    pub generalization: Uuid,
    pub subtype: Uuid,
    pub name: String,
    pub detail: String,
}

fn name(store: &Store, o: Obj) -> String {
    archforge_er::rules::name_of(store, o)
}

fn mapping_label(m: archforge_er::SubtypeMapping) -> String {
    MAPPINGS.iter().find(|(x, _)| *x == m).map(|(_, l)| tr(l)).unwrap_or_default()
}

/// The subtypes of `e`, in the schema's order.
pub fn rows(store: &Store, e: Obj) -> Vec<SubtypeRow> {
    use archforge_er::rules as r;
    let mapping = mapping_label(r::mapping_of(store, e));
    r::subtype_links(store, e)
        .into_iter()
        .filter_map(|g| {
            let (_, Some(sub), key) = r::generalization_parts(store, g) else { return None };
            let mut detail = mapping.clone();
            if key.is_some() {
                let columns: Vec<String> = r::primary_key(store, e).into_iter().map(|a| name(store, a)).collect();
                if !columns.is_empty() {
                    detail = format!("{detail} · {}", fmt(&tr("key on {columns}"), &[("columns", &columns.join(", "))]));
                }
            }
            Some(SubtypeRow { generalization: store.uuid(g).ok()?, subtype: store.uuid(sub).ok()?, name: name(store, sub), detail })
        })
        .collect()
}

/// The schema's tables that can become subtypes of `e`: not `e`, without a supertype, and not one
/// of `e`'s supertypes.
fn candidates(store: &Store, e: Obj) -> Vec<(Uuid, String)> {
    use archforge_er::rules as r;
    let Some(s) = r::schema_of(store, e) else { return Vec::new() };
    store
        .list(s, archforge_er::schema::ENTITY)
        .unwrap_or_default()
        .iter()
        .filter_map(Value::as_ref)
        .filter(|c| store.is_alive(*c) && r::generalization_problem(store, e, *c, None).is_none())
        .filter_map(|c| Some((store.uuid(c).ok()?, name(store, c))))
        .collect()
}

/// Runs `er.entity.set-subtyping` with the values given, then keeps the panel on `subject`.
fn set_subtyping(
    shell: &mut Shell,
    entity: Uuid,
    complete: bool,
    disjoint: bool,
    discriminator: Option<Uuid>,
    mapping: archforge_er::SubtypeMapping,
) {
    let params = json!({
        "entity": entity.to_string(),
        "complete": complete,
        "disjoint": disjoint,
        "discriminator": discriminator.map(|d| d.to_string()).unwrap_or_default(),
        "mapping": archforge_er::commands::mapping_name(mapping),
    });
    match run(shell, "er.entity.set-subtyping", params) {
        Ok(_) => shell.panels.notice = None,
        Err(e) => shell.panels.notice = Some(e),
    }
}

/// Keeps the panel on `subject` after a command selected what it made.
fn reselect(shell: &mut Shell, subject: Uuid) {
    if let Err(e) = run(shell, "view.select", json!({ "ids": [subject.to_string()] })) {
        shell.panels.notice = Some(e);
    }
}

/// The Subtypes section of supertype `e`, with the set's fields when it has subtypes; `keep` is
/// what the panel shows, kept selected after a change. Gives the y under it.
pub(crate) fn subtypes_section(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect, mut y: f32, e: Obj, e_id: Uuid, keep: Uuid) -> f32 {
    use archforge_er::rules as rr;
    let paint = ui.painter().clone();
    let width = (r.width() - 2.0 * PAD).max(0.0);
    let (list, cands, (complete, disjoint), discriminator, mapping, columns) = match shell.session.as_ref() {
        Some(s) => {
            let store = s.doc();
            let columns: Vec<(Uuid, String)> = store
                .list(e, archforge_er::entity::ATTRIBUTE)
                .unwrap_or_default()
                .iter()
                .filter_map(Value::as_ref)
                .filter(|a| store.is_alive(*a))
                .filter_map(|a| Some((store.uuid(a).ok()?, name(store, a))))
                .collect();
            (
                rows(store, e),
                // Only while the menu is open: it walks every table's chain.
                if shell.panels.menu == Some(Menu::SubtypeAdd) { candidates(store, e) } else { Vec::new() },
                rr::subtyping_flags(store, e),
                rr::discriminator_of(store, e).and_then(|a| store.uuid(a).ok()),
                rr::mapping_of(store, e),
                columns,
            )
        }
        None => return y,
    };
    let title = fmt(&tr("Subtypes · {n}"), &[("n", &list.len().to_string())]);
    let add_at = Rect::from_min_size(pos2(r.right() - PAD - 28.0, y + 11.0), vec2(28.0, 28.0));
    let Some(next) = section(shell, ui, p, r, y, &title, Some(("workspace.properties.subtypes.add", "Add subtype"))) else {
        shell.panels.menu = if shell.panels.menu == Some(Menu::SubtypeAdd) { None } else { Some(Menu::SubtypeAdd) };
        return y + 50.0;
    };
    y = next - 6.0;
    let mut remove: Option<Uuid> = None;
    let mut open: Option<Uuid> = None;
    for row in &list {
        let rect = Rect::from_min_size(pos2(r.left() + PAD, y), vec2(width, 44.0));
        let resp = control(shell, ui, &format!("workspace.properties.subtype.row.{}", row.generalization), "row", rect, true);
        if resp.hovered() {
            paint.rect_filled(rect, 6.0, p.page);
        }
        badge(&paint, rect.min + vec2(2.0, 6.0), "ST", p.green_tint, p.green);
        ux::text(&paint, rect.min + vec2(38.0, 4.0), &row.name, types::UI, p.text, Some((width - 70.0).max(0.0)));
        ux::text(&paint, rect.min + vec2(38.0, 23.0), &row.detail, types::CAPTION, p.text_secondary, Some((width - 70.0).max(0.0)));
        let x = Rect::from_min_size(pos2(rect.right() - 24.0, rect.top() + 14.0), Vec2::splat(16.0));
        ux::icon_in(&paint, x, &icons::CLOSE, p.text_secondary);
        if control(shell, ui, &format!("workspace.properties.subtype.row.{}.remove", row.generalization), "button", x.expand(4.0), true)
            .on_hover_text(tr("Remove"))
            .clicked()
        {
            remove = Some(row.generalization);
        } else if resp.clicked() {
            open = Some(row.subtype);
        }
        y += 44.0;
    }
    if !list.is_empty() {
        y += 6.0;
        let mut c = complete;
        let right = check(shell, ui, p, pos2(r.left() + PAD, y), "workspace.properties.subtypes.complete", &tr("Complete"), &mut c);
        let mut d = disjoint;
        check(shell, ui, p, pos2(right.max(r.left() + PAD + 110.0), y), "workspace.properties.subtypes.disjoint", &tr("Disjoint"), &mut d);
        if c != complete || d != disjoint {
            set_subtyping(shell, e_id, c, d, discriminator, mapping);
        }
        y += 30.0;
        let disc_word = discriminator.and_then(|id| columns.iter().find(|(c, _)| *c == id)).map(|(_, n)| n.clone()).unwrap_or_else(|| tr("None"));
        let disc_field = select_field(
            shell,
            ui,
            p,
            pos2(r.left() + PAD, y),
            width,
            "workspace.properties.subtypes.discriminator",
            &tr("Discriminator"),
            &disc_word,
            Menu::Discriminator,
        );
        y += 17.0 + FIELD + 12.0;
        let map_field = select_field(
            shell,
            ui,
            p,
            pos2(r.left() + PAD, y),
            width,
            "workspace.properties.subtypes.mapping",
            &tr("Mapping"),
            &mapping_label(mapping),
            Menu::Mapping,
        );
        y += 17.0 + FIELD + 12.0;
        if shell.panels.menu == Some(Menu::Discriminator) {
            let mut items = vec![tr("None")];
            items.extend(columns.iter().map(|(_, n)| n.clone()));
            if let Some(i) = menu(shell, ui, p, disc_field, "workspace.properties.subtypes.discriminator", &items) {
                let picked = if i == 0 { None } else { columns.get(i - 1).map(|(id, _)| *id) };
                set_subtyping(shell, e_id, complete, disjoint, picked, mapping);
            }
        }
        if shell.panels.menu == Some(Menu::Mapping) {
            let items: Vec<String> = MAPPINGS.iter().map(|(_, l)| tr(l)).collect();
            if let Some(i) = menu(shell, ui, p, map_field, "workspace.properties.subtypes.mapping", &items)
                && let Some((m, _)) = MAPPINGS.get(i)
            {
                set_subtyping(shell, e_id, complete, disjoint, discriminator, *m);
            }
        }
    }
    // The tables that can become subtypes, under the +.
    if shell.panels.menu == Some(Menu::SubtypeAdd) {
        if cands.is_empty() {
            shell.panels.menu = None;
        } else {
            let items: Vec<String> = cands.iter().map(|(_, n)| n.clone()).collect();
            let anchor = Rect::from_min_size(pos2(r.right() - PAD - 200.0, add_at.top()), vec2(200.0, 28.0));
            if let Some(i) = menu(shell, ui, p, anchor, "workspace.properties.subtypes.add", &items)
                && let Some((sub, _)) = cands.get(i)
            {
                let params = json!({ "supertype": e_id.to_string(), "subtype": sub.to_string() });
                match crate::editing::run_fitting(shell, &tr("Add subtype"), "er.generalization.add", params, &[*sub]) {
                    Ok(_) => {
                        shell.panels.notice = None;
                        reselect(shell, keep);
                    }
                    Err(err) => shell.panels.notice = Some(err),
                }
            }
        }
    }
    if let Some(g) = remove {
        match run(shell, "er.generalization.remove", json!({ "generalization": g.to_string() })) {
            Ok(_) => {
                shell.panels.notice = None;
                // A removed generalization cannot stay selected; the supertype can.
                reselect(shell, if keep == g { e_id } else { keep });
            }
            Err(err) => shell.panels.notice = Some(err),
        }
    } else if let Some(sub) = open {
        reselect(shell, sub);
    }
    y
}

/// A generalization's supertype, for its Properties.
pub(crate) fn supertype_of_link(store: &Store, g: Obj) -> Option<Obj> {
    archforge_er::rules::generalization_parts(store, g).0
}

/// The label a supertype's set shows on the canvas: complete, disjoint or overlapping, and the
/// discriminator, when any of them says more than the defaults.
pub(crate) fn set_label(store: &Store, e: Obj) -> Option<String> {
    let (complete, disjoint) = archforge_er::rules::subtyping_flags(store, e);
    let discriminator = archforge_er::rules::discriminator_of(store, e).map(|a| name(store, a));
    if !complete && disjoint && discriminator.is_none() {
        return None;
    }
    let mut parts = Vec::new();
    if complete {
        parts.push(tr("complete"));
    }
    parts.push(if disjoint { tr("disjoint") } else { tr("overlapping") });
    parts.extend(discriminator);
    Some(parts.join(" · "))
}

/// Every text of the tables, for the catalogue tests.
#[cfg(test)]
pub(crate) fn texts() -> impl Iterator<Item = &'static str> {
    ["Add subtype"].into_iter().chain(MAPPINGS.iter().map(|(_, l)| *l))
}
