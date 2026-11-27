//! What the narrow context holds of the shell's in-window menu bar, and whether an inline editor
//! commits while the bar holds the keyboard (`ends_cx`).

use egui::Rect;

use crate::cx::Ctx;

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

/// Whether the pointer is down on the bar drawn at `rect`, or let go on it this frame.
pub fn on(rect: Option<Rect>, ctx: &egui::Context) -> bool {
    ctx.input(|i| (i.pointer.any_down() || i.pointer.any_released()) && rect.zip(i.pointer.interact_pos()).is_some_and(|(bar, at)| bar.contains(at)))
}

/// Whether the inline editor `id`, which commits when it loses the keyboard, commits now: on `lost`,
/// unless the bar holds it, having taken the keyboard for the Edit menu, for an Edit row chosen, or
/// for a press on the bar before its menu opens. A held editor commits in the first frame another
/// menu is open, or a row of another menu is chosen, so that row finds it committed when it runs
/// the frame after, as with no menu; an Edit row or a press that opens nothing gives it the
/// keyboard back instead.
pub fn ends_cx(cx: &mut Ctx, ctx: &egui::Context, id: egui::Id, lost: bool, focused: bool) -> bool {
    let bar = &mut cx.menu_bar;
    let edit = bar.open == Some("edit") || bar.chosen.is_some_and(|r| r.starts_with("edit.")) || (bar.open.is_none() && on(bar.rect, ctx));
    if bar.back == Some(id) && edit {
        *bar.held = Some(id);
        return false;
    }
    let held = bar.held.take_if(|h| *h == id).is_some();
    lost || (held && !focused)
}
