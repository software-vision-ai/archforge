//! A skill open in its own tab in the centre, beside the diagrams (the design's frame "Skills · A
//! skill open in its own tab, edited as Markdown"): the skill's name and where it sits, the
//! formatting toolbar, the Write / Preview switch, and its text. Write edits the Markdown source,
//! styled as it is typed (`crate::markdown`); Preview draws the page it makes. The text is kept
//! through `skill.set-body` once typing pauses, when the tab closes, on Save and before the
//! project is parked or the window quits; each keeping is one undo step. Above the text sit the
//! skill's settings: its name as the page's title, what it applies to and when; each is kept as
//! it is chosen. The Skills panel lists the skills; its + makes one and opens it here, its name
//! ready to type.

use archforge_mof::{Obj, Store, Uuid};
use egui::{Rect, Stroke, StrokeKind, Vec2, pos2, vec2};
use serde_json::json;

use crate::home::control;
use crate::i18n::{fmt, tr};
use crate::markdown::{self, Format};
use crate::panels::{Applies, Menu};
use crate::ux::{self, Palette, icons, types};
use crate::{Shell, widgets};

/// How long typing pauses before the text is kept, in seconds.
const IDLE: f64 = 0.8;
/// The page's margins and its column's widest.
const MARGIN: f32 = 56.0;
const COLUMN: f32 = 760.0;
/// A toolbar button's side and the gap after it.
const BUTTON: f32 = 28.0;
const GAP: f32 = 8.0;

/// One skill's text while its tab is open.
#[derive(Debug, Clone, PartialEq)]
pub struct Doc {
    pub skill: Uuid,
    /// The text as typed, and as the store last had it.
    pub text: String,
    pub saved: String,
    /// When the text last changed, in the window's seconds.
    pub changed_at: f64,
    /// The rendered page instead of the source.
    pub preview: bool,
    /// The selection while the text had the keyboard last, in chars: the toolbar's buttons take
    /// the keyboard away, and the editor drops its selection with it.
    pub selection: Option<(usize, usize)>,
    /// The text the store refused, and why; kept until the text changes.
    pub refused: Option<(String, String)>,
    /// Why the last toolbar edit was not made; cleared by the next change.
    pub notice: Option<String>,
    /// A link clicked in Preview, waiting for the confirmation to open it.
    pub link: Option<String>,
    /// The name, column type and dialect as typed in their fields.
    pub name: String,
    pub type_pattern: String,
    pub dialect: String,
    /// The name takes the keyboard, all of it selected, at the next frame: a skill just made.
    pub focus_name: bool,
    /// Why the last change of a setting was refused; cleared by the next one kept.
    pub settings_refused: Option<String>,
}

impl Doc {
    fn pending(&self) -> bool {
        self.text != self.saved && self.refused.as_ref().is_none_or(|(t, _)| *t != self.text)
    }
}

/// Whether `id` names a live skill.
pub fn is_skill(store: &Store, id: Uuid) -> bool {
    skill_obj(store, id).is_some()
}

fn skill_obj(store: &Store, id: Uuid) -> Option<Obj> {
    let s = store.by_uuid(id)?;
    (store.is_alive(s) && store.class_of(s).ok() == Some(archforge_core::SKILL)).then_some(s)
}

fn body_of(store: &Store, s: Obj) -> String {
    store.get(s, archforge_core::skill::BODY).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default()
}

/// Opens a skill in its tab, added when it has none.
pub fn open(shell: &mut Shell, id: Uuid) {
    let Some(store) = shell.session.as_ref().map(|s| s.doc()) else { return };
    let Some(s) = skill_obj(store, id) else { return };
    if !shell.canvas.skills.iter().any(|d| d.skill == id) {
        let body = body_of(store, s);
        let set = settings(store, s);
        shell.canvas.skills.push(Doc {
            name: set.name,
            type_pattern: set.type_pattern,
            dialect: set.dialect,
            focus_name: false,
            settings_refused: None,
            skill: id,
            text: body.clone(),
            saved: body,
            changed_at: 0.0,
            preview: false,
            selection: None,
            refused: None,
            notice: None,
            link: None,
        });
    }
    if !shell.canvas.tabs.contains(&id) {
        shell.canvas.tabs.push(id);
    }
    shell.canvas.active = Some(id);
    shell.canvas.notice = None;
    shell.canvas.focused = false;
}

/// Makes a skill on `subject` (the model when none) under a fresh name and opens it in its tab,
/// the name selected for typing.
pub fn create(shell: &mut Shell, subject: Option<Uuid>) {
    let Some(session) = shell.session.as_mut() else { return };
    let store = session.doc();
    let taken = archforge_core::skills::skills(store)
        .into_iter()
        .filter(|s| archforge_core::skills::subject_of(store, *s).and_then(|o| store.uuid(o).ok()) == subject)
        .map(|s| archforge_core::skills::name_of(store, s));
    let name = crate::explorer::fresh_name(&tr("Skill"), taken, false).unwrap_or_default();
    let subject_id = subject.map(|s| s.to_string()).unwrap_or_default();
    match session.execute("skill.add", &json!({ "subject": subject_id, "name": name })) {
        Ok(out) => {
            // skill.add selects the new skill; the panel stays on the element it sits on.
            let ids: Vec<String> = subject.into_iter().map(|s| s.to_string()).collect();
            let _ = session.execute("view.select", &json!({ "ids": ids }));
            let Some(id) = out.value["id"].as_str().and_then(Uuid::parse) else { return };
            crate::canvas::open(shell, id);
            if let Some(doc) = shell.canvas.skills.iter_mut().find(|d| d.skill == id) {
                doc.focus_name = true;
            }
        }
        Err(e) => shell.canvas.notice = Some(e.to_string()),
    }
}

/// A skill's settings, as its tab shows and writes them.
struct Settings {
    name: String,
    description: String,
    body: String,
    triggers: [bool; 4],
    dialect: String,
    applies: Applies,
    type_pattern: String,
}

/// The triggers in the order of the When chips, with their automation keys, icons and words.
const TRIGGERS: [(archforge_core::SkillTrigger, &str, &ux::Icon, &str); 4] = [
    (archforge_core::SkillTrigger::Modelling, "modelling", &icons::TABLE, "Modelling"),
    (archforge_core::SkillTrigger::Drawing, "drawing", &icons::BRUSH, "Drawing"),
    (archforge_core::SkillTrigger::Naming, "naming", &icons::NAMING, "Naming"),
    (archforge_core::SkillTrigger::Export, "export", &icons::EXPORT, "Export"),
];

fn string(store: &Store, s: Obj, prop: archforge_mof::PropId) -> String {
    store.get(s, prop).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default()
}

fn settings(store: &Store, s: Obj) -> Settings {
    use archforge_core::skill;
    let on = archforge_core::skills::triggers_of(store, s);
    let applies = string(store, s, skill::APPLIES_TO);
    Settings {
        name: archforge_core::skills::name_of(store, s),
        description: string(store, s, skill::DESCRIPTION),
        body: body_of(store, s),
        triggers: TRIGGERS.map(|(t, ..)| on.contains(&t)),
        dialect: string(store, s, skill::DIALECT),
        applies: match archforge_core::skills::kind_by_key::<archforge_er::ErView>(store, &applies).map(|(_, w)| w) {
            Some("table") => Applies::Tables,
            Some("relationship") => Applies::Relationships,
            Some(_) => Applies::Columns,
            None => Applies::Subject,
        },
        type_pattern: string(store, s, skill::TYPE_PATTERN),
    }
}

/// Writes a skill's settings through `skill.set`; a dialect only with Export, a column type only
/// for columns.
fn write_settings(shell: &mut Shell, id: Uuid, f: &Settings) -> Result<(), String> {
    let session = shell.session.as_mut().ok_or_else(|| tr("The model could not be opened"))?;
    let triggers: Vec<&str> = TRIGGERS.iter().zip(f.triggers).filter(|(_, on)| *on).map(|((t, ..), _)| t.literal()).collect();
    let applies_to = match f.applies {
        Applies::Subject => "",
        Applies::Tables => "table",
        Applies::Relationships => "relationship",
        Applies::Columns => "column",
    };
    let params = json!({
        "skill": id.to_string(), "name": f.name, "description": f.description, "body": f.body, "triggers": triggers,
        "dialect": if f.triggers[3] { f.dialect.trim() } else { "" },
        "applies_to": applies_to,
        "type_pattern": if f.applies == Applies::Columns { f.type_pattern.trim() } else { "" },
    });
    session.execute("skill.set", &params).map(|_| ()).map_err(|e| e.to_string())
}

/// Changes one setting of the skill and keeps it, or says why not.
fn change(shell: &mut Shell, i: usize, s: Obj, edit: impl FnOnce(&mut Settings)) {
    let Some(store) = shell.session.as_ref().map(|x| x.doc()) else { return };
    let mut f = settings(store, s);
    edit(&mut f);
    let id = shell.canvas.skills[i].skill;
    shell.canvas.skills[i].settings_refused = write_settings(shell, id, &f).err();
}

/// "This table" and the like: the word for the element the skill sits on.
fn this_word(store: &Store, subject: Option<Obj>) -> String {
    let Some(o) = subject else { return tr("This model") };
    match store.class_of(o).ok() {
        Some(c) if c == archforge_er::SCHEMA => tr("This schema"),
        Some(c) if c == archforge_er::ENTITY => tr("This table"),
        Some(c) if c == archforge_er::ATTRIBUTE => tr("This column"),
        Some(c) if c == archforge_er::RELATIONSHIP => tr("This relationship"),
        Some(c) if c == archforge_er::DOMAIN => tr("This domain"),
        Some(c) if c == archforge_er::ENUM_TYPE => tr("This enum type"),
        Some(c) if c == archforge_er::VIEW => tr("This view"),
        Some(c) if c == archforge_er::SEQUENCE => tr("This sequence"),
        _ => tr("This element"),
    }
}

/// Keeps every open skill's text the store does not have yet: the ones whose typing paused, or
/// all of them when `force`, or only `only`'s when given. A text the store took back, through
/// undo or another editor, replaces one with nothing waiting.
pub fn sync(shell: &mut Shell, now: f64, force: bool, only: Option<Uuid>) {
    let Some(session) = shell.session.as_mut() else { return };
    for doc in shell.canvas.skills.iter_mut().filter(|d| only.is_none_or(|o| o == d.skill)) {
        let Some(s) = skill_obj(session.doc(), doc.skill) else { continue };
        if doc.pending() && (force || only.is_some() || now - doc.changed_at >= IDLE) {
            match session.execute("skill.set-body", &json!({ "skill": doc.skill.to_string(), "body": doc.text })) {
                Ok(_) => {
                    doc.saved = doc.text.clone();
                    doc.refused = None;
                }
                Err(e) => doc.refused = Some((doc.text.clone(), e.to_string())),
            }
        } else if doc.text == doc.saved {
            let body = body_of(session.doc(), s);
            if body != doc.saved {
                doc.text = body.clone();
                doc.saved = body;
            }
        }
    }
}

/// The open skills' texts the store would not take, for the log at exit: how many and how long.
pub fn unkept(state: &crate::canvas::State) -> Vec<(Uuid, usize)> {
    state.skills.iter().filter(|d| d.text != d.saved).map(|d| (d.skill, d.text.chars().count())).collect()
}

/// Why a project cannot close while a skill's text in it is refused.
pub fn unkept_reason() -> String {
    tr("A skill's text could not be kept; its tab says why.")
}

/// Whether some open skill has text waiting to be kept.
pub fn waiting(shell: &Shell) -> bool {
    shell.canvas.skills.iter().any(Doc::pending)
}

/// Drops the texts of skills whose tab closed.
pub fn retain_valid(state: &mut crate::canvas::State) {
    let tabs = state.tabs.clone();
    state.skills.retain(|d| tabs.contains(&d.skill));
}

/// Where the skill sits, as the line under its name.
fn place(store: &Store, s: Obj) -> String {
    match archforge_core::skills::subject_of(store, s) {
        Some(o) => fmt(&tr("Skill · on {subject}"), &[("subject", &archforge_er::rules::name_of(store, o))]),
        None => tr("Skill · on the project"),
    }
}

/// The settings above the text: the name as the title with the bin at its right, where the skill
/// sits, what it applies to and when. Gives the y under them.
#[allow(clippy::too_many_arguments)]
fn header(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, left: f32, width: f32, mut y: f32, i: usize, s: Obj) -> f32 {
    let paint = ui.painter().clone();
    let ctx = ui.ctx().clone();
    let Some(store) = shell.session.as_ref().map(|x| x.doc()) else { return y };
    let set = settings(store, s);
    let place = place(store, s);
    let this = this_word(store, archforge_core::skills::subject_of(store, s));
    let id = shell.canvas.skills[i].skill;

    // The name, typed in place as the title.
    let name_id = widgets::egui_id("workspace.skill.name");
    let typing = ctx.memory(|m| m.has_focus(name_id));
    if !typing && !shell.canvas.skills[i].focus_name {
        shell.canvas.skills[i].name = set.name.clone();
    }
    let field = Rect::from_min_size(pos2(left - 8.0, y), vec2((width - 28.0).max(0.0), 40.0));
    widgets::record_visible(ui, &mut shell.registry, "workspace.skill.name", "field", field);
    if typing {
        paint.rect_filled(field, 6.0, p.field);
        paint.rect_stroke(field, 6.0, Stroke::new(1.0, p.focus), StrokeKind::Inside);
    } else if ui.rect_contains_pointer(field) {
        paint.rect_filled(field, 6.0, p.field);
    }
    if std::mem::take(&mut shell.canvas.skills[i].focus_name) {
        ctx.memory_mut(|m| m.request_focus(name_id));
        let mut state = egui::text_edit::TextEditState::load(&ctx, name_id).unwrap_or_default();
        let all = shell.canvas.skills[i].name.chars().count();
        state.cursor.set_char_range(Some(egui::text::CCursorRange::two(egui::text::CCursor::new(0), egui::text::CCursor::new(all))));
        state.store(&ctx, name_id);
    }
    let inner = Rect::from_min_max(field.min + vec2(8.0, 6.0), field.max - vec2(8.0, 6.0));
    let mut name = shell.canvas.skills[i].name.clone();
    let out = egui::TextEdit::singleline(&mut name)
        .id(name_id)
        .frame(egui::Frame::NONE)
        .margin(egui::Margin::ZERO)
        .font(types::SECTION.font())
        .text_color(p.text)
        .char_limit(archforge_core::skills::MAX_NAME)
        .hint_text(tr("Skill name"))
        .desired_width(inner.width())
        .show(&mut ui.new_child(egui::UiBuilder::new().max_rect(inner)));
    let _ = (*out.response).clone().accessible_name(tr("Skill name"));
    shell.canvas.skills[i].name = name.clone();
    let escape = ctx.input(|inp| inp.key_pressed(egui::Key::Escape));
    let enter = out.response.has_focus() && ctx.input(|inp| inp.key_pressed(egui::Key::Enter));
    if (out.response.lost_focus() || enter) && !escape && name.trim() != set.name {
        let outcome = shell.session.as_mut().map(|x| x.execute("skill.rename", &json!({ "skill": id.to_string(), "name": name.trim() })));
        shell.canvas.skills[i].settings_refused = match outcome {
            Some(Err(e)) => Some(e.to_string()),
            _ => None,
        };
    } else if escape && out.response.lost_focus() {
        shell.canvas.skills[i].name = set.name.clone();
    }
    if crate::panels::icon_button(shell, ui, p, pos2(left + width - 28.0, y + 6.0), "workspace.skill.remove", &icons::TRASH, p.danger, None, "Remove")
    {
        if let Some(Err(e)) = shell.session.as_mut().map(|x| x.execute("skill.remove", &json!({ "skill": id.to_string() }))) {
            shell.canvas.skills[i].settings_refused = Some(e.to_string());
        }
        return y;
    }
    y += 44.0;
    ux::text(&paint, pos2(left, y), &place, types::CAPTION, p.text_muted, Some(width));
    y += 26.0;

    // What it applies to, and a column type for columns.
    let anchor = crate::panels::select_field(
        shell,
        ui,
        p,
        pos2(left, y),
        240.0,
        "workspace.skill.applies",
        &tr("Applies to"),
        &crate::panels::applies_word(set.applies, &this),
        Menu::Applies,
    );
    if set.applies == Applies::Columns {
        let type_id = widgets::egui_id("workspace.skill.type");
        if !ctx.memory(|m| m.has_focus(type_id)) {
            shell.canvas.skills[i].type_pattern = set.type_pattern.clone();
        }
        let mut typed = shell.canvas.skills[i].type_pattern.clone();
        let done = crate::panels::text_field(
            shell,
            ui,
            p,
            pos2(left + 256.0, y),
            200.0,
            "workspace.skill.type",
            &tr("Column type"),
            &mut typed,
            archforge_core::skills::MAX_TYPE_PATTERN,
            false,
            true,
        );
        shell.canvas.skills[i].type_pattern = typed.clone();
        if done && typed.trim() != set.type_pattern {
            change(shell, i, s, |f| f.type_pattern = typed);
        }
    }
    y += 55.0;

    // When it applies: a chip per trigger, none on meaning always.
    ux::text(&paint, pos2(left, y), &tr("When"), types::CAPTION, p.text_secondary, Some(width));
    let mut x = left;
    let mut flip: Option<usize> = None;
    for (n, (_, key, icon, word)) in TRIGGERS.iter().enumerate() {
        let on = set.triggers[n];
        let g = ux::galley(&paint, &tr(word), types::SMALL, if on { p.accent } else { p.text }, None);
        let chip = Rect::from_min_size(pos2(x, y + 17.0), vec2(10.0 + 16.0 + 6.0 + g.size().x + 12.0, crate::panels::FIELD));
        let resp = control(shell, ui, &format!("workspace.skill.when.{key}"), "toggle", chip, true);
        paint.rect_filled(
            chip,
            8.0,
            if on {
                p.accent_tint
            } else if resp.hovered() {
                p.page
            } else {
                p.surface
            },
        );
        if !on {
            paint.rect_stroke(chip, 8.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
        }
        ux::icon_in(
            &paint,
            Rect::from_min_size(pos2(chip.left() + 10.0, chip.center().y - 8.0), Vec2::splat(16.0)),
            icon,
            if on { p.accent } else { p.text_secondary },
        );
        paint.galley(pos2(chip.left() + 32.0, chip.center().y - g.size().y / 2.0), g, p.text);
        if resp.clicked() {
            flip = Some(n);
        }
        x = chip.right() + 8.0;
    }
    if set.triggers[3] {
        let dialect_id = widgets::egui_id("workspace.skill.dialect");
        if !ctx.memory(|m| m.has_focus(dialect_id)) {
            shell.canvas.skills[i].dialect = set.dialect.clone();
        }
        let mut typed = shell.canvas.skills[i].dialect.clone();
        let done = crate::panels::text_field(
            shell,
            ui,
            p,
            pos2(x + 8.0, y),
            (left + width - x - 8.0).clamp(0.0, 180.0),
            "workspace.skill.dialect",
            &tr("Dialect"),
            &mut typed,
            archforge_core::skills::MAX_DIALECT,
            false,
            true,
        );
        shell.canvas.skills[i].dialect = typed.clone();
        if done && typed.trim() != set.dialect {
            change(shell, i, s, |f| f.dialect = typed);
        }
    } else if !set.triggers.contains(&true) {
        ux::text_mid(&paint, x + 4.0, y + 17.0 + crate::panels::FIELD / 2.0, &tr("None chosen: always"), types::CAPTION, p.text_muted, None);
    }
    if let Some(n) = flip {
        change(shell, i, s, |f| f.triggers[n] = !f.triggers[n]);
    }
    y += 55.0;
    if let Some(why) = shell.canvas.skills[i].settings_refused.clone() {
        y = ux::text(&paint, pos2(left, y), &why, types::CAPTION, p.amber, Some(width)).bottom() + 6.0;
    }
    if shell.panels.menu == Some(Menu::Applies) {
        let choices = [Applies::Subject, Applies::Tables, Applies::Relationships, Applies::Columns];
        let items = choices.map(|a| crate::panels::applies_word(a, &this));
        if let Some(k) = crate::panels::menu(shell, ui, p, anchor, "workspace.skill.applies", &items) {
            change(shell, i, s, |f| f.applies = choices[k.min(3)]);
        }
    }
    y
}

/// Draws the open skill over `r`, the centre under the tab strip.
pub fn draw(shell: &mut Shell, ui: &mut egui::Ui, p: &Palette, r: Rect, id: Uuid) {
    let paint = ui.painter().clone();
    paint.rect_filled(r, 0.0, p.surface);
    shell.canvas.focused = false;
    let Some(store) = shell.session.as_ref().map(|s| s.doc()) else { return };
    let Some(s) = skill_obj(store, id) else { return };
    let Some(i) = shell.canvas.skills.iter().position(|d| d.skill == id) else { return };
    let left = r.left() + MARGIN;
    let width = (r.width() - 2.0 * MARGIN).clamp(0.0, COLUMN);
    let mut y = header(shell, ui, p, left, width, r.top() + 28.0, i, s) + 6.0;
    if shell.session.as_ref().is_none_or(|x| skill_obj(x.doc(), id).is_none()) {
        return;
    }

    // The toolbar, and the Write / Preview switch at its right.
    let editor_id = widgets::egui_id("workspace.skill.text");
    let preview = shell.canvas.skills[i].preview;
    let mut pressed: Option<Format> = None;
    let mut x = left;
    for (n, (f, key, hint)) in Format::ALL.iter().enumerate() {
        if n == 3 {
            paint.vline(x + 1.0, (y + 6.0)..=(y + BUTTON - 6.0), Stroke::new(1.0, p.border));
            x += 10.0;
        }
        let b = Rect::from_min_size(pos2(x, y), Vec2::splat(BUTTON));
        let resp = control(shell, ui, &format!("workspace.skill.format.{key}"), "button", b, !preview).on_hover_text(tr(hint));
        if resp.hovered() && !preview {
            paint.rect_filled(b, 6.0, p.page);
        }
        glyph(&paint, b, *f, if preview { p.text_muted } else { p.text_secondary });
        if resp.clicked() {
            pressed = Some(*f);
        }
        x += BUTTON + GAP;
    }
    let switch = Rect::from_min_size(pos2(left + width - 172.0, y - 1.0), vec2(172.0, 30.0));
    paint.rect_filled(switch, 8.0, p.page);
    let mut choose: Option<bool> = None;
    for (n, (key, word)) in [("write", "Write"), ("preview", "Preview")].iter().enumerate() {
        let half = Rect::from_min_size(pos2(switch.left() + 2.0 + n as f32 * 84.0, switch.top() + 2.0), vec2(84.0, 26.0));
        let on = preview == (n == 1);
        if on {
            paint.rect_filled(half, 6.0, p.surface);
            paint.rect_stroke(half, 6.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
        }
        let g = ux::galley(&paint, &tr(word), types::SMALL, if on { p.text } else { p.text_secondary }, Some(80.0));
        paint.galley(half.center() - g.size() / 2.0, g, p.text);
        if control(shell, ui, &format!("workspace.skill.{key}"), "toggle", half, true).clicked() {
            choose = Some(n == 1);
        }
    }
    y += BUTTON + 12.0;
    paint.hline(left..=left + width, y, Stroke::new(1.0, p.border));
    y += 16.0;
    if let Some(v) = choose {
        shell.canvas.skills[i].preview = v;
    }
    let doc = &shell.canvas.skills[i];
    let refused = doc.refused.as_ref().filter(|(t, _)| *t == doc.text).map(|(_, why)| why.clone()).or_else(|| doc.notice.clone());
    if let Some(why) = &refused {
        y = ux::text(&paint, pos2(left, y), why, types::CAPTION, p.amber, Some(width)).bottom() + 8.0;
    }
    let page = Rect::from_min_max(pos2(left, y), pos2(left + width, r.bottom()));
    if page.height() <= 0.0 || page.width() <= 0.0 {
        return;
    }
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(page).id_salt(("workspace.skill.page", id)));
    child.set_clip_rect(page.intersect(r));
    if shell.canvas.skills[i].preview {
        let now = ui.input(|inp| inp.time);
        sync(shell, now, true, Some(id));
        let text = shell.canvas.skills[i].text.clone();
        widgets::record_visible(ui, &mut shell.registry, "workspace.skill.page", "text", page);
        if !markdown::within_budget(&text) {
            let why = tr("This text is too long or nested too deeply to preview.");
            ux::text(&paint, page.min, &why, types::BODY, p.text_secondary, Some(page.width()));
            return;
        }
        preview_style(child.style_mut(), p);
        let before = ui.ctx().output(|o| o.commands.len());
        egui::ScrollArea::vertical().id_salt(("workspace.skill.preview", id)).auto_shrink([false, false]).show(&mut child, |ui| {
            egui_commonmark::CommonMarkViewer::new().show(ui, &mut shell.markdown, &text);
        });
        // A link the page asks to open waits for the confirmation; only a web or mail address
        // that shows what it is gets that far.
        let asked: Vec<String> = ui.ctx().output_mut(|o| {
            let mut asked = Vec::new();
            let mut n = 0;
            o.commands.retain(|c| {
                n += 1;
                match c {
                    egui::OutputCommand::OpenUrl(u) if n > before => {
                        asked.push(u.url.clone());
                        false
                    }
                    _ => true,
                }
            });
            asked
        });
        if let Some(url) = asked.iter().find_map(|u| openable(u)) {
            shell.canvas.skills[i].link = Some(url);
        }
        confirm_link(shell, ui, p, page, i);
        return;
    }
    write(shell, ui, &mut child, p, page, i, editor_id, pressed);
}

/// The page's look in Preview: the shell's faces and colours in place of egui's. The viewer sizes
/// its headings between Body and Heading, and marks strong text by its colour alone.
fn preview_style(style: &mut egui::Style, p: &Palette) {
    use egui::{FontId, TextStyle};
    style.text_styles.insert(TextStyle::Body, types::BODY.font());
    style.text_styles.insert(TextStyle::Heading, FontId::new(22.0, types::BODY_STRONG.font().family));
    style.text_styles.insert(TextStyle::Monospace, FontId::new(13.0, egui::FontFamily::Monospace));
    style.spacing.item_spacing = vec2(8.0, 8.0);
    let v = &mut style.visuals;
    v.widgets.noninteractive.fg_stroke.color = p.text_secondary;
    v.widgets.active.fg_stroke.color = p.text;
    v.weak_text_color = Some(p.text_muted);
    v.hyperlink_color = p.accent;
    v.code_bg_color = p.page;
    v.widgets.noninteractive.bg_stroke.color = p.border;
    // A link's target shows on hover, whatever its words say.
    style.url_in_tooltip = true;
}

/// The link to offer to open, or none: a web address with a host and no user part before it, or a
/// mail address whose only fields are subject, body, cc and bcc, with no hidden character or
/// space anywhere in it.
fn openable(url: &str) -> Option<String> {
    let url = url.trim();
    if url.is_empty() || url.chars().any(|c| archforge_core::skills::hidden(c) || c.is_whitespace()) {
        return None;
    }
    let lower = url.to_ascii_lowercase();
    if let Some(rest) = lower.strip_prefix("https://").or_else(|| lower.strip_prefix("http://")) {
        let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
        return (!authority.is_empty() && !authority.contains('@')).then(|| url.to_string());
    }
    if let Some(rest) = lower.strip_prefix("mailto:") {
        let fields = rest.split_once('?').map_or("", |(_, q)| q);
        let known = fields.split('&').filter(|f| !f.is_empty()).all(|f| {
            let key = f.split('=').next().unwrap_or_default();
            matches!(key, "subject" | "body" | "cc" | "bcc")
        });
        return known.then(|| url.to_string());
    }
    None
}

/// The site a link leads to, as the confirmation names it: a web address's host, a mail address.
fn site_of(url: &str) -> String {
    let lower = url.to_ascii_lowercase();
    if let Some(rest) = lower.strip_prefix("https://").or_else(|| lower.strip_prefix("http://")) {
        return rest.split(['/', '?', '#']).next().unwrap_or_default().to_string();
    }
    let rest = lower.strip_prefix("mailto:").unwrap_or(&lower);
    rest.split('?').next().unwrap_or_default().to_string()
}

/// The confirmation over the page before a link opens: the address in full, Cancel and Open.
/// Esc cancels; nothing opens without the click on Open.
fn confirm_link(shell: &mut Shell, ui: &egui::Ui, p: &Palette, page: Rect, i: usize) {
    let Some(url) = shell.canvas.skills[i].link.clone() else { return };
    let width = page.width().clamp(0.0, 420.0);
    let inner = (width - 32.0).max(0.0);
    // The site on a line of its own, its end kept when it is long, since the end names it; then the
    // whole address over up to three rows.
    let site = site_of(&url);
    let site = if site.chars().count() > 44 { format!("…{}", site.chars().skip(site.chars().count() - 43).collect::<String>()) } else { site };
    let paint = ui.painter();
    let site_g = ux::galley(paint, &site, types::UI_STRONG, p.text, Some(inner));
    let mut job = egui::text::LayoutJob::single_section(
        url.clone(),
        egui::text::TextFormat { font_id: types::SMALL.font(), color: p.text_secondary, line_height: Some(types::SMALL.line), ..Default::default() },
    );
    job.wrap = egui::text::TextWrapping { max_width: inner, max_rows: 3, break_anywhere: true, overflow_character: Some('…') };
    let url_g = paint.layout_job(job);
    let height = 16.0 + 24.0 + site_g.size().y + 4.0 + url_g.size().y + 14.0 + crate::panels::FIELD + 16.0;
    let card = Rect::from_min_size(pos2(page.center().x - width / 2.0, page.top() + 24.0), vec2(width, height));
    let area = egui::Area::new(widgets::egui_id(&format!("workspace.skill.link.{}", shell.canvas.skills[i].skill)))
        .order(egui::Order::Foreground)
        .fixed_pos(card.min)
        .show(ui.ctx(), |ui| {
            // The card takes every click on it, so none reaches a link under it.
            let _ = ui.interact(card, widgets::egui_id("workspace.skill.link.card"), egui::Sense::click());
            let paint = ui.painter().clone();
            paint.rect_filled(card, 8.0, p.surface);
            paint.rect_stroke(card, 8.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
            ux::text(&paint, card.min + vec2(16.0, 16.0), &tr("Open this link?"), types::BODY_STRONG, p.text, Some(inner));
            let mut y = card.top() + 40.0;
            let h = site_g.size().y;
            paint.galley(pos2(card.left() + 16.0, y), site_g, p.text);
            y += h + 4.0;
            paint.galley(pos2(card.left() + 16.0, y), url_g, p.text_secondary);
            let y = card.bottom() - 16.0 - crate::panels::FIELD;
            let cancel = crate::panels::icon_button(
                shell,
                ui,
                p,
                pos2(card.right() - 16.0 - 2.0 * crate::panels::FIELD - 8.0, y),
                "workspace.skill.link.cancel",
                &icons::CLOSE,
                p.text_secondary,
                None,
                "Cancel",
            );
            let open = crate::panels::icon_button(
                shell,
                ui,
                p,
                pos2(card.right() - 16.0 - crate::panels::FIELD, y),
                "workspace.skill.link.open",
                &icons::TICK,
                p.on_accent,
                Some(p.accent_fill),
                "Open link",
            );
            (cancel, open)
        });
    let (cancel, open) = area.inner;
    if open {
        ui.ctx().open_url(egui::OpenUrl::new_tab(&url));
    }
    if open || cancel || ui.input(|inp| inp.key_pressed(egui::Key::Escape)) {
        shell.canvas.skills[i].link = None;
    }
}

/// The source, styled as it is typed, with the toolbar's edit and its shortcuts applied.
#[allow(clippy::too_many_arguments)]
fn write(shell: &mut Shell, ui: &egui::Ui, child: &mut egui::Ui, p: &Palette, page: Rect, i: usize, editor_id: egui::Id, pressed: Option<Format>) {
    let ctx = ui.ctx().clone();
    let focused = ctx.memory(|m| m.has_focus(editor_id));
    let shortcut = if focused {
        ctx.input_mut(|inp| {
            [(egui::Key::B, Format::Bold), (egui::Key::I, Format::Italic), (egui::Key::K, Format::Link)]
                .into_iter()
                .find(|(k, _)| inp.consume_key(egui::Modifiers::COMMAND, *k))
                .map(|(_, f)| f)
        })
    } else {
        None
    };
    let doc = &mut shell.canvas.skills[i];
    if let Some(f) = pressed.or(shortcut) {
        let mut state = egui::text_edit::TextEditState::load(&ctx, editor_id).unwrap_or_default();
        let len = doc.text.chars().count();
        let (a, b) = doc.selection.map_or((len, len), |(a, b)| (a.min(len), b.min(len)));
        let mut text = doc.text.clone();
        let sel = markdown::apply(&mut text, a..b, f);
        doc.notice = None;
        if text.chars().count() > archforge_core::skills::MAX_BODY {
            let n = archforge_core::skills::MAX_BODY.to_string();
            doc.notice = Some(fmt(&tr("A skill's text is at most {n} characters."), &[("n", &n)]));
        } else {
            doc.text = text;
            doc.changed_at = ctx.input(|inp| inp.time);
            let range = egui::text::CCursorRange::two(egui::text::CCursor::new(sel.start), egui::text::CCursor::new(sel.end));
            state.cursor.set_char_range(Some(range));
            state.store(&ctx, editor_id);
            doc.selection = Some((sel.start, sel.end));
        }
        ctx.memory_mut(|m| m.request_focus(editor_id));
    }
    let mut layouter = |ui: &egui::Ui, buf: &dyn egui::TextBuffer, wrap: f32| {
        let job = markdown::layout_job(buf.as_str(), p, wrap);
        ui.fonts_mut(|f| f.layout_job(job))
    };
    let before = doc.text.clone();
    let out = egui::ScrollArea::vertical()
        .id_salt(("workspace.skill.scroll", doc.skill))
        .auto_shrink([false, false])
        .show(child, |ui| {
            egui::TextEdit::multiline(&mut doc.text)
                .id(editor_id)
                .frame(egui::Frame::NONE)
                .char_limit(archforge_core::skills::MAX_BODY)
                .desired_width(page.width())
                .desired_rows(12)
                .hint_text(tr("Write the guidance in Markdown"))
                .layouter(&mut layouter)
                .show(ui)
        })
        .inner;
    let _ = (*out.response).clone().accessible_name(tr("Skill text"));
    if doc.text != before {
        doc.changed_at = ctx.input(|inp| inp.time);
        doc.notice = None;
    }
    if out.response.has_focus()
        && let Some(range) = out.cursor_range
    {
        let (x, y) = (range.primary.index.0, range.secondary.index.0);
        doc.selection = Some((x.min(y), x.max(y)));
    }
    let lost = out.response.lost_focus();
    let id = doc.skill;
    widgets::record_visible(ui, &mut shell.registry, "workspace.skill.text", "field", page);
    if lost {
        sync(shell, ctx.input(|inp| inp.time), true, Some(id));
    }
}

/// A toolbar button's mark: a letter in the look it gives, or an icon.
fn glyph(paint: &egui::Painter, b: Rect, f: Format, ink: egui::Color32) {
    let letter = |text: &str, strong: bool, italic: bool| {
        let ty = if strong { types::UI_STRONG } else { types::UI };
        let mut job = egui::text::LayoutJob::default();
        job.append(text, 0.0, egui::text::TextFormat { font_id: ty.font(), color: ink, italics: italic, ..Default::default() });
        let g = paint.layout_job(job);
        paint.galley(b.center() - g.size() / 2.0, g, ink);
    };
    match f {
        Format::Bold => letter("B", true, false),
        Format::Italic => letter("I", false, true),
        Format::Heading => letter("H", true, false),
        Format::Bulleted => ux::icon_in(paint, Rect::from_center_size(b.center(), Vec2::splat(16.0)), &icons::LIST, ink),
        Format::Numbered => letter("1.", false, false),
        Format::Code => letter("</>", false, false),
        Format::Link => ux::icon_in(paint, Rect::from_center_size(b.center(), vec2(14.0, 7.0)), &icons::LINK, ink),
        Format::Quote => letter("\u{201C}", true, false),
    }
}

/// Every text that reaches `tr` through a variable, for the catalogue tests.
#[cfg(test)]
pub(crate) fn texts() -> impl Iterator<Item = &'static str> {
    Format::ALL.iter().map(|(_, _, hint)| *hint).chain(["Write", "Preview", "Open link", "Remove"]).chain(TRIGGERS.iter().map(|(.., w)| *w))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_plain_web_and_mail_links_are_offered() {
        for url in ["https://example.com/a?b#c", "HTTP://x", "mailto:a@b", "mailto:a@b?subject=Hi&body=x", " https://x.org "] {
            assert_eq!(openable(url).as_deref(), Some(url.trim()), "{url}");
        }
        for url in [
            "file:///etc/passwd",
            "javascript:alert(1)",
            "archforge://x",
            "/Users/x",
            "smb://host/share",
            "https://bank.example@evil.example/",
            "https://",
            "https://exa\u{202E}mple.com",
            "https://exa\u{200B}mple.com",
            "https://a.com/x y",
            "mailto:a@b?attach=/Users/x/.ssh/id_rsa",
            "mailto:a@b?Attachment=x",
            "",
        ] {
            assert_eq!(openable(url), None, "{url:?}");
        }
        assert_eq!(site_of("https://Docs.Example.com.evil.example/x?y"), "docs.example.com.evil.example");
        assert_eq!(site_of("mailto:a@b.org?subject=x"), "a@b.org");
    }
}
