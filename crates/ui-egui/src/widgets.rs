//! The automation registry. Every interactive control is recorded each frame with an automation
//! identifier, its kind and its rectangle, so tests can find controls by id. The identifier is
//! also the control's toolkit id.

use egui::{Rect, Ui};
use serde::Serialize;

/// One registered widget.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Entry {
    pub id: String,
    pub kind: &'static str,
    pub rect: [f32; 4],
}

/// The widgets drawn in the last complete frame.
#[derive(Debug, Default)]
pub struct Registry {
    current: Vec<Entry>,
    last: Vec<Entry>,
}

impl Registry {
    pub fn begin_frame(&mut self) {
        self.last = std::mem::take(&mut self.current);
    }

    pub fn record(&mut self, id: &str, kind: &'static str, rect: Rect) {
        self.current.push(Entry { id: id.to_string(), kind, rect: [rect.min.x, rect.min.y, rect.max.x, rect.max.y] });
    }

    /// The widgets of the most recent frame (the frame in progress once it has drawn any).
    pub fn entries(&self) -> &[Entry] {
        if self.current.is_empty() { &self.last } else { &self.current }
    }
}

/// Records a widget only while it is inside its clip rectangle, so a control scrolled out of view
/// is neither listed nor clickable.
pub fn record_visible(ui: &Ui, reg: &mut Registry, id: &str, kind: &'static str, rect: Rect) {
    if ui.clip_rect().intersects(rect) {
        reg.record(id, kind, rect);
    }
}

/// The toolkit id for an automation identifier.
pub fn egui_id(id: &str) -> egui::Id {
    egui::Id::new(("automation", id))
}
