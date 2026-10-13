/// Workspace spacing, type scale, and window chrome.
///
/// Mobile hosts install this on the egui context via [`init`]. Desktop keeps
/// its own context style (zero `item_spacing`, 14pt UI type) and applies the
/// same setup on the workspace `Ui` so shell chrome is unaffected.
pub fn apply(style: &mut egui::Style) {
    style.spacing = egui::style::Spacing {
        button_padding: egui::vec2(7.0, 7.0),
        menu_margin: egui::Margin::same(10),
        combo_width: 50.0,
        ..egui::style::Spacing::default()
    };

    style.visuals.menu_corner_radius = egui::CornerRadius::same(10);
    style.visuals.window_corner_radius = egui::CornerRadius::same(10);

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
}

pub fn init(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    apply(&mut style);
    ctx.set_style(style);
}
