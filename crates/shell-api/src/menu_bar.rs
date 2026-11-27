//! What the narrow context holds of the shell's in-window menu bar.

use egui::Rect;

/// What the narrow context ([`crate::cx::Ctx`]) holds of the bar: what it took the keyboard for, the
/// field it took it from, where it was drawn, and the inline editor it holds open, to change.
pub struct Hold<'a> {
    pub open: Option<&'static str>,
    pub chosen: Option<&'static str>,
    pub back: Option<egui::Id>,
    pub rect: Option<Rect>,
    /// The inline editor the bar keeps open after taking its keyboard, to change.
    pub held: &'a mut Option<egui::Id>,
}
