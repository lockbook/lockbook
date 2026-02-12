use egui::style::{WidgetVisuals, Widgets};
use egui::{Color32, Context, Rounding, Stroke, Ui};

use crate::theme::palette_v2::{Mode, ThemeExt};

#[derive(Clone)]
pub struct Theme {
    ctx: Context,
}

impl Theme {
    pub fn new(ctx: Context) -> Self {
        Self { ctx }
    }

    /// Get the color set closest to the background color.
    /// todo: the editor should use the paletted directly once there are secondary helpers to do
    /// these in-between colors uniformly
    pub fn bg(&self) -> ColorSet {
        let theme = self.ctx.get_theme();

        match theme.current {
            Mode::Light => ColorSet {
                neutral_primary: theme.bg().white,
                neutral_secondary: theme.bg().grey,
                neutral_tertiary: theme.bg().grey.lerp_to_gamma(theme.bg().black, 0.25),
                neutral_quarternary: theme.bg().grey.lerp_to_gamma(theme.bg().black, 0.50),
                red: theme.bg().red,
                green: theme.bg().green,
                yellow: theme.bg().yellow,
                blue: theme.bg().blue,
                magenta: theme.bg().magenta,
                accent_primary: theme.bg().get_color(theme.prefs().primary),
                accent_secondary: theme.bg().get_color(theme.prefs().secondary),
                accent_tertiary: theme.bg().get_color(theme.prefs().tertiary),
            },
            Mode::Dark => ColorSet {
                neutral_primary: theme.bg().black,
                neutral_secondary: theme.bg().grey,
                neutral_tertiary: theme.bg().grey.lerp_to_gamma(theme.bg().white, 0.25),
                neutral_quarternary: theme.bg().grey.lerp_to_gamma(theme.bg().white, 0.50),
                red: theme.bg().red,
                green: theme.bg().green,
                yellow: theme.bg().yellow,
                blue: theme.bg().blue,
                magenta: theme.bg().magenta,
                accent_primary: theme.bg().get_color(theme.prefs().primary),
                accent_secondary: theme.bg().get_color(theme.prefs().secondary),
                accent_tertiary: theme.bg().get_color(theme.prefs().tertiary),
            },
        }
    }

    /// Get the color set closest to the foreground color.
    /// todo: the editor should use the paletted directly once there are secondary helpers to do
    /// these in-between colors uniformly
    pub fn fg(&self) -> ColorSet {
        let theme = self.ctx.get_theme();

        match theme.current {
            Mode::Light => ColorSet {
                neutral_primary: theme.fg().black,
                neutral_secondary: theme.fg().grey,
                neutral_tertiary: theme.fg().grey.lerp_to_gamma(theme.fg().white, 0.25),
                neutral_quarternary: theme.fg().grey.lerp_to_gamma(theme.fg().white, 0.50),
                red: theme.fg().red,
                green: theme.fg().green,
                yellow: theme.fg().yellow,
                blue: theme.fg().blue,
                magenta: theme.fg().magenta,
                accent_primary: theme.fg().get_color(theme.prefs().primary),
                accent_secondary: theme.fg().get_color(theme.prefs().secondary),
                accent_tertiary: theme.fg().get_color(theme.prefs().tertiary),
            },
            Mode::Dark => ColorSet {
                neutral_primary: theme.fg().white,
                neutral_secondary: theme.fg().grey,
                neutral_tertiary: theme.fg().grey.lerp_to_gamma(theme.fg().black, 0.25),
                neutral_quarternary: theme.fg().grey.lerp_to_gamma(theme.fg().black, 0.50),
                red: theme.fg().red,
                green: theme.fg().green,
                yellow: theme.fg().yellow,
                blue: theme.fg().blue,
                magenta: theme.fg().magenta,
                accent_primary: theme.fg().get_color(theme.prefs().primary),
                accent_secondary: theme.fg().get_color(theme.prefs().secondary),
                accent_tertiary: theme.fg().get_color(theme.prefs().tertiary),
            },
        }
    }

    // todo: all egui needs to be themed this way and this should be removed
    pub fn apply(&self, ui: &mut Ui) {
        let rounding = Rounding::same(2.0);
        let expansion = 0.0;
        let bg_stroke = Stroke::new(1.0, self.bg().neutral_tertiary);
        let fg_stroke = Stroke::new(1.5, self.fg().neutral_secondary);
        ui.visuals_mut().widgets = Widgets {
            noninteractive: WidgetVisuals {
                weak_bg_fill: self.bg().neutral_tertiary,
                bg_fill: self.bg().neutral_tertiary,
                bg_stroke,
                fg_stroke,
                rounding,
                expansion,
            },
            inactive: WidgetVisuals {
                weak_bg_fill: self.bg().neutral_secondary, // button background
                bg_fill: self.bg().neutral_secondary,      // checkbox background
                bg_stroke,
                fg_stroke,
                rounding,
                expansion,
            },
            hovered: WidgetVisuals {
                weak_bg_fill: self.bg().neutral_tertiary,
                bg_fill: self.bg().neutral_tertiary,
                bg_stroke,
                fg_stroke,
                rounding,
                expansion,
            },
            active: WidgetVisuals {
                weak_bg_fill: self.bg().accent_primary.gamma_multiply(0.2),
                bg_fill: self.bg().accent_primary.gamma_multiply(0.2),
                bg_stroke,
                fg_stroke,
                rounding,
                expansion,
            },
            open: WidgetVisuals {
                weak_bg_fill: self.bg().neutral_tertiary,
                bg_fill: self.bg().neutral_tertiary,
                bg_stroke,
                fg_stroke,
                rounding,
                expansion,
            },
        };
    }
}

#[derive(Clone, Copy)]
#[expect(dead_code)]
pub struct ColorSet {
    pub neutral_primary: Color32,
    pub neutral_secondary: Color32,
    pub neutral_tertiary: Color32,
    pub neutral_quarternary: Color32,

    pub red: Color32,
    pub green: Color32,
    pub yellow: Color32,
    pub blue: Color32,
    pub magenta: Color32,
    // todo: cyan?
    pub accent_primary: Color32,
    pub accent_secondary: Color32,
    pub accent_tertiary: Color32,
}
