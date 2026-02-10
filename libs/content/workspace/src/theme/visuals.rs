pub fn init(ctx: &egui::Context) {
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
