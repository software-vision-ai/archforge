//! The right panels (slice W3 of the workspace chrome, from the design's pages 03 and 04):
//! Properties shows the selection under its heading and description — a diagram's name and the
//! element it is of, an element's body as its notation draws it, and the problems the rules find —
//! and Skills shows the skills on the selection and the ones it inherits, each opening in its tab. Every change goes through the
//! engine's commands, so undo covers it; a refusal shows at the panel's foot as the engine's
//! reason, without the command's id, until the selection moves on.
//! Agent and Appearance say what is true until their slices.
//!
//! The geometry is the design's: a section is 16 px in from the panel's edges with a full-width
//! line over it, a field is 28 px tall and filled, its label 11 px above, a row 36 px, an editor
//! a bordered card.

use archforge_mof::{Obj, Store, Uuid};
use egui::{Rect, Vec2, pos2, vec2};
use serde_json::json;

pub use archforge_shell_api::notation_ui::MenuId;
pub(crate) use archforge_shell_api::notation_ui::{Editor, OpenEditor, Subject};
pub(crate) use archforge_shell_api::panels::{
    Combo, ComboOut, FIELD, FieldOut, PAD, PATH_SEP, ROW, check_cx, control_cx, edit_button_cx, edit_field_cx, editable_select_cx, field_inner,
    ghost_button_cx, icon_button_cx, into_view, keys_at_start, menu_cx, menu_noted_cx, menu_paths_cx, name_field_cx, off_words_cx,
    path_select_field_cx, problems_section_cx, refuse_empty_name_cx, refused_on_cx, reveal_cx, section_cx, select_element_cx, select_field_cx,
    selected_id_cx, text_field_cx,
};
pub use archforge_shell_api::panels::{DescriptionDraft, NameEdit, State};
pub(crate) use archforge_shell_api::panels::{a_kind, refusal, run_cx, said};

use crate::home::control;
use crate::i18n::{fmt, tr};
use crate::notation_ui::Words;
use crate::state::Panel;
use crate::ux::{self, Palette, icons, types};
use crate::{Shell, widgets};

/// What is left under the last control of a scrolled body, for its trailing text.
const SCROLL_TAIL: f32 = 48.0;

/// The editors of `now` to bring into view: one opened or moved to another place since `was`, or,
/// true beside it, one whose card shows a new refusal (AR-469). A refusal that goes or stays, and an
/// editor that changed nothing, bring nothing into view, so a body scrolled away stays put.
fn to_reveal(now: &[(Editor, OpenEditor)], was: &[(Editor, OpenEditor)]) -> Vec<(Editor, bool)> {
    let before = |editor: Editor| was.iter().find(|(e, _)| *e == editor).and_then(|(_, open)| open.as_ref());
    let reveal = |(editor, now): &(Editor, OpenEditor)| match (now, before(*editor)) {
        (Some((place, why)), Some((was, said))) if place == was => (why.is_some() && why != said).then_some((*editor, true)),
        (Some(_), _) => Some((*editor, false)),
        (None, _) => None,
    };
    now.iter().filter_map(reveal).collect()
}

/// Whether a name is being typed in `s`, or something of a notation's `notations` is being typed
/// or drawn or one of their editor cards is open ([`crate::notation_ui::NotationState::editing`]):
/// the fields that only show what is stored are not counted.
pub(crate) fn editing(s: &State, notations: &crate::notation_ui::NotationStates) -> bool {
    s.name.is_some() || notations.editing()
}

/// The body follows the keyboard (AR-470). Before it draws: while one of its controls has the
/// keyboard, no text is being typed and no menu is open, Page Up, Page Down, Home and End scroll
/// it, as they scroll the lists of versions.
fn body_keys(shell: &mut Shell, ui: &egui::Ui, body: Rect) {
    let held = ui.memory(|m| m.focused());
    if held.is_some() && held == shell.panels.held && shell.panels.menu.is_none() && !ui.ctx().text_edit_focused() {
        crate::version_details::page_keys(ui, &mut shell.panels.scroll, (body.height() - ROW).max(ROW));
    }
}

/// After the body has drawn at the scroll `drawn`: a control of the body the keyboard has just
/// moved to, by Tab or any way but the pointer, is brought into view, so Tab goes on past the
/// fold. The toolkit hands the keyboard to a control that is laid out, in view or not.
fn follow_keys(shell: &mut Shell, ui: &egui::Ui, body: Rect, drawn: f32) {
    let now = ui.memory(|m| m.focused()).and_then(|id| Some((id, shell.registry.laid_out(id)?)));
    let was = std::mem::replace(&mut shell.panels.held, now.map(|(id, _)| id));
    let pointer = ui.input(|i| i.pointer.any_down() || i.pointer.any_released());
    if let Some((_, at)) = now.filter(|(id, _)| Some(*id) != was && !pointer) {
        // An editor brought into view this frame has moved the scroll already.
        shell.panels.scroll += into_view(body, at.translate(vec2(0.0, drawn - shell.panels.scroll)));
        ui.ctx().request_repaint();
    }
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

/// [`run_cx`] over the whole shell, for code not on the narrow context yet.
pub(crate) fn run(shell: &mut Shell, id: &str, params: serde_json::Value) -> Result<serde_json::Value, String> {
    shell.cx(|cx| run_cx(cx, id, params))
}

/// The string `prop` of `o`; empty when it has none.
pub(crate) fn string_of(store: &Store, o: Obj, prop: archforge_mof::PropId) -> String {
    store.get(o, prop).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default()
}

/// Drops a name or description being typed whose element is gone; the notations drop their own
/// editors ([`crate::notation_ui::NotationState::retain_valid`]).
pub fn retain_valid(state: &mut State, store: &Store) {
    if state.name.as_ref().is_some_and(|n| store.by_uuid(n.target).is_none()) {
        state.name = None;
    }
    if state.description.as_ref().is_some_and(|d| store.by_uuid(d.target).is_none()) {
        state.description = None;
    }
}

// Shared drawing.

/// [`section_cx`] over the whole shell, for code not on the narrow context yet.
pub(crate) fn section(shell: &mut Shell, ui: &egui::Ui, p: &Palette, r: Rect, y: f32, title: &str, add: Option<(&str, &str)>) -> Option<f32> {
    shell.cx(|cx| section_cx(cx, ui, p, r, y, title, add))
}

/// [`text_field_cx`] over the whole shell, for code not on the narrow context yet.
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
    shell.cx(|cx| text_field_cx(cx, ui, p, at, width, id, label, text, limit, take_focus, commit_on_blur))
}

/// [`edit_field_cx`] over the whole shell, for code not on the narrow context yet.
pub(crate) fn edit_field(
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
) -> FieldOut {
    shell.cx(|cx| edit_field_cx(cx, ui, p, at, width, id, label, text, limit, take_focus, commit_on_blur))
}

/// [`menu_cx`] over the whole shell, for code not on the narrow context yet.
pub(crate) fn menu(shell: &mut Shell, ui: &egui::Ui, p: &Palette, anchor: Rect, id: &str, items: &[String]) -> Option<usize> {
    shell.cx(|cx| menu_cx(cx, ui, p, anchor, id, items))
}

/// A type menu's choices kept between frames: whose and which state of it they were built for (a
/// revision means something only in its session's history, as another session of the same model,
/// a version shown or a replacement, starts its revisions anew), in which interface language (the
/// labels are translated), their labels, the values the commands take and the muted notes that
/// tell same-named ones apart, in the same order.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TypeChoices {
    pub project: Option<Uuid>,
    pub history: u64,
    pub revision: u64,
    pub language: &'static str,
    /// The classifier whose member is typed, whose template parameters the menu offers (AR-130).
    pub within: Option<Uuid>,
    pub labels: Vec<String>,
    pub values: Vec<String>,
    pub notes: Vec<String>,
    /// Each note's parts, a package's path by its names, folded from its front when its row is
    /// too narrow; empty for none.
    pub note_parts: Vec<Vec<String>>,
}

/// [`icon_button_cx`] over the whole shell, for code not on the narrow context yet.
pub(crate) fn icon_button(
    shell: &mut Shell,
    ui: &egui::Ui,
    p: &Palette,
    at: egui::Pos2,
    id: &str,
    icon: &ux::Icon,
    ink: ux::Colour,
    fill: Option<ux::Colour>,
    hint: &str,
) -> bool {
    shell.cx(|cx| icon_button_cx(cx, ui, p, at, id, icon, ink, fill, hint))
}

/// [`ghost_button_cx`] over the whole shell, for code not on the narrow context yet.
pub(crate) fn ghost_button(shell: &mut Shell, ui: &egui::Ui, p: &Palette, at: egui::Pos2, id: &str, icon: &ux::Icon, label: &str) -> bool {
    shell.cx(|cx| ghost_button_cx(cx, ui, p, at, id, icon, label))
}

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
                problems_section(shell, ui, p, r, y + 17.0 + FIELD + 16.0, o);
            }
        }
        // An element of a notation: its notation's sections.
        Some(_) => {
            if let Some(notation) = notation {
                shell.cx(|cx| notation.body(cx, ui, p, r, y, subject));
            }
        }
    }
}

/// [`problems_section_cx`] over the whole shell, for code not on the narrow context yet.
pub(crate) fn problems_section(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect, y: f32, subject: Obj) -> f32 {
    shell.cx(|cx| problems_section_cx(cx, ui, p, r, y, subject))
}

/// [`name_field_cx`] over the whole shell, for code not on the narrow context yet.
pub(crate) fn name_field(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, at: egui::Pos2, width: f32, subject: &Subject, cmd: &str, key: &str) {
    shell.cx(|cx| name_field_cx(cx, ui, p, at, width, subject, cmd, key))
}

/// The description of the element `obj` as its notation keeps it, or nothing.
pub(crate) fn description_now(shell: &Shell, obj: Obj) -> String {
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
            shell.panels.notice = Some(refused_on(shell, target, e));
        }
    }
}

/// [`refused_on_cx`] over the whole shell, for code not on the narrow context yet.
pub(crate) fn refused_on(shell: &Shell, target: Uuid, why: String) -> String {
    refused_on_cx(&shell.cx_ref(), target, why)
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
/// or when the keyboard leaves it, and when the selection moves on ([`flush_description`]); Escape
/// gives the typing up, a refused draft too, and the description as written comes back. It
/// shows what the element's notation keeps: an ER element's description, a UML element's
/// description Comment (AR-186).
fn description_field(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, at: egui::Pos2, width: f32, subject: &Subject) {
    let (Some(id), Some(obj)) = (subject.id, subject.obj) else { return };
    let current = description_now(shell, obj);
    let Some(limit) = shell.session.as_ref().map(|s| crate::notation_ui::description_limit(shell.notation_uis, s.doc(), obj)) else { return };
    let mut text = match &shell.panels.description {
        Some(d) if d.target == id => d.text.clone(),
        _ => current.clone(),
    };
    let out = edit_field(shell, ui, p, at, width, "workspace.properties.description", &tr("Description"), &mut text, limit, false, true);
    if out == FieldOut::Cancel {
        shell.panels.description = None;
    } else if out == FieldOut::Commit {
        write_description(shell, id, text);
    } else if text != current {
        // The refused mark stays while the text does; a change makes it a draft to write again.
        let refused = shell.panels.description.as_ref().is_some_and(|d| d.target == id && d.text == text && d.refused);
        shell.panels.description = Some(DescriptionDraft { target: id, text, refused });
    } else if shell.panels.description.as_ref().is_some_and(|d| d.target == id) {
        shell.panels.description = None;
    }
}

/// Closes every Properties editor, as Cancel would, except one opened this frame and not yet
/// shown: an editor belongs to the selection it was opened on. Every editor is a notation's
/// ([`crate::notation_ui::NotationState::close_editors`]).
fn close_editors(shell: &mut Shell) {
    shell.notation_states.close_editors(&mut shell.panels.menu);
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

/// What a skill aims at, as the Skills panel says it: the words of the notation among `uis` whose
/// kind it names ([`NotationUi::skill_target`](crate::notation_ui::NotationUi::skill_target));
/// where that notation has none, the skill's type pattern; nothing for a skill that names no kind.
pub(crate) fn target_word(uis: &[&'static dyn crate::notation_ui::NotationUi], store: &Store, s: Obj) -> String {
    let applies = string_of(store, s, archforge_core::skill::APPLIES_TO);
    let pattern = string_of(store, s, archforge_core::skill::TYPE_PATTERN);
    let Some((kind, _)) = archforge_core::skills::kind_by_key(&archforge_notations::view(), store, &applies) else { return String::new() };
    crate::notation_ui::of_class(uis, kind).and_then(|ui| ui.skill_target(kind, &pattern)).unwrap_or(pattern)
}

/// The selection's own skills and those it inherits, each as its row says it, the targets in the
/// words of the notations among `uis` ([`target_word`]).
fn skill_rows(uis: &[&'static dyn crate::notation_ui::NotationUi], store: &Store, subject: &Subject) -> (Vec<SkillRow>, Vec<SkillRow>) {
    let this = subject.this.clone();
    let subject = subject.obj;
    let own: Vec<SkillRow> = archforge_core::skills::skills(store)
        .into_iter()
        .filter(|s| archforge_core::skills::subject_of(store, *s) == subject)
        .filter_map(|s| {
            let target = target_word(uis, store, s);
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
        Some(s) => skill_rows(shell.notation_uis, s.doc(), subject),
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
    paint.galley(pos2(rect.right() - 8.0 - bin_w - detail.size().x, rect.top() + 8.0), detail, p.text_muted.into());
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
    ["Add skill"].into_iter()
}

// The panel.

/// Draws the panel body for `panel` under its header, from `y`: Properties and Skills follow the
/// selection; Chat holds the project's conversations; Appearance says what is true.
pub fn draw(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect, y: f32, panel: Panel, project: &str) {
    let paint = ui.painter().clone();
    let s = subject(shell, project);
    // A refusal is about what was selected: it goes with another selection. The drafts left on the
    // element selected before are written after, so a refusal of theirs shows, naming that element
    // ([`refused_on`]), until the selection moves on again.
    if shell.panels.notice_for != s.id {
        shell.panels.notice_for = s.id;
        shell.panels.notice = None;
    }
    // A description typed on the element selected before goes to it first.
    flush_description(shell, s.id);
    shell.cx(|cx| crate::uml_relate::finish_ends(cx, s.id));
    match panel {
        Panel::Agent => {
            crate::chat_view::draw(shell, ui, p, r, y);
            return;
        }
        Panel::Appearance => {
            crate::appearance::draw(shell, ui, p, r, y);
            return;
        }
        Panel::Properties | Panel::Skills => {}
    }
    // A template binding being drawn: its draft, nothing in the model yet (AR-131).
    if panel == Panel::Properties && shell.cx(|cx| crate::uml_bindings::draft_panel(cx, ui, p, r, y)) {
        return;
    }
    // Several elements, generalizations among them: what a set may be made of them (AR-129).
    if panel == Panel::Properties
        && let Some(all) = crate::uml_sets::selected(&shell.cx_ref()).filter(|all| all.len() >= 2)
    {
        shell.cx(|cx| crate::uml_sets::selection_body(cx, ui, p, r, y, &all));
        return;
    }
    // An n-ary association being drawn: its draft, not yet in the model (AR-128).
    if panel == Panel::Properties && shell.cx(|cx| crate::uml_nary::draft_body(cx, ui, p, r, y)) {
        return;
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
    // or selection starts at the top, an editor that opens or is refused is brought into view, the
    // wheel over the body moves it within its content.
    let body = Rect::from_min_max(pos2(r.left(), body_y), r.max);
    // A notation may show another element's page for its element (a column shows its table's), so
    // selecting one after the other in the same panel keeps the scroll ([`NotationUi::page`](crate::notation_ui::NotationUi::page)).
    let page = match (shell.session.as_ref(), s.obj) {
        (Some(session), Some(o)) => {
            let store = session.doc();
            match crate::notation_ui::of(shell.notation_uis, store, o) {
                Some(notation) => notation.page(store, o).and_then(|e| store.uuid(e).ok()),
                None => s.id,
            }
        }
        _ => s.id,
    };
    if shell.panels.scrolled_for != Some((panel, page)) {
        shell.panels.scrolled_for = Some((panel, page));
        shell.panels.scroll = 0.0;
    }
    let open = shell.notation_states.open_editors();
    if open != shell.panels.open_editors {
        shell.panels.reveal.extend(to_reveal(&open, &shell.panels.open_editors));
        shell.panels.open_editors = open;
    }
    let room = (shell.panels.content - body.height()).max(0.0);
    body_keys(shell, ui, body);
    shell.panels.scroll = shell.panels.scroll.clamp(0.0, room);
    shell.panels.body = Some(body);
    let drawn = shell.panels.scroll;
    shell.panels.drawn = drawn;
    let top = body_y - drawn;
    let mut clip = ui.new_child(egui::UiBuilder::new().max_rect(body));
    clip.set_clip_rect(body);
    shell.registry.reach_from(top);
    match panel {
        Panel::Properties => properties(shell, &mut clip, p, r, top, &s),
        Panel::Skills => skills(shell, &mut clip, p, r, top, &s),
        _ => {}
    }
    shell.registry.reach_end();
    shell.panels.content = (shell.registry.reach() - top + SCROLL_TAIL).max(0.0);
    shell.panels.reveal.clear();
    follow_keys(shell, ui, body, drawn);
    if ui.rect_contains_pointer(body) && shell.panels.menu.is_none() {
        let dy = ui.input(|i| i.smooth_scroll_delta.y);
        if dy != 0.0 {
            shell.panels.scroll = (shell.panels.scroll - dy).clamp(0.0, (shell.panels.content - body.height()).max(0.0));
        }
    }
    // Content that shrank under the scroll, as a card does when its refusal goes, is followed on the
    // next frame, not on the next input: until then the body would show past its end.
    if shell.panels.scroll > (shell.panels.content - body.height()).max(0.0) {
        ui.ctx().request_repaint();
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
        let area =
            ux::text(&paint, pos2(r.left() + PAD, r.bottom() - 36.0), &notice, types::CAPTION, p.amber, Some((r.width() - 2.0 * PAD).max(0.0)));
        widgets::announce(ui, &mut shell.registry, "workspace.properties.notice", area, &notice);
    }
}
