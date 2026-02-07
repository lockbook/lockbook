use egui::style::{WidgetVisuals, Widgets};
use egui::{Color32, Context, Rounding, Stroke, Ui};

use crate::theme::palette_v2::ThemeExt;

macro_rules! hex_color {
    ($hex:expr) => {{
        let hex = $hex;
        assert!(hex.len() == 7 && &hex[0..1] == "#", "Invalid hex color format");
        let r = u8::from_str_radix(&hex[1..3], 16).unwrap();
        let g = u8::from_str_radix(&hex[3..5], 16).unwrap();
        let b = u8::from_str_radix(&hex[5..7], 16).unwrap();
        Color32::from_rgb(r, g, b)
    }};
}

#[derive(Clone)]
pub struct Theme {
    dim: ColorSet,
    bright: ColorSet,
    ctx: Context,
}

impl Theme {
    pub fn new(ctx: Context) -> Self {
        Self {
            dim: ColorSet {
                neutral_primary: hex_color!("#101010"),
                neutral_secondary: hex_color!("#222222"),
                neutral_tertiary: hex_color!("#555555"),
                neutral_quarternary: hex_color!("#777777"),

                red: hex_color!("#DF2040"),
                green: hex_color!("#00B371"),
                yellow: hex_color!("#E6AC00"),
                blue: hex_color!("#207FDF"),
                magenta: hex_color!("#7855AA"),

                accent_primary: hex_color!("#7855AA"),
                accent_secondary: hex_color!("#207FDF"),
                accent_tertiary: hex_color!("#00B371"),
            },
            bright: ColorSet {
                neutral_primary: hex_color!("#FFFFFF"),
                neutral_secondary: hex_color!("#FCFCFC"),
                neutral_tertiary: hex_color!("#EEEEEE"),
                neutral_quarternary: hex_color!("#777777"),

                red: hex_color!("#FF6680"),
                green: hex_color!("#67E4B6"),
                yellow: hex_color!("#FFDB70"),
                blue: hex_color!("#66B2FF"),
                magenta: hex_color!("#AC8CD9"),

                accent_primary: hex_color!("#207FDF"),
                accent_secondary: hex_color!("#67E4B6"),
                accent_tertiary: hex_color!("#FFDB70"),
            },
            ctx,
        }
    }

    /// Get the color set closest to the background color.
    pub fn bg(&self) -> ColorSet {
        let theme = self.ctx.get_theme();


        if self.ctx.style().visuals.dark_mode { self.dim } else { self.bright }
    }

    /// Get the color set closest to the foreground color.
    pub fn fg(&self) -> ColorSet {
        if self.ctx.style().visuals.dark_mode { self.bright } else { self.dim }
    }

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
