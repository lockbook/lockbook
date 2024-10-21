use crate::theme::palette::ThemePalette;
use egui::Color32;

use super::palette::ColorAlias;

pub fn init(ctx: &egui::Context, dark_mode: bool) {
    let visuals = if dark_mode { dark(ColorAlias::Blue) } else { light(ColorAlias::Blue) };
    ctx.set_visuals(visuals);

    let mut style = (*ctx.style()).clone();
    style.spacing.button_padding = egui::vec2(7.0, 7.0);
    style.spacing.menu_margin = egui::Margin::same(10.0);
    style.spacing.combo_width = 50.0;

    style.visuals.menu_rounding = egui::Rounding::same(10.0);
    style.visuals.window_rounding = egui::Rounding::same(10.0);

    style
        .text_styles
        .insert(egui::TextStyle::Body, egui::FontId::new(17.0, egui::FontFamily::Proportional));
    style
        .text_styles
        .insert(egui::TextStyle::Small, egui::FontId::new(15.0, egui::FontFamily::Proportional));

    style
        .text_styles
        .insert(egui::TextStyle::Monospace, egui::FontId::new(17.0, egui::FontFamily::Monospace));

    style
        .text_styles
        .insert(egui::TextStyle::Button, egui::FontId::new(17.0, egui::FontFamily::Proportional));
    ctx.set_style(style);
}

pub fn dark(primary: ColorAlias) -> egui::Visuals {
    let mut v = egui::Visuals::dark();
    v.faint_bg_color = Color32::from_rgb(35, 35, 37);
    v.widgets.noninteractive.bg_fill = Color32::from_rgb(25, 25, 27);
    v.widgets.noninteractive.fg_stroke.color = Color32::from_rgb(242, 242, 247);
    v.widgets.inactive.fg_stroke.color = Color32::from_rgb(242, 242, 247);
    v.widgets.hovered.bg_fill = v.widgets.active.bg_fill;
    v.widgets.active.bg_fill = ThemePalette::DARK[primary];
    v
}

pub fn light(primary: ColorAlias) -> egui::Visuals {
    let mut v = egui::Visuals::light();
    v.widgets.hovered.bg_fill = v.widgets.active.bg_fill;
    v.widgets.active.bg_fill = ThemePalette::LIGHT[primary];
    v
}
