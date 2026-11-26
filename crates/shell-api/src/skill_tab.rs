//! A skill open in its own tab in the centre, as the canvas's state keeps it between frames
//! (`canvas::State::skills`), which names no shell. The shell draws and keeps the tab
//! (`archforge_ui_egui::skill_tab` re-exports it under its old path).

use archforge_mof::Uuid;

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
