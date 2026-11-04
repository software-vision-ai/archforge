//! The Diagrams explorer (slice W4 of the workspace chrome, from the design's "Diagram navigation"
//! frame and the card "New diagram (dialog)"; the groups from "Workspace · Diagrams grouped by type
//! · AR-103"): the tree of the open project's diagrams, a group for each diagram kind, in the
//! folders the model keeps them in, with the model behind the open diagram named under the tree.
//! A click opens the diagram in the centre and selects it, so Properties shows it. The `+`, like
//! the one in the tab strip, opens the New diagram dialog: a name, the kind and the schema the
//! diagram is of, or a new schema when the model has none yet (owner decision).
//! Every change goes through the engine, so undo and redo cover it.
//!
//! The geometry is the Models tree's: rows 34 px apart from y 119, a level 13 px deeper than its
//! parent, the chevron at x 13, the icon at x 31 and the name at x 53 of the first level; the
//! footer 128 px above the column's bottom.

use std::collections::{BTreeSet, HashMap};

use archforge_diagram::{DIAGRAM, diagram};
use archforge_mof::{Obj, Store, Uuid, Value};
use egui::{Rect, Sense, Stroke, StrokeKind, Vec2, pos2, vec2};
use serde_json::json;

use crate::home::control;
use crate::i18n::tr;
use crate::ux::{self, Palette, icons, types};
use crate::{Shell, widgets};

const FIRST_ROW: f32 = 119.0;
const ROW: f32 = 34.0;
const ROW_HEIGHT: f32 = 32.0;
const INDENT: f32 = 13.0;
/// The most rows the tree draws and the deepest folder level it enters.
const MAX_ROWS: usize = 2000;
const MAX_DEPTH: u8 = 32;
/// The most characters a diagram's name may have: the diagram rule's.
const MAX_NAME: usize = archforge_diagram::rules::MAX_NAME;
/// The footer's lines, from the column's bottom.
const FOOTER_LINE: f32 = 128.0;
const FOOTER_TITLE: f32 = 105.0;
const FOOTER_TEXT: f32 = 80.0;

/// What the explorer keeps between frames: nothing the model holds.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct State {
    /// The folders folded shut; everything else is open.
    pub collapsed: BTreeSet<Uuid>,
    /// The groups folded shut, by their diagram kind.
    pub folded: BTreeSet<&'static str>,
    /// The filter field's text.
    pub filter: String,
    /// The New diagram dialog, while it is open.
    pub new: Option<NewDiagram>,
    /// The last refusal, shown under the tree until the next action.
    pub notice: Option<String>,
    /// How far the tree is scrolled, and the row it last brought into view.
    pub scroll: crate::explorer::TreeScroll,
}

/// The New diagram dialog's fields: the step it is at, the name, the kind, the element the
/// diagram is of (a schema, a package; none for a new one named like the diagram), the select that
/// is open, the engine's refusal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewDiagram {
    pub step: Step,
    pub name: String,
    pub kind: &'static str,
    pub schema: Option<Uuid>,
    pub menu: Option<Field>,
    pub refused: Option<String>,
    /// True until the name field has taken the keyboard once on the second step.
    pub opened: bool,
}

/// The New diagram dialog's steps: the kind first, on a card for each, then its name and what it
/// is of.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Kind,
    Details,
}

/// The dialog's selects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Schema,
}

/// What a row shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// The group of a diagram kind's diagrams, by the kind; "" for the kinds the shell doesn't know.
    Group(&'static str),
    /// A folder, in the group of the kind named: a folder holding diagrams of two kinds shows in
    /// both groups, a row in each.
    Folder(&'static str),
    Diagram,
}

/// One row of the tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub id: Uuid,
    pub obj: Obj,
    pub depth: u8,
    pub kind: Kind,
    pub label: String,
    pub expandable: bool,
    pub expanded: bool,
    /// The diagrams a group or folder holds, as the filter stands.
    pub count: usize,
}

/// A diagram's name as stored, or nothing.
pub fn diagram_name(store: &Store, d: Obj) -> String {
    store.get(d, diagram::NAME).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default()
}

/// What a skill's subject or origin is called: a diagram by its name, or "Unnamed diagram" when it
/// has none; any other element by its name.
pub fn subject_name(store: &Store, o: Obj) -> String {
    if store.class_of(o).ok() != Some(archforge_diagram::DIAGRAM) {
        return archforge_core::name_of(store, o);
    }
    let name = diagram_name(store, o);
    if name.is_empty() { tr("Unnamed diagram") } else { name }
}

/// The schema a diagram is of, when it has one.
pub fn schema_of(store: &Store, d: Obj) -> Option<Obj> {
    store.get(d, diagram::ELEMENT).ok().and_then(|v| v.as_ref()).filter(|o| store.is_alive(*o))
}

fn members(store: &Store, owner: Obj, prop: archforge_mof::PropId) -> Vec<Obj> {
    store.list(owner, prop).unwrap_or_default().iter().filter_map(Value::as_ref).filter(|o| store.is_alive(*o)).collect()
}

/// The id a group's row carries: a group is no element of the model.
const GROUP_ID: Uuid = Uuid([0; 16]);

/// The kind a diagram is of, as the diagram metamodel keeps it.
fn kind_word(store: &Store, d: Obj) -> String {
    store.get(d, diagram::KIND).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default()
}

/// The most members of the model the tree visits in a frame, once to count and once to walk; a
/// model past it gets the filter.
const MAX_VISITS: usize = 200_000;

/// The tree's rows, and whether the walk stopped short of the model: at `MAX_ROWS` rows,
/// `MAX_VISITS` visits or `MAX_DEPTH` levels.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Tree {
    pub rows: Vec<Row>,
    pub cut: bool,
}

/// A group of the tree: its diagram kind ("" for those the shell doesn't know), its name and the
/// filter its diagrams answer to (none when the typed filter names the group).
struct Group {
    kind: &'static str,
    label: String,
    filter: String,
}

impl Group {
    /// Whether a diagram of `kind` named `name` (lower-cased) shows in the group.
    fn shows(&self, known: &[&str], kind: &str, name: &str) -> bool {
        let ours = if self.kind.is_empty() { !known.contains(&kind) } else { kind == self.kind };
        ours && (self.filter.is_empty() || name.contains(&self.filter))
    }
}

/// The diagrams the model's folders hold for each group, from one visit of the model.
struct Tally {
    folders: HashMap<(Obj, usize), usize>,
    left: usize,
    cut: bool,
}

/// Counts the diagrams under `owner` that show in each group, noting each folder's counts in
/// `t`; every member visited is taken from its budget.
fn tally(store: &Store, owner: Obj, prop: archforge_mof::PropId, groups: &[Group], known: &[&str], depth: u8, t: &mut Tally) -> Vec<usize> {
    let mut here = vec![0; groups.len()];
    if depth >= MAX_DEPTH {
        t.cut = true;
        return here;
    }
    for m in members(store, owner, prop) {
        if t.left == 0 {
            t.cut = true;
            break;
        }
        t.left -= 1;
        match store.class_of(m).ok() {
            Some(c) if c == archforge_core::FOLDER => {
                let held = tally(store, m, archforge_core::folder::MEMBER, groups, known, depth.saturating_add(1), t);
                for (i, n) in held.into_iter().enumerate().filter(|(_, n)| *n > 0) {
                    t.folders.insert((m, i), n);
                    here[i] += n;
                }
            }
            Some(c) if c == DIAGRAM => {
                let (kind, name) = (kind_word(store, m), diagram_name(store, m).to_lowercase());
                for (i, g) in groups.iter().enumerate() {
                    here[i] += usize::from(g.shows(known, &kind, &name));
                }
            }
            _ => {}
        }
    }
    here
}

/// The tree's rows: a group for each kind the shell knows, in the order of their names, holding
/// that kind's diagrams in the folders the model keeps them in, in the model's order; then the
/// diagrams of kinds the shell doesn't know, when there are any. Grouping is the explorer's alone:
/// the model keeps each diagram once, where it was. A filter shows the diagrams whose name holds
/// it, or every diagram of a group whose name holds it, with their folders and groups, all open;
/// without one every known kind's group shows, empty or not.
pub(crate) fn rows(store: &Store, state: &State, kinds: &[&dyn crate::kinds::DiagramKind]) -> Tree {
    rows_within(store, state, kinds, MAX_VISITS)
}

/// The tree's rows, visiting at most `visits` members of the model to count them and as many to
/// walk them.
pub(crate) fn rows_within(store: &Store, state: &State, kinds: &[&dyn crate::kinds::DiagramKind], visits: usize) -> Tree {
    let typed = state.filter.trim().to_lowercase();
    let open = !typed.is_empty();
    let mut tree = Tree::default();
    let Some(model) = archforge_core::skills::model_of(store) else { return tree };
    let mut groups: Vec<Group> = kinds.iter().map(|k| Group { kind: k.kind(), label: k.group_word(), filter: String::new() }).collect();
    groups.sort_by_key(|g| g.label.to_lowercase());
    let known: Vec<&'static str> = groups.iter().map(|g| g.kind).collect();
    groups.push(Group { kind: "", label: tr("Other diagrams"), filter: String::new() });
    for g in &mut groups {
        if !g.label.to_lowercase().contains(&typed) {
            g.filter = typed.clone();
        }
    }
    let mut t = Tally { folders: HashMap::new(), left: visits, cut: false };
    let counts = tally(store, model, archforge_core::model::MEMBER, &groups, &known, 0, &mut t);
    tree.cut = t.cut;
    let mut left = visits;
    for (i, g) in groups.iter().enumerate() {
        let count = counts[i];
        if count == 0 && (g.kind.is_empty() || open) {
            continue;
        }
        if tree.rows.len() >= MAX_ROWS {
            tree.cut = true;
            break;
        }
        let expanded = count > 0 && (open || !state.folded.contains(g.kind));
        tree.rows.push(Row {
            id: GROUP_ID,
            obj: model,
            depth: 0,
            kind: Kind::Group(g.kind),
            label: g.label.clone(),
            expandable: count > 0,
            expanded,
            count,
        });
        if expanded {
            let walk = Walk { store, state, open, group: i, g, known: &known, folders: &t.folders };
            if walk.walk(model, archforge_core::model::MEMBER, 1, &mut tree.rows, &mut left) {
                tree.cut = true;
                break;
            }
        }
    }
    tree
}

/// What a group's walk reads: the model, the explorer's state, whether a filter is typed (which
/// opens every folder), the group and the counts.
struct Walk<'a> {
    store: &'a Store,
    state: &'a State,
    open: bool,
    group: usize,
    g: &'a Group,
    known: &'a [&'static str],
    folders: &'a HashMap<(Obj, usize), usize>,
}

impl Walk<'_> {
    /// Adds the group's rows under `owner`: each folder holding a diagram the group shows, open
    /// unless folded shut (a typed filter opens it), and the group's diagrams. True when it stopped
    /// short, at `MAX_ROWS` rows or with `left` spent.
    fn walk(&self, owner: Obj, prop: archforge_mof::PropId, depth: u8, out: &mut Vec<Row>, left: &mut usize) -> bool {
        if depth >= MAX_DEPTH {
            return true;
        }
        for m in members(self.store, owner, prop) {
            if out.len() >= MAX_ROWS || *left == 0 {
                return true;
            }
            *left -= 1;
            let Ok(id) = self.store.uuid(m) else { continue };
            match self.store.class_of(m).ok() {
                Some(c) if c == archforge_core::FOLDER => {
                    let Some(&count) = self.folders.get(&(m, self.group)) else { continue };
                    let label = archforge_core::name_of(self.store, m);
                    let expanded = self.open || !self.state.collapsed.contains(&id);
                    out.push(Row { id, obj: m, depth, kind: Kind::Folder(self.g.kind), label, expandable: true, expanded, count });
                    if expanded && self.walk(m, archforge_core::folder::MEMBER, depth.saturating_add(1), out, left) {
                        return true;
                    }
                }
                Some(c) if c == DIAGRAM => {
                    let label = diagram_name(self.store, m);
                    if self.g.shows(self.known, &kind_word(self.store, m), &label.to_lowercase()) {
                        out.push(Row { id, obj: m, depth, kind: Kind::Diagram, label, expandable: false, expanded: false, count: 0 });
                    }
                }
                _ => {}
            }
        }
        false
    }
}

/// Where the element `of` sits, for the footer and a diagram's hover text: the project, then the
/// elements holding it from the model down, then it. The model's root package carries the
/// project's name, so the name is said once.
pub fn trail(store: &Store, project: &str, of: Obj) -> String {
    let mut names = Vec::new();
    let mut at = Some(of);
    for _ in 0..MAX_DEPTH {
        let Some(o) = at.filter(|o| store.is_alive(*o)) else { break };
        if store.class_of(o).ok() == Some(archforge_core::MODEL) {
            break;
        }
        names.push(archforge_core::name_of(store, o));
        at = store.container(o).ok().flatten().map(|(c, _)| c);
    }
    names.reverse();
    if !project.is_empty() && names.first().is_some_and(|n| n == project) {
        names.remove(0);
    }
    let head = (!project.is_empty()).then(|| project.to_string());
    head.into_iter().chain(names).collect::<Vec<_>>().join(" / ")
}

fn run(shell: &mut Shell, id: &str, params: serde_json::Value) -> Result<serde_json::Value, String> {
    let Some(session) = shell.session.as_mut() else { return Err(tr("The model could not be opened")) };
    session.execute(id, &params).map(|o| o.value).map_err(|e| e.to_string())
}

/// The kind the dialog makes a diagram of.
fn kind_named(shell: &Shell, kind: &str) -> Option<&'static dyn crate::kinds::DiagramKind> {
    shell.kinds.iter().copied().find(|k| k.kind() == kind)
}

/// Opens the New diagram dialog at its first step, the kind: the kind and what it is of start from
/// the selection (its schema or package), else the first kind and its first element, else a new
/// one named like the diagram.
pub fn open_new(shell: &mut Shell) {
    open_new_of(shell, None);
}

/// Opens the New diagram dialog on the kind `kind` when it is given (a group's `+`), at its second
/// step, else as `open_new` does, at its first; what the diagram is of starts from the selection
/// when it fits the kind.
pub(crate) fn open_new_of(shell: &mut Shell, kind: Option<&'static dyn crate::kinds::DiagramKind>) {
    let from_group = kind.is_some();
    let Some(session) = shell.session.as_ref() else { return };
    let store = session.doc();
    let selected = session.view().selection.first().copied().filter(|o| store.is_alive(*o));
    let from_selection = selected
        .and_then(|o| shell.kinds.iter().filter(|k| kind.is_none_or(|w| w.kind() == k.kind())).find_map(|k| k.owner_of(store, o).map(|of| (*k, of))));
    let Some(kind) = from_selection.map(|(k, _)| k).or(kind).or_else(|| shell.kinds.first().copied()) else { return };
    let schema = from_selection.map(|(_, of)| of).or_else(|| kind.owners(store).first().copied());
    let taken = store.alive().filter(|o| store.class_of(*o).ok() == Some(DIAGRAM)).map(|d| diagram_name(store, d));
    let name = match schema {
        Some(s) => archforge_core::name_of(store, s),
        None => crate::explorer::fresh_name(&tr("Diagram"), taken, false).unwrap_or_default(),
    };
    // A group's + has chosen the kind already: the dialog opens on its details, Back before them.
    let step = if from_group { Step::Details } else { Step::Kind };
    shell.diagrams.new =
        Some(NewDiagram { step, name, kind: kind.kind(), schema: schema.and_then(|s| store.uuid(s).ok()), menu: None, refused: None, opened: true });
    shell.diagrams.notice = None;
}

/// What a diagram of `kind` is of when nothing else says, and the name it starts with: the kind's
/// first element and its name, else none and a free "Diagram" name.
fn defaults(store: &Store, kind: &dyn crate::kinds::DiagramKind) -> (Option<Uuid>, String) {
    let schema = kind.owners(store).first().copied();
    let taken = store.alive().filter(|o| store.class_of(*o).ok() == Some(DIAGRAM)).map(|d| diagram_name(store, d));
    let name = match schema {
        Some(s) => archforge_core::name_of(store, s),
        None => crate::explorer::fresh_name(&tr("Diagram"), taken, false).unwrap_or_default(),
    };
    (schema.and_then(|s| store.uuid(s).ok()), name)
}

/// Creates the diagram the dialog describes: the schema first when a new one is asked for, then
/// the diagram of it, opened in the centre. A refusal stays in the dialog.
fn create(shell: &mut Shell) {
    let Some(edit) = shell.diagrams.new.clone() else { return };
    let name = edit.name.trim().to_string();
    if name.is_empty() {
        if let Some(e) = shell.diagrams.new.as_mut() {
            e.refused = Some(tr("Name the diagram"));
        }
        return;
    }
    let Some(kind) = kind_named(shell, edit.kind) else { return };
    let fresh_schema = edit.schema.is_none();
    let schema = match edit.schema {
        Some(s) => Ok(s.to_string()),
        None => kind.add_owner(shell, &name),
    };
    let made = schema.and_then(|schema| {
        let made = run(shell, "diagram.add", json!({ "kind": kind.kind(), "element": &schema, "name": name }));
        // A schema made for a diagram that was then refused goes again, so nothing is left behind.
        if made.is_err() && fresh_schema {
            let _ = run(shell, "element.remove", json!({ "ids": [schema] }));
        }
        made
    });
    match made {
        Ok(v) => {
            shell.diagrams.new = None;
            if let Some(id) = v.get("id").and_then(|i| i.as_str()).and_then(Uuid::parse) {
                crate::canvas::open(shell, id);
            }
        }
        Err(e) => {
            if let Some(edit) = shell.diagrams.new.as_mut() {
                edit.refused = Some(e);
            }
        }
    }
}

/// Drops what the store no longer has.
pub fn retain_valid(state: &mut State, store: &Store) {
    state.collapsed.retain(|id| store.by_uuid(*id).is_some());
    if let Some(n) = state.new.as_mut()
        && n.schema.is_some_and(|s| store.by_uuid(s).is_none())
    {
        n.schema = None;
    }
}

/// Draws the Diagrams explorer into `r`: the title with `+`, the filter field, the tree and the
/// model behind the open diagram.
pub fn draw(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect) {
    let paint = ui.painter().clone();
    let at = |x: f32, y: f32| r.min + vec2(x, y);
    ux::text(&paint, at(20.0, 25.0), &tr("Diagrams").to_uppercase(), types::EYEBROW, p.text, Some((r.width() - 80.0).max(0.0)));
    let add = Rect::from_min_size(pos2(r.right() - 40.0, r.top() + 20.0), Vec2::splat(24.0));
    ux::icon_in(&paint, Rect::from_center_size(add.center(), Vec2::splat(16.0)), &icons::PLUS, p.text_secondary);
    let live = shell.session.is_some();
    if control(shell, ui, "workspace.diagrams.add", "button", add, live).on_hover_text(tr("New diagram")).clicked() {
        open_new(shell);
    }
    filter_field(shell, ui, p, Rect::from_min_max(at(16.0, 65.0), pos2(r.right() - 16.0, r.top() + 97.0)));
    let Tree { rows, cut } = match &shell.session {
        Some(s) => rows(s.doc(), &shell.diagrams, shell.kinds),
        None => Tree::default(),
    };
    let active = shell.canvas.active;
    let footer = active.is_some();
    let tree = Rect::from_min_max(at(0.0, FIRST_ROW - 12.0), pos2(r.right(), if footer { r.bottom() - FOOTER_LINE } else { r.bottom() }));
    // Under the groups, while no group holds a diagram: no diagram yet, or none matching.
    if rows.iter().all(|row| row.count == 0) {
        let empty = if shell.diagrams.filter.trim().is_empty() { tr("No diagrams yet") } else { tr("Nothing matches the filter") };
        let y = FIRST_ROW + rows.len() as f32 * ROW;
        ux::text(&paint, at(36.0, y), &empty, types::SMALL, p.text_muted, Some((r.width() - 52.0).max(0.0)));
    }
    // The rows scroll under the filter; the diagram newly opened is brought into view.
    let follow = active.and_then(|a| rows.iter().position(|row| row.id == a).zip(Some(a)));
    let tail = if cut { 40.0 } else { 12.0 };
    let wheel = shell.diagrams.new.is_none();
    let offset = crate::explorer::scroll_tree(ui, p, tree, r.top() + FIRST_ROW, rows.len(), ROW, tail, wheel, follow, &mut shell.diagrams.scroll);
    let mut clip = ui.new_child(egui::UiBuilder::new().max_rect(tree));
    clip.set_clip_rect(tree);
    for (i, row) in rows.iter().enumerate() {
        let y = r.top() + FIRST_ROW + i as f32 * ROW - offset;
        if y + ROW_HEIGHT < tree.top() || y - 7.0 > tree.bottom() {
            continue;
        }
        tree_row(shell, &mut clip, p, r, y, row, active == Some(row.id));
    }
    if cut {
        let y = r.top() + FIRST_ROW + rows.len() as f32 * ROW - offset;
        ux::text(
            &clip.painter().clone(),
            pos2(r.left() + 36.0, y),
            &tr("Narrow the filter to see more"),
            types::SMALL,
            p.text_muted,
            Some((r.width() - 52.0).max(0.0)),
        );
    }
    if footer {
        footer_lines(shell, &paint, p, r);
    }
    if let Some(notice) = shell.diagrams.notice.clone() {
        ux::text(&paint, pos2(r.left() + 16.0, r.bottom() - 40.0), &notice, types::CAPTION, p.amber, Some((r.width() - 32.0).max(0.0)));
    }
}

/// The model behind the open diagram: the project and where the schema or package the diagram is
/// of sits.
fn footer_lines(shell: &Shell, paint: &egui::Painter, p: &Palette, r: Rect) {
    let Some(session) = shell.session.as_ref() else { return };
    let store = session.doc();
    let Some(d) = shell.canvas.active.and_then(|id| store.by_uuid(id)) else { return };
    let project = shell.project.as_ref().map(|o| o.name.clone()).unwrap_or_default();
    let trail = schema_of(store, d).map_or_else(|| project.clone(), |s| trail(store, &project, s));
    paint.rect_filled(Rect::from_min_size(pos2(r.left() + 16.0, r.bottom() - FOOTER_LINE), vec2((r.width() - 32.0).max(0.0), 1.0)), 0.0, p.border);
    let room = Some((r.width() - 40.0).max(0.0));
    ux::text(
        paint,
        pos2(r.left() + 20.0, r.bottom() - FOOTER_TITLE),
        &tr("Model behind this diagram").to_uppercase(),
        types::EYEBROW,
        p.text_muted,
        room,
    );
    ux::text(paint, pos2(r.left() + 20.0, r.bottom() - FOOTER_TEXT), &trail, types::UI, p.text, room);
}

/// The filter field: a real text field narrowing the tree as it is typed.
fn filter_field(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, field: Rect) {
    let paint = ui.painter().clone();
    paint.rect_filled(field, 8.0, p.page);
    paint.rect_stroke(field, 8.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
    ux::icon_mid(&paint, pos2(field.left() + 17.0, field.center().y), &icons::SEARCH, p.text_secondary);
    widgets::record_visible(ui, &mut shell.registry, "workspace.diagrams.filter", "field", field);
    let inner = crate::panels::field_inner(ui, field.with_min_x(field.left() + 22.0));
    let edit = egui::TextEdit::singleline(&mut shell.diagrams.filter)
        .id(widgets::egui_id("workspace.diagrams.filter"))
        .frame(egui::Frame::NONE)
        .margin(egui::Margin::ZERO)
        .font(types::SMALL.font())
        .text_color(p.text)
        .hint_text(egui::RichText::new(tr("Filter diagrams…")).color(p.text_secondary).font(types::SMALL.font()))
        .char_limit(MAX_NAME)
        .desired_width(inner.width());
    let _ = ui.put(inner, edit).accessible_name(tr("Filter diagrams…"));
}

/// The key a group's controls carry: its diagram kind, or "other".
fn group_key(kind: &str) -> &str {
    if kind.is_empty() { "other" } else { kind }
}

/// One row: its chevron, icon and name; a click on a diagram opens it, on a folder or a group
/// folds it. A group also shows how many diagrams it holds and, for a kind the shell knows, a `+`
/// that opens the New diagram dialog on that kind; a diagram's hover text says where what it is of
/// sits.
fn tree_row(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, column: Rect, y: f32, row: &Row, is_active: bool) {
    let paint = ui.painter().clone();
    let indent = row.depth as f32 * INDENT;
    let hit = Rect::from_min_max(pos2(column.left() + 8.0, y - 7.0), pos2(column.right() - 8.0, y - 7.0 + ROW_HEIGHT));
    if is_active {
        paint.rect_filled(hit, 6.0, p.accent_tint);
    }
    let id = match row.kind {
        Kind::Group(kind) => format!("workspace.diagrams.group.{}", group_key(kind)),
        // A folder may show in two groups: each row its own control.
        Kind::Folder(kind) => format!("workspace.diagrams.group.{}.folder.{}", group_key(kind), row.id),
        Kind::Diagram => format!("workspace.diagrams.row.{}", row.id),
    };
    let mut resp = control(shell, ui, &id, "row", hit, true);
    if row.kind == Kind::Diagram
        && let Some(s) = shell.session.as_ref()
        && let Some(of) = schema_of(s.doc(), row.obj)
    {
        let project = shell.project.as_ref().map(|o| o.name.clone()).unwrap_or_default();
        resp = resp.on_hover_text(trail(s.doc(), &project, of));
    }
    if row.expandable {
        let chevron = Rect::from_center_size(pos2(column.left() + 21.0 + indent, hit.center().y), Vec2::splat(16.0));
        ux::icon_in(&paint, chevron, if row.expanded { &icons::CHEVRON_DOWN } else { &icons::CHEVRON_RIGHT }, p.text_secondary);
    }
    let ink = if is_active { p.accent } else { p.text };
    let mid = hit.center().y;
    let icon_ink = if is_active { p.accent } else { p.text_secondary };
    let known = |kind: &str| shell.kinds.iter().copied().find(|k| k.kind() == kind);
    let icon = match (row.kind, shell.session.as_ref()) {
        (Kind::Diagram, Some(s)) => crate::kinds::diagram_icon(shell.kinds, s.doc(), row.obj),
        (Kind::Diagram, None) => &icons::ER_DIAGRAM,
        (Kind::Group(kind), _) => known(kind).map_or(&icons::FOLDER, |k| k.icon()),
        (Kind::Folder(_), _) => &icons::FOLDER,
    };
    ux::icon_mid(&paint, pos2(column.left() + 38.5 + indent, mid), icon, icon_ink);
    let name_x = column.left() + 53.0 + indent;
    let label = if row.label.is_empty() { tr("Unnamed diagram") } else { row.label.clone() };
    // A group keeps room at its right for its count and its +.
    let room = if matches!(row.kind, Kind::Group(_)) { 70.0 } else { 16.0 };
    ux::text_mid(&paint, name_x, mid, &label, types::UI, ink, Some((column.right() - room - name_x).max(0.0)));
    let mut add = None;
    if let Kind::Group(kind) = row.kind {
        ux::text_mid(&paint, column.right() - 62.0, mid, &row.count.to_string(), types::SMALL, p.text_muted, Some(20.0));
        if let Some(k) = known(kind) {
            let plus = Rect::from_center_size(pos2(column.right() - 29.0, mid), Vec2::splat(24.0));
            ux::icon_in(&paint, Rect::from_center_size(plus.center(), Vec2::splat(16.0)), &icons::PLUS, p.text_secondary);
            let live = shell.session.is_some();
            let key = format!("workspace.diagrams.group.{}.add", group_key(kind));
            if control(shell, ui, &key, "button", plus, live).on_hover_text(tr("New diagram")).clicked() {
                add = Some(k);
            }
        }
    }
    if let Some(k) = add {
        open_new_of(shell, Some(k));
        return;
    }
    if resp.clicked() {
        match row.kind {
            Kind::Group(kind) => {
                if row.expanded {
                    shell.diagrams.folded.insert(kind);
                } else {
                    shell.diagrams.folded.remove(kind);
                }
            }
            Kind::Folder(_) => {
                if row.expanded {
                    shell.diagrams.collapsed.insert(row.id);
                } else {
                    shell.diagrams.collapsed.remove(&row.id);
                }
            }
            Kind::Diagram => {
                shell.diagrams.notice = None;
                crate::canvas::open(shell, row.id);
            }
        }
    }
}

// The New diagram dialog.

/// Draws the New diagram dialog over the window while it is open.
pub(crate) fn new_dialog(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette) {
    if shell.diagrams.new.is_none() {
        return;
    }
    let window = ui.ctx().content_rect();
    let p = *p;
    egui::Area::new(widgets::egui_id("workspace.new-diagram")).order(egui::Order::Foreground).fixed_pos(window.min).show(ui.ctx(), |ui| {
        ui.set_clip_rect(window);
        let paint = ui.painter().clone();
        paint.rect_filled(window, 0.0, p.scrim);
        ui.interact(window, widgets::egui_id("workspace.new-diagram.veil"), Sense::click_and_drag());
        new_dialog_body(shell, ui, &p, &paint, window);
    });
}

fn new_dialog_body(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, paint: &egui::Painter, window: Rect) {
    match shell.diagrams.new.as_ref().map(|n| n.step) {
        Some(Step::Kind) => kind_step(shell, ui, p, paint, window),
        Some(Step::Details) => details_step(shell, ui, p, paint, window),
        None => {}
    }
}

/// The dialog's frame of `size`, centred in `window`, with its title and the step under it.
fn dialog_frame(paint: &egui::Painter, p: &Palette, window: Rect, size: Vec2, title: &str, step: &str) -> Rect {
    let dialog = Rect::from_center_size(window.center(), size);
    ux::shadow(paint, dialog, 10.0, p.shadow);
    paint.rect_filled(dialog, 10.0, p.surface);
    paint.rect_stroke(dialog, 10.0, Stroke::new(1.0, p.dialog_edge), StrokeKind::Inside);
    let width = Some((size.x - 40.0).max(0.0));
    ux::text(paint, dialog.min + vec2(20.0, 20.0), title, types::CARD_TITLE, p.text, width);
    ux::text(paint, dialog.min + vec2(20.0, 46.0), step, types::SMALL, p.text_secondary, width);
    dialog
}

/// The dialog's buttons from the right edge leftwards, each `(id, label, primary)`; the one
/// clicked.
fn buttons(shell: &mut Shell, ui: &egui::Ui, p: &Palette, paint: &egui::Painter, dialog: Rect, row: &[(&str, String, bool)]) -> Option<usize> {
    let mut x = dialog.right() - 20.0;
    let mut clicked = None;
    for (i, (id, label, primary)) in row.iter().enumerate() {
        // Laid out in the ink it is drawn in: a galley keeps the colour it was laid out with.
        let ink = if *primary { p.on_accent } else { p.text };
        let g = ux::galley(paint, label, types::SMALL, ink, None);
        let b = Rect::from_min_max(pos2(x - g.size().x - 24.0, dialog.bottom() - 48.0), pos2(x, dialog.bottom() - 20.0));
        paint.rect_filled(b, 6.0, if *primary { p.accent_fill } else { p.field });
        paint.galley(b.min + vec2(12.0, (b.height() - g.size().y) / 2.0), g, ink);
        let resp = control(shell, ui, id, "button", b, true);
        if resp.has_focus() {
            paint.rect_stroke(b.expand(2.0), 8.0, Stroke::new(2.0, p.focus), StrokeKind::Outside);
        }
        if resp.clicked() {
            clicked = Some(i);
        }
        x = b.left() - 8.0;
    }
    clicked
}

/// The card of a kind: its sketch, its name and what its diagrams show.
const CARD: f32 = 196.0;
/// Narrower than this, the cards stand one above the other.
const SIDE_BY_SIDE: f32 = 560.0;

/// The first step: a card for each kind the shell knows, the chosen one marked; Continue (or
/// Enter, or a double click) goes on to its details, the arrow keys move between the cards, and
/// Cancel (or Escape) closes the dialog with nothing made.
fn kind_step(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, paint: &egui::Painter, window: Rect) {
    let kinds = shell.kinds;
    let Some(chosen) = shell.diagrams.new.as_ref().map(|n| n.kind) else { return };
    let width = 640.0_f32.min((window.width() - 32.0).max(240.0));
    let stacked = width < SIDE_BY_SIDE;
    let count = kinds.len().max(1) as f32;
    let card_w = if stacked { width - 40.0 } else { (width - 40.0 - 16.0 * (count - 1.0)) / count };
    let cards_h = if stacked { count * CARD + (count - 1.0) * 16.0 } else { CARD };
    let dialog =
        dialog_frame(paint, p, window, vec2(width, 76.0 + cards_h + 72.0), &tr("New diagram"), &tr("Step 1 of 2 · Choose what the diagram shows"));
    let mut pick: Option<(&'static str, bool)> = None;
    for (i, kind) in kinds.iter().enumerate() {
        let offset = i as f32 * if stacked { CARD + 16.0 } else { card_w + 16.0 };
        let card = if stacked {
            Rect::from_min_size(dialog.min + vec2(20.0, 76.0 + offset), vec2(card_w, CARD))
        } else {
            Rect::from_min_size(dialog.min + vec2(20.0 + offset, 76.0), vec2(card_w, CARD))
        };
        let selected = kind.kind() == chosen;
        let id = format!("workspace.new-diagram.type.{}", kind.kind());
        widgets::record_visible(ui, &mut shell.registry, &id, "radio", card);
        let resp = ui.interact(card, widgets::egui_id(&id), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
        resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::RadioButton, true, selected, kind.word()));
        if resp.has_focus() {
            paint.rect_stroke(card.expand(3.0), 11.0, Stroke::new(2.0, p.focus), StrokeKind::Inside);
        }
        paint.rect_filled(card, 8.0, if selected { p.accent_tint } else { p.surface });
        paint.rect_stroke(card, 8.0, Stroke::new(if selected { 2.0 } else { 1.0 }, if selected { p.accent } else { p.border }), StrokeKind::Inside);
        let picture = Rect::from_min_size(card.min + vec2(12.0, 12.0), vec2(card.width() - 24.0, 100.0));
        paint.rect_filled(picture, 6.0, p.page);
        kind.sketch(&paint.with_clip_rect(picture), p, picture);
        let room = Some((card.width() - 32.0).max(0.0));
        ux::text(paint, card.min + vec2(16.0, 124.0), &kind.word(), types::UI_STRONG, p.text, room);
        let g = ux::galley_rows(paint, &kind.description(), types::CAPTION, p.text_secondary, room, 3);
        paint.galley(card.min + vec2(16.0, 148.0), g, p.text_secondary);
        if selected {
            let mark = pos2(card.right() - 21.0, card.top() + 27.0);
            paint.circle_filled(mark, 9.0, p.accent_fill);
            let ink = Stroke::new(1.8, p.on_accent);
            paint.line_segment([mark + vec2(-5.0, 0.0), mark + vec2(-1.5, 3.5)], ink);
            paint.line_segment([mark + vec2(-1.5, 3.5), mark + vec2(5.0, -3.0)], ink);
        }
        if resp.double_clicked() {
            pick = Some((kind.kind(), true));
        } else if resp.clicked() {
            pick = Some((kind.kind(), false));
        }
    }
    // The arrow keys move the choice; Enter goes on with it, unless Tab has reached Continue or
    // Cancel, whose own press it then is.
    let on_button = ui
        .memory(|m| m.focused())
        .is_some_and(|f| [widgets::egui_id("workspace.new-diagram.continue"), widgets::egui_id("workspace.new-diagram.cancel")].contains(&f));
    let (back, forth, enter) = ui.input_mut(|i| {
        (
            i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowLeft) || i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp),
            i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowRight) || i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown),
            !on_button && i.consume_key(egui::Modifiers::NONE, egui::Key::Enter),
        )
    });
    if (back || forth)
        && let Some(at) = kinds.iter().position(|k| k.kind() == chosen)
    {
        let next = if forth { (at + 1).min(kinds.len() - 1) } else { at.saturating_sub(1) };
        pick = kinds.get(next).map(|k| (k.kind(), false));
    }
    let clicked = buttons(
        shell,
        ui,
        p,
        paint,
        dialog,
        &[("workspace.new-diagram.continue", tr("Continue"), true), ("workspace.new-diagram.cancel", tr("Cancel"), false)],
    );
    if let Some((kind, go_on)) = pick {
        choose_kind(shell, kind);
        if go_on {
            continue_to_details(shell);
        }
    }
    if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) || clicked == Some(1) {
        shell.diagrams.new = None;
    } else if clicked == Some(0) || enter {
        continue_to_details(shell);
    }
}

/// Chooses another kind on the first step: what the diagram is of becomes the kind's default, and
/// so does the name while it is still the one the dialog gave; a name typed is kept.
fn choose_kind(shell: &mut Shell, kind: &'static str) {
    let Some(picked) = kind_named(shell, kind) else { return };
    let Some(store) = shell.session.as_ref().map(|s| s.doc()) else { return };
    let Some(edit) = shell.diagrams.new.as_ref() else { return };
    if edit.kind == kind {
        return;
    }
    let given = kind_named(shell, edit.kind).map(|k| defaults(store, k).1);
    let (schema, name) = defaults(store, picked);
    let keep = given.is_none_or(|g| g != edit.name) && !edit.name.trim().is_empty();
    if let Some(edit) = shell.diagrams.new.as_mut() {
        edit.kind = kind;
        edit.schema = schema;
        if !keep {
            edit.name = name;
        }
        edit.refused = None;
        edit.menu = None;
    }
}

fn continue_to_details(shell: &mut Shell) {
    if let Some(edit) = shell.diagrams.new.as_mut() {
        edit.step = Step::Details;
        edit.opened = true;
        edit.refused = None;
    }
}

/// The second step: the kind's name for a new diagram as the title, the name, what it is of, the
/// engine's refusal; Back returns to the kinds with all of it kept, Cancel (or Escape) closes with
/// nothing made, Create (or Enter) makes it.
fn details_step(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, paint: &egui::Painter, window: Rect) {
    let Some(kind) = shell.diagrams.new.as_ref().and_then(|n| kind_named(shell, n.kind)) else { return };
    let refused = shell.diagrams.new.as_ref().and_then(|e| e.refused.clone());
    let width = 440.0_f32.min((window.width() - 32.0).max(240.0));
    let size = vec2(width, 242.0 + if refused.is_some() { 36.0 } else { 0.0 });
    let dialog = dialog_frame(paint, p, window, size, &kind.new_word(), &tr("Step 2 of 2 · Name it and say what it is of"));
    let at = |x: f32, y: f32| dialog.min + vec2(x, y);
    let inner_w = width - 40.0;
    // The elements the diagram may be of, read once.
    let (schemas, taken_focus) = match (shell.session.as_ref(), shell.diagrams.new.as_ref()) {
        (Some(s), Some(n)) => {
            let store = s.doc();
            (
                kind.owners(store).into_iter().filter_map(|o| Some((store.uuid(o).ok()?, archforge_core::name_of(store, o)))).collect::<Vec<_>>(),
                n.opened,
            )
        }
        _ => return,
    };
    // The name.
    ux::text(paint, at(20.0, 76.0), &tr("Name"), types::CAPTION, p.text_secondary, Some(inner_w));
    let field = Rect::from_min_size(at(20.0, 93.0), vec2(inner_w, 28.0));
    paint.rect_filled(field, 6.0, p.field);
    widgets::record_visible(ui, &mut shell.registry, "workspace.new-diagram.name", "field", field);
    let inner = crate::panels::field_inner(ui, field);
    let Some(edit) = shell.diagrams.new.as_mut() else { return };
    let text_edit = egui::TextEdit::singleline(&mut edit.name)
        .id(widgets::egui_id("workspace.new-diagram.name"))
        .frame(egui::Frame::NONE)
        .margin(egui::Margin::ZERO)
        .font(types::SMALL.font())
        .text_color(p.text)
        .hint_text(egui::RichText::new(tr("Diagram name")).color(p.text_muted).font(types::SMALL.font()))
        .char_limit(MAX_NAME)
        .desired_width(inner.width());
    let out = text_edit.show(&mut ui.new_child(egui::UiBuilder::new().max_rect(inner)));
    let _ = (*out.response).clone().accessible_name(tr("Name"));
    if taken_focus {
        out.response.request_focus();
        edit.opened = false;
    }
    if refused.is_some() {
        paint.rect_stroke(field, 6.0, Stroke::new(1.0, p.amber), StrokeKind::Inside);
    } else if out.response.has_focus() {
        paint.rect_stroke(field, 6.0, Stroke::new(1.0, p.focus), StrokeKind::Inside);
    }
    let enter = out.response.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
    // What it is of, the whole width.
    let new_word = shell.session.as_ref().map(|s| kind.new_owner_in(s.doc())).unwrap_or_else(|| kind.new_owner());
    let schema_word = match shell.diagrams.new.as_ref().and_then(|e| e.schema) {
        Some(id) => schemas.iter().find(|(s, _)| *s == id).map(|(_, n)| n.clone()).unwrap_or_else(|| new_word.clone()),
        None => new_word.clone(),
    };
    let schema_field =
        select(shell, ui, p, at(20.0, 133.0), inner_w, "workspace.new-diagram.schema", &kind.owner_word(), &schema_word, Field::Schema);
    if let Some(refused) = &refused {
        let r = Rect::from_min_size(at(20.0, 190.0), vec2(inner_w, 24.0));
        paint.rect_filled(r, 6.0, p.amber_tint);
        ux::text_mid(paint, r.left() + 10.0, r.center().y, refused, types::CAPTION, p.amber, Some((inner_w - 20.0).max(0.0)));
    }
    // Back at the left; Cancel and Create at the right.
    let back = Rect::from_min_size(pos2(dialog.left() + 20.0, dialog.bottom() - 48.0), vec2(70.0, 28.0));
    paint.rect_filled(back, 6.0, p.field);
    ux::icon_mid(paint, pos2(back.left() + 16.0, back.center().y), &icons::CHEVRON_LEFT, p.text);
    ux::text_mid(paint, back.left() + 28.0, back.center().y, &tr("Back"), types::SMALL, p.text, Some(38.0));
    let back_resp = control(shell, ui, "workspace.new-diagram.back", "button", back, true);
    if back_resp.has_focus() {
        paint.rect_stroke(back.expand(2.0), 8.0, Stroke::new(2.0, p.focus), StrokeKind::Outside);
    }
    let clicked = buttons(
        shell,
        ui,
        p,
        paint,
        dialog,
        &[("workspace.new-diagram.create", tr("Create"), true), ("workspace.new-diagram.cancel", tr("Cancel"), false)],
    );
    // The open select's menu, over the rest.
    let open = shell.diagrams.new.as_ref().and_then(|e| e.menu);
    if open == Some(Field::Schema) {
        let mut items: Vec<String> = schemas.iter().map(|(_, n)| n.clone()).collect();
        items.push(new_word.clone());
        if let Some(i) = menu(shell, ui, p, schema_field, "workspace.new-diagram.schema", &items)
            && let Some(edit) = shell.diagrams.new.as_mut()
        {
            edit.schema = schemas.get(i).map(|(id, _)| *id);
            edit.refused = None;
        }
    }
    let escape = open.is_none() && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
    if back_resp.clicked() {
        if let Some(edit) = shell.diagrams.new.as_mut() {
            edit.step = Step::Kind;
            edit.menu = None;
            edit.refused = None;
        }
    } else if escape || clicked == Some(1) {
        shell.diagrams.new = None;
    } else if clicked == Some(0) || (enter && open.is_none()) {
        create(shell);
    }
}

/// A filled select showing `value`; a click opens or closes its menu.
fn select(shell: &mut Shell, ui: &egui::Ui, p: &Palette, at: egui::Pos2, width: f32, id: &str, label: &str, value: &str, which: Field) -> Rect {
    let paint = ui.painter();
    ux::text(paint, at, label, types::CAPTION, p.text_secondary, Some(width.max(0.0)));
    let field = Rect::from_min_size(at + vec2(0.0, 17.0), vec2(width.max(0.0), 28.0));
    paint.rect_filled(field, 6.0, p.field);
    ux::text_mid(paint, field.left() + 8.0, field.center().y, value, types::SMALL, p.text, Some((width - 36.0).max(0.0)));
    ux::icon_mid(paint, pos2(field.right() - 14.0, field.center().y), &icons::CHEVRON_DOWN, p.text_secondary);
    if control(shell, ui, id, "select", field, true).clicked()
        && let Some(edit) = shell.diagrams.new.as_mut()
    {
        edit.menu = if edit.menu == Some(which) { None } else { Some(which) };
    }
    field
}

/// A menu under `anchor`; gives the index picked, closing on a pick or a click elsewhere.
fn menu(shell: &mut Shell, ui: &egui::Ui, p: &Palette, anchor: Rect, id: &str, items: &[String]) -> Option<usize> {
    let height = 8.0 + items.len() as f32 * 30.0;
    let r = ux::drop_rect(ui, anchor, anchor.left(), vec2(anchor.width(), height));
    // Over the dialog's fields below it, on a layer of its own.
    ux::overlay(ui, crate::widgets::egui_id(&format!("{id}.layer")), r, |ui| menu_body(shell, ui, p, anchor, r, id, items))
}

fn menu_body(shell: &mut Shell, ui: &egui::Ui, p: &Palette, anchor: Rect, r: Rect, id: &str, items: &[String]) -> Option<usize> {
    let paint = ui.painter();
    ux::shadow(paint, r, 8.0, p.shadow);
    paint.rect_filled(r, 8.0, p.surface);
    paint.rect_stroke(r, 8.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
    let mut picked = None;
    for (i, item) in items.iter().enumerate() {
        let row = Rect::from_min_size(pos2(r.left() + 4.0, r.top() + 4.0 + i as f32 * 30.0), vec2((r.width() - 8.0).max(0.0), 30.0));
        let resp = control(shell, ui, &format!("{id}.{i}"), "menu-item", row, true);
        if resp.hovered() {
            paint.rect_filled(row, 5.0, p.page);
        }
        ux::text(paint, row.min + vec2(10.0, 7.5), item, types::SMALL, p.text, Some((row.width() - 20.0).max(0.0)));
        if resp.clicked() {
            picked = Some(i);
        }
    }
    let outside = ui.input(|i| i.pointer.any_pressed() && !i.pointer.interact_pos().is_some_and(|pos| r.contains(pos) || anchor.contains(pos)));
    if (picked.is_some() || outside || ui.input(|i| i.key_pressed(egui::Key::Escape)))
        && let Some(edit) = shell.diagrams.new.as_mut()
    {
        edit.menu = None;
    }
    picked
}
