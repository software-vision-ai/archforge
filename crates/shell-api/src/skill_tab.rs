//! A skill open in its own tab in the centre, as the canvas's state keeps it between frames
//! (`canvas::State::skills`), which names no shell, and its opening through the narrow context
//! (`open_cx`), which the canvas's opening takes for a skill. The shell draws and keeps the tab
//! (`archforge_ui_egui::skill_tab` re-exports it under its old path).

use archforge_mof::{Obj, Store, Uuid};

use crate::cx::Ctx;

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
    /// The name as typed in the title.
    pub name: String,
    /// The name takes the keyboard, all of it selected, at the next frame: a skill just made.
    pub focus_name: bool,
    /// Why the last rename or removal was refused; cleared by the next one kept.
    pub settings_refused: Option<String>,
}

/// Whether `id` names a live skill.
pub fn is_skill(store: &Store, id: Uuid) -> bool {
    skill_obj(store, id).is_some()
}

/// The live skill `id` names, when it names one.
pub fn skill_obj(store: &Store, id: Uuid) -> Option<Obj> {
    let s = store.by_uuid(id)?;
    (store.is_alive(s) && store.class_of(s).ok() == Some(archforge_core::SKILL)).then_some(s)
}

/// The text the skill `s` holds, empty when it holds none.
pub fn body_of(store: &Store, s: Obj) -> String {
    store.get(s, archforge_core::skill::BODY).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default()
}

/// Opens a skill in its tab through the narrow context, the tab added when it has none.
pub fn open_cx(cx: &mut Ctx, id: Uuid) {
    let Some(store) = cx.session.as_ref().map(|s| s.doc()) else { return };
    let Some(s) = skill_obj(store, id) else { return };
    if !cx.canvas.skills.iter().any(|d| d.skill == id) {
        let body = body_of(store, s);
        cx.canvas.skills.push(Doc {
            name: archforge_core::skills::name_of(store, s),
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
    if !cx.canvas.tabs.contains(&id) {
        cx.canvas.tabs.push(id);
    }
    cx.canvas.active = Some(id);
    cx.canvas.notice = None;
    cx.canvas.focused = false;
}
