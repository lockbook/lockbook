use egui::style::{WidgetVisuals, Widgets};
use egui::{Stroke, Ui};

use crate::tab::markdown_editor::MdRender;
use crate::theme::palette_v2::ThemeExt;

impl MdRender {
    // todo: all egui needs to be themed this way and this should be removed
    pub fn apply_theme(&self, ui: &mut Ui) {
        let theme = self.ctx.get_lb_theme();

        let rounding = egui::CornerRadius::same(2);
        let expansion = 0.0;
        let bg_stroke = Stroke::new(1.0, theme.neutral_bg_tertiary());
        let fg_stroke = Stroke::new(1.5, self.ctx.get_lb_theme().neutral_fg());
        ui.visuals_mut().widgets = Widgets {
            noninteractive: WidgetVisuals {
                weak_bg_fill: theme.neutral_bg_tertiary(),
                bg_fill: theme.neutral_bg_tertiary(),
                bg_stroke,
                fg_stroke,
                corner_radius: rounding,
                expansion,
            },
            inactive: WidgetVisuals {
                weak_bg_fill: theme.neutral_bg_secondary(), // button background
                bg_fill: theme.neutral_bg_secondary(),      // checkbox background
                bg_stroke,
                fg_stroke,
                corner_radius: rounding,
                expansion,
            },
            hovered: WidgetVisuals {
                weak_bg_fill: theme.neutral_bg_tertiary(),
                bg_fill: theme.neutral_bg_tertiary(),
                bg_stroke,
                fg_stroke,
                corner_radius: rounding,
                expansion,
            },
            active: WidgetVisuals {
                weak_bg_fill: theme
                    .bg()
                    .get_color(theme.prefs().primary)
                    .gamma_multiply(0.2),
                bg_fill: theme
                    .bg()
                    .get_color(theme.prefs().primary)
                    .gamma_multiply(0.2),
                bg_stroke,
                fg_stroke,
                corner_radius: rounding,
                expansion,
            },
            open: WidgetVisuals {
                weak_bg_fill: theme.neutral_bg_tertiary(),
                bg_fill: theme.neutral_bg_tertiary(),
                bg_stroke,
                fg_stroke,
                corner_radius: rounding,
                expansion,
            },
        };
    }
}
