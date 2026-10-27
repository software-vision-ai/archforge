//! macOS window style: the content extends under a transparent title bar, and the shell leaves the
//! title bar's height empty at the top, so the window has one surface from edge to edge.
#![cfg(target_os = "macos")]

/// Logical pixels the title bar covers.
pub const TITLE_BAR_INSET: f32 = 28.0;

pub fn style(viewport: egui::ViewportBuilder) -> egui::ViewportBuilder {
    viewport.with_fullsize_content_view(true).with_title_shown(false).with_titlebar_shown(false)
}
