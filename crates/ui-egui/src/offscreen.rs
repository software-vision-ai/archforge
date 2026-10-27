//! Offscreen rendering of the window, behind the `snapshot` feature: a fresh shell in
//! the headless harness, drawn once settled and rendered to RGBA.

use crate::theme::ThemeKind;
use crate::{Services, Shell};

/// Renders the window at `width` by `height` logical pixels, `scale` pixels a point, in `theme`:
/// `(width, height, rgba)` in pixels.
pub fn render(width: f32, height: f32, scale: f32, theme: ThemeKind) -> Result<(u32, u32, Vec<u8>), String> {
    crate::i18n::set_language("en");
    let mut shell = Shell::new(Services::default());
    shell.state.theme = theme;
    let mut harness = egui_kittest::Harness::builder().with_size(egui::vec2(width, height)).with_pixels_per_point(scale).build_ui_state(
        |ui, shell: &mut Shell| {
            shell.logic(ui.ctx());
            // The harness frames its ui with an 8 px margin; the shell draws on the whole window,
            // as it does in the application.
            let window = ui.ctx().content_rect();
            let mut full = ui.new_child(egui::UiBuilder::new().max_rect(window));
            full.set_clip_rect(window);
            shell.ui(&mut full);
        },
        shell,
    );
    harness.run();
    let image = harness.render()?;
    let (w, h) = (image.width(), image.height());
    Ok((w, h, image.into_raw()))
}
