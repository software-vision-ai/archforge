//! The right panels (slice W3 of the workspace chrome, from the design's pages 03 and 04):
//! Properties shows the selection as the ER design does — a schema's name; a table's name and
//! kind, its columns with the in-place column editor, its keys — and Skills shows the skills on
//! the selection and the ones it inherits, each opening in its tab. Every change goes through the
//! engine's commands, so undo covers it; a refusal stays with the field in the engine's words.
//! Agent and Appearance say what is true until their slices.
//!
//! The geometry is the design's: a section is 16 px in from the panel's edges with a full-width
//! line over it, a field is 28 px tall and filled, its label 11 px above, a row 36 px, an editor
//! a bordered card.

use archforge_mof::{Obj, Store, Uuid, Value};
use egui::{Rect, Stroke, StrokeKind, Vec2, pos2, vec2};
use serde_json::json;

use crate::home::control;
use crate::i18n::{fmt, tr};
use crate::notation_ui::Words;
use crate::state::Panel;
use crate::ux::{self, Palette, icons, types};
use crate::{Shell, widgets};

pub(crate) const PAD: f32 = 16.0;
pub(crate) const FIELD: f32 = 28.0;
pub(crate) const ROW: f32 = 36.0;
const MAX_NAME: usize = archforge_er::rules::MAX_NAME;
const MAX_TYPE: usize = archforge_er::rules::MAX_TYPE;

/// What the panels keep between frames: the open editors and the last refusal.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct State {
    /// A name field being typed: the element and the text so far.
    pub name: Option<NameEdit>,
    /// The column editor, open on a column or on a column about to be added.
    pub column: Option<ColumnEdit>,
    /// The select menu that is open.
    pub menu: Option<Menu>,
    /// How far the open menu's rows are scrolled, by the menu's id: a menu longer than the window
    /// has room for scrolls within it.
    pub menu_scroll: Option<(String, f32)>,
    /// The row of the open menu the arrow keys have reached, by the menu's id: Enter picks it.
    pub menu_reached: Option<(String, usize)>,
    /// A type menu's choices as built for one state of one project (its model root and its
    /// revision), each label with the value the commands take: the model is walked for them once
    /// per change, not on every frame the menu is open.
    pub type_choices: Option<TypeChoices>,
    /// The properties a property's Subsets and Redefines offer, each list built once for one state
    /// of the project and one shape of the property being edited, not on every frame (AR-126).
    pub targets: Vec<crate::uml_panel::TargetChoices>,
    /// The item of an editable select's open menu the arrow keys have reached.
    pub highlight: Option<usize>,
    /// The Appearance panel's colour picker, while it is open.
    pub picker: Option<crate::appearance::Picker>,
    /// The last refusal of a change, shown at the panel's foot until the next change.
    pub notice: Option<String>,
    /// The selection the open editors belong to: selecting anything else closes them.
    pub editors_for: Option<Uuid>,
    /// A relationship's Properties (slice E3): a role being typed, whether More is open, and the
    /// relationship whose switch asked for a key its parent cannot give.
    pub role: Option<crate::relation::RoleEdit>,
    pub more_open: bool,
    pub no_key_card: Option<Uuid>,
    /// The key editor, open on a key or on a unique constraint about to be added (slice E4).
    pub key: Option<crate::keys::KeyEdit>,
    /// A description being typed under the heading (slice S2).
    pub description: Option<DescriptionDraft>,
    /// The check and index editors (slice S3).
    pub check: Option<crate::checks::CheckEdit>,
    pub index: Option<crate::indexes::IndexEdit>,
    /// The domain or enum editor (slice S5).
    pub type_edit: Option<crate::types::TypeEdit>,
    /// The sequence or view editor (slice S7).
    pub object_edit: Option<crate::objects::ObjectEdit>,
    /// A class's attribute or operation editor.
    pub member: Option<crate::uml_panel::MemberEdit>,
    /// An association's ends' roles and multiplicities as typed.
    pub ends: Option<crate::uml_relate::EndsEdit>,
    /// The body's scroll: how far it is scrolled, how tall its content was last
    /// frame, what it was scrolled for (a new panel or selection starts at the top), the editors
    /// open last frame, whether an editor that just opened is still to be brought into view, and
    /// the body's visible rectangle.
    pub scroll: f32,
    pub content: f32,
    pub scrolled_for: Option<(Panel, Option<Uuid>)>,
    pub open_editors: Vec<Option<Option<Uuid>>>,
    pub reveal: bool,
    pub body: Option<Rect>,
}

/// What is left under the last control of a scrolled body, for its trailing text.
const SCROLL_TAIL: f32 = 48.0;

/// The editors open, by what they edit, to tell when one opens.
fn open_editors(s: &State) -> Vec<Option<Option<Uuid>>> {
    vec![
        s.column.as_ref().map(|e| e.target),
        s.key.as_ref().map(|e| e.target),
        s.check.as_ref().map(|e| e.target),
        s.index.as_ref().map(|e| e.target),
        s.type_edit.as_ref().map(|e| e.target),
        s.object_edit.as_ref().map(|e| e.target),
        s.member.as_ref().map(|e| e.target),
    ]
}

/// Brings an editor card that just opened into the scrolled body's view: its foot if it fits,
/// else its top. Editors call it with their card once drawn.
pub(crate) fn reveal(shell: &mut Shell, ui: &egui::Ui, card: Rect) {
    let Some(body) = shell.panels.body.filter(|_| shell.panels.reveal) else { return };
    shell.panels.reveal = false;
    let down = (card.bottom() + 8.0 - body.bottom()).min(card.top() - 8.0 - body.top());
    let up = body.top() + 8.0 - card.top();
    if down > 0.0 {
        shell.panels.scroll += down;
    } else if up > 0.0 {
        shell.panels.scroll -= up;
    }
    ui.ctx().request_repaint();
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NameEdit {
    pub target: Uuid,
    pub text: String,
}

/// A description being typed under the heading for the element `target`, written through
/// `element.describe` on Enter, when the keyboard leaves the field, or when the selection moves on
/// to another element ([`flush_description`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DescriptionDraft {
    pub target: Uuid,
    pub text: String,
    /// Its write was refused: it is kept, for the user to see again on `target` beside the
    /// refusal, and not written again until its text changes.
    pub refused: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnEdit {
    /// The column, or none for one being added to `entity`.
    pub target: Option<Uuid>,
    pub entity: Uuid,
    pub name: String,
    pub ty: String,
    /// The size fields as typed: a length, or a precision and a scale, empty for none.
    pub length: String,
    pub precision: String,
    pub scale: String,
    pub default: String,
    pub generation: archforge_er::Generation,
    pub expression: String,
    pub collation: String,
    pub primary_key: bool,
    pub not_null: bool,
    pub unique: bool,
    /// Whether More (the collation) is open.
    pub more: bool,
    /// The column this one follows, as "Table.column" (slice S4), until unlinked.
    pub linked: Option<String>,
    /// Whether Unlink was clicked; the unlink goes out with Apply.
    pub unlink: bool,
    /// The domain or enum type the column takes instead of a type name (slice S5).
    pub domain: Option<Uuid>,
    pub enum_type: Option<Uuid>,
    pub refused: Option<String>,
    pub opened: bool,
}

/// The column `a` follows, as "Table.column".
fn linked_label(store: &Store, a: Obj) -> Option<String> {
    let t = archforge_er::rules::linked_to(store, a)?;
    let table = archforge_er::rules::entity_of(store, t).map(|e| archforge_core::name_of(store, e)).unwrap_or_default();
    Some(format!("{table}.{}", archforge_core::name_of(store, t)))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Menu {
    /// Home's order of the recent projects, a recent project's menu and a pinned folder's.
    RecentSort,
    Recent(usize),
    Pinned(usize),
    Kind,
    /// A relationship's foreign key (slice E3): what it points to, one key pair's child column,
    /// the two referential actions, deferrability and match.
    Points,
    Pair(usize),
    OnDelete,
    OnUpdate,
    Deferrable,
    Match,
    /// The key editor's Add a column select (slice E4).
    KeyColumn,
    /// The column editor's Generation select (slice S2).
    Generation,
    /// The index editor's Add a column select, and a column row's nulls order (slice S3).
    IndexColumn,
    IndexNulls(usize),
    /// Which kind of type the Types section adds, and the column editor's type menu (slice S5).
    TypeKind,
    ColumnType,
    /// The Subtypes section's table menu, discriminator and mapping (slice S6).
    SubtypeAdd,
    Discriminator,
    Mapping,
    /// The sequence editor's Owned by select (slice S7).
    OwnedBy,
    /// A class member editor's visibility, its type or return type, and a parameter's type
    ///.
    MemberVisibility,
    MemberType,
    ParamType(usize),
    /// An attribute editor's default literal (for an enumeration's type), Subsets and Redefines
    /// (AR-126).
    MemberDefault,
    MemberSubsets,
    MemberRedefines,
    /// The type menu of a member typed in place on its class's card.
    InlineType,
    /// An association end's aggregation and its multiplicity's presets.
    EndAggregation(usize),
    EndMultiplicity(usize),
    /// An association end's default literal (for an enumeration's type), Subsets and Redefines
    /// (AR-126).
    EndDefault(usize),
    EndSubsets(usize),
    EndRedefines(usize),
    /// A class's package.
    ClassPackage,
    /// The package a package sits in (AR-102).
    PackageParent,
}

/// The selection the panels describe.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Subject {
    pub obj: Option<Obj>,
    pub id: Option<Uuid>,
    pub class: Option<archforge_mof::ClassId>,
    pub name: String,
    /// The kind line under the heading and the word for the selection.
    pub(crate) kind: String,
    pub(crate) word: String,
    /// "This table": the selection itself, as the applies-to choice and the own skill rows say it.
    pub(crate) this: String,
    /// "On this table · {n}": the title of the own skills section.
    pub(crate) on_this: String,
}

fn subject(shell: &Shell, project: &str) -> Subject {
    let Some(s) = shell.session.as_ref() else {
        return Subject::root(project);
    };
    let store = s.doc();
    match s.view().selection.first().copied().filter(|o| store.is_alive(*o)) {
        Some(o) => {
            let class = store.class_of(o).ok();
            let words = match class {
                Some(c) if c == archforge_core::SKILL => Words {
                    kind: tr("Skill · model"),
                    word: tr("Skill"),
                    this: tr("This skill"),
                    on_this: tr("On this skill · {n}"),
                    name: archforge_core::skills::name_of(store, o),
                },
                Some(c) if c == archforge_diagram::DIAGRAM => {
                    let schema = crate::diagrams::schema_of(store, o).map(|s| archforge_core::name_of(store, s)).unwrap_or_default();
                    Words {
                        kind: fmt(&tr("Diagram · of {schema}"), &[("schema", &schema)]),
                        word: tr("Diagram"),
                        this: tr("This diagram"),
                        on_this: tr("On this diagram · {n}"),
                        name: crate::diagrams::diagram_name(store, o),
                    }
                }
                _ => crate::notation_ui::of(shell.notation_uis, store, o).and_then(|ui| ui.words(store, o)).unwrap_or_else(|| Words {
                    kind: tr("Element · model"),
                    word: tr("Element"),
                    this: tr("This element"),
                    on_this: tr("On this element · {n}"),
                    name: archforge_core::name_of(store, o),
                }),
            };
            Subject::of(o, store.uuid(o).ok(), class, words)
        }
        None => Subject::root(project),
    }
}

impl Subject {
    /// The subject `o` of `class`, as its words name it.
    pub(crate) fn of(o: Obj, id: impl Into<Option<Uuid>>, class: impl Into<Option<archforge_mof::ClassId>>, words: Words) -> Subject {
        let Words { kind, word, this, on_this, name } = words;
        Subject { obj: Some(o), id: id.into(), class: class.into(), name, kind, word, this, on_this }
    }
    /// The model root, which the project's name names.
    fn root(project: &str) -> Subject {
        Subject {
            obj: None,
            id: None,
            class: None,
            name: project.to_string(),
            kind: tr("Model · the project's root"),
            word: tr("Model"),
            this: tr("This model"),
            on_this: tr("On this model · {n}"),
        }
    }
}

pub(crate) fn run(shell: &mut Shell, id: &str, params: serde_json::Value) -> Result<serde_json::Value, String> {
    let Some(session) = shell.session.as_mut() else { return Err(tr("The model could not be opened")) };
    session.execute(id, &params).map(|o| o.value).map_err(|e| e.to_string())
}

fn string_of(store: &Store, o: Obj, prop: archforge_mof::PropId) -> String {
    store.get(o, prop).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default()
}

fn children(store: &Store, o: Obj, prop: archforge_mof::PropId) -> Vec<Obj> {
    store.list(o, prop).unwrap_or_default().iter().filter_map(Value::as_ref).filter(|c| store.is_alive(*c)).collect()
}

/// Drops editors whose elements are gone.
pub fn retain_valid(state: &mut State, store: &Store) {
    if state.name.as_ref().is_some_and(|n| store.by_uuid(n.target).is_none()) {
        state.name = None;
    }
    if state.column.as_ref().is_some_and(|c| store.by_uuid(c.entity).is_none() || c.target.is_some_and(|t| store.by_uuid(t).is_none())) {
        state.column = None;
    }
    crate::relation::retain_valid(state, store);
    if state.key.as_ref().is_some_and(|k| store.by_uuid(k.entity).is_none() || k.target.is_some_and(|t| store.by_uuid(t).is_none())) {
        state.key = None;
    }
    if state.description.as_ref().is_some_and(|d| store.by_uuid(d.target).is_none()) {
        state.description = None;
    }
    if state.check.as_ref().is_some_and(|c| store.by_uuid(c.entity).is_none() || c.target.is_some_and(|t| store.by_uuid(t).is_none())) {
        state.check = None;
    }
    if state.index.as_ref().is_some_and(|i| store.by_uuid(i.entity).is_none() || i.target.is_some_and(|t| store.by_uuid(t).is_none())) {
        state.index = None;
    }
    if state.type_edit.as_ref().is_some_and(|t| store.by_uuid(t.schema).is_none() || t.target.is_some_and(|t| store.by_uuid(t).is_none())) {
        state.type_edit = None;
    }
    if state.object_edit.as_ref().is_some_and(|o| store.by_uuid(o.schema).is_none() || o.target.is_some_and(|t| store.by_uuid(t).is_none())) {
        state.object_edit = None;
    }
    crate::uml_panel::retain_valid(&mut state.member, store);
    if state.ends.as_ref().is_some_and(|e| store.by_uuid(e.association).is_none()) {
        state.ends = None;
    }
}

// Shared drawing.

/// A section's full-width top line and its title row with an optional `+`; gives the y under it,
/// or none when the `+` was clicked this frame.
pub(crate) fn section(shell: &mut Shell, ui: &egui::Ui, p: &Palette, r: Rect, y: f32, title: &str, add: Option<(&str, &str)>) -> Option<f32> {
    let paint = ui.painter();
    paint.rect_filled(Rect::from_min_size(pos2(r.left(), y), vec2(r.width(), 1.0)), 0.0, p.border);
    ux::text(paint, pos2(r.left() + PAD, y + 17.0), title, types::UI, p.text, Some((r.width() - 2.0 * PAD - 40.0).max(0.0)));
    if let Some((id, hint)) = add {
        let b = Rect::from_min_size(pos2(r.right() - PAD - 28.0, y + 11.0), vec2(28.0, 28.0));
        ux::icon_in(paint, Rect::from_center_size(b.center(), Vec2::splat(16.0)), &icons::PLUS, p.text_secondary);
        if control(shell, ui, id, "button", b, true).on_hover_text(tr(hint)).clicked() {
            return None;
        }
    }
    Some(y + 50.0)
}

/// A label over a filled text field that commits on Enter or when it loses the keyboard; gives
/// the text as typed and whether it was committed this frame.
pub(crate) fn text_field(
    shell: &mut Shell,
    ui: &mut egui::Ui,
    p: &Palette,
    at: egui::Pos2,
    width: f32,
    id: &str,
    label: &str,
    text: &mut String,
    limit: usize,
    take_focus: bool,
    commit_on_blur: bool,
) -> bool {
    let paint = ui.painter().clone();
    ux::text(&paint, at, label, types::CAPTION, p.text_secondary, Some(width.max(0.0)));
    let field = Rect::from_min_size(at + vec2(0.0, 17.0), vec2(width.max(0.0), FIELD));
    paint.rect_filled(field, 6.0, p.field);
    widgets::record_visible(ui, &mut shell.registry, id, "field", field);
    let inner = field_inner(ui, field);
    let eid = widgets::egui_id(id);
    let edit = egui::TextEdit::singleline(text)
        .id(eid)
        .frame(egui::Frame::NONE)
        .margin(egui::Margin::ZERO)
        .font(types::SMALL.font())
        .text_color(p.text)
        .char_limit(limit)
        .desired_width(inner.width());
    let out = edit.show(&mut ui.new_child(egui::UiBuilder::new().max_rect(inner)));
    let _ = (*out.response).clone().accessible_name(label);
    if take_focus {
        out.response.request_focus();
    }
    if out.response.has_focus() {
        paint.rect_stroke(field, 6.0, Stroke::new(1.0, p.focus), StrokeKind::Inside);
    }
    // A one-line field gives up the keyboard on Enter in the same frame.
    let enter = (out.response.has_focus() || out.response.lost_focus()) && ui.input(|i| i.key_pressed(egui::Key::Enter));
    enter || (commit_on_blur && out.response.lost_focus())
}

/// Where a field's one line of text goes: 8 points in from the sides, its row centred on the
/// field's middle line, as a select's value is.
pub(crate) fn field_inner(ui: &egui::Ui, field: Rect) -> Rect {
    let row = ui.fonts_mut(|f| f.row_height(&types::SMALL.font()));
    Rect::from_min_max(
        pos2(field.left() + 8.0, field.center().y - row / 2.0),
        pos2((field.right() - 8.0).max(field.left() + 8.0), field.center().y + row / 2.0),
    )
}

/// A filled select showing `value`; a click opens or closes `menu`.
pub(crate) fn select_field(
    shell: &mut Shell,
    ui: &egui::Ui,
    p: &Palette,
    at: egui::Pos2,
    width: f32,
    id: &str,
    label: &str,
    value: &str,
    menu: Menu,
) -> Rect {
    let paint = ui.painter();
    ux::text(paint, at, label, types::CAPTION, p.text_secondary, Some(width.max(0.0)));
    let field = Rect::from_min_size(at + vec2(0.0, 17.0), vec2(width.max(0.0), FIELD));
    paint.rect_filled(field, 6.0, p.field);
    ux::text_mid(paint, field.left() + 8.0, field.center().y, value, types::SMALL, p.text, Some((width - 36.0).max(0.0)));
    ux::icon_mid(paint, pos2(field.right() - 14.0, field.center().y), &icons::CHEVRON_DOWN, p.text_secondary);
    // Its label and what it shows, for assistive technology.
    if control(shell, ui, id, "select", field, true).accessible_name(format!("{label}: {value}")).clicked() {
        shell.panels.menu = if shell.panels.menu == Some(menu) { None } else { Some(menu) };
    }
    field
}

/// An editable select: what it shows and offers.
pub(crate) struct Combo<'a> {
    pub id: &'a str,
    pub label: &'a str,
    /// Its accessible name, which tells it from its namesakes.
    pub name: &'a str,
    /// Shown, muted, while the text is empty; never a value.
    pub placeholder: &'a str,
    pub options: &'a [&'a str],
    /// The option the model's value is, ticked in the menu.
    pub current: Option<usize>,
    /// The chevron's tooltip and its accessible name, which names the field too; the line under
    /// the options, and the last row, which empties it.
    pub pick: &'a str,
    pub pick_name: &'a str,
    pub hint: &'a str,
    pub clear: &'a str,
    /// Why the text was refused: ringed in the danger colour, and told to assistive technology;
    /// the caller draws it under the field.
    pub refused: Option<&'a str>,
    pub menu: Menu,
    pub limit: usize,
}

/// What an editable select asked for this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ComboOut {
    Idle,
    /// The text as typed is to be written: Enter, or the field left.
    Commit,
    /// A row of the menu was picked: an option's index, or the options' count for Clear.
    Pick(usize),
    /// Escape with the menu closed: the value as last written comes back.
    Cancel,
}

/// The editable select's menu width, its widest, and its height for `n` options: the options, the
/// hint under them and Clear.
const COMBO_W: f32 = 160.0;
const COMBO_MAX_W: f32 = 264.0;
fn combo_height(n: usize) -> f32 {
    n as f32 * 30.0 + 78.0
}

/// A label over a filled field that takes typing, with a chevron that opens its options above or
/// under it. The keys go to the menu while it is open (the arrows move, Enter picks, Escape
/// closes it) and to the field otherwise (Alt+Down opens the menu, Enter commits, Escape cancels).
/// A refused field is ringed in the danger colour and its reason is the field's description;
/// the caller draws the reason under it. Leaving the field closes its menu.
pub(crate) fn editable_select(
    shell: &mut Shell,
    ui: &mut egui::Ui,
    p: &Palette,
    at: egui::Pos2,
    width: f32,
    text: &mut String,
    c: &Combo,
) -> ComboOut {
    let paint = ui.painter().clone();
    ux::text(&paint, at, c.label, types::CAPTION, p.text_secondary, Some(width.max(0.0)));
    let field = Rect::from_min_size(at + vec2(0.0, 17.0), vec2(width.max(0.0), FIELD));
    paint.rect_filled(field, 6.0, p.field);
    widgets::record_visible(ui, &mut shell.registry, c.id, "field", field);
    let eid = widgets::egui_id(c.id);
    let rows = c.options.len() + 1;
    // The menu is as wide as the design's, or wider when its hint needs it (a longer language).
    let hint_w = ux::galley(&paint, c.hint, types::CAPTION, p.text_secondary, None).size().x;
    let list_w = (hint_w + 24.0).clamp(COMBO_W, COMBO_MAX_W);
    let list = ux::drop_rect(ui, field, field.right() - list_w, vec2(list_w, combo_height(c.options.len())));
    let was_open = shell.panels.menu == Some(c.menu);
    let focused = ui.memory(|m| m.has_focus(eid));
    let mut out = ComboOut::Idle;
    // The keys first, before the text field takes them.
    if was_open {
        if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown)) {
            shell.panels.highlight = Some(shell.panels.highlight.map_or(0, |h| (h + 1).min(rows - 1)));
        }
        if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp)) {
            shell.panels.highlight = Some(shell.panels.highlight.map_or(0, |h| h.saturating_sub(1)));
        }
        if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter)) {
            out = shell.panels.highlight.map_or(ComboOut::Commit, ComboOut::Pick);
            shell.panels.menu = None;
        }
        if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            shell.panels.menu = None;
        }
    } else if focused {
        if ui.input_mut(|i| i.consume_key(egui::Modifiers::ALT, egui::Key::ArrowDown)) {
            shell.panels.menu = Some(c.menu);
            shell.panels.highlight = c.current;
        } else if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            out = ComboOut::Cancel;
            ui.memory_mut(|m| m.surrender_focus(eid));
        }
    }
    let chevron = Rect::from_min_size(pos2(field.right() - 28.0, field.top()), vec2(28.0, FIELD));
    let inner = field_inner(ui, field);
    let inner = Rect::from_min_max(inner.min, pos2((chevron.left() - 2.0).max(inner.left()), inner.bottom()));
    // Escape stays with the field, so the menu or the edit can take it.
    let filter = egui::EventFilter { escape: true, horizontal_arrows: true, vertical_arrows: true, tab: false };
    let edit = egui::TextEdit::singleline(text)
        .id(eid)
        .frame(egui::Frame::NONE)
        .margin(egui::Margin::ZERO)
        .font(types::SMALL.font())
        .text_color(p.text)
        .char_limit(c.limit)
        .hint_text(egui::RichText::new(c.placeholder).color(p.text_muted).font(types::SMALL.font()))
        .event_filter(filter)
        .desired_width(inner.width());
    let resp = edit.show(&mut ui.new_child(egui::UiBuilder::new().max_rect(inner))).response;
    // Typing takes over from the row the keys reached: Enter then writes the typing.
    if resp.changed() && shell.panels.menu == Some(c.menu) {
        shell.panels.highlight = None;
    }
    // The row the keys reached is the field's active descendant, so a screen reader says which
    // one Enter picks as the arrows move.
    let open = shell.panels.menu == Some(c.menu);
    let reached = shell.panels.highlight.filter(|_| open).map(|h| {
        let row = if h < c.options.len() { format!("{}.menu.{h}", c.id) } else { format!("{}.menu.clear", c.id) };
        widgets::egui_id(&row).accesskit_id()
    });
    ui.ctx().accesskit_node_builder(resp.id, |node| {
        node.set_label(c.name);
        node.set_expanded(open);
        if let Some(row) = reached {
            node.set_active_descendant(row);
        }
        if let Some(why) = c.refused {
            node.set_description(why);
            node.set_invalid(egui::accesskit::Invalid::True);
        }
    });
    // Leaving the field writes what was typed, unless the press that left it is on the field's own
    // chevron or its open menu: then the typing stays a draft and the field takes the keyboard back,
    // so a pick replaces the draft and Escape still brings the written value back.
    // The field gives the keyboard up on the press or on the click, so either edge counts.
    let pressed_in =
        |r: Rect| ui.input(|i| (i.pointer.any_pressed() || i.pointer.any_released()) && i.pointer.interact_pos().is_some_and(|o| r.contains(o)));
    let own = pressed_in(chevron) || (was_open && pressed_in(list));
    if out == ComboOut::Idle && resp.lost_focus() {
        if own {
            ui.memory_mut(|m| m.request_focus(eid));
        } else {
            // Left another way (a click elsewhere, Tab): its menu goes too, so its keys stop.
            out = ComboOut::Commit;
            if shell.panels.menu == Some(c.menu) {
                shell.panels.menu = None;
            }
        }
    }
    let ring = if c.refused.is_some() { Some(p.danger) } else { (resp.has_focus() || shell.panels.menu == Some(c.menu)).then_some(p.focus) };
    if let Some(ink) = ring {
        paint.rect_stroke(field, 6.0, Stroke::new(1.0, ink), StrokeKind::Inside);
    }
    ux::icon_mid(&paint, pos2(field.right() - 16.0, field.center().y), &icons::CHEVRON_DOWN, p.text_secondary);
    if control(shell, ui, &format!("{}.menu", c.id), "button", chevron, true).on_hover_text(c.pick).accessible_name(c.pick_name).clicked() {
        if shell.panels.menu == Some(c.menu) {
            shell.panels.menu = None;
        } else {
            shell.panels.menu = Some(c.menu);
            shell.panels.highlight = c.current;
            // The field keeps the keyboard, so the keys reach the menu and not the canvas.
            ui.memory_mut(|m| m.request_focus(eid));
        }
    }
    if shell.panels.menu == Some(c.menu)
        && let Some(k) = ux::overlay(ui, widgets::egui_id(&format!("{}.menu.layer", c.id)), list, |ui| combo_menu(shell, ui, p, field, list, c))
    {
        out = ComboOut::Pick(k);
    }
    out
}

/// The editable select's open menu over `r`: its options, the current one ticked, the hint, then
/// Clear; gives the row picked and closes on a pick or a press elsewhere.
fn combo_menu(shell: &mut Shell, ui: &egui::Ui, p: &Palette, anchor: Rect, r: Rect, c: &Combo) -> Option<usize> {
    let paint = ui.painter();
    ux::shadow(paint, r, 8.0, p.shadow);
    paint.rect_filled(r, 8.0, p.surface);
    paint.rect_stroke(r, 8.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
    let n = c.options.len();
    let rows = c.options.iter().copied().enumerate().map(|(i, o)| (i, o.to_string(), 4.0 + i as f32 * 30.0, format!("{}.menu.{i}", c.id)));
    let clear = (n, c.clear.to_string(), 4.0 + n as f32 * 30.0 + 40.0, format!("{}.menu.clear", c.id));
    let mut picked = None;
    for (i, word, top, id) in rows.chain([clear]) {
        let row = Rect::from_min_size(pos2(r.left() + 4.0, r.top() + top), vec2(r.width() - 8.0, 30.0));
        let resp = control(shell, ui, &id, "menu-item", row, true).accessible_name(word.as_str());
        let on = c.current == Some(i);
        ui.ctx().accesskit_node_builder(resp.id, |node| node.set_selected(on));
        if on {
            paint.rect_filled(row, 5.0, p.accent_tint);
        } else if resp.hovered() || shell.panels.highlight == Some(i) {
            paint.rect_filled(row, 5.0, p.page);
        }
        if on && shell.panels.highlight == Some(i) {
            paint.rect_stroke(row, 5.0, Stroke::new(1.0, p.focus), StrokeKind::Inside);
        }
        ux::text(paint, row.min + vec2(10.0, 7.5), &word, types::SMALL, p.text, Some((row.width() - 40.0).max(0.0)));
        if on {
            ux::icon_in(paint, Rect::from_center_size(pos2(row.right() - 16.0, row.center().y), Vec2::splat(14.0)), &icons::TICK, p.accent);
        }
        if resp.clicked() {
            picked = Some(i);
        }
    }
    ux::text(paint, pos2(r.left() + 12.0, r.top() + 4.0 + n as f32 * 30.0 + 10.0), c.hint, types::CAPTION, p.text_secondary, Some(r.width() - 24.0));
    let outside = ui.input(|i| i.pointer.any_pressed() && !i.pointer.interact_pos().is_some_and(|pos| r.contains(pos) || anchor.contains(pos)));
    if picked.is_some() || outside {
        shell.panels.menu = None;
    }
    picked
}

/// A menu under `anchor` with `items`; gives the index picked, closing the menu on a pick or a
/// click elsewhere.
pub(crate) fn menu(shell: &mut Shell, ui: &egui::Ui, p: &Palette, anchor: Rect, id: &str, items: &[String]) -> Option<usize> {
    menu_noted(shell, ui, p, anchor, id, items, &[])
}

/// [`menu`] with a muted note after an item's text where `notes` has one (empty for none), such as
/// the package that tells two types of one name apart; the note joins the row's accessible name.
pub(crate) fn menu_noted(shell: &mut Shell, ui: &egui::Ui, p: &Palette, anchor: Rect, id: &str, items: &[String], notes: &[String]) -> Option<usize> {
    let full = 8.0 + items.len() as f32 * MENU_ROW;
    // Under the field, else above it; longer than either side has room for, it takes the side with
    // more and its rows scroll.
    let window = ui.ctx().content_rect();
    let (below, above) = (window.bottom() - 8.0 - (anchor.bottom() + 4.0), anchor.top() - 4.0 - (window.top() + 8.0));
    let width = noted_width(ui, p, items, notes).max(anchor.width()).min((window.width() - 16.0).max(anchor.width()));
    // As wide as its noted rows need, kept inside the window.
    let left = anchor.left().min(window.right() - 8.0 - width).max(window.left() + 8.0);
    let r = if full <= below.max(0.0) || (full > above && below >= above) {
        Rect::from_min_size(pos2(left, anchor.bottom() + 4.0), vec2(width, full.min(below.max(8.0 + MENU_ROW))))
    } else {
        let height = full.min(above.max(8.0 + MENU_ROW));
        Rect::from_min_size(pos2(left, anchor.top() - 4.0 - height), vec2(width, height))
    };
    ux::overlay(ui, crate::widgets::egui_id(&format!("{id}.layer")), r, |ui| menu_body(shell, ui, p, anchor, r, id, items, notes))
}

/// A menu row's height.
const MENU_ROW: f32 = 30.0;
/// The most noted rows a menu measures for its width: a note marks a name several types share,
/// which few do, and the rest fit or are cut as any row is.
const MAX_NOTED: usize = 64;

/// How wide a menu is to show its noted rows whole, each item, a gap and its muted note inside the
/// row's insets; nothing without a note. Only the first [`MAX_NOTED`] noted rows are measured.
fn noted_width(ui: &egui::Ui, p: &Palette, items: &[String], notes: &[String]) -> f32 {
    let paint = ui.painter();
    let measure = |t: &str| ux::galley(paint, t, types::SMALL, p.text, None).size().x;
    items
        .iter()
        .zip(notes)
        .filter(|(_, n)| !n.is_empty())
        .take(MAX_NOTED)
        .map(|(item, note)| 8.0 + 10.0 + measure(item) + 6.0 + measure(note) + 10.0)
        .fold(0.0, f32::max)
}

/// A type menu's choices kept between frames: whose and which state of it they were built for, in
/// which interface language (the labels are translated), their labels, the values the commands
/// take and the muted notes that tell same-named ones apart, in the same order.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TypeChoices {
    pub project: Option<Uuid>,
    pub revision: u64,
    pub language: &'static str,
    pub labels: Vec<String>,
    pub values: Vec<String>,
    pub notes: Vec<String>,
}

/// The open menu's frame inside `r`: its rows, those in sight laid out, each with its note muted
/// after its text where `notes` has one; the wheel and the arrow keys scroll, Enter or a click
/// picks, Escape or a press elsewhere closes.
#[allow(clippy::too_many_arguments, reason = "a menu is drawn for its anchor, frame, id, items and notes")]
fn menu_body(shell: &mut Shell, ui: &egui::Ui, p: &Palette, anchor: Rect, r: Rect, id: &str, items: &[String], notes: &[String]) -> Option<usize> {
    let paint = ui.painter();
    ux::shadow(paint, r, 8.0, p.shadow);
    paint.rect_filled(r, 8.0, p.surface);
    paint.rect_stroke(r, 8.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
    // Its rows scroll with the wheel over it, as far as the last.
    let most = (8.0 + items.len() as f32 * MENU_ROW - r.height()).max(0.0);
    let mut offset = shell.panels.menu_scroll.as_ref().filter(|(of, _)| of == id).map_or(0.0, |(_, o)| *o);
    if ui.input(|i| i.pointer.hover_pos().is_some_and(|at| r.contains(at))) {
        offset -= ui.input(|i| i.smooth_scroll_delta.y);
    }
    // The arrow keys reach a row, scrolled into sight; Enter picks it.
    let mut reached = shell.panels.menu_reached.as_ref().filter(|(of, _)| of == id).map(|(_, i)| *i);
    let (down, up, enter) = ui.input_mut(|i| {
        (
            i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown),
            i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp),
            reached.is_some() && i.consume_key(egui::Modifiers::NONE, egui::Key::Enter),
        )
    });
    if !items.is_empty() && (down || up) {
        let next = match reached {
            None => 0,
            Some(i) if down => (i + 1).min(items.len() - 1),
            Some(i) => i.saturating_sub(1),
        };
        reached = Some(next);
        let (top, bottom) = (4.0 + next as f32 * MENU_ROW, 4.0 + (next + 1) as f32 * MENU_ROW);
        if top < offset + 4.0 {
            offset = top - 4.0;
        } else if bottom > offset + r.height() - 4.0 {
            offset = bottom - r.height() + 4.0;
        }
    }
    shell.panels.menu_reached = reached.map(|i| (id.to_string(), i));
    let offset = offset.clamp(0.0, most);
    shell.panels.menu_scroll = Some((id.to_string(), offset));
    let rows = r.shrink2(vec2(0.0, 4.0));
    let paint = paint.with_clip_rect(rows);
    let mut picked = None;
    // Only the rows in sight are laid out, however many the menu holds.
    let first = ((offset / MENU_ROW).floor().max(0.0) as usize).min(items.len());
    let in_sight = ((r.height() / MENU_ROW).ceil().max(0.0) as usize).saturating_add(1);
    for (i, item) in items.iter().enumerate().skip(first).take(in_sight) {
        let row = Rect::from_min_size(pos2(r.left() + 4.0, r.top() + 4.0 + i as f32 * MENU_ROW - offset), vec2(r.width() - 8.0, MENU_ROW));
        if !rows.intersects(row) {
            continue;
        }
        let note = notes.get(i).filter(|n| !n.is_empty());
        let resp = control(shell, ui, &format!("{id}.{i}"), "menu-item", row.intersect(rows), true);
        let resp = match note {
            Some(n) => resp.accessible_name(fmt(&tr("{name} · {where}"), &[("name", item), ("where", n)])),
            None => resp,
        };
        if resp.hovered() || reached == Some(i) {
            paint.rect_filled(row, 5.0, p.page);
        }
        let room = (row.width() - 20.0).max(0.0);
        let drawn = ux::text(&paint, row.min + vec2(10.0, 7.5), item, types::SMALL, p.text, Some(room));
        if let Some(n) = note {
            let left = drawn.right() + 6.0;
            let rest = (row.left() + 10.0 + room - left).max(0.0);
            ux::text(&paint, pos2(left, row.top() + 7.5), n, types::SMALL, p.text_muted, Some(rest));
        }
        if resp.clicked() || (enter && reached == Some(i)) {
            picked = Some(i);
        }
    }
    let outside = ui.input(|i| i.pointer.any_pressed() && !i.pointer.interact_pos().is_some_and(|pos| r.contains(pos) || anchor.contains(pos)));
    if picked.is_some() || outside || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        shell.panels.menu = None;
        shell.panels.menu_reached = None;
    }
    picked
}

/// A check box with its label; a click flips `on`.
pub(crate) fn check(shell: &mut Shell, ui: &egui::Ui, p: &Palette, at: egui::Pos2, id: &str, label: &str, on: &mut bool) -> f32 {
    let paint = ui.painter();
    let b = Rect::from_min_size(at, Vec2::splat(16.0));
    paint.rect_filled(b, 4.0, if *on { p.accent_fill } else { p.surface });
    paint.rect_stroke(b, 4.0, Stroke::new(1.0, if *on { p.accent_fill } else { p.border }), StrokeKind::Inside);
    if *on {
        ux::icon_in(paint, b.shrink(3.0), &icons::TICK, p.on_accent);
    }
    let g = ux::text(paint, at + vec2(24.0, -1.0), label, types::SMALL, p.text, None);
    let hit = Rect::from_min_max(b.min, pos2(g.right(), b.bottom()));
    if control(shell, ui, id, "check", hit, true).accessible_name(label).clicked() {
        *on = !*on;
    }
    g.right() + 20.0
}

/// An icon button of 28 px: quiet, bordered or primary.
pub(crate) fn icon_button(
    shell: &mut Shell,
    ui: &egui::Ui,
    p: &Palette,
    at: egui::Pos2,
    id: &str,
    icon: &ux::Icon,
    ink: egui::Color32,
    fill: Option<egui::Color32>,
    hint: &str,
) -> bool {
    let paint = ui.painter();
    let b = Rect::from_min_size(at, Vec2::splat(FIELD));
    match fill {
        Some(f) => {
            paint.rect_filled(b, 6.0, f);
        }
        None => {
            paint.rect_stroke(b, 6.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
        }
    }
    ux::icon_in(paint, Rect::from_center_size(b.center(), Vec2::splat(14.0)), icon, ink);
    control(shell, ui, id, "button", b, true).on_hover_text(tr(hint)).clicked()
}

/// A quiet button of an icon and a label (the design's Button / Ghost); gives whether it was clicked.
pub(crate) fn ghost_button(shell: &mut Shell, ui: &egui::Ui, p: &Palette, at: egui::Pos2, id: &str, icon: &ux::Icon, label: &str) -> bool {
    let paint = ui.painter();
    let g = ux::galley(paint, label, types::UI, p.text, None);
    let b = Rect::from_min_size(at, vec2(6.0 + 14.0 + 7.0 + g.size().x + 8.0, 24.0));
    ux::icon_in(paint, Rect::from_min_size(b.min + vec2(6.0, 5.0), Vec2::splat(14.0)), icon, p.text);
    paint.galley(b.min + vec2(27.0, 2.5), g, p.text);
    let resp = control(shell, ui, id, "button", b, true);
    if resp.hovered() {
        paint.rect_stroke(b, 6.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
    }
    resp.clicked()
}

/// The base types the column type menu offers after the schema's own types: the names the size
/// rules know, and the common plain ones.
pub(crate) const BASE_TYPES: [&str; 12] =
    ["VARCHAR", "CHAR", "TEXT", "SMALLINT", "INTEGER", "BIGINT", "NUMERIC", "BOOLEAN", "DATE", "TIMESTAMP", "TIME", "UUID"];

// Properties.

/// Draws the Properties panel's body under the header, from `y`.
fn properties(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect, mut y: f32, subject: &Subject) {
    let paint = ui.painter().clone();
    let width = r.width() - 2.0 * PAD;
    // A notation may title the first section otherwise (a column's panel is its table's).
    let notation = shell.session.as_ref().zip(subject.obj).and_then(|(s, o)| crate::notation_ui::of(shell.notation_uis, s.doc(), o));
    let heading = shell.session.as_ref().zip(subject.obj).zip(notation).and_then(|((s, o), n)| n.heading(s.doc(), o));
    let word = heading.unwrap_or_else(|| subject.word.clone());
    y = section(shell, ui, p, r, y, &word, None).unwrap_or(y + 50.0);
    if shell.panels.editors_for != subject.id {
        shell.panels.editors_for = subject.id;
        close_editors(shell);
    }
    match subject.class {
        None => {
            // The model root: its name, which the project's name is.
            ux::text(&paint, pos2(r.left() + PAD, y), &tr("Name"), types::CAPTION, p.text_secondary, Some(width.max(0.0)));
            let field = Rect::from_min_size(pos2(r.left() + PAD, y + 17.0), vec2(width.max(0.0), FIELD));
            paint.rect_filled(field, 6.0, p.field);
            ux::text_mid(&paint, field.left() + 8.0, field.center().y, &subject.name, types::SMALL, p.text, Some((width - 16.0).max(0.0)));
            control(shell, ui, "workspace.properties.name", "button", field, false);
        }
        Some(c) if c == archforge_diagram::DIAGRAM => {
            // A diagram: its name, and the schema it is of, which does not change.
            let half = (width - 16.0) / 2.0;
            name_field(shell, ui, p, pos2(r.left() + PAD, y), half, subject, "diagram.rename", "diagram");
            // The element it is of, named by its notation's word (ER's Schema).
            let (schema, word) = shell
                .session
                .as_ref()
                .and_then(|s| {
                    let store = s.doc();
                    let of = crate::diagrams::schema_of(store, subject.obj?)?;
                    let word = crate::notation_ui::of(shell.notation_uis, store, of).and_then(|ui| ui.words(store, of)).map(|w| w.word);
                    Some((archforge_core::name_of(store, of), word))
                })
                .unwrap_or_default();
            let at = pos2(r.left() + PAD + half + 16.0, y);
            ux::text(&paint, at, &word.unwrap_or_else(|| tr("Schema")), types::CAPTION, p.text_secondary, Some(half.max(0.0)));
            let field = Rect::from_min_size(at + vec2(0.0, 17.0), vec2(half.max(0.0), FIELD));
            paint.rect_filled(field, 6.0, p.field);
            ux::text_mid(&paint, field.left() + 8.0, field.center().y, &schema, types::SMALL, p.text, Some((half - 36.0).max(0.0)));
            ux::icon_mid(&paint, pos2(field.right() - 14.0, field.center().y), &icons::CHEVRON_DOWN, p.text_muted);
            control(shell, ui, "workspace.properties.schema", "select", field, false);
            if let Some(o) = subject.obj {
                crate::keys::problems_section(shell, ui, p, r, y + 17.0 + FIELD + 16.0, o);
            }
        }
        // An element of a notation: its notation's sections.
        Some(_) => {
            if let Some(notation) = notation {
                notation.body(shell, ui, p, r, y, subject);
            }
        }
    }
}

/// The name field of a schema or table, committed through its rename command.
pub(crate) fn name_field(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, at: egui::Pos2, width: f32, subject: &Subject, cmd: &str, key: &str) {
    let Some(id) = subject.id else { return };
    let mut text = match &shell.panels.name {
        Some(n) if n.target == id => n.text.clone(),
        _ => subject.name.clone(),
    };
    let committed = text_field(shell, ui, p, at, width, "workspace.properties.name", &tr("Name"), &mut text, MAX_NAME, false, true);
    if committed {
        shell.panels.name = None;
        if text.trim() != subject.name {
            match run(shell, cmd, json!({ key: id.to_string(), "name": text.trim() })) {
                Ok(_) => shell.panels.notice = None,
                Err(e) => shell.panels.notice = Some(e),
            }
        }
    } else if text != subject.name {
        shell.panels.name = Some(NameEdit { target: id, text });
    } else if shell.panels.name.as_ref().is_some_and(|n| n.target == id) {
        shell.panels.name = None;
    }
}

/// The description of the element `obj` as its notation keeps it, or nothing.
fn description_now(shell: &Shell, obj: Obj) -> String {
    shell.session.as_ref().and_then(|s| s.view().of_element(s.doc(), obj).map(|n| n.description(s.doc(), obj))).unwrap_or_default()
}

/// Writes `text` as the description of `target` through `element.describe` when it differs from
/// what the element has, and keeps the draft until the write succeeds: done, the draft and the
/// notice clear; refused, the draft stays, marked refused, and the refusal is the notice.
fn write_description(shell: &mut Shell, target: Uuid, text: String) {
    let obj = shell.session.as_ref().and_then(|s| s.doc().by_uuid(target));
    if obj.is_none_or(|o| text.trim() == description_now(shell, o)) {
        shell.panels.description = None;
        return;
    }
    match run(shell, "element.describe", json!({ "element": target.to_string(), "text": text.trim() })) {
        Ok(_) => {
            shell.panels.description = None;
            shell.panels.notice = None;
        }
        Err(e) => {
            shell.panels.description = Some(DescriptionDraft { target, text, refused: true });
            shell.panels.notice = Some(e);
        }
    }
}

/// A description draft left behind when the selection moved on to `subject` (another element, a
/// diagram or nothing) is written to the element it was typed on, as Enter would, before the
/// panel shows the new selection: the draft is never lost and never lands on another element. A
/// draft already refused is not written again until its text changes.
pub(crate) fn flush_description(shell: &mut Shell, subject: Option<Uuid>) {
    let Some(d) = shell.panels.description.clone() else { return };
    if Some(d.target) != subject && !d.refused {
        write_description(shell, d.target, d.text);
    }
}

/// Writes the description being typed, on whichever element, as Enter would, before a save, a close
/// or a quit takes stock of what the project holds; a draft refused before is tried again, so its
/// refusal shows. Gives whether a draft is left: one whose write was refused, kept with the
/// refusal as the notice, which the project counts as unsaved work.
pub(crate) fn commit_description(shell: &mut Shell) -> bool {
    if let Some(d) = shell.panels.description.clone() {
        write_description(shell, d.target, d.text);
    }
    shell.panels.description.is_some()
}

/// Whether `panels` keep a description draft whose write was refused: a save stops on it.
pub(crate) fn refused_description(panels: &State) -> bool {
    panels.description.as_ref().is_some_and(|d| d.refused)
}

/// Whether `panels` keep a description draft not yet written, being typed or refused: unsaved work
/// the status bar, the tab, `document.inspect` and a close or a quit count.
pub(crate) fn description_pending(panels: &State) -> bool {
    panels.description.is_some()
}

/// The description under the heading: typed in place, committed through `element.describe` on Enter
/// or when the keyboard leaves it, and when the selection moves on ([`flush_description`]). It
/// shows what the element's notation keeps: an ER element's description, a UML element's
/// description Comment (AR-186).
fn description_field(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, at: egui::Pos2, width: f32, subject: &Subject) {
    let (Some(id), Some(obj)) = (subject.id, subject.obj) else { return };
    let current = description_now(shell, obj);
    let mut text = match &shell.panels.description {
        Some(d) if d.target == id => d.text.clone(),
        _ => current.clone(),
    };
    let committed = text_field(
        shell,
        ui,
        p,
        at,
        width,
        "workspace.properties.description",
        &tr("Description"),
        &mut text,
        archforge_er::rules::MAX_TEXT,
        false,
        true,
    );
    if committed {
        write_description(shell, id, text);
    } else if text != current {
        // The refused mark stays while the text does; a change makes it a draft to write again.
        let refused = shell.panels.description.as_ref().is_some_and(|d| d.target == id && d.text == text && d.refused);
        shell.panels.description = Some(DescriptionDraft { target: id, text, refused });
    } else if shell.panels.description.as_ref().is_some_and(|d| d.target == id) {
        shell.panels.description = None;
    }
}

/// One column as the panel lists it.
struct ColumnRow {
    id: Uuid,
    name: String,
    ty: String,
    primary: bool,
    unique: bool,
    foreign: bool,
    /// The column this one follows, as "Table.column".
    linked: Option<String>,
}

fn entity_rows(store: &Store, e: Obj) -> (Vec<ColumnRow>, bool) {
    let columns = children(store, e, archforge_er::entity::ATTRIBUTE)
        .into_iter()
        .filter_map(|a| {
            Some(ColumnRow {
                id: store.uuid(a).ok()?,
                name: archforge_core::name_of(store, a),
                ty: archforge_er::rules::type_display(store, a),
                primary: archforge_er::rules::in_primary_key(store, a),
                unique: archforge_er::rules::is_unique(store, a),
                foreign: store.referrers(a).iter().any(|(_, p)| *p == archforge_er::foreign_key::COLUMN),
                linked: linked_label(store, a),
            })
        })
        .collect();
    let associative = store.get(e, archforge_er::entity::KIND).ok().and_then(|v| match v {
        Value::Enum(i) => archforge_er::EntityKind::from_index(i),
        _ => None,
    }) == Some(archforge_er::EntityKind::Associative);
    (columns, associative)
}

/// A table: name and kind, its columns with the editor, its keys.
pub(crate) fn entity_body(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect, mut y: f32, subject: &Subject) {
    let paint = ui.painter().clone();
    let width = r.width() - 2.0 * PAD;
    let (Some(entity), Some(entity_id)) = (subject.obj, subject.id) else { return };
    let (columns, associative) = match shell.session.as_ref() {
        Some(s) => entity_rows(s.doc(), entity),
        None => return,
    };
    let half = ((width - 8.0) / 2.0).max(0.0);
    name_field(shell, ui, p, pos2(r.left() + PAD, y), half, subject, "er.entity.rename", "entity");
    let kind_word = if associative { tr("Mapping table") } else { tr("Regular") };
    let kind_field =
        select_field(shell, ui, p, pos2(r.left() + PAD + half + 8.0, y), half, "workspace.properties.kind", &tr("Kind"), &kind_word, Menu::Kind);
    y += 17.0 + FIELD + 16.0;
    // A pure mapping table offers to collapse back into its line (slice S4).
    let collapsible = shell.session.as_ref().is_some_and(|s| archforge_er::rules::collapse_problem(s.doc(), entity).is_none());
    if collapsible {
        y = collapse_offer(shell, ui, p, r, y, entity, entity_id, &subject.name);
    }
    // Columns.
    let title = fmt(&tr("Columns · {n}"), &[("n", &columns.len().to_string())]);
    let Some(next) = section(shell, ui, p, r, y, &title, Some(("workspace.properties.columns.add", "Add column"))) else {
        open_column(shell, entity_id, None);
        return;
    };
    y = next - 6.0;
    let chosen = selected_id(shell);
    for col in &columns {
        if shell.panels.column.as_ref().is_some_and(|c| c.target == Some(col.id)) {
            y = column_editor(shell, ui, p, r, y);
            continue;
        }
        let row = Rect::from_min_size(pos2(r.left() + PAD, y), vec2(width.max(0.0), ROW));
        let resp = control(shell, ui, &format!("workspace.properties.column.{}", col.id), "row", row, true);
        if Some(col.id) == chosen {
            paint.rect_filled(row, 6.0, p.accent_tint);
        } else if resp.contains_pointer() {
            paint.rect_filled(row, 6.0, p.page);
        }
        ux::icon_mid(&paint, pos2(row.left() + 8.0, row.center().y), &icons::GRIP, p.text_muted);
        let (badge, tint, ink) = if col.primary {
            (Some("PK"), p.amber_tint, p.amber)
        } else if col.foreign {
            (Some("FK"), p.amber_tint, p.amber)
        } else if col.unique {
            (Some("UQ"), p.accent_tint, p.accent)
        } else {
            (None, p.surface, p.text)
        };
        // Badge, name, link mark and type share the row's middle line.
        let mid = row.center().y;
        if let Some(b) = badge {
            let br = Rect::from_center_size(pos2(row.left() + 37.0, mid), vec2(26.0, 16.0));
            paint.rect_filled(br, 4.0, tint);
            let g = ux::galley(&paint, b, types::TINY, ink, None);
            paint.galley(br.center() - g.size() / 2.0, g, ink);
        }
        let name = ux::text_mid(&paint, row.left() + 58.0, mid, &col.name, types::UI, p.text, Some((width - 58.0 - 40.0).max(0.0)));
        let mut after = name.right() + 8.0;
        if let Some(target) = &col.linked {
            // The link mark of a key column that follows the column it points at.
            let mark = Rect::from_min_size(pos2(after, mid - 7.0), vec2(14.0, 14.0));
            ux::icon_in(&paint, mark, &icons::LINK, p.text_secondary);
            control(shell, ui, &format!("workspace.properties.column.{}.link", col.id), "mark", mark.expand(3.0), true)
                .on_hover_text(fmt(&tr("Follows {target}"), &[("target", target)]));
            after += 20.0;
        }
        ux::text_mid(&paint, after, mid + 0.5, &col.ty, types::CAPTION, p.text_secondary, Some((row.right() - after - 32.0).max(0.0)));
        let id = format!("workspace.properties.column.{}", col.id);
        if edit_button(shell, ui, p, row, &id, resp.contains_pointer()) || resp.double_clicked() {
            select_element(shell, col.id);
            open_column(shell, entity_id, Some(col.id));
        } else if resp.clicked() {
            select_element(shell, col.id);
        }
        y += ROW;
    }
    if shell.panels.column.as_ref().is_some_and(|c| c.entity == entity_id && c.target.is_none()) {
        y = column_editor(shell, ui, p, r, y);
    }
    y += 10.0;
    // Keys, checks and indexes, then the problems the rules find on the table.
    y = crate::keys::keys_section(shell, ui, p, r, y, entity, entity_id);
    y = crate::checks::checks_section(shell, ui, p, r, y + 4.0, entity, entity_id);
    y = crate::indexes::indexes_section(shell, ui, p, r, y + 4.0, entity, entity_id);
    y = crate::subtypes::subtypes_section(shell, ui, p, r, y + 4.0, entity, entity_id, entity_id);
    crate::keys::problems_section(shell, ui, p, r, y + 4.0, entity);
    // The kind menu last, over the rows.
    if shell.panels.menu == Some(Menu::Kind) {
        let items = [tr("Regular"), tr("Mapping table")];
        if let Some(i) = menu(shell, ui, p, kind_field, "workspace.properties.kind", &items) {
            let kind = if i == 1 { "associative" } else { "regular" };
            match run(shell, "er.entity.set-kind", json!({ "entity": entity_id.to_string(), "kind": kind })) {
                Ok(_) => shell.panels.notice = None,
                Err(e) => shell.panels.notice = Some(e),
            }
        }
    }
}

/// The first selected element's id, which a list in Properties marks.
pub(crate) fn selected_id(shell: &Shell) -> Option<Uuid> {
    let s = shell.session.as_ref()?;
    s.view().selection.first().and_then(|o| s.doc().uuid(*o).ok())
}

/// Selects an element a Properties row lists, so the Models tree and the canvas mark it as they
/// would for a click there; its table and schema open in the tree so its row shows.
pub(crate) fn select_element(shell: &mut Shell, id: Uuid) {
    if let Some(store) = shell.session.as_ref().map(|s| s.doc())
        && let Some(o) = store.by_uuid(id)
    {
        let parents = [archforge_er::rules::entity_of(store, o), archforge_er::rules::schema_of(store, o)];
        shell.explorer.expanded.extend(parents.into_iter().flatten().filter_map(|p| store.uuid(p).ok()));
    }
    if let Err(e) = run(shell, "view.select", json!({ "ids": [id.to_string()] })) {
        shell.panels.notice = Some(e);
    }
}

/// A list row's edit button at its right end. Only it, or a double click on the row, opens the
/// row's editor: selecting an element never does. True when it was pressed.
pub(crate) fn edit_button(shell: &mut Shell, ui: &egui::Ui, p: &Palette, row: Rect, id: &str, row_hovered: bool) -> bool {
    let at = Rect::from_center_size(pos2(row.right() - 16.0, row.center().y), vec2(24.0, 24.0));
    let resp = control(shell, ui, &format!("{id}.edit"), "button", at, true).on_hover_text(tr("Edit"));
    let paint = ui.painter();
    if resp.hovered() {
        paint.rect_filled(at, 6.0, p.border);
    }
    let ink = if resp.hovered() || row_hovered { p.text_secondary } else { p.text_muted };
    ux::icon_mid(paint, at.center(), &icons::EDIT, ink);
    resp.clicked()
}

/// Closes every Properties editor, as Cancel would, except one opened this frame and not yet
/// shown: an editor belongs to the selection it was opened on.
fn close_editors(shell: &mut Shell) {
    let panels = &mut shell.panels;
    if panels.column.as_ref().is_some_and(|e| !e.opened) {
        panels.column = None;
    }
    if panels.key.as_ref().is_some_and(|e| !e.opened) {
        panels.key = None;
    }
    if panels.check.as_ref().is_some_and(|e| !e.opened) {
        panels.check = None;
    }
    if panels.index.as_ref().is_some_and(|e| !e.opened) {
        panels.index = None;
    }
    if panels.type_edit.as_ref().is_some_and(|e| !e.opened) {
        panels.type_edit = None;
    }
    if panels.object_edit.as_ref().is_some_and(|e| !e.opened) {
        panels.object_edit = None;
    }
    if panels.member.as_ref().is_some_and(|e| !e.opened) {
        panels.member = None;
    }
}

/// Opens the column editor on `target`, or on a column to add to `entity`.
pub(crate) fn open_column(shell: &mut Shell, entity: Uuid, target: Option<Uuid>) {
    let Some(store) = shell.session.as_ref().map(|s| s.doc()) else { return };
    let edit = match target.and_then(|t| store.by_uuid(t)) {
        Some(a) => {
            let (length, precision, scale) = archforge_er::rules::own_size_of(store, a);
            ColumnEdit {
                target,
                entity,
                name: archforge_core::name_of(store, a),
                ty: if archforge_er::rules::domain_of(store, a).is_some() || archforge_er::rules::enum_of(store, a).is_some() {
                    archforge_er::rules::type_display(store, a)
                } else {
                    string_of(store, a, archforge_er::attribute::TYPE)
                },
                length: number(length),
                precision: number(precision),
                scale: number(scale),
                default: string_of(store, a, archforge_er::attribute::DEFAULT),
                generation: archforge_er::rules::generation_of(store, a),
                expression: string_of(store, a, archforge_er::attribute::EXPRESSION),
                collation: string_of(store, a, archforge_er::attribute::COLLATION),
                primary_key: archforge_er::rules::in_primary_key(store, a),
                not_null: !archforge_er::rules::is_nullable(store, a),
                unique: archforge_er::rules::is_unique(store, a),
                more: false,
                linked: linked_label(store, a),
                unlink: false,
                domain: archforge_er::rules::domain_of(store, a).and_then(|d| store.uuid(d).ok()),
                enum_type: archforge_er::rules::enum_of(store, a).and_then(|e| store.uuid(e).ok()),
                refused: None,
                opened: true,
            }
        }
        None => {
            let taken = store
                .by_uuid(entity)
                .map(|e| {
                    children(store, e, archforge_er::entity::ATTRIBUTE).into_iter().map(|a| archforge_core::name_of(store, a)).collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let name = crate::explorer::fresh_name("column", taken.into_iter(), true).unwrap_or_default();
            ColumnEdit {
                target: None,
                entity,
                name,
                ty: "TEXT".into(),
                length: String::new(),
                precision: String::new(),
                scale: String::new(),
                default: String::new(),
                generation: archforge_er::Generation::NoneKind,
                expression: String::new(),
                collation: String::new(),
                primary_key: false,
                not_null: false,
                unique: false,
                more: false,
                linked: None,
                unlink: false,
                domain: None,
                enum_type: None,
                refused: None,
                opened: true,
            }
        }
    };
    shell.panels.column = Some(edit);
    shell.panels.menu = None;
}

/// The generation choices, in the engine's order, with their words for the select.
const GENERATIONS: [(archforge_er::Generation, &str); 5] = [
    (archforge_er::Generation::NoneKind, "None"),
    (archforge_er::Generation::IdentityAlways, "Identity, always"),
    (archforge_er::Generation::IdentityByDefault, "Identity, by default"),
    (archforge_er::Generation::ComputedStored, "Computed, stored"),
    (archforge_er::Generation::ComputedVirtual, "Computed, virtual"),
];

fn is_computed(g: archforge_er::Generation) -> bool {
    matches!(g, archforge_er::Generation::ComputedStored | archforge_er::Generation::ComputedVirtual)
}

/// The column editor card, as the design's (slice S2 adds the size, default, generation and,
/// under More, the collation); gives the y under it.
/// The offer on a pure mapping table to become its many-to-many line again (the design's card
/// "Mapping table that can collapse"); gives the y under it.
fn collapse_offer(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect, y: f32, entity: Obj, entity_id: Uuid, name: &str) -> f32 {
    let paint = ui.painter().clone();
    let width = r.width() - 2.0 * PAD;
    let (from, to) = match shell.session.as_ref() {
        Some(s) => {
            let store = s.doc();
            let mut parents = archforge_er::rules::foreign_keys(store, entity).into_iter().filter_map(|fk| {
                let k = archforge_er::rules::key_parts(store, fk).key?;
                Some(archforge_core::name_of(store, archforge_er::rules::entity_of(store, k)?))
            });
            (parents.next().unwrap_or_default(), parents.next().unwrap_or_default())
        }
        None => return y,
    };
    let y = section(shell, ui, p, r, y, &tr("Mapping table"), None).unwrap_or(y + 50.0) - 6.0;
    let message = fmt(
        &tr("{name} holds nothing but its two keys, so it can become the many-to-many line between {from} and {to} again."),
        &[("name", name), ("from", &from), ("to", &to)],
    );
    let inner = (width - 24.0).max(0.0);
    let g = ux::galley_rows(&paint, &message, types::SMALL, p.text, Some(inner), 4);
    let text_h = g.size().y;
    let card = Rect::from_min_size(pos2(r.left() + PAD, y), vec2(width.max(0.0), 12.0 + text_h + 12.0 + 24.0 + 12.0));
    paint.rect_filled(card, 8.0, p.accent_tint);
    paint.rect_stroke(card, 8.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
    paint.galley(card.min + vec2(12.0, 12.0), g, p.text);
    let label = tr("Collapse into a relationship");
    let g = ux::galley(&paint, &label, types::UI, p.text, None);
    let button = Rect::from_min_size(card.min + vec2(12.0, 12.0 + text_h + 12.0), vec2(6.0 + 14.0 + 7.0 + g.size().x + 8.0, 24.0));
    ux::icon_in(&paint, Rect::from_min_size(button.min + vec2(6.0, 5.0), Vec2::splat(14.0)), &icons::MERGE_LINE, p.text);
    paint.galley(button.min + vec2(27.0, 2.5), g, p.text);
    let resp = control(shell, ui, "workspace.properties.collapse", "button", button, true);
    if resp.hovered() {
        paint.rect_stroke(button, 6.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
    }
    if resp.clicked() {
        // The command selects the relationship it makes.
        match run(shell, "er.relationship.collapse", json!({ "entity": entity_id.to_string() })) {
            Ok(_) => shell.panels.notice = None,
            Err(e) => shell.panels.notice = Some(e),
        }
    }
    card.bottom() + 10.0
}

fn column_editor(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect, y: f32) -> f32 {
    let paint = ui.painter().clone();
    let width = r.width() - 2.0 * PAD;
    let Some(mut edit) = shell.panels.column.clone() else { return y };
    let typed = edit.domain.is_some() || edit.enum_type.is_some();
    let shape = if typed { archforge_er::rules::TypeShape::Plain } else { archforge_er::rules::type_shape(&edit.ty) };
    let computed = is_computed(edit.generation);
    // The card's height from its rows.
    let row = 17.0 + FIELD + 10.0;
    let on_domain = edit.domain.is_some();
    let mut height = 12.0 + row * 4.0 + 20.0 + 8.0 + 20.0 + 14.0 + 24.0 + 10.0 + FIELD + 12.0;
    if on_domain {
        // The domain brings the default and the collation: neither row shows.
        height -= row + 24.0 + 10.0;
    }
    if computed {
        height += row;
    }
    if edit.more && !on_domain {
        height += row;
    }
    if edit.linked.is_some() {
        height += 28.0;
    }
    if edit.refused.is_some() {
        height += 20.0;
    }
    let card = Rect::from_min_size(pos2(r.left() + PAD, y), vec2(width.max(0.0), height));
    paint.rect_filled(card, 8.0, p.surface);
    paint.rect_stroke(card, 8.0, Stroke::new(1.0, p.accent), StrokeKind::Inside);
    reveal(shell, ui, card);
    let inner = (width - 24.0).max(0.0);
    let at = |x: f32, dy: f32| card.min + vec2(12.0 + x, dy);
    let take = std::mem::take(&mut edit.opened);
    let mut cy = 12.0;
    let mut enter =
        text_field(shell, ui, p, at(0.0, cy), inner, "workspace.properties.column.name", &tr("Name"), &mut edit.name, MAX_NAME, take, false);
    cy += row;
    if let Some(target) = edit.linked.clone() {
        // What the column follows, with the way out of it.
        ux::icon_in(&paint, Rect::from_min_size(at(0.0, cy + 3.0), Vec2::splat(14.0)), &icons::LINK, p.text_secondary);
        ux::text(
            &paint,
            at(20.0, cy),
            &fmt(&tr("Follows {target}"), &[("target", &target)]),
            types::CAPTION,
            p.text_secondary,
            Some((inner - 20.0 - 32.0).max(0.0)),
        );
        let unlink = Rect::from_min_size(pos2(card.right() - 12.0 - 24.0, card.top() + cy - 4.0), Vec2::splat(24.0));
        ux::icon_in(&paint, Rect::from_center_size(unlink.center(), Vec2::splat(12.0)), &icons::CLOSE, p.text_secondary);
        if control(shell, ui, "workspace.properties.column.unlink", "button", unlink, true).on_hover_text(tr("Unlink")).clicked() {
            // Pending until Apply, so the card stays one step and Cancel keeps the link.
            edit.linked = None;
            edit.unlink = true;
        }
        cy += 28.0;
    }
    // The type, with the size fields its shape takes beside it.
    let (type_w, size_w) = match shape {
        archforge_er::rules::TypeShape::Plain => (inner, 0.0),
        archforge_er::rules::TypeShape::PrecisionScale => ((inner - 2.0 * 64.0 - 16.0).max(60.0), 64.0),
        _ => ((inner - 80.0 - 8.0).max(60.0), 80.0),
    };
    let typed_name = edit.ty.clone();
    enter |= text_field(shell, ui, p, at(0.0, cy), type_w, "workspace.properties.column.type", &tr("Type"), &mut edit.ty, MAX_TYPE, false, false);
    if typed && edit.ty != typed_name {
        // Typing a name over a domain or enum type leaves it.
        edit.domain = None;
        edit.enum_type = None;
    }
    // The chevron inside the type field opens the schema's types and the known base types.
    let type_field = Rect::from_min_size(at(0.0, cy + 17.0), vec2(type_w, FIELD));
    let chevron = Rect::from_min_size(pos2(type_field.right() - 24.0, type_field.top() + 2.0), Vec2::splat(24.0));
    ux::icon(&paint, pos2(chevron.left() + 4.0, chevron.top() + 10.0), &icons::CHEVRON_DOWN, 16.0, p.text_secondary);
    if control(shell, ui, "workspace.properties.column.type.menu", "button", chevron, true).on_hover_text(tr("Pick a type")).clicked() {
        shell.panels.menu = if shell.panels.menu == Some(Menu::ColumnType) { None } else { Some(Menu::ColumnType) };
    }
    match shape {
        archforge_er::rules::TypeShape::Length => {
            enter |= text_field(
                shell,
                ui,
                p,
                at(type_w + 8.0, cy),
                size_w,
                "workspace.properties.column.length",
                &tr("Length"),
                &mut edit.length,
                5,
                false,
                false,
            );
        }
        archforge_er::rules::TypeShape::Precision => {
            enter |= text_field(
                shell,
                ui,
                p,
                at(type_w + 8.0, cy),
                size_w,
                "workspace.properties.column.precision",
                &tr("Precision"),
                &mut edit.precision,
                5,
                false,
                false,
            );
        }
        archforge_er::rules::TypeShape::PrecisionScale => {
            enter |= text_field(
                shell,
                ui,
                p,
                at(type_w + 8.0, cy),
                size_w,
                "workspace.properties.column.precision",
                &tr("Precision"),
                &mut edit.precision,
                5,
                false,
                false,
            );
            enter |= text_field(
                shell,
                ui,
                p,
                at(type_w + 8.0 + size_w + 8.0, cy),
                size_w,
                "workspace.properties.column.scale",
                &tr("Scale"),
                &mut edit.scale,
                5,
                false,
                false,
            );
        }
        archforge_er::rules::TypeShape::Plain => {}
    }
    cy += row;
    if !on_domain {
        enter |= text_field(
            shell,
            ui,
            p,
            at(0.0, cy),
            inner,
            "workspace.properties.column.default",
            &tr("Default"),
            &mut edit.default,
            archforge_er::rules::MAX_TEXT,
            false,
            false,
        );
        cy += row;
    }
    let generation_word = GENERATIONS.iter().find(|(g, _)| *g == edit.generation).map(|(_, w)| tr(w)).unwrap_or_default();
    let generation_field = select_field(
        shell,
        ui,
        p,
        at(0.0, cy),
        inner,
        "workspace.properties.column.generation",
        &tr("Generation"),
        &generation_word,
        Menu::Generation,
    );
    cy += row;
    if computed {
        enter |= text_field(
            shell,
            ui,
            p,
            at(0.0, cy),
            inner,
            "workspace.properties.column.expression",
            &tr("Expression"),
            &mut edit.expression,
            archforge_er::rules::MAX_TEXT,
            false,
            false,
        );
        cy += row;
    }
    // The checks in a row: the second starts where the first's label ended.
    // A key column is never null: ticking Primary key ticks Not null, and unticking Not null
    // unticks Primary key.
    let was_key = edit.primary_key;
    let next_x = check(shell, ui, p, at(0.0, cy), "workspace.properties.column.primary-key", &tr("Primary key"), &mut edit.primary_key);
    if edit.primary_key && !was_key {
        edit.not_null = true;
    }
    let was_not_null = edit.not_null;
    check(shell, ui, p, pos2(next_x - 12.0, card.top() + cy), "workspace.properties.column.not-null", &tr("Not null"), &mut edit.not_null);
    if was_not_null && !edit.not_null {
        edit.primary_key = false;
    }
    cy += 28.0;
    check(shell, ui, p, at(0.0, cy), "workspace.properties.column.unique", &tr("Unique"), &mut edit.unique);
    cy += 20.0 + 14.0;
    // More: the collation; a domain brings its own, so a column on one shows neither.
    if !on_domain {
        let more = Rect::from_min_size(at(0.0, cy), vec2(inner, 24.0));
        ux::icon_in(
            &paint,
            Rect::from_min_size(more.min + vec2(0.0, 4.0), Vec2::splat(16.0)),
            if edit.more { &icons::CHEVRON_DOWN } else { &icons::CHEVRON_RIGHT },
            p.text_secondary,
        );
        ux::text(&paint, more.min + vec2(22.0, 2.0), &tr("More"), types::UI, p.text, Some((inner - 22.0).max(0.0)));
        if control(shell, ui, "workspace.properties.column.more", "disclosure", more, true).clicked() {
            edit.more = !edit.more;
        }
        cy += 24.0 + 10.0;
        if edit.more {
            enter |= text_field(
                shell,
                ui,
                p,
                at(0.0, cy),
                inner,
                "workspace.properties.column.collation",
                &tr("Collation"),
                &mut edit.collation,
                MAX_NAME,
                false,
                false,
            );
            cy += row;
        }
    }
    let mut remove = false;
    if edit.target.is_some() {
        remove = icon_button(shell, ui, p, at(0.0, cy), "workspace.properties.column.remove", &icons::TRASH, p.danger, None, "Remove");
    }
    let cancel = icon_button(
        shell,
        ui,
        p,
        pos2(card.right() - 12.0 - 2.0 * FIELD - 8.0, card.top() + cy),
        "workspace.properties.column.cancel",
        &icons::CLOSE,
        p.text_secondary,
        None,
        "Cancel",
    );
    let apply = icon_button(
        shell,
        ui,
        p,
        pos2(card.right() - 12.0 - FIELD, card.top() + cy),
        "workspace.properties.column.apply",
        &icons::TICK,
        p.on_accent,
        Some(p.accent_fill),
        "Apply",
    );
    if let Some(why) = &edit.refused {
        ux::text(&paint, at(0.0, cy + FIELD + 8.0), why, types::CAPTION, p.amber, Some(inner));
    }
    // The type menu, over the card: the schema's domains and enum types, then the base types.
    if shell.panels.menu == Some(Menu::ColumnType) {
        let types: Vec<(Option<(bool, Uuid)>, String)> = shell
            .session
            .as_ref()
            .and_then(|s| {
                let store = s.doc();
                let e = store.by_uuid(edit.entity)?;
                let schema = archforge_er::rules::schema_of(store, e)?;
                let mut out: Vec<(Option<(bool, Uuid)>, String)> = Vec::new();
                for d in archforge_er::rules::domains_of(store, schema) {
                    if let Ok(id) = store.uuid(d) {
                        out.push((Some((true, id)), format!("{} · {}", archforge_core::name_of(store, d), tr("domain"))));
                    }
                }
                for en in archforge_er::rules::enums_of(store, schema) {
                    if let Ok(id) = store.uuid(en) {
                        out.push((Some((false, id)), format!("{} · {}", archforge_core::name_of(store, en), tr("enum"))));
                    }
                }
                Some(out)
            })
            .unwrap_or_default();
        let mut items: Vec<(Option<(bool, Uuid)>, String)> = types;
        items.extend(BASE_TYPES.iter().map(|t| (None, (*t).to_string())));
        let labels: Vec<String> = items.iter().map(|(_, l)| l.clone()).collect();
        if let Some(i) = menu(shell, ui, p, type_field, "workspace.properties.column.type.menu", &labels)
            && let Some((picked, label)) = items.get(i)
        {
            match picked {
                Some((true, id)) => {
                    edit.domain = Some(*id);
                    edit.enum_type = None;
                    edit.ty = label.split(" · ").next().unwrap_or_default().to_string();
                    // The domain brings the default and the collation.
                    edit.default.clear();
                    edit.collation.clear();
                    edit.more = false;
                }
                Some((false, id)) => {
                    edit.enum_type = Some(*id);
                    edit.domain = None;
                    edit.ty = label.split(" · ").next().unwrap_or_default().to_string();
                }
                None => {
                    edit.domain = None;
                    edit.enum_type = None;
                    edit.ty = label.clone();
                }
            }
            edit.length.clear();
            edit.precision.clear();
            edit.scale.clear();
        }
    }
    // The generation menu, over the card.
    if shell.panels.menu == Some(Menu::Generation) {
        let items: Vec<String> = GENERATIONS.iter().map(|(_, w)| tr(w)).collect();
        if let Some(i) = menu(shell, ui, p, generation_field, "workspace.properties.column.generation", &items)
            && let Some((g, _)) = GENERATIONS.get(i)
        {
            edit.generation = *g;
        }
    }
    shell.panels.column = Some(edit.clone());
    if cancel || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        shell.panels.column = None;
    } else if remove {
        if let Some(t) = edit.target {
            // What goes with the column, named for the toast.
            let went: Vec<String> = shell
                .session
                .as_ref()
                .and_then(|s| {
                    let store = s.doc();
                    let a = store.by_uuid(t)?;
                    let (keys, fks) = archforge_er::commands::dependents(store, a).ok()?;
                    Some(keys.iter().chain(fks.iter()).filter(|k| **k != a).map(|k| archforge_er::rules::label(store, *k)).collect())
                })
                .unwrap_or_default();
            match run(shell, "er.attribute.remove", json!({ "attribute": t.to_string() })) {
                Ok(_) => {
                    shell.panels.column = None;
                    let text = if went.is_empty() {
                        fmt(&tr("{name} removed"), &[("name", &edit.name)])
                    } else {
                        fmt(&tr("{name} removed · {keys} went with it"), &[("name", &edit.name), ("keys", &went.join(", "))])
                    };
                    let undo_label = shell.session.as_ref().and_then(|s| s.undo_label().map(str::to_string));
                    shell.canvas.toast = Some(crate::connect::Toast { text, until: shell.frames.saturating_add(300), undo_label });
                }
                Err(e) => set_column_refusal(shell, e),
            }
        }
    } else if apply || enter {
        commit_column(shell, &edit);
    }
    card.bottom() + 8.0
}

fn set_column_refusal(shell: &mut Shell, why: String) {
    if let Some(c) = shell.panels.column.as_mut() {
        c.refused = Some(why);
    }
}

/// A size field as typed: empty is none, else a number.
pub(crate) fn number(v: Option<i64>) -> String {
    v.map(|n| n.to_string()).unwrap_or_default()
}

pub(crate) fn size_value(text: &str) -> Result<i64, String> {
    let t = text.trim();
    if t.is_empty() {
        return Ok(0);
    }
    t.parse::<i64>().map_err(|_| fmt(&tr("{text} is not a number"), &[("text", t)]))
}

/// Writes the column editor through the commands: add, or rename, type with its size, default,
/// generation, collation and flags, each only when it changed.
fn commit_column(shell: &mut Shell, edit: &ColumnEdit) {
    let nullable = if edit.not_null { "no" } else { "yes" };
    let size = match (size_value(&edit.length), size_value(&edit.precision), size_value(&edit.scale)) {
        (Ok(l), Ok(p), Ok(s)) => (l, p, s),
        (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => {
            set_column_refusal(shell, e);
            return;
        }
    };
    let generation = archforge_er::commands::GENERATION_WORDS.iter().find(|(g, _)| *g == edit.generation).map(|(_, w)| *w).unwrap_or("none");
    let outcome = match edit.target {
        None => crate::editing::run_growing(
            shell,
            &tr("Add column"),
            "er.attribute.add",
            json!({
                "entity": edit.entity.to_string(), "name": edit.name.trim(),
                "type": if edit.domain.is_some() || edit.enum_type.is_some() { "" } else { edit.ty.trim() },
                "domain": edit.domain.map(|d| d.to_string()).unwrap_or_default(),
                "enum": edit.enum_type.map(|e| e.to_string()).unwrap_or_default(),
                "length": size.0, "precision": size.1, "scale": size.2,
                "default": if edit.domain.is_some() { "" } else { edit.default.trim() },
                "generation": generation, "expression": edit.expression.trim(),
                "collation": if edit.domain.is_some() { "" } else { edit.collation.trim() },
                "primary_key": edit.primary_key, "nullable": nullable, "unique": edit.unique,
            }),
            edit.entity,
        )
        .map(|_| ()),
        Some(t) => {
            // Every changed field in one batch: a refusal rolls the whole card back, so what the
            // card shows and what landed never drift apart.
            let Some(session) = shell.session.as_mut() else { return };
            let store = session.doc();
            let Some(a) = store.by_uuid(t) else { return };
            let old_name = archforge_core::name_of(store, a);
            let old_type = string_of(store, a, archforge_er::attribute::TYPE);
            let old_domain = archforge_er::rules::domain_of(store, a).and_then(|d| store.uuid(d).ok());
            let old_enum = archforge_er::rules::enum_of(store, a).and_then(|e| store.uuid(e).ok());
            let (ol, op, os) = archforge_er::rules::own_size_of(store, a);
            let old_size = (ol.unwrap_or(0), op.unwrap_or(0), os.unwrap_or(0));
            let old_default = string_of(store, a, archforge_er::attribute::DEFAULT);
            let old_generation = archforge_er::rules::generation_of(store, a);
            let old_expression = string_of(store, a, archforge_er::attribute::EXPRESSION);
            let old_collation = string_of(store, a, archforge_er::attribute::COLLATION);
            let id = t.to_string();
            let mut steps: Vec<(&str, serde_json::Value)> = Vec::new();
            if edit.unlink {
                steps.push(("er.attribute.unlink", json!({ "attribute": id })));
            }
            if edit.name.trim() != old_name {
                steps.push(("er.attribute.rename", json!({ "attribute": id, "name": edit.name.trim() })));
            }
            let typed = edit.domain.is_some() || edit.enum_type.is_some();
            if (!typed && (edit.ty.trim() != old_type || size != old_size)) || edit.domain != old_domain || edit.enum_type != old_enum {
                steps.push((
                    "er.attribute.set-type",
                    json!({
                        "attribute": id,
                        "type": if typed { "" } else { edit.ty.trim() },
                        "domain": edit.domain.map(|d| d.to_string()).unwrap_or_default(),
                        "enum": edit.enum_type.map(|e| e.to_string()).unwrap_or_default(),
                        "length": size.0, "precision": size.1, "scale": size.2,
                    }),
                ));
            }
            if edit.domain.is_none() && edit.default.trim() != old_default {
                steps.push(("er.attribute.set-default", json!({ "attribute": id, "default": edit.default.trim() })));
            }
            if edit.generation != old_generation || edit.expression.trim() != old_expression {
                steps.push((
                    "er.attribute.set-generation",
                    json!({ "attribute": id, "generation": generation, "expression": edit.expression.trim() }),
                ));
            }
            if edit.domain.is_none() && edit.collation.trim() != old_collation {
                steps.push(("er.attribute.set-collation", json!({ "attribute": id, "collation": edit.collation.trim() })));
            }
            steps.push((
                "er.attribute.set-flags",
                json!({ "attribute": id, "primary_key": edit.primary_key, "nullable": nullable, "unique": edit.unique }),
            ));
            if steps.len() == 1 {
                session.execute(steps[0].0, &steps[0].1).map(|_| ()).map_err(|e| e.to_string())
            } else {
                session
                    .batch(&tr("Edit column"), |b| {
                        for (cmd, params) in &steps {
                            b.execute(cmd, params)?;
                        }
                        Ok(())
                    })
                    .map(|_| ())
                    .map_err(|e| e.to_string())
            }
        }
    };
    match outcome {
        Ok(()) => shell.panels.column = None,
        Err(e) => set_column_refusal(shell, e),
    }
}

// Skills.

/// One skill as the panel lists it.
struct SkillRow {
    id: Uuid,
    name: String,
    /// What it aims at, or where it comes from.
    detail: String,
    /// The subject's own, which the row can remove; an inherited one is removed where it sits.
    own: bool,
}

/// What a skill aims at, as Properties says it: every class, every interface, every enumeration,
/// every data type, every primitive type, every association class, its type pattern, or nothing.
pub(crate) fn target_word(store: &Store, s: Obj) -> String {
    let applies = string_of(store, s, archforge_core::skill::APPLIES_TO);
    let pattern = string_of(store, s, archforge_core::skill::TYPE_PATTERN);
    match archforge_core::skills::kind_by_key(&archforge_notations::view(), store, &applies).map(|(_, w)| w) {
        Some("table") => tr("every table"),
        Some("relationship") => tr("every relationship"),
        Some("class") => tr("every class"),
        Some("interface") => tr("every interface"),
        Some("enumeration") => tr("every enumeration"),
        Some("data type") => tr("every data type"),
        Some("primitive type") => tr("every primitive type"),
        Some("association class") => tr("every association class"),
        Some("attribute") => tr("every attribute"),
        Some("operation") => tr("every operation"),
        Some("association") => tr("every association"),
        Some(_) if !pattern.is_empty() => pattern,
        Some("column") => tr("every column"),
        Some(_) | None => String::new(),
    }
}

/// The selection's own skills and those it inherits, each as its row says it.
fn skill_rows(store: &Store, subject: &Subject) -> (Vec<SkillRow>, Vec<SkillRow>) {
    let this = subject.this.clone();
    let subject = subject.obj;
    let own: Vec<SkillRow> = archforge_core::skills::skills(store)
        .into_iter()
        .filter(|s| archforge_core::skills::subject_of(store, *s) == subject)
        .filter_map(|s| {
            let target = target_word(store, s);
            Some(SkillRow {
                id: store.uuid(s).ok()?,
                name: archforge_core::skills::name_of(store, s),
                detail: if target.is_empty() { this.clone() } else { target },
                own: true,
            })
        })
        .collect();
    let inherited = match subject {
        Some(element) => archforge_core::skills::effective_skills(&archforge_notations::view(), store, element, None)
            .into_iter()
            .filter(|e| e.origin != Some(element))
            .filter_map(|e| {
                let origin = match e.origin {
                    Some(o) => crate::diagrams::subject_name(store, o),
                    None => tr("project"),
                };
                Some(SkillRow {
                    id: store.uuid(e.skill).ok()?,
                    name: archforge_core::skills::name_of(store, e.skill),
                    detail: if e.overridden_by.is_some() { tr("overridden below") } else { origin },
                    own: false,
                })
            })
            .collect(),
        None => Vec::new(),
    };
    (own, inherited)
}

/// Why the selection carries no skill, in the user's words, when it carries none: asked of the
/// model's own rule, so the panel offers to add a skill only where `skill.add` takes one.
fn no_skills(shell: &Shell, subject: &Subject) -> Option<String> {
    let (s, o) = (shell.session.as_ref()?, subject.obj?);
    let store = s.doc();
    archforge_core::skills::subject_problem(s.view(), store, o)?;
    let said = crate::notation_ui::of(shell.notation_uis, store, o).and_then(|ui| ui.no_skills(store, o));
    Some(said.unwrap_or_else(|| tr("This element carries no skills")))
}

/// Draws the Skills panel's body under the header, from `y`: the selection's own skills with `+`
/// to add one where it carries skills, else a line saying it carries none, then those it inherits.
fn skills(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect, mut y: f32, subject: &Subject) {
    let paint = ui.painter().clone();
    let width = r.width() - 2.0 * PAD;
    let (own, inherited) = match shell.session.as_ref() {
        Some(s) => skill_rows(s.doc(), subject),
        None => return,
    };
    let none = no_skills(shell, subject);
    let title = fmt(&subject.on_this, &[("n", &own.len().to_string())]);
    let add = none.is_none().then_some(("workspace.skills.add", "Add skill"));
    let Some(next) = section(shell, ui, p, r, y, &title, add) else {
        crate::skill_tab::create(shell, subject.id);
        return;
    };
    y = next - 6.0;
    if let Some(why) = none {
        ux::text(&paint, pos2(r.left() + PAD + 8.0, y + 7.0), &why, types::SMALL, p.text_muted, Some((width - 8.0).max(0.0)));
        y += 30.0;
    }
    for row in &own {
        y = skill_row(shell, ui, &paint, p, r, y, row, width);
    }
    y += 10.0;
    let title = fmt(&tr("Inherited · {n}"), &[("n", &inherited.len().to_string())]);
    y = section(shell, ui, p, r, y, &title, None).unwrap_or(y + 50.0) - 6.0;
    for row in &inherited {
        y = skill_row(shell, ui, &paint, p, r, y, row, width);
    }
}

fn skill_row(shell: &mut Shell, ui: &egui::Ui, paint: &egui::Painter, p: &Palette, r: Rect, y: f32, row: &SkillRow, width: f32) -> f32 {
    let rect = Rect::from_min_size(pos2(r.left() + PAD, y), vec2(width.max(0.0), 30.0));
    let resp = control(shell, ui, &format!("workspace.skills.row.{}", row.id), "row", rect, true).on_hover_text(tr("Open the skill"));
    // The subject's own skill can be removed from its row, the bin showing while the row is hovered.
    let bin = Rect::from_min_size(pos2(rect.right() - 28.0, rect.top() + 3.0), Vec2::splat(24.0));
    let bin_resp =
        row.own.then(|| control(shell, ui, &format!("workspace.skills.remove.{}", row.id), "button", bin, true).on_hover_text(tr("Remove")));
    let hovered = resp.hovered() || bin_resp.as_ref().is_some_and(|b| b.hovered());
    if shell.canvas.active == Some(row.id) {
        paint.rect_filled(rect, 5.0, p.accent_tint);
    } else if hovered {
        paint.rect_filled(rect, 5.0, p.page);
    }
    ux::icon_in(paint, Rect::from_min_size(rect.min + vec2(8.0, 6.0), Vec2::splat(18.0)), &icons::SKILL, p.text_secondary);
    let bin_w = if row.own { 26.0 } else { 0.0 };
    let detail = ux::galley(paint, &row.detail, types::CAPTION, p.text_muted, Some((width / 2.0).max(0.0)));
    let name_room = (width - 34.0 - detail.size().x - 16.0 - bin_w).max(0.0);
    ux::text(paint, rect.min + vec2(34.0, 6.5), &row.name, types::SMALL, p.text, Some(name_room));
    paint.galley(pos2(rect.right() - 8.0 - bin_w - detail.size().x, rect.top() + 8.0), detail, p.text_muted);
    if row.own && hovered {
        let ink = if bin_resp.as_ref().is_some_and(|b| b.hovered()) { p.danger } else { p.text_secondary };
        ux::icon_in(paint, Rect::from_center_size(bin.center(), Vec2::splat(14.0)), &icons::TRASH, ink);
    }
    if bin_resp.is_some_and(|b| b.clicked()) {
        if let Err(e) = run(shell, "skill.remove", json!({ "skill": row.id.to_string() })) {
            shell.panels.notice = Some(e);
        }
    } else if resp.clicked() {
        crate::canvas::open(shell, row.id);
    }
    y + 30.0
}

/// Every text that reaches `tr` through a variable: the hover hints of the icon buttons, for the
/// catalogue tests.
#[cfg(test)]
pub(crate) fn texts() -> impl Iterator<Item = &'static str> {
    ["Add column", "Add skill", "Remove", "Cancel", "Apply", "Unlink", "Pick a type"].into_iter().chain(GENERATIONS.iter().map(|(_, w)| *w))
}

// The panel.

/// Draws the panel body for `panel` under its header, from `y`: Properties and Skills follow the
/// selection; Chat and Appearance say what is true, Chat that it is coming.
pub fn draw(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect, y: f32, panel: Panel, project: &str) {
    let paint = ui.painter().clone();
    let s = subject(shell, project);
    // A description typed on the element selected before goes to it first.
    flush_description(shell, s.id);
    match panel {
        Panel::Agent => {
            ux::text(&paint, r.min + vec2(PAD, y), &tr("Coming soon"), types::SECTION, p.text, Some((r.width() - 2.0 * PAD).max(0.0)));
            return;
        }
        Panel::Appearance => {
            crate::appearance::draw(shell, ui, p, r, y);
            return;
        }
        Panel::Properties | Panel::Skills => {}
    }
    ux::text(&paint, r.min + vec2(PAD, y), &s.name, types::CARD_TITLE, p.text, Some((r.width() - 2.0 * PAD).max(0.0)));
    ux::text(&paint, r.min + vec2(PAD, y + 26.0), &s.kind, types::CAPTION, p.text_secondary, Some((r.width() - 2.0 * PAD).max(0.0)));
    // Every element of the model takes a description: an ER element's is the text DDL writes as its
    // comment, a UML element's a Comment it owns (AR-186).
    let mut body_y = r.top() + y + 51.0;
    if panel == Panel::Properties && s.obj.is_some() && s.class != Some(archforge_diagram::DIAGRAM) {
        description_field(shell, ui, p, pos2(r.left() + PAD, r.top() + y + 49.0), (r.width() - 2.0 * PAD).max(0.0), &s);
        body_y += 17.0 + FIELD + 14.0;
    }
    // The body scrolls under the pinned heading and description: a new panel
    // or selection starts at the top, an editor that opens is brought into view, the wheel over
    // the body moves it within its content.
    let body = Rect::from_min_max(pos2(r.left(), body_y), r.max);
    // A column shows its table's page, so moving between a table and its columns keeps the scroll.
    let page = match (s.class, shell.session.as_ref(), s.obj) {
        (Some(c), Some(session), Some(o)) if c == archforge_er::ATTRIBUTE => {
            let store = session.doc();
            archforge_er::rules::entity_of(store, o).and_then(|e| store.uuid(e).ok())
        }
        _ => s.id,
    };
    if shell.panels.scrolled_for != Some((panel, page)) {
        shell.panels.scrolled_for = Some((panel, page));
        shell.panels.scroll = 0.0;
    }
    let open = open_editors(&shell.panels);
    if open != shell.panels.open_editors {
        let opened = open.iter().zip(shell.panels.open_editors.iter().chain(std::iter::repeat(&None))).any(|(now, was)| now.is_some() && now != was);
        shell.panels.reveal |= opened;
        shell.panels.open_editors = open;
    }
    let room = (shell.panels.content - body.height()).max(0.0);
    shell.panels.scroll = shell.panels.scroll.clamp(0.0, room);
    shell.panels.body = Some(body);
    let top = body_y - shell.panels.scroll;
    let mut clip = ui.new_child(egui::UiBuilder::new().max_rect(body));
    clip.set_clip_rect(body);
    shell.registry.reach_from(top);
    match panel {
        Panel::Properties => properties(shell, &mut clip, p, r, top, &s),
        Panel::Skills => skills(shell, &mut clip, p, r, top, &s),
        _ => {}
    }
    shell.panels.content = (shell.registry.reach() - top + SCROLL_TAIL).max(0.0);
    shell.panels.reveal = false;
    if ui.rect_contains_pointer(body) && shell.panels.menu.is_none() {
        let dy = ui.input(|i| i.smooth_scroll_delta.y);
        if dy != 0.0 {
            shell.panels.scroll = (shell.panels.scroll - dy).clamp(0.0, (shell.panels.content - body.height()).max(0.0));
        }
    }
    // A thin thumb at the right edge while the content overflows (the design has no scroll bar;
    // owner-delegated UX).
    if shell.panels.content > body.height() + 1.0 {
        let track = body.shrink2(vec2(0.0, 6.0));
        let h = (track.height() * body.height() / shell.panels.content).max(24.0).min(track.height());
        let at = shell.panels.scroll / (shell.panels.content - body.height()).max(1.0);
        let thumb = Rect::from_min_size(pos2(r.right() - 6.0, track.top() + at * (track.height() - h)), vec2(4.0, h));
        paint.rect_filled(thumb, 2.0, p.scroll_thumb);
    }
    if let Some(notice) = shell.panels.notice.clone() {
        ux::text(&paint, pos2(r.left() + PAD, r.bottom() - 36.0), &notice, types::CAPTION, p.amber, Some((r.width() - 2.0 * PAD).max(0.0)));
    }
}
