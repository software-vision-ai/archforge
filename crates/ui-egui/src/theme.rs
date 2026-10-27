//! The appearance: Light, the design's primary variant and the default, or Dark. `apply`
//! gives the toolkit's own visuals, its tooltips and scroll bars, the design's palette (`crate::ux`).

use egui::{CornerRadius, Stroke};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemeKind {
    Dark,
    #[default]
    Light,
}

impl ThemeKind {
    pub fn is_dark(self) -> bool {
        matches!(self, ThemeKind::Dark)
    }
}

/// Installs the appearance's palette as the toolkit's visuals.
pub fn apply(ctx: &egui::Context, kind: ThemeKind) {
    let theme = if kind.is_dark() { egui::Theme::Dark } else { egui::Theme::Light };
    let p = crate::ux::palette(kind.is_dark());
    let mut v = theme.default_visuals();
    v.panel_fill = p.page;
    v.window_fill = p.surface;
    v.extreme_bg_color = p.page;
    v.override_text_color = Some(p.text);
    v.window_stroke = Stroke::new(1.0, p.border);
    v.window_corner_radius = CornerRadius::same(8);
    v.menu_corner_radius = CornerRadius::same(8);
    v.selection.bg_fill = p.accent_tint;
    v.selection.stroke = Stroke::new(1.0, p.accent);
    v.hyperlink_color = p.accent;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, p.border);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, p.text_secondary);
    ctx.set_theme(theme);
    ctx.set_visuals_of(theme, v);
    ctx.all_styles_mut(|s| s.interaction.tooltip_delay = 0.4);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_design_variant_is_the_default() {
        assert_eq!(ThemeKind::default(), ThemeKind::Light);
        assert!(ThemeKind::Dark.is_dark() && !ThemeKind::Light.is_dark());
    }
}
