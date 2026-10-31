//! A relationship's Properties (slice E3 of archforge-gm2.10, from the design's frame "ER ·
//! Properties · Relationship drawn" and its panel-state cards): the name and Identifying, the two
//! ends with their cardinality buttons and roles, and the Foreign key section with its switch,
//! Points to, the key pairs (parent key column · child column), On delete / On update and, under
//! More, Deferrable and Match. A parent without a primary key gets the card that offers to add an
//! id column or to choose key columns. Every change goes through the `er.*` commands; the foreign
//! key's columns come and go with the switch in one undo step.

use archforge_er::{Cardinality, Deferrability, MatchType, ReferentialAction};
use archforge_mof::{Obj, Store, Uuid, Value};
use egui::{Rect, Stroke, StrokeKind, Vec2, pos2, vec2};
use serde_json::json;

use crate::Shell;
use crate::canvas::cardinality_text;
use crate::home::control;
use crate::i18n::{fmt, tr};
use crate::panels::{FIELD, Menu, PAD, ROW, Subject, check, name_field, run, section, select_field, text_field};
use crate::ux::{self, Palette, icons, types};

/// A cardinality button's size and pitch.
const END_BUTTON: Vec2 = vec2(40.0, 26.0);
const END_PITCH: f32 = 46.0;
/// The ends a row offers, in the design's order.
const ENDS: [Cardinality; 4] = [Cardinality::ZeroOrOne, Cardinality::ExactlyOne, Cardinality::ZeroOrMore, Cardinality::OneOrMore];
/// The referential actions, in the engine's order, with their words.
const ACTIONS: [(ReferentialAction, &str, &str); 5] = [
    (ReferentialAction::NoAction, "no-action", "No action"),
    (ReferentialAction::Restrict, "restrict", "Restrict"),
    (ReferentialAction::Cascade, "cascade", "Cascade"),
    (ReferentialAction::SetNull, "set-null", "Set null"),
    (ReferentialAction::SetDefault, "set-default", "Set default"),
];
const DEFERRABLE: [(Deferrability, &str, &str); 3] = [
    (Deferrability::NotDeferrable, "not-deferrable", "No"),
    (Deferrability::InitiallyImmediate, "initially-immediate", "Yes – checked at once"),
    (Deferrability::InitiallyDeferred, "initially-deferred", "Yes – checked at commit"),
];
const MATCH: [(MatchType, &str, &str); 3] =
    [(MatchType::Simple, "simple", "Simple"), (MatchType::Full, "full", "Full"), (MatchType::Partial, "partial", "Partial")];

/// A role being typed: the relationship, which end, the text so far.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoleEdit {
    pub target: Uuid,
    pub end: usize,
    pub text: String,
}

/// What the panel reads of a relationship once per frame.
struct Rel {
    id: Uuid,
    from: Obj,
    to: Obj,
    from_end: Cardinality,
    to_end: Cardinality,
    roles: [String; 2],
    identifying: bool,
    /// The parent (the one side) and the child (the many side) as the cardinalities say.
    parent: Obj,
    child: Obj,
    fk: Option<Fk>,
}

/// The foreign key that realises the relationship.
struct Fk {
    obj: Obj,
    id: Uuid,
    key: Option<Obj>,
    columns: Vec<Obj>,
    on_delete: ReferentialAction,
    on_update: ReferentialAction,
    deferrable: Deferrability,
    matching: MatchType,
}

fn name(store: &Store, o: Obj) -> String {
    archforge_er::rules::name_of(store, o)
}

fn string(store: &Store, o: Obj, prop: archforge_mof::PropId) -> String {
    store.get(o, prop).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default()
}

fn enum_of<T>(store: &Store, o: Obj, prop: archforge_mof::PropId, from: fn(u32) -> Option<T>, default: T) -> T {
    match store.get(o, prop) {
        Ok(Value::Enum(i)) => from(i).unwrap_or(default),
        _ => default,
    }
}

fn read(store: &Store, rel: Obj) -> Option<Rel> {
    use archforge_er::relationship as r;
    let end = |p| store.get(rel, p).ok().and_then(|v| v.as_ref()).filter(|o| store.is_alive(*o));
    let (from, to) = (end(r::FROM)?, end(r::TO)?);
    let from_end = archforge_er::rules::cardinality_of(store, rel, r::FROM_END).unwrap_or(Cardinality::ExactlyOne);
    let to_end = archforge_er::rules::cardinality_of(store, rel, r::TO_END).unwrap_or(Cardinality::ZeroOrMore);
    let fk = store.referrers(rel).into_iter().find(|(_, p)| *p == archforge_er::foreign_key::RELATIONSHIP).map(|(f, _)| f).and_then(|f| {
        use archforge_er::foreign_key as k;
        Some(Fk {
            obj: f,
            id: store.uuid(f).ok()?,
            key: store.get(f, k::REFERENCES).ok().and_then(|v| v.as_ref()).filter(|o| store.is_alive(*o)),
            columns: store.list(f, k::COLUMN).unwrap_or_default().iter().filter_map(Value::as_ref).filter(|c| store.is_alive(*c)).collect(),
            on_delete: enum_of(store, f, k::ON_DELETE, ReferentialAction::from_index, ReferentialAction::NoAction),
            on_update: enum_of(store, f, k::ON_UPDATE, ReferentialAction::from_index, ReferentialAction::NoAction),
            deferrable: enum_of(store, f, k::DEFERRABLE, Deferrability::from_index, Deferrability::NotDeferrable),
            matching: enum_of(store, f, k::MATCH, MatchType::from_index, MatchType::Simple),
        })
    });
    // The child holds the key: the foreign key's entity when there is one, else the many side.
    let (parent, child) = match fk.as_ref().and_then(|f| archforge_er::rules::entity_of(store, f.obj)) {
        Some(c) if c == from => (to, from),
        Some(_) => (from, to),
        None if archforge_er::rules::is_many(from_end) && !archforge_er::rules::is_many(to_end) => (to, from),
        None => (from, to),
    };
    Some(Rel {
        id: store.uuid(rel).ok()?,
        from,
        to,
        from_end,
        to_end,
        roles: [string(store, rel, r::FROM_ROLE), string(store, rel, r::TO_ROLE)],
        identifying: matches!(store.get(rel, r::IDENTIFYING), Ok(Value::Bool(true))),
        parent,
        child,
        fk,
    })
}

fn cardinality_word(c: Cardinality) -> &'static str {
    match c {
        Cardinality::ZeroOrOne => "zero-or-one",
        Cardinality::ExactlyOne => "exactly-one",
        Cardinality::ZeroOrMore => "zero-or-more",
        Cardinality::OneOrMore => "one-or-more",
    }
}

/// Takes a command's result: a refusal goes to the notice; after a success the relationship is
/// selected again, since commands that make columns or keys select what they made.
fn apply(shell: &mut Shell, rel: Uuid, result: Result<serde_json::Value, String>) {
    match result {
        Ok(_) => {
            shell.panels.notice = None;
            let still = shell.session.as_ref().is_some_and(|s| s.view().selection.iter().any(|o| s.doc().uuid(*o).ok() == Some(rel)));
            if !still && let Err(e) = run(shell, "view.select", json!({ "ids": [rel.to_string()] })) {
                shell.panels.notice = Some(e);
            }
        }
        Err(e) => shell.panels.notice = Some(e),
    }
}

/// Draws a relationship's Properties under the heading, from `y`.
pub(crate) fn body(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect, mut y: f32, subject: &Subject) {
    let paint = ui.painter().clone();
    let width = (r.width() - 2.0 * PAD).max(0.0);
    let Some(rel) = shell.session.as_ref().and_then(|s| subject.obj.and_then(|o| read(s.doc(), o))) else { return };
    let (from_name, to_name, parent_name, child_name) = match shell.session.as_ref() {
        Some(s) => (name(s.doc(), rel.from), name(s.doc(), rel.to), name(s.doc(), rel.parent), name(s.doc(), rel.child)),
        None => return,
    };
    // Relationship: name and Identifying.
    name_field(shell, ui, p, pos2(r.left() + PAD, y), width, subject, "er.relationship.rename", "relationship");
    y += 17.0 + FIELD + 12.0;
    let mut identifying = rel.identifying;
    check(shell, ui, p, pos2(r.left() + PAD, y), "workspace.properties.identifying", &tr("Identifying"), &mut identifying);
    if identifying != rel.identifying {
        set_identifying(shell, &rel, identifying);
    }
    y += 32.0;
    // Ends.
    y = section(shell, ui, p, r, y, &tr("Ends"), None).unwrap_or(y + 50.0) - 6.0;
    for (i, (entity_name, current)) in [(from_name.clone(), rel.from_end), (to_name.clone(), rel.to_end)].into_iter().enumerate() {
        let caption = fmt(&tr("{entity} end"), &[("entity", &entity_name)]);
        ux::text(&paint, pos2(r.left() + PAD, y), &caption, types::CAPTION, p.text_secondary, Some(width));
        y += 18.0;
        let mut picked = None;
        for (j, c) in ENDS.iter().enumerate() {
            let b = Rect::from_min_size(pos2(r.left() + PAD + j as f32 * END_PITCH, y), END_BUTTON);
            let on = *c == current;
            paint.rect_filled(b, 6.0, if on { p.accent_tint } else { p.field });
            if on {
                paint.rect_stroke(b, 6.0, Stroke::new(1.0, p.accent), StrokeKind::Inside);
            }
            let (f, t) = if i == 0 { (Some(*c), None) } else { (None, Some(*c)) };
            crate::connect::sample_line(&paint, p, b.center(), f, t, if on { p.accent } else { p.text_secondary });
            let id = format!("workspace.properties.end.{}.{j}", if i == 0 { "from" } else { "to" });
            if control(shell, ui, &id, "button", b, true).on_hover_text(cardinality_text(*c)).clicked() {
                picked = Some(*c);
            }
        }
        if let Some(c) = picked {
            let (f, t) = if i == 0 { (c, rel.to_end) } else { (rel.from_end, c) };
            let out = run(
                shell,
                "er.relationship.set-ends",
                json!({ "relationship": rel.id.to_string(), "from_end": cardinality_word(f), "to_end": cardinality_word(t) }),
            );
            apply(shell, rel.id, out);
        }
        y += END_BUTTON.y + 10.0;
        role_field(shell, ui, p, pos2(r.left() + PAD, y), width, &rel, i);
        y += 17.0 + FIELD + 14.0;
    }
    // Foreign key.
    y += 4.0;
    foreign_key(shell, ui, p, r, y, &rel, &parent_name, &child_name);
}

/// A role field: typed in place, committed on Enter or when the keyboard leaves it.
fn role_field(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, at: egui::Pos2, width: f32, rel: &Rel, end: usize) {
    let mut text = match &shell.panels.role {
        Some(e) if e.target == rel.id && e.end == end => e.text.clone(),
        _ => rel.roles[end].clone(),
    };
    let id = format!("workspace.properties.role.{}", if end == 0 { "from" } else { "to" });
    let committed = text_field(shell, ui, p, at, width, &id, &tr("Role"), &mut text, archforge_er::rules::MAX_NAME, false, true);
    if committed {
        shell.panels.role = None;
        if text.trim() != rel.roles[end] {
            let mut roles = rel.roles.clone();
            roles[end] = text.trim().to_string();
            let out =
                run(shell, "er.relationship.set-roles", json!({ "relationship": rel.id.to_string(), "from_role": roles[0], "to_role": roles[1] }));
            apply(shell, rel.id, out);
        }
    } else if text != rel.roles[end] {
        shell.panels.role = Some(RoleEdit { target: rel.id, end, text });
    } else if shell.panels.role.as_ref().is_some_and(|e| e.target == rel.id && e.end == end) {
        shell.panels.role = None;
    }
}

/// What the Foreign key section reads of the store once per frame, owned so the drawing may
/// change the shell.
struct FkView {
    fk_id: String,
    key: Option<Uuid>,
    key_word: String,
    /// The keys of the parent the foreign key may point at: id, kind, name, columns.
    keys: Vec<(Uuid, String, String, String)>,
    /// The parent key column against the child column, by name.
    pairs: Vec<(String, String)>,
    columns: Vec<String>,
    any_nullable: bool,
    on_delete: ReferentialAction,
    on_update: ReferentialAction,
    deferrable: Deferrability,
    matching: MatchType,
    /// The open pair's menu: its rows, the columns they stand for, and the heading before the
    /// child's other columns.
    pair_menu: Option<(usize, Vec<Item>, Vec<Uuid>, String)>,
}

fn fk_view(store: &Store, rel: &Rel, fk: &Fk, open_pair: Option<usize>, child_name: &str) -> FkView {
    let mut keys = Vec::new();
    if let Some(pk) = archforge_er::rules::primary_key_of(store, rel.parent)
        && let Ok(id) = store.uuid(pk)
    {
        let columns = archforge_er::rules::key_columns(store, pk).iter().map(|c| name(store, *c)).collect::<Vec<_>>().join(", ");
        keys.push((id, tr("Primary key"), name(store, pk), columns));
    }
    for u in archforge_er::rules::unique_constraints(store, rel.parent) {
        if let Ok(id) = store.uuid(u) {
            let columns = archforge_er::rules::key_columns(store, u).iter().map(|c| name(store, *c)).collect::<Vec<_>>().join(", ");
            keys.push((id, tr("Unique"), name(store, u), columns));
        }
    }
    let key = fk.key.and_then(|k| store.uuid(k).ok());
    let key_word = key
        .and_then(|k| keys.iter().find(|(id, _, _, _)| *id == k))
        .map(|(_, kind, n, _)| if n.is_empty() { kind.clone() } else { format!("{kind} · {n}") });
    let key_columns: Vec<Obj> = fk.key.map(|k| archforge_er::rules::key_columns(store, k)).unwrap_or_default();
    let pairs: Vec<(String, String)> =
        key_columns.iter().enumerate().map(|(i, kc)| (name(store, *kc), fk.columns.get(i).map(|c| name(store, *c)).unwrap_or_default())).collect();
    let pair_menu = open_pair.and_then(|i| {
        let kc = *key_columns.get(i)?;
        let wanted = archforge_er::rules::type_display(store, kc);
        let current = fk.columns.get(i).copied();
        let mut items = Vec::new();
        let mut targets = Vec::new();
        if let Some(c) = current {
            items.push(Item { label: name(store, c), detail: archforge_er::rules::type_display(store, c), enabled: true, on: true });
            targets.push(store.uuid(c).ok()?);
        }
        for c in store.list(rel.child, archforge_er::entity::ATTRIBUTE).unwrap_or_default().iter().filter_map(Value::as_ref) {
            if !store.is_alive(c) || Some(c) == current || fk.columns.contains(&c) {
                continue;
            }
            let ty = archforge_er::rules::type_display(store, c);
            let same = ty.eq_ignore_ascii_case(&wanted);
            items.push(Item {
                label: name(store, c),
                detail: if same { ty } else { fmt(&tr("{type} · type differs"), &[("type", &ty)]) },
                enabled: same,
                on: false,
            });
            targets.push(store.uuid(c).ok()?);
        }
        Some((i, items, targets, fmt(&tr("Existing columns of {entity}"), &[("entity", child_name)])))
    });
    FkView {
        fk_id: fk.id.to_string(),
        key,
        key_word: key_word.unwrap_or_default(),
        keys,
        pairs,
        columns: fk.columns.iter().filter_map(|c| store.uuid(*c).ok()).map(|u| u.to_string()).collect(),
        any_nullable: fk.columns.iter().any(|c| archforge_er::rules::is_nullable(store, *c)),
        on_delete: fk.on_delete,
        on_update: fk.on_update,
        deferrable: fk.deferrable,
        matching: fk.matching,
        pair_menu,
    }
}

fn word_of<T: PartialEq + Copy>(table: &[(T, &'static str, &'static str)], value: T) -> (&'static str, String) {
    table.iter().find(|(v, _, _)| *v == value).map(|(_, w, l)| (*w, tr(l))).unwrap_or(("", String::new()))
}

/// The Foreign key section: its header with the tag and the switch, then the key, the pairs, the
/// actions and More; or the card for a parent without a key.
fn foreign_key(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect, mut y: f32, rel: &Rel, parent_name: &str, child_name: &str) {
    let paint = ui.painter().clone();
    let width = (r.width() - 2.0 * PAD).max(0.0);
    paint.rect_filled(Rect::from_min_size(pos2(r.left(), y), vec2(r.width(), 1.0)), 0.0, p.border);
    let title = ux::text(&paint, pos2(r.left() + PAD, y + 17.0), &tr("Foreign key"), types::UI, p.text, Some((width - 80.0).max(0.0)));
    if rel.fk.is_some() {
        let tag = Rect::from_min_size(pos2(title.right() + 8.0, y + 19.0), vec2(26.0, 16.0));
        paint.rect_filled(tag, 4.0, p.amber_tint);
        let g = ux::galley(&paint, "FK1", types::TINY, p.amber, None);
        paint.galley(tag.center() - g.size() / 2.0, g, p.amber);
    }
    let on = rel.fk.is_some();
    let sw = Rect::from_min_size(pos2(r.right() - PAD - 36.0, y + 17.0), vec2(36.0, 20.0));
    paint.rect_filled(sw, 10.0, if on { p.accent_fill } else { p.border });
    let knob = if on { sw.right_center() - vec2(10.0, 0.0) } else { sw.left_center() + vec2(10.0, 0.0) };
    paint.circle_filled(knob, 8.0, p.surface);
    if control(shell, ui, "workspace.properties.foreign-key", "toggle", sw, true).on_hover_text(tr("Foreign key")).clicked() {
        if on {
            remove_foreign_key(shell, rel);
        } else {
            add_foreign_key(shell, rel, parent_name);
        }
        return;
    }
    y += 50.0;
    let Some(fk) = rel.fk.as_ref() else {
        if shell.panels.no_key_card == Some(rel.id) {
            let has_key = shell.session.as_ref().is_some_and(|s| archforge_er::rules::primary_key_of(s.doc(), rel.parent).is_some());
            if has_key {
                shell.panels.no_key_card = None;
            } else {
                parent_without_key(shell, ui, p, r, y, rel, parent_name, child_name);
            }
        }
        return;
    };
    let open_pair = match shell.panels.menu {
        Some(Menu::Pair(i)) => Some(i),
        _ => None,
    };
    let Some(v) = shell.session.as_ref().map(|s| fk_view(s.doc(), rel, fk, open_pair, child_name)) else { return };
    // Points to.
    let points =
        select_field(shell, ui, p, pos2(r.left() + PAD, y), width, "workspace.properties.points-to", &tr("Points to"), &v.key_word, Menu::Points);
    y += 17.0 + FIELD + 14.0;
    // The pairs: the parent's key columns against the child's.
    let half = ((width - 8.0) / 2.0).max(0.0);
    ux::text(&paint, pos2(r.left() + PAD, y), parent_name, types::CAPTION, p.text_secondary, Some(half));
    ux::text(&paint, pos2(r.left() + PAD + half + 8.0, y), child_name, types::CAPTION, p.text_secondary, Some(half));
    y += 20.0;
    let mut pair_fields: Vec<Rect> = Vec::new();
    for (i, (parent_col, child_col)) in v.pairs.iter().enumerate() {
        let tag = Rect::from_min_size(pos2(r.left() + PAD, y + 6.0), vec2(22.0, 16.0));
        paint.rect_filled(tag, 4.0, p.amber_tint);
        let g = ux::galley(&paint, "PK", types::TINY, p.amber, None);
        paint.galley(tag.center() - g.size() / 2.0, g, p.amber);
        ux::text(&paint, pos2(tag.right() + 6.0, y + 5.0), parent_col, types::SMALL, p.text, Some((half - 34.0).max(0.0)));
        let field = Rect::from_min_size(pos2(r.left() + PAD + half + 8.0, y), vec2(half, FIELD));
        paint.rect_filled(field, 6.0, p.field);
        ux::text_mid(&paint, field.left() + 8.0, field.center().y, child_col, types::SMALL, p.text, Some((half - 36.0).max(0.0)));
        ux::icon_mid(&paint, pos2(field.right() - 14.0, field.center().y), &icons::CHEVRON_DOWN, p.text_secondary);
        if control(shell, ui, &format!("workspace.properties.pair.{i}"), "select", field, true).clicked() {
            shell.panels.menu = if shell.panels.menu == Some(Menu::Pair(i)) { None } else { Some(Menu::Pair(i)) };
        }
        pair_fields.push(field);
        y += ROW;
    }
    y += 8.0;
    // On delete and On update.
    let (delete_word, update_word) = (word_of(&ACTIONS, v.on_delete).1, word_of(&ACTIONS, v.on_update).1);
    let on_delete =
        select_field(shell, ui, p, pos2(r.left() + PAD, y), half, "workspace.properties.on-delete", &tr("On delete"), &delete_word, Menu::OnDelete);
    let on_update = select_field(
        shell,
        ui,
        p,
        pos2(r.left() + PAD + half + 8.0, y),
        half,
        "workspace.properties.on-update",
        &tr("On update"),
        &update_word,
        Menu::OnUpdate,
    );
    y += 17.0 + FIELD + 14.0;
    // More: Deferrable, and Match when it can matter.
    let more = Rect::from_min_size(pos2(r.left() + PAD, y), vec2(width, 24.0));
    let open = shell.panels.more_open;
    ux::icon_in(
        &paint,
        Rect::from_min_size(more.min + vec2(0.0, 4.0), Vec2::splat(16.0)),
        if open { &icons::CHEVRON_DOWN } else { &icons::CHEVRON_RIGHT },
        p.text_secondary,
    );
    ux::text(&paint, more.min + vec2(22.0, 2.0), &tr("More"), types::UI, p.text, Some((width - 22.0).max(0.0)));
    if control(shell, ui, "workspace.properties.more", "disclosure", more, true).clicked() {
        shell.panels.more_open = !open;
    }
    y += 34.0;
    let mut deferrable = None;
    let mut matching = None;
    if open {
        let word = word_of(&DEFERRABLE, v.deferrable).1;
        deferrable = Some(select_field(
            shell,
            ui,
            p,
            pos2(r.left() + PAD, y),
            width,
            "workspace.properties.deferrable",
            &tr("Deferrable"),
            &word,
            Menu::Deferrable,
        ));
        y += 17.0 + FIELD + 14.0;
        if v.columns.len() >= 2 && v.any_nullable {
            let word = word_of(&MATCH, v.matching).1;
            matching =
                Some(select_field(shell, ui, p, pos2(r.left() + PAD, y), width, "workspace.properties.match", &tr("Match"), &word, Menu::Match));
        }
    }
    // The open menu, over the rest.
    match shell.panels.menu {
        Some(Menu::Points) => {
            let items: Vec<Item> = v
                .keys
                .iter()
                .map(|(id, kind, _, columns)| Item { label: kind.clone(), detail: columns.clone(), enabled: true, on: v.key == Some(*id) })
                .collect();
            if let Some(i) = menu(shell, ui, p, points, "workspace.properties.points-to", &items, None)
                && let Some((key, _, _, _)) = v.keys.get(i)
            {
                let out = run(shell, "er.foreign-key.set-key", json!({ "foreign_key": v.fk_id, "key": key.to_string() }));
                apply(shell, rel.id, out);
            }
        }
        Some(Menu::Pair(i)) => {
            // The rows are read at the frame's start, so a menu just opened draws next frame.
            let Some((_, items, targets, heading)) = v.pair_menu.as_ref() else {
                if i >= v.pairs.len() {
                    shell.panels.menu = None;
                }
                return;
            };
            let anchor = pair_fields.get(i).copied().unwrap_or(more);
            if let Some(j) = menu(shell, ui, p, anchor, &format!("workspace.properties.pair.{i}"), items, Some((1, heading.clone())))
                && let Some(target) = targets.get(j)
                && let Some(key) = v.key
            {
                let mut columns = v.columns.clone();
                if let Some(slot) = columns.get_mut(i)
                    && *slot != target.to_string()
                {
                    *slot = target.to_string();
                    let out = run(shell, "er.foreign-key.set-key", json!({ "foreign_key": v.fk_id, "key": key.to_string(), "columns": columns }));
                    apply(shell, rel.id, out);
                }
            }
        }
        Some(Menu::OnDelete) | Some(Menu::OnUpdate) => {
            let which_delete = shell.panels.menu == Some(Menu::OnDelete);
            let current = if which_delete { v.on_delete } else { v.on_update };
            let items: Vec<Item> =
                ACTIONS.iter().map(|(a, _, w)| Item { label: tr(w), detail: String::new(), enabled: true, on: *a == current }).collect();
            let (anchor, id) =
                if which_delete { (on_delete, "workspace.properties.on-delete") } else { (on_update, "workspace.properties.on-update") };
            if let Some(i) = menu(shell, ui, p, anchor, id, &items, None)
                && let Some((_, word, _)) = ACTIONS.get(i)
            {
                let (d, u) = if which_delete { (*word, word_of(&ACTIONS, v.on_update).0) } else { (word_of(&ACTIONS, v.on_delete).0, *word) };
                let out = run(shell, "er.foreign-key.set-actions", json!({ "foreign_key": v.fk_id, "on_delete": d, "on_update": u }));
                apply(shell, rel.id, out);
            }
        }
        Some(Menu::Deferrable) | Some(Menu::Match) => {
            let is_deferrable = shell.panels.menu == Some(Menu::Deferrable);
            let Some(anchor) = (if is_deferrable { deferrable } else { matching }) else {
                shell.panels.menu = None;
                return;
            };
            let items: Vec<Item> = if is_deferrable {
                DEFERRABLE.iter().map(|(d, _, w)| Item { label: tr(w), detail: String::new(), enabled: true, on: *d == v.deferrable }).collect()
            } else {
                MATCH.iter().map(|(m, _, w)| Item { label: tr(w), detail: String::new(), enabled: true, on: *m == v.matching }).collect()
            };
            let id = if is_deferrable { "workspace.properties.deferrable" } else { "workspace.properties.match" };
            if let Some(i) = menu(shell, ui, p, anchor, id, &items, None) {
                let m = if is_deferrable { word_of(&MATCH, v.matching).0 } else { MATCH.get(i).map(|(_, w, _)| *w).unwrap_or("simple") };
                let d = if is_deferrable {
                    DEFERRABLE.get(i).map(|(_, w, _)| *w).unwrap_or("not-deferrable")
                } else {
                    word_of(&DEFERRABLE, v.deferrable).0
                };
                let out = run(shell, "er.foreign-key.set-options", json!({ "foreign_key": v.fk_id, "match": m, "deferrable": d }));
                apply(shell, rel.id, out);
            }
        }
        _ => {}
    }
}

/// The card for a parent without a primary key, with its two offers.
fn parent_without_key(shell: &mut Shell, ui: &egui::Ui, p: &Palette, r: Rect, y: f32, rel: &Rel, parent_name: &str, child_name: &str) {
    let paint = ui.painter().clone();
    let width = (r.width() - 2.0 * PAD).max(0.0);
    let card = Rect::from_min_size(pos2(r.left() + PAD, y), vec2(width, 118.0));
    paint.rect_filled(card, 8.0, p.amber_tint);
    paint.rect_stroke(card, 8.0, Stroke::new(1.0, p.amber), StrokeKind::Inside);
    let text =
        fmt(&tr("{parent} has no primary key yet, so there is nothing in {child} to point at."), &[("parent", parent_name), ("child", child_name)]);
    let g = ux::galley_rows(&paint, &text, types::SMALL, p.text, Some((width - 24.0).max(0.0)), 2);
    paint.galley(card.min + vec2(12.0, 10.0), g, p.text);
    let mut ry = card.top() + 54.0;
    for (key, icon, label) in [("add-id", &icons::PLUS, "Add id column"), ("choose-keys", &icons::LIST, "Choose key columns")] {
        let row = Rect::from_min_size(pos2(card.left() + 8.0, ry), vec2((width - 16.0).max(0.0), 28.0));
        let resp = control(shell, ui, &format!("workspace.properties.{key}"), "button", row, true);
        if resp.hovered() {
            paint.rect_filled(row, 5.0, p.surface);
        }
        ux::icon_in(&paint, Rect::from_min_size(row.min + vec2(6.0, 6.0), Vec2::splat(16.0)), icon, p.text_secondary);
        ux::text(&paint, row.min + vec2(30.0, 5.0), &tr(label), types::SMALL, p.text, Some((width - 50.0).max(0.0)));
        if resp.clicked() {
            match key {
                "add-id" => add_id_column(shell, rel, parent_name),
                _ => {
                    // The parent's Properties, where its columns are made key columns.
                    if let Some(id) = shell.session.as_ref().and_then(|s| s.doc().uuid(rel.parent).ok()) {
                        let out = run(shell, "view.select", json!({ "ids": [id.to_string()] }));
                        apply(shell, rel.id, out);
                    }
                }
            }
            return;
        }
        ry += 30.0;
    }
}

/// Makes the relationship identifying or not: the foreign key's columns join or leave the
/// child's primary key, then the flag follows, as one step.
fn set_identifying(shell: &mut Shell, rel: &Rel, on: bool) {
    let Some(session) = shell.session.as_mut() else { return };
    let store = session.doc();
    let Some(fk) = rel.fk.as_ref() else {
        shell.panels.notice = Some(tr("An identifying relationship needs its foreign key; turn the switch on first"));
        return;
    };
    let Some(child) = store.uuid(rel.child).ok() else { return };
    let pk = archforge_er::rules::primary_key_of(store, rel.child);
    let pk_name = pk.map(|k| name(store, k)).unwrap_or_default();
    let mut columns: Vec<Obj> = pk.map(|k| archforge_er::rules::key_columns(store, k)).unwrap_or_default();
    if on {
        for c in &fk.columns {
            if !columns.contains(c) {
                columns.push(*c);
            }
        }
    } else {
        columns.retain(|c| !fk.columns.contains(c));
    }
    let column_ids: Vec<String> = columns.iter().filter_map(|c| store.uuid(*c).ok()).map(|u| u.to_string()).collect();
    let rel_id = rel.id.to_string();
    let out = session.batch(&tr("Identifying"), |b| {
        if column_ids.is_empty() {
            if pk.is_some() {
                b.execute("er.primary-key.remove", &json!({ "entity": child.to_string() }))?;
            }
        } else {
            b.execute("er.primary-key.set", &json!({ "entity": child.to_string(), "columns": column_ids, "name": pk_name }))?;
        }
        b.execute("er.relationship.set-identifying", &json!({ "relationship": rel_id, "identifying": on }))?;
        Ok(())
    });
    apply(shell, rel.id, out.map(|o| o.value).map_err(|e| e.to_string()));
}

/// Adds an `id` column as the parent's primary key, then the foreign key with its new child
/// columns, as one step.
fn add_id_column(shell: &mut Shell, rel: &Rel, _parent_name: &str) {
    let Some(session) = shell.session.as_mut() else { return };
    let store = session.doc();
    let (Some(parent), Some(child)) = (store.uuid(rel.parent).ok(), store.uuid(rel.child).ok()) else { return };
    let rel_id = rel.id.to_string();
    let out = session.batch(&tr("Add id column"), |b| {
        b.execute("er.attribute.add", &json!({ "entity": parent.to_string(), "name": "id", "type": "BIGINT", "primary_key": true }))?;
        b.execute("er.foreign-key.add", &json!({ "entity": child.to_string(), "references": parent.to_string(), "relationship": rel_id }))?;
        Ok(())
    });
    apply(shell, rel.id, out.map(|o| o.value).map_err(|e| e.to_string()));
}

/// Turns the foreign key on: the engine adds one child column per parent key column with the key.
fn add_foreign_key(shell: &mut Shell, rel: &Rel, _parent_name: &str) {
    let Some(session) = shell.session.as_mut() else { return };
    let store = session.doc();
    if archforge_er::rules::primary_key_of(store, rel.parent).is_none() {
        // Nothing to point at: the card below the switch says so.
        shell.panels.menu = None;
        shell.panels.no_key_card = Some(rel.id);
        return;
    }
    let (Some(parent), Some(child)) = (store.uuid(rel.parent).ok(), store.uuid(rel.child).ok()) else { return };
    let out = session
        .execute("er.foreign-key.add", &json!({ "entity": child.to_string(), "references": parent.to_string(), "relationship": rel.id.to_string() }))
        .map(|o| o.value)
        .map_err(|e| e.to_string());
    apply(shell, rel.id, out);
}

/// Turns the foreign key off: the key and its columns go, as one step.
fn remove_foreign_key(shell: &mut Shell, rel: &Rel) {
    let Some(fk) = rel.fk.as_ref() else { return };
    let Some(session) = shell.session.as_mut() else { return };
    let store = session.doc();
    let columns: Vec<String> = fk.columns.iter().filter_map(|c| store.uuid(*c).ok()).map(|u| u.to_string()).collect();
    let fk_id = fk.id.to_string();
    let out = session.batch(&tr("Remove foreign key"), |b| {
        b.execute("er.foreign-key.remove", &json!({ "foreign_key": fk_id }))?;
        if !columns.is_empty() {
            b.execute("er.remove", &json!({ "ids": columns }))?;
        }
        Ok(())
    });
    apply(shell, rel.id, out.map(|o| o.value).map_err(|e| e.to_string()));
}

/// One row of a menu: its label, a detail at the right, whether it can be picked, whether it is
/// the current choice.
struct Item {
    label: String,
    detail: String,
    enabled: bool,
    on: bool,
}

/// A menu under `anchor` with rich rows and an optional heading before row `at`; gives the index
/// picked, closing on a pick or a click elsewhere.
fn menu(shell: &mut Shell, ui: &egui::Ui, p: &Palette, anchor: Rect, id: &str, items: &[Item], heading: Option<(usize, String)>) -> Option<usize> {
    let heading_h = if heading.is_some() { 22.0 } else { 0.0 };
    let height = 8.0 + items.len() as f32 * 30.0 + heading_h;
    let width = anchor.width().max(200.0);
    let m = ux::drop_rect(ui, anchor, anchor.right() - width, vec2(width, height));
    // Over the panel's rows below it, on a layer of its own.
    ux::overlay(ui, crate::widgets::egui_id(&format!("{id}.layer")), m, |ui| menu_body(shell, ui, p, anchor, m, id, items, heading, heading_h))
}

#[allow(clippy::too_many_arguments)]
fn menu_body(
    shell: &mut Shell,
    ui: &egui::Ui,
    p: &Palette,
    anchor: Rect,
    m: Rect,
    id: &str,
    items: &[Item],
    heading: Option<(usize, String)>,
    heading_h: f32,
) -> Option<usize> {
    let paint = ui.painter().clone();
    let width = m.width();
    ux::shadow(&paint, m, 8.0, p.shadow);
    paint.rect_filled(m, 8.0, p.surface);
    paint.rect_stroke(m, 8.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
    let mut picked = None;
    let mut ry = m.top() + 4.0;
    for (i, item) in items.iter().enumerate() {
        if let Some((at, text)) = &heading
            && *at == i
        {
            ux::text(&paint, pos2(m.left() + 12.0, ry + 4.0), text, types::CAPTION, p.text_muted, Some(width - 24.0));
            ry += heading_h;
        }
        let row = Rect::from_min_size(pos2(m.left() + 4.0, ry), vec2((width - 8.0).max(0.0), 30.0));
        let resp = control(shell, ui, &format!("{id}.{i}"), "menu-item", row, item.enabled);
        if item.on {
            paint.rect_filled(row, 5.0, p.accent_tint);
        } else if resp.hovered() && item.enabled {
            paint.rect_filled(row, 5.0, p.page);
        }
        let ink = if item.enabled { p.text } else { p.text_muted };
        let detail = ux::galley(&paint, &item.detail, types::CAPTION, p.text_muted, Some(width / 2.0));
        let tick = if item.on { 20.0 } else { 0.0 };
        ux::text(&paint, row.min + vec2(10.0, 7.5), &item.label, types::SMALL, ink, Some((row.width() - 30.0 - detail.size().x - tick).max(0.0)));
        if item.on {
            ux::icon_mid(&paint, pos2(row.right() - 16.0, row.center().y), &icons::TICK, p.accent);
        }
        paint.galley(pos2(row.right() - 10.0 - tick - detail.size().x, row.top() + 9.0), detail, p.text_muted);
        if resp.clicked() && item.enabled {
            picked = Some(i);
        }
        ry += 30.0;
    }
    let outside = ui.input(|i| i.pointer.any_pressed() && !i.pointer.interact_pos().is_some_and(|pos| m.contains(pos) || anchor.contains(pos)));
    if picked.is_some() || outside || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        shell.panels.menu = None;
    }
    picked
}

/// Every table text to translate, for the catalogue tests.
#[cfg(test)]
pub(crate) fn texts() -> impl Iterator<Item = &'static str> {
    ACTIONS
        .iter()
        .map(|(_, _, w)| *w)
        .chain(DEFERRABLE.iter().map(|(_, _, w)| *w))
        .chain(MATCH.iter().map(|(_, _, w)| *w))
        .chain(["Add id column", "Choose key columns"])
}

/// Drops what the store no longer has.
pub fn retain_valid(state: &mut crate::panels::State, store: &Store) {
    if state.role.as_ref().is_some_and(|e| store.by_uuid(e.target).is_none()) {
        state.role = None;
    }
    if state.no_key_card.is_some_and(|id| store.by_uuid(id).is_none()) {
        state.no_key_card = None;
    }
}
